"""Naive privileged PUCT using a fair network leaf evaluator."""

from __future__ import annotations

import json
import math
from collections.abc import Callable, Mapping, Sequence
from typing import cast

import torch

from ..fair import FairCombatObservation
from ..run import Action, ActionDescriptor, Decision, RunEnv
from .gameplay import PolicyEpisode, _capped_public_episode
from .model import FairCombatPolicyValueNet
from .records import fair_observation_from_payload
from .rewards import COMBAT_PROXY_V1, CombatRewardConfig
from .tensor import Vocabularies, collate_combat_tensors, tensorize_combat

FAIR_LEAF_BATCH_SCHEMA = "fair_leaf_batch_v1"
PUCT_TEACHER_NAME = "privileged_puct"
PUCT_TEACHER_VERSION = "synchronous_batch1_v3"
# fair_leaf_batch_v1 is intentionally batch-size 1 and not an extensible request
# protocol. Request/response correlation ids are deferred until batched search.

_FORBIDDEN_LEAF_FIELDS = (
    "card_id",
    "monster_id",
    "content_id",
    "rng",
    "move_history",
    "queued_decisions",
    "pending_actions",
)


def _mapping(value: object) -> dict[str, object]:
    if not isinstance(value, dict):
        raise TypeError("PUCT payload must be an object")
    return cast(dict[str, object], value)


def _optional_int(choice: Mapping[str, object], name: str) -> int | None:
    value = choice.get(name)
    if value is None:
        return None
    if type(value) is not int:
        raise TypeError(f"public choice {name} must be an int")
    return value


def _descriptor_from_choice(choice: Mapping[str, object]) -> ActionDescriptor:
    kind = choice.get("kind")
    if type(kind) is not str:
        raise TypeError("public choice kind must be a string")
    return ActionDescriptor(
        family="combat",
        kind=kind,
        hand_slot=_optional_int(choice, "hand_slot"),
        potion_slot=_optional_int(choice, "potion_slot"),
        option_slot=_optional_int(choice, "option_slot"),
        target_slot=_optional_int(choice, "target_slot"),
    )


def _reject_hidden_fields(payload: object) -> None:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False)
    for field in _FORBIDDEN_LEAF_FIELDS:
        if field in encoded:
            raise ValueError(f"hidden field {field} reached the PUCT evaluator")


def _positive_budget(value: int, label: str) -> int:
    if type(value) is not int or value < 1:
        raise ValueError(f"{label} must be positive")
    return value


def _positive_exploration(value: float, label: str) -> float:
    if type(value) not in {int, float} or not math.isfinite(float(value)) or float(value) <= 0.0:
        raise ValueError(f"{label} must be finite and positive")
    return float(value)


def _episode_root_baselines(
    observation: FairCombatObservation,
    episode_root_max_hp: int | None,
    episode_root_gold: int | None,
) -> tuple[int, int]:
    max_hp = observation.player.max_hp if episode_root_max_hp is None else episode_root_max_hp
    gold = observation.context.gold if episode_root_gold is None else episode_root_gold
    if type(max_hp) is not int or max_hp <= 0:
        raise ValueError("episode root max HP must be positive")
    if type(gold) is not int or gold < 0:
        raise ValueError("episode root gold must be nonnegative")
    return max_hp, gold


def _require_aligned_public_rows(decision: Decision, payload: Mapping[str, object]) -> None:
    raw_choices = payload.get("choices")
    if not isinstance(raw_choices, list) or len(raw_choices) != len(decision.actions):
        raise ValueError("PUCT choice rows are not aligned with the public Decision")
    for action, raw_choice in zip(decision.actions, raw_choices, strict=True):
        if action.descriptor() != _descriptor_from_choice(_mapping(raw_choice)):
            raise ValueError("PUCT choice rows are not aligned with the public Decision")


def _parse_fair_leaf_request(
    request_json: str,
) -> tuple[FairCombatObservation, tuple[ActionDescriptor, ...]]:
    request = _mapping(json.loads(request_json))
    if request.get("schema") != FAIR_LEAF_BATCH_SCHEMA:
        raise ValueError("PUCT evaluator requires fair_leaf_batch_v1")
    extra = set(request) - {"schema", "batch"}
    if extra:
        raise ValueError(f"PUCT evaluator request has unknown fields: {sorted(extra)}")
    batch = request.get("batch")
    if not isinstance(batch, list) or len(batch) != 1:
        raise ValueError("naive PUCT evaluator requires batch size 1")
    item = _mapping(batch[0])
    extra_item = set(item) - {"observation", "choices"}
    if extra_item:
        raise ValueError(f"PUCT leaf has unknown fields: {sorted(extra_item)}")
    _reject_hidden_fields(item)
    observation = fair_observation_from_payload(item["observation"])
    raw_choices = item["choices"]
    if not isinstance(raw_choices, list) or not raw_choices:
        raise ValueError("PUCT leaf requires a nonempty public choice list")
    descriptors = tuple(_descriptor_from_choice(_mapping(choice)) for choice in raw_choices)
    return observation, descriptors


