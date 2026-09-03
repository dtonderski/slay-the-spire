"""Privileged PUCT distillation datasets over legal combat roots."""

from __future__ import annotations

import math
from pathlib import Path
from typing import Any, cast

from .data import (
    _NATIVE_EPISODE_ERROR,
    _SOURCE_KIND,
    DatasetExclusion,
    DatasetManifest,
    DatasetRootMembership,
    _package_repository_root,
    _publish_dataset,
    _require_empty_output_dir,
    _require_loadable_split,
    _restore_labeled_root,
    load_root_manifest,
)
from .model import CombatModelConfig, FairCombatPolicyValueNet
from .provenance import capture_repository_version
from .puct import network_leaf_evaluator, puct_clone_episode_payload
from .records import (
    COMBAT_PROXY_VALUE_TARGET_NAME,
    PUCT_SEARCH_ROOT_MEAN_NAME,
    PUCT_TEACHER_NAME,
    PUCT_TEACHER_VERSION,
    RECORD_VERSION,
    CombatOutcome,
    JsonValue,
    SymbolicTrainingRecord,
    action_descriptor_from_payload,
    canonical_episode_id,
    fair_observation_digest,
    fair_observation_from_payload,
    first_argmax_visits,
    validate_puct_search_config,
)
from .rewards import COMBAT_PROXY_V1, CombatRewardConfig
from .tensor import Vocabularies, encoder_contract_digest
from .training import (
    _configure_cpu,
    _digest,
    _model_state_digest,
    _runtime_identity,
    _source_digest,
    load_training_checkpoint,
)


class AuthoritativeRootMutationError(RuntimeError):
    """Hard abort if PUCT labeling mutates a restored authoritative root."""


def _require_int(value: object, label: str) -> int:
    if type(value) is not int:
        raise TypeError(f"{label} must be an integer")
    return value


def _puct_search_config(
    *,
    c_puct: float,
    simulation_budget: int,
    transition_budget: int,
    max_decisions: int,
    max_player_turns: int,
    payload: dict[str, Any],
) -> dict[str, object]:
    search_config: dict[str, object] = {
        "c_puct": float(c_puct),
        "simulation_budget": simulation_budget,
        "transition_budget": transition_budget,
        "max_decisions": max_decisions,
        "max_player_turns": max_player_turns,
        "deadline": None,
        "replan": "every_public_decision",
        "privileged": True,
        "leaf_schema": "fair_leaf_batch_v1",
        "value_target_name": COMBAT_PROXY_VALUE_TARGET_NAME,
        "search_root_mean_name": PUCT_SEARCH_ROOT_MEAN_NAME,
        "checkpoint_file_digest": payload["checkpoint_file_digest"],
        "checkpoint_model_state_digest": _model_state_digest(payload["model_state"]),
        "checkpoint_config_digest": payload["config_digest"],
        "source_digest": payload["source_digest"],
        "runtime_identity_digest": payload["runtime_identity_digest"],
        "vocabulary_fingerprint": payload["vocabulary_fingerprint"],
        "encoder_contract_digest": payload["encoder_contract_digest"],
    }
    validate_puct_search_config(search_config)
    return search_config


def _load_teacher_checkpoint(
    checkpoint_path: Path,
) -> tuple[FairCombatPolicyValueNet, Vocabularies, dict[str, Any]]:
    payload, stored_config, file_digest = load_training_checkpoint(checkpoint_path)
    _configure_cpu(stored_config.torch_threads)
    source_digest = _source_digest()
    runtime_identity_digest = _digest(_runtime_identity())
    if payload["source_digest"] != source_digest:
        raise ValueError("PUCT teacher checkpoint source digest mismatch")
    if payload["runtime_identity_digest"] != runtime_identity_digest:
        raise ValueError("PUCT teacher checkpoint runtime identity mismatch")
    vocabularies = Vocabularies.from_dict(payload["vocabularies"])
    if payload["vocabulary_fingerprint"] != vocabularies.fingerprint:
        raise ValueError("PUCT teacher checkpoint vocabulary mismatch")
    if payload["encoder_contract_digest"] != encoder_contract_digest(vocabularies):
        raise ValueError("PUCT teacher checkpoint encoder contract mismatch")
    model = FairCombatPolicyValueNet(vocabularies, CombatModelConfig(**payload["model_config"]))
    model.load_state_dict(payload["model_state"], strict=True)
    model.eval()
    payload = dict(payload)
    payload["checkpoint_file_digest"] = file_digest
    return model, vocabularies, payload


