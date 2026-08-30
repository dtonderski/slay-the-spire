"""Privileged PUCT distillation datasets over legal combat roots."""

from __future__ import annotations

import math
from dataclasses import asdict
from pathlib import Path
from typing import Any, cast

import torch

from ..fair import FairCombatObservation
from ..run import RunEnv
from .data import (
    _ALLOWED_SPLITS,
    _AUDITED_SPLITS,
    _DATASET_ROOT_MANIFEST_PATH,
    _NATIVE_EPISODE_ERROR,
    _SOURCE_KIND,
    DATASET_MANIFEST_V6,
    DatasetExclusion,
    DatasetManifest,
    DatasetRootMembership,
    _atomic_write,
    _canonical_bytes,
    _require_empty_output_dir,
    _sha256_bytes,
    _teacher_search_contract_digest,
    load_dataset_manifest,
    load_root_manifest,
)
from .model import CombatModelConfig, FairCombatPolicyValueNet
from .provenance import capture_repository_version
from .puct import network_leaf_evaluator, puct_clone_episode_payload
from .records import (
    PUCT_TEACHER_NAME,
    PUCT_TEACHER_VERSION,
    PUCT_VALUE_TARGET_NAME,
    CombatOutcome,
    JsonValue,
    SymbolicTrainingRecord,
    action_descriptor_from_payload,
    fair_observation_digest,
    fair_observation_from_payload,
    fair_observation_payload,
    first_argmax_visits,
    validate_v3_search_config,
)
from .rewards import COMBAT_PROXY_V1, CombatRewardConfig
from .tensor import Vocabularies, encoder_contract_digest
from .training import (
    _digest,
    _model_state_digest,
    _runtime_identity,
    _source_digest,
    _validate_checkpoint_envelope,
)


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
    checkpoint_path: Path,
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
        "value_target_name": PUCT_VALUE_TARGET_NAME,
        "checkpoint_file_digest": _sha256_bytes(checkpoint_path.read_bytes()),
        "checkpoint_model_state_digest": _model_state_digest(payload["model_state"]),
        "checkpoint_config_digest": payload["config_digest"],
        "source_digest": payload["source_digest"],
        "runtime_identity_digest": payload["runtime_identity_digest"],
        "vocabulary_fingerprint": payload["vocabulary_fingerprint"],
        "encoder_contract_digest": payload["encoder_contract_digest"],
    }
    validate_v3_search_config(search_config)
    return search_config


def _load_teacher_checkpoint(
    checkpoint_path: Path,
) -> tuple[FairCombatPolicyValueNet, Vocabularies, dict[str, Any]]:
    payload, stored_config = _validate_checkpoint_envelope(
        torch.load(checkpoint_path, map_location="cpu", weights_only=False)
    )
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
    del stored_config
    return model, vocabularies, payload


