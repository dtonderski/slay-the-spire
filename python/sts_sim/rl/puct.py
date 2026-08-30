"""Naive privileged PUCT using a fair network leaf evaluator."""

from __future__ import annotations

import json
import math
from collections.abc import Callable, Mapping
from typing import cast

import torch

from ..fair import FairCombatObservation
from ..run import Action, ActionDescriptor, Decision, RunEnv
from .gameplay import PolicyEpisode, _capped_public_episode
from .model import FairCombatPolicyValueNet
from .rewards import COMBAT_PROXY_V1, CombatRewardConfig
from .tensor import Vocabularies, collate_combat_tensors, tensorize_combat

FAIR_LEAF_BATCH_SCHEMA = "fair_leaf_batch_v1"
PUCT_TEACHER_NAME = "privileged_puct"
PUCT_TEACHER_VERSION = "synchronous_batch1_v1"

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


def _descriptor_from_choice(choice: Mapping[str, object]) -> ActionDescriptor:
    def optional_int(name: str) -> int | None:
        value = choice.get(name)
        return None if value is None else cast(int, value)

    kind = choice.get("kind")
    if type(kind) is not str:
        raise TypeError("public choice kind must be a string")
    return ActionDescriptor(
        family="combat",
        kind=kind,
        hand_slot=optional_int("hand_slot"),
        potion_slot=optional_int("potion_slot"),
        option_slot=optional_int("option_slot"),
        target_slot=optional_int("target_slot"),
    )


def _reject_hidden_fields(payload: object) -> None:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False)
    for field in _FORBIDDEN_LEAF_FIELDS:
        if field in encoded:
            raise ValueError(f"hidden field {field} reached the PUCT evaluator")


def network_leaf_evaluator(
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
) -> Callable[[str], str]:
    """Return a batch-shaped JSON callback that scores one fair leaf."""

    def evaluate(request_json: str) -> str:
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
        observation = FairCombatObservation._from_payload(item["observation"])
        raw_choices = item["choices"]
        if not isinstance(raw_choices, list) or not raw_choices:
            raise ValueError("PUCT leaf requires a nonempty public choice list")
        descriptors = tuple(_descriptor_from_choice(_mapping(choice)) for choice in raw_choices)
        tensors = tensorize_combat(observation, descriptors, vocabularies)
        batched = collate_combat_tensors((tensors,))
        was_training = model.training
        model.eval()
        try:
            with torch.inference_mode():
                output = model(batched)
                logits = output.logits[0, : len(descriptors)]
                if logits.shape != (len(descriptors),) or not torch.isfinite(logits).all():
                    raise ValueError("PUCT network logits are not finite or aligned")
                priors = torch.softmax(logits, dim=-1)
                value = float(output.value[0].item())
        finally:
            model.train(was_training)
        if not torch.isfinite(priors).all() or torch.any(priors < 0):
            raise ValueError("PUCT network priors are not finite and nonnegative")
        if not math.isfinite(value) or abs(value) > 1:
            raise ValueError("PUCT network value must be finite and in [-1, 1]")
        return json.dumps(
            {
                "schema": FAIR_LEAF_BATCH_SCHEMA,
                "batch": [
                    {
                        "priors": [float(prior) for prior in priors.tolist()],
                        "value": value,
                    }
                ],
            },
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )

    return evaluate


def puct_search_payload(
    env: RunEnv,
    evaluator: Callable[[str], str],
    *,
    c_puct: float = 1.5,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
) -> dict[str, object]:
    if transition_budget < 0:
        raise ValueError("transition budget must be nonnegative")
    config = COMBAT_PROXY_V1 if reward_config is None else reward_config
    payload = env.puct_search_payload(
        evaluator,
        c_puct=c_puct,
        transition_budget=transition_budget,
        reward_config_json=json.dumps(config.to_dict(), sort_keys=True, separators=(",", ":")),
    )
    if payload.get("teacher_name") != PUCT_TEACHER_NAME:
        raise ValueError("PUCT payload teacher_name mismatch")
    if payload.get("teacher_version") != PUCT_TEACHER_VERSION:
        raise ValueError("PUCT payload teacher_version mismatch")
    return payload


def select_puct_action(
    env: RunEnv,
    decision: Decision,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    *,
    c_puct: float = 1.5,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
) -> Action:
    """Choose by PUCT visits and return the original public Action sidecar."""

    if decision.revision != env.revision:
        raise ValueError("PUCT selector requires the current environment decision")
    observation = decision.observation
    if not isinstance(observation, FairCombatObservation):
        raise TypeError("PUCT selector requires a fair combat observation")
    if not decision.actions:
        raise ValueError("PUCT selector requires at least one public action")
    payload = puct_search_payload(
        env,
        network_leaf_evaluator(model, vocabularies),
        c_puct=c_puct,
        transition_budget=transition_budget,
        reward_config=reward_config,
    )
    index = payload.get("selected_index")
    if type(index) is not int or not 0 <= index < len(decision.actions):
        raise ValueError("PUCT selected an out-of-range public row")
    selected = decision.actions[index]
    if not any(candidate is selected for candidate in decision.actions):
        raise ValueError("PUCT must select an original public Action")
    return selected


def rollout_puct_policy(
    env: RunEnv,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    *,
    max_decisions: int,
    max_player_turns: int,
    c_puct: float = 1.5,
    transition_budget: int = 64,
    reward_config: CombatRewardConfig | None = None,
) -> PolicyEpisode:
    def choose(decision: Decision, accepted_decision_index: int) -> Action:
        del accepted_decision_index
        return select_puct_action(
            env,
            decision,
            model,
            vocabularies,
            c_puct=c_puct,
            transition_budget=transition_budget,
            reward_config=reward_config,
        )

    return _capped_public_episode(
        env,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        choose=choose,
    )