def _uniform_priors(count: int) -> list[float]:
    if count < 1:
        raise ValueError("PUCT leaf requires a nonempty public choice list")
    prior = 1.0 / float(count)
    return [prior] * count


def _encode_leaf_evaluation(priors: Sequence[float], value: float) -> str:
    for prior in priors:
        if type(prior) not in {int, float} or not math.isfinite(float(prior)) or float(prior) < 0:
            raise ValueError("PUCT network priors are not finite and nonnegative")
    if type(value) not in {int, float} or not math.isfinite(float(value)) or abs(float(value)) > 1:
        raise ValueError("PUCT network value must be finite and in [-1, 1]")
    return json.dumps(
        {
            "schema": FAIR_LEAF_BATCH_SCHEMA,
            "batch": [
                {
                    "priors": [float(prior) for prior in priors],
                    "value": float(value),
                }
            ],
        },
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    )


def _eval_network_leaf(
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    observation: FairCombatObservation,
    descriptors: tuple[ActionDescriptor, ...],
) -> tuple[torch.Tensor, float]:
    tensors = tensorize_combat(observation, descriptors, vocabularies)
    batched = collate_combat_tensors((tensors,))
    was_training = model.training
    model.eval()
    try:
        with torch.inference_mode():
            output = model(batched)
            logits = output.logits[0, : len(descriptors)].clone()
            value = float(output.value[0].item())
    finally:
        model.train(was_training)
    return logits, value


def network_leaf_evaluator(
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
) -> Callable[[str], str]:
    """Return a batch-shaped JSON callback that scores one fair leaf.

    The callback is invoked synchronously while search holds the Python GIL.
    Priors come from the learned policy head; the leaf value comes from the
    learned value head.
    """

    def evaluate(request_json: str) -> str:
        observation, descriptors = _parse_fair_leaf_request(request_json)
        logits, value = _eval_network_leaf(model, vocabularies, observation, descriptors)
        if logits.shape != (len(descriptors),) or not torch.isfinite(logits).all():
            raise ValueError("PUCT network logits are not finite or aligned")
        priors = torch.softmax(logits, dim=-1)
        if not torch.isfinite(priors).all() or torch.any(priors < 0):
            raise ValueError("PUCT network priors are not finite and nonnegative")
        return _encode_leaf_evaluation(priors.tolist(), value)

    return evaluate


def uniform_prior_network_value_leaf_evaluator(
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
) -> Callable[[str], str]:
    """Uniform priors over legal public actions; checkpoint value head.

    This is a policy-prior ablation, not unguided search. Policy logits are
    ignored; only the learned value head is used for non-terminal leaves.
    """

    def evaluate(request_json: str) -> str:
        observation, descriptors = _parse_fair_leaf_request(request_json)
        _logits, value = _eval_network_leaf(model, vocabularies, observation, descriptors)
        return _encode_leaf_evaluation(_uniform_priors(len(descriptors)), value)

    return evaluate


def constant_value_uniform_prior_leaf_evaluator() -> Callable[[str], str]:
    """Uniform priors and constant non-terminal leaf value 0.0.

    Does not require or evaluate a checkpoint. Genuine terminals still receive
    combat_proxy_v1 inside native search.
    """

    def evaluate(request_json: str) -> str:
        _observation, descriptors = _parse_fair_leaf_request(request_json)
        return _encode_leaf_evaluation(_uniform_priors(len(descriptors)), 0.0)

    return evaluate


