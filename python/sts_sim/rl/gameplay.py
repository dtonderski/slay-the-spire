"""Six-arm matched PUCT gameplay over independently restored root snapshots.

This is a diagnostic rollout. It is not static imitation accuracy and does not
promote a candidate.
"""

from __future__ import annotations

import hashlib
import math
from collections.abc import Callable, Mapping, Sequence
from copy import deepcopy
from dataclasses import asdict, dataclass
from pathlib import Path
from types import MappingProxyType
from typing import Literal, cast

import torch

from ..fair import FairCombatObservation
from ..run import Action, ActionDescriptor, Decision, RunEnv
from .authorization import require_held_out_evaluation
from .data import (
    _LOADABLE_SPLITS,
    _require_canonical_root_snapshot,
    load_root_manifest,
)
from .experiment import _read_contained_regular_file_bytes
from .model import CombatModelConfig, FairCombatPolicyValueNet
from .provenance import canonical_bytes, sha256_bytes
from .records import validate_beam_search_config
from .tensor import Vocabularies, collate_combat_tensors, tensorize_combat
from .training import (
    _configure_cpu,
    _digest,
    _model_state_digest,
    _runtime_identity,
    _source_digest,
    load_training_checkpoint,
)

EpisodeStatus = Literal["won", "lost", "escaped", "truncated", "error"]
_POLICY_FAILURES = (OverflowError, RuntimeError, TypeError, ValueError)
RANDOM_CONTRACT: dict[str, object] = {
    "name": "sha256_public_descriptor_choice",
    "version": "evaluation_seed_root_id_decision_index_v1",
    "fields": [
        "evaluation_seed",
        "root_id",
        "accepted_decision_index",
        "canonical_public_action_descriptors",
    ],
}
_MATCHED_PUCT_RESTORE = "independent_from_identical_evaluation_roots"
DEFAULT_MATCHED_PUCT_MAX_DECISIONS = 128
DEFAULT_MATCHED_PUCT_MAX_PLAYER_TURNS = 40
MATCHED_PUCT_REPORT_ARMS: tuple[str, ...] = (
    "random",
    "network",
    "beam",
    "network_puct",
    "uniform_prior_network_value_puct",
    "uniform_prior_constant_value_puct",
)
_MATCHED_PUCT_SEARCH_ARMS: dict[str, dict[str, object]] = {
    "random": {"restore": _MATCHED_PUCT_RESTORE, "uses_checkpoint": False},
    "network": {"restore": _MATCHED_PUCT_RESTORE, "uses_checkpoint": True},
    "beam": {"restore": _MATCHED_PUCT_RESTORE, "uses_checkpoint": False},
    "network_puct": {
        "restore": _MATCHED_PUCT_RESTORE,
        "uses_checkpoint": True,
        "puct_search_contract": "shared",
        "prior": {"kind": "learned_policy_head", "over": "legal public actions"},
        "leaf_value": {"kind": "learned_value_head", "range": [-1.0, 1.0]},
    },
    "uniform_prior_network_value_puct": {
        "restore": _MATCHED_PUCT_RESTORE,
        "uses_checkpoint": True,
        "role": "policy-prior ablation, not an unguided baseline",
        "puct_search_contract": "shared",
        "prior": {"kind": "uniform", "over": "legal public actions"},
        "leaf_value": {
            "kind": "learned_value_head",
            "note": "uses the checkpoint value head; this is not unguided PUCT",
            "range": [-1.0, 1.0],
        },
    },
    "uniform_prior_constant_value_puct": {
        "restore": _MATCHED_PUCT_RESTORE,
        "uses_checkpoint": False,
        "role": "equal-budget unguided-search arm",
        "puct_search_contract": "shared",
        "prior": {"kind": "uniform", "over": "legal public actions"},
        "leaf_value": {
            "kind": "constant_nonlearned",
            "note": (
                "no network prior and no network value; only combat_proxy_v1 at true terminals"
            ),
            "value": 0.0,
        },
    },
}


def _freeze_mapping(value: object) -> object:
    if type(value) is dict:
        return MappingProxyType(
            {key: _freeze_mapping(item) for key, item in cast(dict[str, object], value).items()}
        )
    if type(value) is list:
        return tuple(_freeze_mapping(item) for item in cast(list[object], value))
    return value