def generate_puct_dataset(
    root_manifest_path: Path,
    output_dir: Path,
    checkpoint_path: Path,
    *,
    split: str = "train",
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    max_decisions: int = 512,
    max_player_turns: int = 100,
    reward_config: CombatRewardConfig = COMBAT_PROXY_V1,
) -> DatasetManifest:
    split = _require_loadable_split(split)
    if type(c_puct) not in {int, float} or not math.isfinite(float(c_puct)) or float(c_puct) <= 0:
        raise ValueError("c_puct must be finite and positive")
    for label, value in (
        ("simulation_budget", simulation_budget),
        ("transition_budget", transition_budget),
        ("max_decisions", max_decisions),
        ("max_player_turns", max_player_turns),
    ):
        if type(value) is not int or value <= 0:
            raise ValueError(f"{label} must be positive")
    _require_empty_output_dir(output_dir)
    root_manifest = load_root_manifest(root_manifest_path)
    roots = [root for root in root_manifest.roots if root.split == split]
    if not roots:
        raise ValueError(f"root manifest contains no {split} roots")
    repository = capture_repository_version(_package_repository_root())
    if repository != root_manifest.repository:
        raise ValueError(
            "package repository identity does not match the authenticated root manifest"
        )
    model, vocabularies, checkpoint_payload = _load_teacher_checkpoint(checkpoint_path)
    if checkpoint_payload["source_epoch_bundle_digest"] != root_manifest.source_epoch_bundle_digest:
        raise ValueError("PUCT teacher checkpoint source-epoch-bundle digest mismatch")
    search_config = _puct_search_config(
        c_puct=float(c_puct),
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        payload=checkpoint_payload,
    )
    evaluator = network_leaf_evaluator(model, vocabularies)
    records: list[SymbolicTrainingRecord] = []
    teacher: tuple[str, str] | None = None
    used_roots: list[DatasetRootMembership] = []
    exclusions: list[DatasetExclusion] = []
    for root in roots:
        try:
            env = _restore_labeled_root(root_manifest_path.parent, root)
            before_hash = env.snapshot().hash
            payload = puct_clone_episode_payload(
                env,
                evaluator,
                c_puct=float(c_puct),
                simulation_budget=simulation_budget,
                transition_budget=transition_budget,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
                reward_config=reward_config,
                leaf_cache="exact_state",
            )
            if env.snapshot().hash != before_hash:
                raise AuthoritativeRootMutationError(
                    f"PUCT labeling mutated restored root {root.root_id}"
                )
            native_teacher = (
                cast(str, payload["teacher_name"]),
                cast(str, payload["teacher_version"]),
            )
            if native_teacher != (PUCT_TEACHER_NAME, PUCT_TEACHER_VERSION):
                raise ValueError("native PUCT teacher metadata mismatch")
            if teacher is not None and teacher != native_teacher:
                raise ValueError("native teacher metadata changed within dataset")
            outcome = CombatOutcome.from_dict(payload["outcome"])
            episode_id = canonical_episode_id(root.root_id, search_config, reward_config.digest)
            steps = cast(list[dict[str, object]], payload["steps"])
            if not steps:
                raise ValueError("terminal or post-combat root cannot produce training records")
            root_records: list[SymbolicTrainingRecord] = []
            for decision_index, step in enumerate(steps):
                observation = fair_observation_from_payload(step["observation"])
                actions = tuple(
                    action_descriptor_from_payload(choice)
                    for choice in cast(list[object], step["choices"])
                )
                selected = _require_int(step["selected_index"], "selected index")
                counts = tuple(
                    _require_int(value, "teacher visit count")
                    for value in cast(list[object], step["visits"])
                )
                simulations = _require_int(step["completed_simulations"], "completed simulations")
                transitions = _require_int(step["transitions"], "transitions")
                if sum(counts) != simulations:
                    raise ValueError("PUCT visit mass must equal completed simulations")
                if transitions > transition_budget or simulations > simulation_budget:
                    raise ValueError("PUCT episode overshot its search budgets")
                if selected != first_argmax_visits(counts):
                    raise ValueError("PUCT selected index is not the first visit-count argmax")
                raw_target = step["value"]
                if type(raw_target) is int:
                    numeric_target: int | float = raw_target
                elif type(raw_target) is float:
                    numeric_target = raw_target
                else:
                    raise TypeError("PUCT root value must be numeric")
                try:
                    root_mean = float(numeric_target)
                except OverflowError as error:
                    raise ValueError("PUCT root value is not representable as float") from error
                if not math.isfinite(root_mean) or not -1.0 <= root_mean <= 1.0:
                    raise ValueError("PUCT root value must be finite and in [-1, 1]")
                terminal_target = reward_config.value(outcome)
                root_records.append(
                    SymbolicTrainingRecord.create(
                        observation,
                        actions,
                        selected,
                        actions[selected],
                        counts,
                        terminal_target,
                        COMBAT_PROXY_VALUE_TARGET_NAME,
                        outcome,
                        native_teacher[0],
                        native_teacher[1],
                        cast(dict[str, JsonValue], search_config),
                        root.root_id,
                        root.split_group_id,
                        None,
                        repository,
                        fair_observation_digest(observation),
                        RECORD_VERSION,
                        root_manifest.manifest_digest,
                        reward_config.digest,
                        _SOURCE_KIND,
                        episode_id,
                        decision_index,
                        terminal_target is not None,
                        root_mean,
                    )
                )
        except AuthoritativeRootMutationError:
            raise
        except (
            AttributeError,
            IndexError,
            KeyError,
            OverflowError,
            RuntimeError,
            TypeError,
            ValueError,
        ) as error:
            detail = str(error).strip() or type(error).__name__
            exclusions.append(DatasetExclusion(root.root_id, _NATIVE_EPISODE_ERROR, detail))
            continue
        if teacher is None:
            teacher = native_teacher
        records.extend(root_records)
        used_roots.append(
            DatasetRootMembership(root.root_id, root.split_group_id, root.split, root.lineages)
        )
    if teacher is None:
        raise RuntimeError(
            f"all {len(roots)} {split} roots failed native episode labeling; "
            "no dataset was published"
        )
    return _publish_dataset(
        output_dir,
        split=split,
        root_manifest=root_manifest,
        root_manifest_path=root_manifest_path,
        records=records,
        used_roots=used_roots,
        exclusions=exclusions,
        teacher=teacher,
        search_config=search_config,
        reward_config=reward_config,
        repository=repository,
    )
