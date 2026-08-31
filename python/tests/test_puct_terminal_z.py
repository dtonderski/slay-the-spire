from __future__ import annotations

import hashlib
import json
import math
import statistics
from collections import Counter
from pathlib import Path
from typing import cast

import pytest
import torch

import sts_sim.rl.data as data_module
from sts_sim.rl import (
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    TrainingConfig,
    evaluate_beam_clone,
    generate_legal_roots,
    generate_puct_dataset,
    load_dataset_manifest,
    read_jsonl,
    train_beam_clone,
    write_jsonl,
)
from sts_sim.rl.data import DATASET_MANIFEST_V6, DATASET_MANIFEST_V7
from sts_sim.rl.puct import PUCT_TEACHER_NAME
from sts_sim.rl.records import (
    COMBAT_PROXY_VALUE_TARGET_NAME,
    PUCT_SEARCH_ROOT_MEAN_NAME,
    PUCT_VALUE_TARGET_NAME,
    JsonValue,
    collate_training_examples,
)
from sts_sim.rl.rewards import COMBAT_PROXY_V1
from sts_sim.rl.tensor import VocabularyBuilder
from sts_sim.rl.tracking import OfflineWandbConfig
from sts_sim.rl.training import (
    TRAINING_CHECKPOINT_FORMAT,
    TRAINING_CHECKPOINT_FORMAT_V3,
    _compute_training_target_statistics,
    _validate_checkpoint_envelope,
)
from tests.test_puct_distill import _beam_train_checkpoint, _smoke_training_config
from tests.test_wandb_offline import _install_fake_wandb


def _resign_dataset_manifest(payload: dict[str, object]) -> None:
    unsigned = dict(payload)
    unsigned.pop("manifest_digest")
    payload["manifest_digest"] = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def _tiny_puct_dataset(tmp_path: Path) -> tuple[Path, Path]:
    roots, checkpoint = _beam_train_checkpoint(tmp_path)
    generate_puct_dataset(
        roots,
        tmp_path / "puct-train",
        checkpoint,
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=2,
        max_player_turns=3,
    )
    return tmp_path / "puct-train/dataset-manifest.json", checkpoint


def test_v4_record_round_trip_and_terminal_z_identity(tmp_path: Path) -> None:
    manifest_path, _checkpoint = _tiny_puct_dataset(tmp_path)
    manifest = load_dataset_manifest(manifest_path, requested_split="train")
    assert manifest.manifest_version == DATASET_MANIFEST_V7
    records = tuple(read_jsonl(manifest_path.parent / manifest.shard_path))
    assert records
    assert all(record.record_version == 4 for record in records)
    rebuilt = tuple(SymbolicTrainingRecord.from_dict(record.to_dict()) for record in records)
    assert [record.to_dict() for record in rebuilt] == [record.to_dict() for record in records]
    for record in records:
        expected = COMBAT_PROXY_V1.value(record.outcome)
        assert record.value_target_name == COMBAT_PROXY_VALUE_TARGET_NAME
        assert record.target_value == expected
        assert record.value_target_mask is (expected is not None)
        assert record.search_root_mean_value is not None
        assert math.isfinite(record.search_root_mean_value)
        assert -1.0 <= record.search_root_mean_value <= 1.0
        assert record.search_config["value_target_name"] == COMBAT_PROXY_VALUE_TARGET_NAME
        assert record.search_config["search_root_mean_name"] == PUCT_SEARCH_ROOT_MEAN_NAME
        if record.outcome.truncated:
            assert expected is None
            assert record.target_value is None
            assert record.value_target_mask is False


def test_search_root_mean_is_not_tensorized(tmp_path: Path) -> None:
    manifest_path, _checkpoint = _tiny_puct_dataset(tmp_path)
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    builder = VocabularyBuilder()
    for record in records:
        builder.add(record.observation, record.actions)
    dataset = SymbolicCombatDataset(records, builder.freeze())
    example = dataset[0]
    assert not hasattr(example, "search_root_mean_value")
    assert records[0].search_root_mean_value is not None
    if records[0].value_target_mask:
        assert float(example.value_target) == records[0].target_value
        assert float(example.value_target) != records[0].search_root_mean_value or (
            records[0].target_value == records[0].search_root_mean_value
        )
    else:
        assert float(example.value_target) == 0.0
        assert bool(example.value_target_mask) is False
    batch = collate_training_examples((example,))
    assert not hasattr(batch, "search_root_mean_value")