MATCHED_PUCT_SEARCH_ARMS: Mapping[str, Mapping[str, object]] = cast(
    Mapping[str, Mapping[str, object]],
    _freeze_mapping(deepcopy(_MATCHED_PUCT_SEARCH_ARMS)),
)


def matched_puct_search_arms() -> dict[str, dict[str, object]]:
    return deepcopy(_MATCHED_PUCT_SEARCH_ARMS)


def _require_positive_int(value: object, label: str) -> int:
    if type(value) is not int or value <= 0:
        raise ValueError(f"{label} must be a positive integer")
    return value


def canonical_public_action_descriptors(
    actions: Sequence[Action] | Sequence[ActionDescriptor],
) -> list[dict[str, object]]:
    descriptors: list[dict[str, object]] = []
    for action in actions:
        descriptor = action.descriptor() if isinstance(action, Action) else action
        descriptors.append(asdict(descriptor))
    return descriptors


def random_policy_index(
    *,
    evaluation_seed: int,
    root_id: str,
    accepted_decision_index: int,
    descriptors: Sequence[Mapping[str, object]],
) -> int:
    if type(evaluation_seed) is not int:
        raise TypeError("evaluation seed must be an integer")
    if type(root_id) is not str or not root_id:
        raise TypeError("root ID must be a nonempty string")
    if type(accepted_decision_index) is not int or accepted_decision_index < 0:
        raise ValueError("accepted decision index must be a nonnegative integer")
    if not descriptors:
        raise ValueError("random policy requires at least one public action")
    payload = [
        evaluation_seed,
        root_id,
        accepted_decision_index,
        list(descriptors),
    ]
    digest = hashlib.sha256(canonical_bytes(payload)).digest()
    return int.from_bytes(digest[:8], "big") % len(descriptors)


def select_greedy_action(
    decision: Decision,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
) -> Action:
    """Argmax over current public rows; return the original sidecar Action."""

    observation = decision.observation
    if not isinstance(observation, FairCombatObservation):
        raise TypeError("greedy policy requires a fair combat observation")
    if not decision.actions:
        raise ValueError("greedy policy requires at least one public action")
    descriptors = tuple(action.descriptor() for action in decision.actions)
    tensors = tensorize_combat(observation, descriptors, vocabularies)
    batch = collate_combat_tensors((tensors,))
    was_training = model.training
    model.eval()
    try:
        with torch.inference_mode():
            logits = model(batch).logits[0, : len(decision.actions)]
            if not torch.isfinite(logits).all():
                raise ValueError("greedy policy logits are not finite")
            index = int(torch.argmax(logits).item())
    finally:
        model.train(was_training)
    if not 0 <= index < len(decision.actions):
        raise ValueError("greedy policy selected an out-of-range public row")
    return decision.actions[index]


@dataclass(frozen=True, slots=True)
class PolicyEpisode:
    status: EpisodeStatus
    accepted_decisions: int
    player_turns: int
    terminal_hp: int | None
    error: str | None = None
    truncation_trigger: str | None = None


def _require_status(value: str) -> EpisodeStatus:
    if value not in {"won", "lost", "escaped", "truncated", "error"}:
        raise ValueError(f"unknown episode status: {value}")
    return value


def _detached_player_hp(env: RunEnv) -> int:
    state = env.full_state()
    combat = state.get("combat")
    if isinstance(combat, dict):
        player = combat.get("player")
        if isinstance(player, dict):
            hp = player.get("hp")
            if type(hp) is int:
                return hp
    hp = state.get("player_hp")
    if type(hp) is not int:
        raise ValueError("detached terminal HP is missing")
    return hp


def _try_detached_player_hp(env: RunEnv) -> int | None:
    try:
        return _detached_player_hp(env)
    except (RuntimeError, TypeError, ValueError):
        return None


def _policy_error(
    error: BaseException,
    *,
    accepted_decisions: int = 0,
    player_turns: int = 1,
    terminal_hp: int | None,
) -> PolicyEpisode:
    return PolicyEpisode(
        "error",
        accepted_decisions,
        player_turns,
        terminal_hp,
        error=str(error),
    )


def _initial_combat_status(decision: Decision) -> EpisodeStatus | None:
    observation = decision.observation
    if not isinstance(observation, FairCombatObservation):
        return None
    if observation.phase == "lost" or observation.player.hp <= 0:
        return "lost"
    if observation.phase == "won":
        return "won"
    return None