def puct_search_payload(
    env: RunEnv,
    evaluator: Callable[[str], str],
    *,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
    episode_root_max_hp: int | None = None,
    episode_root_gold: int | None = None,
    leaf_cache: str | None = None,
) -> dict[str, object]:
    """Run naive privileged PUCT from the current combat state.

    `selected_index` is valid only against the current environment Decision at
    the time of this call. Do not apply it to a later, cloned, or restored
    decision. Search always stops at `simulation_budget` or `transition_budget`.
    The evaluator callback runs synchronously and holds the Python GIL.
    Generic search defaults to `leaf_cache="off"` so an arbitrary callback is
    not memoized. Deterministic teacher/network call sites pass
    `leaf_cache="exact_state"` after the evaluator is known to be pure for an
    exact `RunState`.
    """
    _positive_exploration(c_puct, "c_puct")
    _positive_budget(simulation_budget, "simulation budget")
    _positive_budget(transition_budget, "transition budget")
    if leaf_cache is not None and leaf_cache not in {"off", "exact_state"}:
        raise ValueError("leaf cache must be 'off' or 'exact_state'")
    config = COMBAT_PROXY_V1 if reward_config is None else reward_config
    payload = env.puct_search_payload(
        evaluator,
        c_puct=c_puct,
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        reward_config_json=json.dumps(config.to_dict(), sort_keys=True, separators=(",", ":")),
        episode_root_max_hp=episode_root_max_hp,
        episode_root_gold=episode_root_gold,
        leaf_cache=leaf_cache,
    )
    if payload.get("teacher_name") != PUCT_TEACHER_NAME:
        raise ValueError("PUCT payload teacher_name mismatch")
    if payload.get("teacher_version") != PUCT_TEACHER_VERSION:
        raise ValueError("PUCT payload teacher_version mismatch")
    return payload


def puct_clone_episode_payload(
    env: RunEnv,
    evaluator: Callable[[str], str],
    *,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    max_decisions: int = 512,
    max_player_turns: int = 100,
    reward_config: CombatRewardConfig | None = None,
    leaf_cache: str | None = None,
) -> dict[str, object]:
    """Run a detached privileged PUCT episode from the current root.

    Generic clone defaults to cache-off. The teacher dataset path passes
    `leaf_cache="exact_state"` because its network evaluator is deterministic
    and pure for an exact `RunState`.
    """

    _positive_exploration(c_puct, "c_puct")
    _positive_budget(simulation_budget, "simulation budget")
    _positive_budget(transition_budget, "transition budget")
    _positive_budget(max_decisions, "max decisions")
    _positive_budget(max_player_turns, "max player turns")
    if leaf_cache is not None and leaf_cache not in {"off", "exact_state"}:
        raise ValueError("leaf cache must be 'off' or 'exact_state'")
    config = COMBAT_PROXY_V1 if reward_config is None else reward_config
    payload = env.puct_clone_episode_payload(
        evaluator,
        c_puct=c_puct,
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        reward_config_json=json.dumps(config.to_dict(), sort_keys=True, separators=(",", ":")),
        leaf_cache=leaf_cache,
    )
    if payload.get("teacher_name") != PUCT_TEACHER_NAME:
        raise ValueError("PUCT episode teacher_name mismatch")
    if payload.get("teacher_version") != PUCT_TEACHER_VERSION:
        raise ValueError("PUCT episode teacher_version mismatch")
    if payload.get("schema_version") != 1:
        raise ValueError("unsupported native PUCT episode schema")
    return payload


def select_puct_action_with_evaluator(
    env: RunEnv,
    decision: Decision,
    evaluator: Callable[[str], str],
    *,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
    episode_root_max_hp: int | None = None,
    episode_root_gold: int | None = None,
    leaf_cache: str | None = None,
) -> Action:
    """Choose by PUCT visits and return the original public Action sidecar.

    The returned action is the current Decision row at `selected_index`.
    Generic/custom evaluators default to cache-off. Deterministic network and
    constant wrappers pass `leaf_cache="exact_state"` explicitly.
    """

    if decision.revision != env.revision:
        raise ValueError("PUCT selector requires the current environment decision")
    observation = decision.observation
    if not isinstance(observation, FairCombatObservation):
        raise TypeError("PUCT selector requires a fair combat observation")
    if not decision.actions:
        raise ValueError("PUCT selector requires at least one public action")
    max_hp, gold = _episode_root_baselines(observation, episode_root_max_hp, episode_root_gold)
    payload = puct_search_payload(
        env,
        evaluator,
        c_puct=c_puct,
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        reward_config=reward_config,
        episode_root_max_hp=max_hp,
        episode_root_gold=gold,
        leaf_cache=leaf_cache,
    )
    _require_aligned_public_rows(decision, payload)
    index = payload.get("selected_index")
    if type(index) is not int or not 0 <= index < len(decision.actions):
        raise ValueError("PUCT selected an out-of-range public row")
    return decision.actions[index]


def select_puct_action(
    env: RunEnv,
    decision: Decision,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    *,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
    episode_root_max_hp: int | None = None,
    episode_root_gold: int | None = None,
) -> Action:
    """Choose by learned-prior/learned-value PUCT and return the original sidecar."""

    return select_puct_action_with_evaluator(
        env,
        decision,
        network_leaf_evaluator(model, vocabularies),
        c_puct=c_puct,
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        reward_config=reward_config,
        episode_root_max_hp=episode_root_max_hp,
        episode_root_gold=episode_root_gold,
        leaf_cache="exact_state",
    )