def test_v4_rejects_malformed_records_and_unknown_fields(tmp_path: Path) -> None:
    manifest_path, _checkpoint = _tiny_puct_dataset(tmp_path)
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    payload = records[0].to_dict()
    extra = dict(payload)
    extra["unexpected"] = True
    with pytest.raises(ValueError, match="missing or unknown"):
        SymbolicTrainingRecord.from_dict(extra)
    missing = dict(payload)
    missing.pop("search_root_mean_value")
    with pytest.raises(ValueError, match="missing or unknown"):
        SymbolicTrainingRecord.from_dict(missing)
    infinite = dict(payload)
    infinite["search_root_mean_value"] = math.inf
    with pytest.raises(ValueError, match="finite"):
        SymbolicTrainingRecord.from_dict(infinite)
    guessed = dict(payload)
    guessed["record_version"] = 3
    with pytest.raises(ValueError, match="missing or unknown"):
        SymbolicTrainingRecord.from_dict(guessed)


def test_v3_v6_compatibility_still_loads(tmp_path: Path) -> None:
    manifest_path, _checkpoint = _tiny_puct_dataset(tmp_path)
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    converted: list[SymbolicTrainingRecord] = []
    for record in records:
        search_config = cast(dict[str, object], record.to_dict()["search_config"])
        search_config.pop("search_root_mean_name")
        search_config["value_target_name"] = PUCT_VALUE_TARGET_NAME
        converted.append(
            SymbolicTrainingRecord(
                observation=record.observation,
                actions=record.actions,
                chosen_action_index=record.chosen_action_index,
                chosen_action=record.chosen_action,
                teacher_visit_counts=record.teacher_visit_counts,
                target_value=record.search_root_mean_value,
                value_target_name=PUCT_VALUE_TARGET_NAME,
                outcome=record.outcome,
                planner_name=record.planner_name,
                planner_version=record.planner_version,
                search_config=cast(dict[str, JsonValue], search_config),
                root_id=record.root_id,
                split_group_id=record.split_group_id,
                teacher_pair_id=record.teacher_pair_id,
                repository=record.repository,
                observation_digest=record.observation_digest,
                record_version=3,
                root_manifest_digest=record.root_manifest_digest,
                reward_config_digest=record.reward_config_digest,
                source_kind=record.source_kind,
                episode_id=record.episode_id,
                decision_index=record.decision_index,
                value_target_mask=True,
            )
        )
    shard = tmp_path / "puct-train/train/train.jsonl"
    write_jsonl(shard, converted)
    manifest_payload = json.loads(manifest_path.read_text())
    manifest_payload["manifest_version"] = DATASET_MANIFEST_V6
    manifest_payload["search_config"] = dict(converted[0].search_config)
    manifest_payload["teacher_search_contract_digest"] = (
        data_module._teacher_search_contract_digest(
            cast(str, manifest_payload["teacher_name"]),
            cast(str, manifest_payload["teacher_version"]),
            cast(dict[str, object], manifest_payload["search_config"]),
        )
    )
    manifest_payload["shard_digest"] = hashlib.sha256(shard.read_bytes()).hexdigest()
    manifest_payload["record_ids"] = [cast(str, record.record_id) for record in converted]
    _resign_dataset_manifest(manifest_payload)
    manifest_path.write_text(json.dumps(manifest_payload, sort_keys=True, separators=(",", ":")))
    loaded = load_dataset_manifest(manifest_path, requested_split="train")
    assert loaded.manifest_version == DATASET_MANIFEST_V6
    assert all(record.record_version == 3 for record in converted)


def test_checkpoint_v4_stores_stats_and_accepts_authentic_v3(tmp_path: Path) -> None:
    manifest_path, _teacher = _tiny_puct_dataset(tmp_path)
    student = tmp_path / "student.pt"
    train_beam_clone(manifest_path, student, _smoke_training_config())
    payload = torch.load(student, map_location="cpu", weights_only=False)
    assert payload["checkpoint_format"] == TRAINING_CHECKPOINT_FORMAT
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    assert payload["training_target_statistics"] == _compute_training_target_statistics(records)
    _validate_checkpoint_envelope(payload)
    v3 = dict(payload)
    v3.pop("training_target_statistics")
    v3["checkpoint_format"] = TRAINING_CHECKPOINT_FORMAT_V3
    v3_path = tmp_path / "student-v3.pt"
    torch.save(v3, v3_path)
    loaded_v3, _config = _validate_checkpoint_envelope(
        torch.load(v3_path, map_location="cpu", weights_only=False)
    )
    assert loaded_v3["checkpoint_format"] == TRAINING_CHECKPOINT_FORMAT_V3
    assert "training_target_statistics" not in loaded_v3
    mixed = dict(payload)
    mixed["checkpoint_format"] = TRAINING_CHECKPOINT_FORMAT_V3
    with pytest.raises(ValueError, match="unsupported or malformed"):
        _validate_checkpoint_envelope(mixed)
    tampered = dict(payload)
    stats = dict(tampered["training_target_statistics"])
    stats["mean"] = 0.123456
    unsigned = {key: stats[key] for key in ("count", "mean", "min", "max", "population_stddev")}
    stats["digest"] = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    ).hexdigest()
    tampered["training_target_statistics"] = stats
    bad = tmp_path / "tampered.pt"
    torch.save(tampered, bad)
    with pytest.raises(ValueError, match="target statistics"):
        train_beam_clone(manifest_path, bad, _smoke_training_config(), resume=True)