def _capped_public_episode(
    env: RunEnv,
    *,
    max_decisions: int,
    max_player_turns: int,
    choose: Callable[[Decision, int], Action],
) -> PolicyEpisode:
    accepted_decisions = 0
    player_turns = 1
    try:
        decision = env.decision()
        initial = _initial_combat_status(decision)
        if initial is not None:
            return PolicyEpisode(initial, 0, 1, _detached_player_hp(env))
        while True:
            if not isinstance(decision.observation, FairCombatObservation):
                raise TypeError("public policy left combat without a native combat outcome")
            if not decision.actions:
                raise ValueError("public policy reached an empty ongoing decision")
            action = choose(decision, accepted_decisions)
            if not any(candidate is action for candidate in decision.actions):
                raise ValueError("policy must select an original public Action")
            step = env.step(action)
            accepted_decisions += 1
            player_turns = player_turns + step.player_turn_advances
            if step.combat_outcome is not None:
                return PolicyEpisode(
                    _require_status(step.combat_outcome),
                    accepted_decisions,
                    player_turns,
                    _detached_player_hp(env),
                )
            if accepted_decisions >= max_decisions:
                return PolicyEpisode(
                    "truncated",
                    accepted_decisions,
                    player_turns,
                    _detached_player_hp(env),
                    truncation_trigger="accepted_decisions",
                )
            if player_turns > max_player_turns:
                return PolicyEpisode(
                    "truncated",
                    accepted_decisions,
                    player_turns,
                    _detached_player_hp(env),
                    truncation_trigger="player_turns",
                )
            decision = step.decision
    except _POLICY_FAILURES as error:
        return _policy_error(
            error,
            accepted_decisions=accepted_decisions,
            player_turns=player_turns,
            terminal_hp=_try_detached_player_hp(env),
        )


def rollout_random_policy(
    env: RunEnv,
    *,
    evaluation_seed: int,
    root_id: str,
    max_decisions: int,
    max_player_turns: int,
) -> PolicyEpisode:
    def choose(decision: Decision, accepted_decision_index: int) -> Action:
        index = random_policy_index(
            evaluation_seed=evaluation_seed,
            root_id=root_id,
            accepted_decision_index=accepted_decision_index,
            descriptors=canonical_public_action_descriptors(decision.actions),
        )
        return decision.actions[index]

    return _capped_public_episode(
        env,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        choose=choose,
    )


def rollout_greedy_policy(
    env: RunEnv,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    *,
    max_decisions: int,
    max_player_turns: int,
) -> PolicyEpisode:
    def choose(decision: Decision, accepted_decision_index: int) -> Action:
        del accepted_decision_index
        return select_greedy_action(decision, model, vocabularies)

    return _capped_public_episode(
        env,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        choose=choose,
    )


def rollout_beam_policy(
    env: RunEnv,
    *,
    depth: int,
    width: int,
    transition_budget: int,
    max_decisions: int,
    max_player_turns: int,
    deduplicate_search_states: bool,
) -> PolicyEpisode:
    try:
        payload = env.beam_clone_episode_payload(
            depth=depth,
            width=width,
            transition_budget=transition_budget,
            max_decisions=max_decisions,
            max_player_turns=max_player_turns,
            deduplicate_search_states=deduplicate_search_states,
        )
        outcome = payload.get("outcome")
        if type(outcome) is not dict:
            raise TypeError("native beam episode outcome must be an object")
        source = cast(dict[str, object], outcome)
        status = source.get("status")
        if type(status) is not str:
            raise TypeError("native beam episode status must be a string")
        trigger = source.get("truncation_trigger")
        if trigger is not None and type(trigger) is not str:
            raise TypeError("native beam truncation trigger must be a string or null")
        hp = source.get("terminal_hp")
        if type(hp) is not int:
            raise TypeError("native beam terminal HP must be an integer")
        accepted = source.get("accepted_decisions")
        turns = source.get("player_turns")
        if type(accepted) is not int or accepted < 0 or type(turns) is not int or turns < 0:
            raise ValueError("native beam counters must be nonnegative integers")
        return PolicyEpisode(
            _require_status(status),
            accepted,
            turns,
            hp,
            truncation_trigger=trigger,
        )
    except _POLICY_FAILURES as error:
        return _policy_error(error, terminal_hp=_try_detached_player_hp(env))