def generate_puct_dataset(
    root_manifest_path: Path,
    output_dir: Path,
    checkpoint_path: Path,
    *,
    split: str = "train",
    allow_audited_split: bool = False,
    c_puct: float = 1.5,
    simulation_budget: int = 64,
    transition_budget: int = 64,
    max_decisions: int = 512,
    max_player_turns: int = 100,
    reward_config: CombatRewardConfig = COMBAT_PROXY_V1,
    repository_root: Path | None = None,
) -> DatasetManifest:
    if split not in _ALLOWED_SPLITS:
        raise ValueError("unknown dataset split")
    if split in _AUDITED_SPLITS and not allow_audited_split:
        raise PermissionError("sealed and audit splits require explicit audited access")
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
    root_manifest = load_root_manifest(
        root_manifest_path,
        allow_audited_materialization=split in _AUDITED_SPLITS and allow_audited_split,
    )
    roots = [root for root in root_manifest.roots if root.split == split]
    if not roots:
        raise ValueError(f"root manifest contains no {split} roots")
    if repository_root is None:
        repository_root = Path(__file__).resolve().parents[3]
    repository = capture_repository_version(repository_root, allow_dirty=True)
    model, vocabularies, checkpoint_payload = _load_teacher_checkpoint(checkpoint_path)
    search_config = _puct_search_config(
        c_puct=float(c_puct),
        simulation_budget=simulation_budget,
        transition_budget=transition_budget,
        max_decisions=max_decisions,
        max_player_turns=max_player_turns,
        checkpoint_path=checkpoint_path,
        payload=checkpoint_payload,
    )
    evaluator = network_leaf_evaluator(model, vocabularies)
    records: list[SymbolicTrainingRecord] = []
    teacher: tuple[str, str] | None = None
    used_roots: list[DatasetRootMembership] = []
    exclusions: list[DatasetExclusion] = []
    for root in roots:
        try:
            snapshot_text = (root_manifest_path.parent / root.relative_path).read_text()
            env = RunEnv.from_snapshot(snapshot_text)
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
            )
            if env.snapshot().hash != before_hash:
                raise ValueError("PUCT labeling mutated the restored root")
            native_teacher = (
                cast(str, payload["teacher_name"]),
                cast(str, payload["teacher_version"]),
            )
            if native_teacher != (PUCT_TEACHER_NAME, PUCT_TEACHER_VERSION):
                raise ValueError("native PUCT teacher metadata mismatch")
            if teacher is not None and teacher != native_teacher:
                raise ValueError("native teacher metadata changed within dataset")
            outcome = CombatOutcome.from_dict(payload["outcome"])
            episode_id = _sha256_bytes(
                _canonical_bytes([root.root_id, search_config, reward_config.digest])
            )
            steps = cast(list[dict[str, object]], payload["steps"])
            if not steps:
                raise ValueError("terminal or post-combat root cannot produce training records")
            root_records: list[SymbolicTrainingRecord] = []
            for decision_index, step in enumerate(steps):
                projected = FairCombatObservation._from_payload(
                    cast(dict[str, object], step["observation"])
                )
                observation = fair_observation_from_payload(fair_observation_payload(projected))
                actions = tuple(
                    action_descriptor_from_payload(
                        {"family": "combat", **cast(dict[str, object], choice)}
                    )
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
                    target = float(raw_target)
                elif type(raw_target) is float:
                    target = raw_target
                else:
                    raise TypeError("PUCT root value must be numeric")
                root_records.append(
                    SymbolicTrainingRecord(
                        observation,
                        actions,
                        selected,
                        actions[selected],
                        counts,
                        target,
                        PUCT_VALUE_TARGET_NAME,
                        outcome,
                        native_teacher[0],
                        native_teacher[1],
                        cast(dict[str, JsonValue], search_config),
                        root.root_id,
                        root.split_group_id,
                        None,
                        repository,
                        fair_observation_digest(observation),
                        3,
                        root_manifest.manifest_digest,
                        reward_config.digest,
                        _SOURCE_KIND,
                        episode_id,
                        decision_index,
                        True,
                    )
                )
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
    lines = b"".join(_canonical_bytes(record.to_dict()) + b"\n" for record in records)
    shard_name = f"{split}/{split}.jsonl"
    _atomic_write(output_dir / shard_name, lines)
    shard_digest = _sha256_bytes(lines)
    memberships = tuple(sorted(used_roots, key=lambda root: root.root_id))
    dataset_exclusions = tuple(
        sorted(
            exclusions,
            key=lambda exclusion: (exclusion.root_id, exclusion.reason, exclusion.detail),
        )
    )
    root_manifest_bytes = _canonical_bytes(root_manifest.to_dict())
    root_manifest_file_digest = _sha256_bytes(root_manifest_bytes)
    _atomic_write(output_dir / _DATASET_ROOT_MANIFEST_PATH, root_manifest_bytes)
    teacher_search_contract_digest = _teacher_search_contract_digest(
        teacher[0], teacher[1], search_config
    )
    unsigned: dict[str, object] = {
        "manifest_version": DATASET_MANIFEST_V6,
        "root_manifest_path": _DATASET_ROOT_MANIFEST_PATH,
        "root_manifest_file_digest": root_manifest_file_digest,
        "root_manifest_digest": root_manifest.manifest_digest,
        "cohort_digest": root_manifest.cohort_digest,
        "roots": [{**asdict(root), "lineages": list(root.lineages)} for root in memberships],
        "exclusions": [asdict(exclusion) for exclusion in dataset_exclusions],
        "split": split,
        "audited_access": split in _AUDITED_SPLITS,
        "reward_config": reward_config.to_dict(),
        "reward_config_digest": reward_config.digest,
        "teacher_name": teacher[0],
        "teacher_version": teacher[1],
        "teacher_search_contract_digest": teacher_search_contract_digest,
        "source_kind": _SOURCE_KIND,
        "search_config": search_config,
        "repository": repository.to_dict(),
        "shard_path": shard_name,
        "shard_digest": shard_digest,
        "record_count": len(records),
        "record_ids": [record.record_id for record in records],
    }
    manifest = DatasetManifest(
        DATASET_MANIFEST_V6,
        _DATASET_ROOT_MANIFEST_PATH,
        root_manifest_file_digest,
        root_manifest.manifest_digest,
        root_manifest.cohort_digest,
        memberships,
        dataset_exclusions,
        split,
        split in _AUDITED_SPLITS,
        reward_config.to_dict(),
        reward_config.digest,
        teacher[0],
        teacher[1],
        teacher_search_contract_digest,
        _SOURCE_KIND,
        search_config,
        repository,
        shard_name,
        shard_digest,
        len(records),
        tuple(cast(str, record.record_id) for record in records),
        _sha256_bytes(_canonical_bytes(unsigned)),
    )
    _atomic_write(output_dir / "dataset-manifest.json", _canonical_bytes(manifest.to_dict()))
    return load_dataset_manifest(
        output_dir / "dataset-manifest.json",
        requested_split=split,
        allow_audited_split=allow_audited_split,
    )