def test_resume_preserves_v4_stats_and_rng(tmp_path: Path) -> None:
    manifest_path, _teacher = _tiny_puct_dataset(tmp_path)
    config = TrainingConfig(
        batch_size=2,
        total_steps=2,
        model_width=16,
        model_heads=4,
        model_layers=1,
        feedforward_width=32,
        minimum_roots=1,
        minimum_lineages=1,
    )
    first = tmp_path / "first.pt"
    train_beam_clone(manifest_path, first, config, stop_after_steps=1)
    resumed = tmp_path / "resumed.pt"
    resumed.write_bytes(first.read_bytes())
    train_beam_clone(manifest_path, resumed, config, resume=True)
    direct = tmp_path / "direct.pt"
    train_beam_clone(manifest_path, direct, config)
    left = torch.load(resumed, map_location="cpu", weights_only=False)
    right = torch.load(direct, map_location="cpu", weights_only=False)
    assert left["training_target_statistics"] == right["training_target_statistics"]
    assert left["global_step"] == right["global_step"] == 2
    for name in left["model_state"]:
        assert torch.equal(left["model_state"][name], right["model_state"][name])


def test_static_v4_metrics_match_formulas(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0", "BEAMCLONE12"], max_run_steps=128)
    from sts_sim.rl import generate_beam_dataset

    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "beam-train",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "beam-dev",
        split="development",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    checkpoint = tmp_path / "beam.pt"
    train_beam_clone(
        tmp_path / "beam-train/dataset-manifest.json", checkpoint, _smoke_training_config()
    )
    report = evaluate_beam_clone(
        tmp_path / "beam-dev/dataset-manifest.json", checkpoint, split="development"
    )
    records = tuple(read_jsonl(tmp_path / "beam-dev/development/development.jsonl"))
    assert report["report_version"] == 4
    assert report["exact_denominator"] == len(records)
    assert cast(int, report["errors"]) + cast(int, report["exact_numerator"]) <= cast(
        int, report["exact_denominator"]
    )
    tied = sum(
        sum(value == max(record.teacher_visit_counts) for value in record.teacher_visit_counts) > 1
        for record in records
    )
    assert report["tied_visit_argmax_records"] == tied
    assert report["always_first_index_numerator"] == sum(
        record.chosen_action_index == 0 for record in records
    )
    assert report["root_count"] == len({record.root_id for record in records})
    sizes = tuple(Counter(record.root_id for record in records).values())
    assert report["kish_cluster_ess"] == (sum(sizes) ** 2) / sum(size * size for size in sizes)
    payload = torch.load(checkpoint, map_location="cpu", weights_only=False)
    training_mean = payload["training_target_statistics"]["mean"]
    assert report["training_target_mean"] == training_mean
    pairs = [
        (row["predicted_value"], row["target_value"])
        for row in cast(list[dict[str, object]], report["per_record"])
        if "error" not in row and row["value_target_mask"] is True
    ]
    assert report["value_mae_rows"] == len(pairs)
    if pairs:
        targets = [float(cast(int | float, target)) for _predicted, target in pairs]
        preds = [float(cast(int | float, predicted)) for predicted, _target in pairs]
        assert report["value_mae"] == statistics.fmean(
            [abs(predicted - target) for predicted, target in zip(preds, targets, strict=True)]
        )
        assert report["training_target_mean_mae"] == statistics.fmean(
            [abs(target - float(training_mean)) for target in targets]
        )
        pred_mean = statistics.fmean(preds)
        assert report["prediction_mean"] == pred_mean
        assert report["prediction_mean_mae"] == statistics.fmean(
            [abs(target - pred_mean) for target in targets]
        )
        if len(pairs) < 8 or statistics.pstdev(preds) == 0.0 or statistics.pstdev(targets) == 0.0:
            assert report["pearson_correlation"] is None
            assert report["pearson_undefined_reason"] in {"n_lt_8", "zero_variance"}
        else:
            assert report["pearson_correlation"] == statistics.correlation(preds, targets)
            assert report["pearson_undefined_reason"] is None


def test_wandb_v7_metadata(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    fake = _install_fake_wandb(monkeypatch)
    manifest_path, _teacher = _tiny_puct_dataset(tmp_path)
    train_beam_clone(
        manifest_path,
        tmp_path / "student.pt",
        _smoke_training_config(),
        wandb_offline=OfflineWandbConfig(project="demo", directory=tmp_path / "wandb"),
    )
    assert fake.run is not None
    assert fake.run.config["dataset_manifest_version"] == DATASET_MANIFEST_V7
    assert fake.run.config["puct_targets_in_training"] is True
    assert fake.run.config["trainer"] == "privileged_puct_distill"
    assert fake.run.config["teacher_name"] == PUCT_TEACHER_NAME