def _episode_row(episode: PolicyEpisode) -> dict[str, object]:
    return {
        "status": episode.status,
        "accepted_decisions": episode.accepted_decisions,
        "player_turns": episode.player_turns,
        "terminal_hp": episode.terminal_hp,
        "error": episode.error,
        "truncation_trigger": episode.truncation_trigger,
    }


def _int_summary(values: list[int]) -> dict[str, object]:
    if not values:
        return {"count": 0, "min": None, "max": None, "sum": 0, "mean": None}
    mean = sum(values) / len(values)
    if not math.isfinite(mean):
        raise ValueError("summary mean is not finite")
    return {
        "count": len(values),
        "min": min(values),
        "max": max(values),
        "sum": sum(values),
        "mean": mean,
    }


def aggregate_policy_metrics(episodes: Sequence[PolicyEpisode]) -> dict[str, object]:
    denominator = len(episodes)
    wins = sum(episode.status == "won" for episode in episodes)
    errors = sum(episode.status == "error" for episode in episodes)
    truncations = sum(episode.status == "truncated" for episode in episodes)
    hp_values = [
        episode.terminal_hp
        for episode in episodes
        if episode.status != "error" and type(episode.terminal_hp) is int
    ]
    decision_values = [episode.accepted_decisions for episode in episodes]
    return {
        "win_numerator": wins,
        "win_denominator": denominator,
        "errors": errors,
        "truncations": truncations,
        "lost": sum(episode.status == "lost" for episode in episodes),
        "escaped": sum(episode.status == "escaped" for episode in episodes),
        "terminal_hp": _int_summary(hp_values),
        "accepted_decisions": _int_summary(decision_values),
    }


def _paired_difference(left: PolicyEpisode, right: PolicyEpisode) -> dict[str, object]:
    errored = left.status == "error" or right.status == "error"
    hp_delta = (
        None
        if errored or left.terminal_hp is None or right.terminal_hp is None
        else right.terminal_hp - left.terminal_hp
    )
    return {
        "left_status": left.status,
        "right_status": right.status,
        "status_equal": left.status == right.status,
        "left_won": left.status == "won",
        "right_won": right.status == "won",
        "errored": errored,
        "hp_delta": hp_delta,
        "accepted_decision_delta": None
        if errored
        else right.accepted_decisions - left.accepted_decisions,
    }


def aggregate_paired_differences(
    left: Sequence[PolicyEpisode], right: Sequence[PolicyEpisode]
) -> dict[str, object]:
    if len(left) != len(right):
        raise ValueError("paired policies must cover the same roots")
    pairs = [_paired_difference(left_row, right_row) for left_row, right_row in zip(left, right)]
    hp_deltas = [delta for pair in pairs if type(delta := pair["hp_delta"]) is int]
    decision_deltas = [
        delta for pair in pairs if type(delta := pair["accepted_decision_delta"]) is int
    ]
    return {
        "roots": len(pairs),
        "same_status": sum(bool(pair["status_equal"]) for pair in pairs),
        "right_won_left_not": sum(
            bool(pair["right_won"]) and not bool(pair["left_won"]) for pair in pairs
        ),
        "left_won_right_not": sum(
            bool(pair["left_won"]) and not bool(pair["right_won"]) for pair in pairs
        ),
        "hp_delta": _int_summary(hp_deltas),
        "accepted_decision_delta": _int_summary(decision_deltas),
        "per_root": pairs,
    }


def _restore_independently(snapshot_bytes: bytes, root_id: str) -> RunEnv:
    canonical = _require_canonical_root_snapshot(snapshot_bytes, root_id)
    return RunEnv.from_snapshot(canonical.decode())


def _run_restored_policy(
    snapshot_bytes: bytes,
    root_id: str,
    fallback_hp: int | None,
    run: Callable[[RunEnv], PolicyEpisode],
) -> PolicyEpisode:
    try:
        env = _restore_independently(snapshot_bytes, root_id)
        return run(env)
    except _POLICY_FAILURES as error:
        return _policy_error(error, terminal_hp=fallback_hp)