def select_uniform_prior_network_value_puct_action(
    env: RunEnv,
    decision: Decision,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    *,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
    episode_root_max_hp: int | None = None,
    episode_root_gold: int | None = None,
) -> Action:
    """Choose by uniform-prior/network-value PUCT and return the original sidecar."""

    return select_puct_action_with_evaluator(
        env,
        decision,
        uniform_prior_network_value_leaf_evaluator(model, vocabularies),
        c_puct=c_puct,
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        reward_config=reward_config,
        episode_root_max_hp=episode_root_max_hp,
        episode_root_gold=episode_root_gold,
        leaf_cache="exact_state",
    )


def select_uniform_prior_constant_value_puct_action(
    env: RunEnv,
    decision: Decision,
    *,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
    episode_root_max_hp: int | None = None,
    episode_root_gold: int | None = None,
) -> Action:
    """Choose by unguided equal-budget PUCT and return the original sidecar.

    Does not require or evaluate a checkpoint.
    """

    return select_puct_action_with_evaluator(
        env,
        decision,
        constant_value_uniform_prior_leaf_evaluator(),
        c_puct=c_puct,
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        reward_config=reward_config,
        episode_root_max_hp=episode_root_max_hp,
        episode_root_gold=episode_root_gold,
        leaf_cache="exact_state",
    )


def rollout_puct_policy(
    env: RunEnv,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    *,
    max_decisions: int,
    max_player_turns: int,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
) -> PolicyEpisode:
    episode_root: tuple[int, int] | None = None

    def choose(decision: Decision, accepted_decision_index: int) -> Action:
        del accepted_decision_index
        observation = decision.observation
        if not isinstance(observation, FairCombatObservation):
            raise TypeError("PUCT selector requires a fair combat observation")
        nonlocal episode_root
        if episode_root is None:
            episode_root = _episode_root_baselines(observation, None, None)
        max_hp, gold = episode_root
        return select_puct_action(
            env,
            decision,
            model,
            vocabularies,
            c_puct=c_puct,
            simulation_budget=simulation_budget,
            transition_budget=transition_budget,
            reward_config=reward_config,
            episode_root_max_hp=max_hp,
            episode_root_gold=gold,
        )

    return _capped_public_episode(
        env,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        choose=choose,
    )


def _rollout_puct_with_evaluator(
    env: RunEnv,
    evaluator: Callable[[str], str],
    *,
    max_decisions: int,
    max_player_turns: int,
    c_puct: float,
    simulation_budget: int,
    transition_budget: int,
    reward_config: CombatRewardConfig | None,
    leaf_cache: str | None,
) -> PolicyEpisode:
    episode_root: tuple[int, int] | None = None

    def choose(decision: Decision, accepted_decision_index: int) -> Action:
        del accepted_decision_index
        observation = decision.observation
        if not isinstance(observation, FairCombatObservation):
            raise TypeError("PUCT selector requires a fair combat observation")
        nonlocal episode_root
        if episode_root is None:
            episode_root = _episode_root_baselines(observation, None, None)
        max_hp, gold = episode_root
        return select_puct_action_with_evaluator(
            env,
            decision,
            evaluator,
            c_puct=c_puct,
            simulation_budget=simulation_budget,
            transition_budget=transition_budget,
            reward_config=reward_config,
            episode_root_max_hp=max_hp,
            episode_root_gold=gold,
            leaf_cache=leaf_cache,
        )

    return _capped_public_episode(
        env,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        choose=choose,
    )


def rollout_uniform_prior_network_value_puct_policy(
    env: RunEnv,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    *,
    max_decisions: int,
    max_player_turns: int,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
) -> PolicyEpisode:
    """Roll out the policy-prior ablation: uniform priors, learned value head."""

    return _rollout_puct_with_evaluator(
        env,
        uniform_prior_network_value_leaf_evaluator(model, vocabularies),
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        c_puct=c_puct,
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        reward_config=reward_config,
        leaf_cache="exact_state",
    )


def rollout_uniform_prior_constant_value_puct_policy(
    env: RunEnv,
    *,
    max_decisions: int,
    max_player_turns: int,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
) -> PolicyEpisode:
    """Roll out equal-budget unguided PUCT. Does not evaluate a checkpoint."""

    return _rollout_puct_with_evaluator(
        env,
        constant_value_uniform_prior_leaf_evaluator(),
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        c_puct=c_puct,
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        reward_config=reward_config,
        leaf_cache="exact_state",
    )