def evaluate_matched_puct_roots(
    *,
    split_roots: Sequence[tuple[str, bytes]],
    evaluation_seed: int,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    transition_budget: int,
    simulation_budget: int,
    c_puct: float,
    beam_depth: int,
    beam_width: int,
    max_decisions: int,
    max_player_turns: int,
    deduplicate_search_states: bool,
) -> dict[str, object]:
    from .puct import (
        rollout_puct_policy,
        rollout_uniform_prior_constant_value_puct_policy,
        rollout_uniform_prior_network_value_puct_policy,
    )

    root_ids = [root_id for root_id, _ in split_roots]
    if root_ids != sorted(root_ids):
        raise ValueError("matched roots must be canonically ordered")
    seen: set[str] = set()
    for root_id in root_ids:
        if root_id in seen:
            raise ValueError(f"duplicate matched root ID {root_id}")
        seen.add(root_id)
    per_root: list[dict[str, object]] = []
    random_episodes: list[PolicyEpisode] = []
    network_episodes: list[PolicyEpisode] = []
    beam_episodes: list[PolicyEpisode] = []
    network_puct_episodes: list[PolicyEpisode] = []
    uniform_prior_network_value_episodes: list[PolicyEpisode] = []
    uniform_prior_constant_value_episodes: list[PolicyEpisode] = []
    for root_id, snapshot_bytes in split_roots:
        hashes: list[str] = []
        root_hp: int | None = None
        for _ in MATCHED_PUCT_REPORT_ARMS:
            restored = _restore_independently(snapshot_bytes, root_id)
            hashes.append(restored.snapshot().hash)
            root_hp = _try_detached_player_hp(restored)
        if len(set(hashes)) != 1:
            raise ValueError(f"independent restores of root {root_id} diverged")
        random_episode = _run_restored_policy(
            snapshot_bytes,
            root_id,
            root_hp,
            lambda env, identity=root_id: rollout_random_policy(
                env,
                evaluation_seed=evaluation_seed,
                root_id=identity,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
            ),
        )
        network_episode = _run_restored_policy(
            snapshot_bytes,
            root_id,
            root_hp,
            lambda env: rollout_greedy_policy(
                env,
                model,
                vocabularies,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
            ),
        )
        beam_episode = _run_restored_policy(
            snapshot_bytes,
            root_id,
            root_hp,
            lambda env: rollout_beam_policy(
                env,
                depth=beam_depth,
                width=beam_width,
                transition_budget=transition_budget,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
                deduplicate_search_states=deduplicate_search_states,
            ),
        )
        network_puct_episode = _run_restored_policy(
            snapshot_bytes,
            root_id,
            root_hp,
            lambda env: rollout_puct_policy(
                env,
                model,
                vocabularies,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
                c_puct=c_puct,
                simulation_budget=simulation_budget,
                transition_budget=transition_budget,
            ),
        )
        uniform_prior_network_value_episode = _run_restored_policy(
            snapshot_bytes,
            root_id,
            root_hp,
            lambda env: rollout_uniform_prior_network_value_puct_policy(
                env,
                model,
                vocabularies,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
                c_puct=c_puct,
                simulation_budget=simulation_budget,
                transition_budget=transition_budget,
            ),
        )
        uniform_prior_constant_value_episode = _run_restored_policy(
            snapshot_bytes,
            root_id,
            root_hp,
            lambda env: rollout_uniform_prior_constant_value_puct_policy(
                env,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
                c_puct=c_puct,
                simulation_budget=simulation_budget,
                transition_budget=transition_budget,
            ),
        )
        random_episodes.append(random_episode)
        network_episodes.append(network_episode)
        beam_episodes.append(beam_episode)
        network_puct_episodes.append(network_puct_episode)
        uniform_prior_network_value_episodes.append(uniform_prior_network_value_episode)
        uniform_prior_constant_value_episodes.append(uniform_prior_constant_value_episode)
        per_root.append(
            {
                "root_id": root_id,
                "snapshot_sha256": sha256_bytes(snapshot_bytes),
                "restore_hash": hashes[0],
                "policies": {
                    "random": _episode_row(random_episode),
                    "network": _episode_row(network_episode),
                    "beam": _episode_row(beam_episode),
                    "network_puct": _episode_row(network_puct_episode),
                    "uniform_prior_network_value_puct": _episode_row(
                        uniform_prior_network_value_episode
                    ),
                    "uniform_prior_constant_value_puct": _episode_row(
                        uniform_prior_constant_value_episode
                    ),
                },
            }
        )
    if [cast(str, row["root_id"]) for row in per_root] != [root_id for root_id, _ in split_roots]:
        raise ValueError("matched root accounting is incomplete")
    return {
        "per_root": per_root,
        "search_arms": matched_puct_search_arms(),
        "aggregates": {
            "random": aggregate_policy_metrics(random_episodes),
            "network": aggregate_policy_metrics(network_episodes),
            "beam": aggregate_policy_metrics(beam_episodes),
            "network_puct": aggregate_policy_metrics(network_puct_episodes),
            "uniform_prior_network_value_puct": aggregate_policy_metrics(
                uniform_prior_network_value_episodes
            ),
            "uniform_prior_constant_value_puct": aggregate_policy_metrics(
                uniform_prior_constant_value_episodes
            ),
        },
        "paired": {
            "network_random": aggregate_paired_differences(random_episodes, network_episodes),
            "beam_network": aggregate_paired_differences(network_episodes, beam_episodes),
            "network_puct_network": aggregate_paired_differences(
                network_episodes, network_puct_episodes
            ),
            "network_puct_beam": aggregate_paired_differences(beam_episodes, network_puct_episodes),
            "uniform_prior_network_value_puct_network_puct": aggregate_paired_differences(
                network_puct_episodes, uniform_prior_network_value_episodes
            ),
            "uniform_prior_constant_value_puct_network_puct": aggregate_paired_differences(
                network_puct_episodes, uniform_prior_constant_value_episodes
            ),
            "uniform_prior_constant_value_puct_uniform_prior_network_value_puct": (
                aggregate_paired_differences(
                    uniform_prior_network_value_episodes, uniform_prior_constant_value_episodes
                )
            ),
        },
    }


def evaluate_matched_puct_gameplay(
    root_manifest_path: Path,
    checkpoint_path: Path,
    *,
    split: str = "development",
    evaluation_seed: int = 0,
    authorization_path: Path | None = None,
    training_root_manifest_path: Path | None = None,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    beam_depth: int = 8,
    beam_width: int = 24,
    max_decisions: int = DEFAULT_MATCHED_PUCT_MAX_DECISIONS,
    max_player_turns: int = DEFAULT_MATCHED_PUCT_MAX_PLAYER_TURNS,
    deduplicate_search_states: bool = True,
) -> dict[str, object]:
    if split not in _LOADABLE_SPLITS:
        raise PermissionError("sealed and audit splits are not available for evaluation")
    if type(evaluation_seed) is not int:
        raise TypeError("evaluation seed must be an integer")
    if type(deduplicate_search_states) is not bool:
        raise TypeError("deduplicate_search_states must be boolean")
    if type(c_puct) not in {int, float} or not math.isfinite(float(c_puct)) or float(c_puct) <= 0:
        raise ValueError("c_puct must be finite and positive")
    simulation_budget = _require_positive_int(simulation_budget, "simulation_budget")
    transition_budget = _require_positive_int(transition_budget, "transition_budget")
    beam_depth = _require_positive_int(beam_depth, "beam_depth")
    beam_width = _require_positive_int(beam_width, "beam_width")
    max_decisions = _require_positive_int(max_decisions, "max_decisions")
    max_player_turns = _require_positive_int(max_player_turns, "max_player_turns")
    beam_search_config: dict[str, object] = {
        "depth": beam_depth,
        "width": beam_width,
        "transition_budget": transition_budget,
        "max_decisions": max_decisions,
        "max_player_turns": max_player_turns,
        "deadline": None,
        "replan": "every_public_decision",
        "deduplicate_search_states": deduplicate_search_states,
    }
    validate_beam_search_config(beam_search_config)
    manifest = load_root_manifest(root_manifest_path)
    split_entries = tuple(root for root in manifest.roots if root.split == split)
    if not split_entries:
        raise ValueError(f"root manifest contains no {split} roots")
    payload, stored_config, checkpoint_file_digest = load_training_checkpoint(checkpoint_path)
    _configure_cpu(stored_config.torch_threads)
    runtime_identity = _runtime_identity()
    runtime_identity_digest = _digest(runtime_identity)
    if payload["runtime_identity_digest"] != runtime_identity_digest:
        raise ValueError("evaluation checkpoint runtime identity mismatch")
    if payload["source_digest"] != _source_digest():
        raise ValueError("evaluation checkpoint source digest mismatch")
    checkpoint_teacher_search_contract_digest = cast(str, payload["teacher_search_contract_digest"])
    training_root_manifest_digest = cast(str, payload["root_manifest_digest"])
    training_cohort_digest = cast(str, payload["cohort_digest"])
    if payload["source_epoch_bundle_digest"] != manifest.source_epoch_bundle_digest:
        raise ValueError("evaluation source-epoch-bundle digest mismatch")
    require_held_out_evaluation(
        training_root_manifest_digest=training_root_manifest_digest,
        training_cohort_digest=training_cohort_digest,
        evaluation_manifest=manifest,
        evaluation_root_manifest_path=root_manifest_path,
        evaluation_split=split,
        evaluation_seed=evaluation_seed,
        requested_evaluator_names=MATCHED_PUCT_REPORT_ARMS,
        authorization_path=authorization_path,
        training_root_manifest_path=training_root_manifest_path,
    )
    vocabularies = Vocabularies.from_dict(payload["vocabularies"])
    config = CombatModelConfig(**payload["model_config"])
    model = FairCombatPolicyValueNet(vocabularies, config)
    model.load_state_dict(payload["model_state"], strict=True)
    model.eval()
    split_roots = tuple(
        (
            root.root_id,
            _require_canonical_root_snapshot(
                _read_contained_regular_file_bytes(
                    root_manifest_path.parent, root.relative_path
                ),
                root.root_id,
            ),
        )
        for root in split_entries
    )
    matched = evaluate_matched_puct_roots(
        split_roots=split_roots,
        evaluation_seed=evaluation_seed,
        model=model,
        vocabularies=vocabularies,
        transition_budget=transition_budget,
        simulation_budget=simulation_budget,
        c_puct=float(c_puct),
        beam_depth=beam_depth,
        beam_width=beam_width,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        deduplicate_search_states=deduplicate_search_states,
    )
    report: dict[str, object] = {
        "report_version": 2,
        "kind": "matched_puct_gameplay_rollout",
        "promotion_claim": False,
        "privileged_puct": True,
        "search_arms": matched_puct_search_arms(),
        "equal_transition_budget_note": (
            "beam and privileged PUCT arms share the per-decision transition budget; "
            "equal transitions do not imply equal compute. "
            "uniform_prior_network_value_puct is a policy-prior ablation, not an unguided baseline"
        ),
        "split": split,
        "evaluation_seed": evaluation_seed,
        "requested_seeds": list(manifest.requested_seeds),
        "requested_seed_count": len(manifest.requested_seeds),
        "materialized_split_root_count": len(split_entries),
        "root_ids": [root.root_id for root in split_entries],
        "checkpoint_step": payload["global_step"],
        "checkpoint_file_digest": checkpoint_file_digest,
        "checkpoint_model_state_digest": _model_state_digest(payload["model_state"]),
        "checkpoint_config_digest": payload["config_digest"],
        "source_digest": payload["source_digest"],
        "runtime_identity_digest": runtime_identity_digest,
        "vocabulary_fingerprint": payload["vocabulary_fingerprint"],
        "encoder_contract_digest": payload["encoder_contract_digest"],
        "checkpoint_training_root_manifest_digest": training_root_manifest_digest,
        "checkpoint_training_cohort_digest": training_cohort_digest,
        "checkpoint_teacher_search_contract_digest": checkpoint_teacher_search_contract_digest,
        "root_manifest_digest": manifest.manifest_digest,
        "cohort_digest": manifest.cohort_digest,
        "c_puct": float(c_puct),
        "simulation_budget": simulation_budget,
        "transition_budget": transition_budget,
        "beam_depth": beam_depth,
        "beam_width": beam_width,
        "beam_search_config": beam_search_config,
        "deduplicate_search_states": deduplicate_search_states,
        "random_contract": RANDOM_CONTRACT,
        "random_contract_digest": _digest(RANDOM_CONTRACT),
        "max_decisions": max_decisions,
        "max_player_turns": max_player_turns,
        **matched,
    }
    report["search_arms"] = matched_puct_search_arms()
    report["report_digest"] = _digest(report)
    return report
