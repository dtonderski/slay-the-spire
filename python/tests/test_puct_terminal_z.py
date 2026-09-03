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

from sts_sim.rl import (
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    TrainingConfig,
    evaluate_beam_clone,
    generate_puct_dataset,
    load_dataset_manifest,
    read_jsonl,
    train_beam_clone,
    write_jsonl,
)
from sts_sim.rl.data import DATASET_MANIFEST_VERSION
from sts_sim.rl.model import FairCombatPolicyValueNet, PolicyValueOutput
from sts_sim.rl.records import (
    COMBAT_PROXY_VALUE_TARGET_NAME,
    PUCT_SEARCH_ROOT_MEAN_NAME,
    CombatOutcome,
    canonical_episode_id,
    collate_training_examples,
)
from sts_sim.rl.rewards import COMBAT_PROXY_V1
from sts_sim.rl.tensor import BatchedCombatDecision, VocabularyBuilder
from sts_sim.rl.training import (
    TRAINING_CHECKPOINT_FORMAT,
    _bounded_fmean,
    _canonical_unmasked_target,
    _compute_training_target_statistics,
    _kish_ess,
    _mean_absolute_deviation,
    _pearson_correlation,
    _validate_checkpoint_envelope,
)
from tests.test_puct_distill import _beam_train_checkpoint, _smoke_training_config


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
        max_decisions=32,
        max_player_turns=20,
    )
    return tmp_path / "puct-train/dataset-manifest.json", checkpoint


def test_v4_record_round_trip_and_terminal_z_identity(tmp_path: Path) -> None:
    manifest_path, _checkpoint = _tiny_puct_dataset(tmp_path)
    manifest = load_dataset_manifest(manifest_path, requested_split="train")
    assert manifest.manifest_version == DATASET_MANIFEST_VERSION
    records = tuple(read_jsonl(manifest_path.parent / manifest.shard_path))
    assert records
    assert all(record.record_version == 4 for record in records)
    terminal = [record for record in records if not record.outcome.truncated]
    assert terminal
    assert {record.outcome.status for record in terminal} <= {"won", "lost", "escaped"}
    assert _canonical_unmasked_target(terminal[0]) == float(
        cast(int | float, terminal[0].target_value)
    )
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
        assert record.episode_id != "legacy"
        assert record.episode_id == canonical_episode_id(
            record.root_id, record.search_config, record.reward_config_digest
        )
        if record.outcome.truncated:
            assert expected is None
            assert record.target_value is None
            assert record.value_target_mask is False
        else:
            assert expected is not None
            assert record.target_value == expected
            assert record.value_target_mask is True


def test_search_root_mean_is_not_tensorized(tmp_path: Path) -> None:
    manifest_path, _checkpoint = _tiny_puct_dataset(tmp_path)
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    builder = VocabularyBuilder()
    for record in records:
        builder.add(record.observation, record.actions)
    dataset = SymbolicCombatDataset(records, builder.freeze())
    distinct = next(
        record
        for record in records
        if record.value_target_mask and record.target_value != record.search_root_mean_value
    )
    index = records.index(distinct)
    example = dataset[index]
    assert not hasattr(example, "search_root_mean_value")
    assert distinct.search_root_mean_value is not None
    assert distinct.target_value is not None
    target = float(distinct.target_value)
    root_mean = float(distinct.search_root_mean_value)
    target32 = float(torch.tensor(target, dtype=torch.float32))
    root_mean32 = float(torch.tensor(root_mean, dtype=torch.float32))
    assert target != root_mean
    assert target32 != root_mean32
    assert float(example.value_target) == target32
    assert bool(example.value_target_mask) is True
    batch = collate_training_examples((example,))
    assert not hasattr(batch, "search_root_mean_value")
    assert float(batch.value_target[0]) == target32


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
    with pytest.raises(ValueError, match="unsupported training record version"):
        SymbolicTrainingRecord.from_dict(guessed)


def test_checkpoint_v4_stores_stats_and_rejects_v3(tmp_path: Path) -> None:
    manifest_path, _teacher = _tiny_puct_dataset(tmp_path)
    student = tmp_path / "student.pt"
    train_beam_clone(manifest_path, student, _smoke_training_config())
    payload = torch.load(student, map_location="cpu", weights_only=False)
    assert payload["checkpoint_format"] == TRAINING_CHECKPOINT_FORMAT
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    stats = _compute_training_target_statistics(records)
    assert cast(int, stats["count"]) > 0
    assert stats["mean"] is not None
    assert payload["training_target_statistics"] == stats
    original_stats = dict(payload["training_target_statistics"])
    _validate_checkpoint_envelope(payload)
    assert payload["training_target_statistics"] == original_stats
    v3 = dict(payload)
    v3.pop("training_target_statistics")
    v3["checkpoint_format"] = 3
    v3_path = tmp_path / "student-v3.pt"
    torch.save(v3, v3_path)
    with pytest.raises(ValueError, match="unsupported or malformed"):
        _validate_checkpoint_envelope(torch.load(v3_path, map_location="cpu", weights_only=False))
    mixed = dict(payload)
    mixed["checkpoint_format"] = 3
    with pytest.raises(ValueError, match="unsupported or malformed"):
        _validate_checkpoint_envelope(mixed)
    tampered = dict(payload)
    stats = dict(tampered["training_target_statistics"])
    stats["population_stddev"] = float(cast(int | float, stats["population_stddev"])) + 0.01
    unsigned = {key: stats[key] for key in ("count", "mean", "min", "max", "population_stddev")}
    stats["digest"] = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    ).hexdigest()
    tampered["training_target_statistics"] = stats
    bad = tmp_path / "tampered.pt"
    torch.save(tampered, bad)
    with pytest.raises(ValueError, match="^training checkpoint target statistics mismatch$"):
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
    manifest_path, _teacher = _tiny_puct_dataset(tmp_path)
    checkpoint = tmp_path / "student.pt"
    train_beam_clone(manifest_path, checkpoint, _smoke_training_config())
    report = evaluate_beam_clone(manifest_path, checkpoint, split="train")
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    assert report["report_version"] == 4
    assert report["exact_denominator"] == len(records)
    assert report["errors"] == 0
    assert cast(int, report["exact_numerator"]) <= len(records)
    tied = sum(
        sum(value == max(record.teacher_visit_counts) for value in record.teacher_visit_counts) > 1
        for record in records
    )
    assert report["tied_visit_argmax_records"] == tied
    assert report["always_first_index_numerator"] == sum(
        record.chosen_action_index == 0 for record in records
    )
    assert report["always_first_index_in_max_visit_set_numerator"] == sum(
        record.teacher_visit_counts[0] == max(record.teacher_visit_counts) for record in records
    )
    assert (
        report["always_first_index_numerator"]
        == report["always_first_index_in_max_visit_set_numerator"]
    )
    assert (
        report["always_first_index_accuracy"]
        == report["always_first_index_in_max_visit_set_accuracy"]
    )
    assert "equivalent to always_first_index" in str(report["always_first_index_denominator_note"])
    assert report["always_first_index_denominator"] == len(records)
    assert report["root_count"] == len({record.root_id for record in records})
    sizes = tuple(Counter(record.root_id for record in records).values())
    assert report["kish_cluster_ess"] == _kish_ess(sizes)
    payload = torch.load(checkpoint, map_location="cpu", weights_only=False)
    training_mean = payload["training_target_statistics"]["mean"]
    assert report["training_target_mean"] == training_mean
    assert report["training_target_mean_undefined_reason"] is None
    successful = [
        row
        for row in cast(list[dict[str, object]], report["per_record"])
        if "error" not in row and row["value_target_mask"] is True
    ]
    assert successful
    shard_targets = [
        _canonical_unmasked_target(record)
        for record, row in zip(
            records, cast(list[dict[str, object]], report["per_record"]), strict=True
        )
        if "error" not in row and record.value_target_mask
    ]
    preds = [float(cast(int | float, row["predicted_value"])) for row in successful]
    assert report["value_mae_rows"] == len(shard_targets) == len(preds)
    expected_mean = _bounded_fmean(shard_targets)
    assert report["target_value_mean"] == expected_mean
    assert report["value_mae"] == statistics.fmean(
        [abs(predicted - target) for predicted, target in zip(preds, shard_targets, strict=True)]
    )
    assert report["training_target_mean_mae"] == _mean_absolute_deviation(
        shard_targets, float(training_mean)
    )
    pred_mean = statistics.fmean(preds)
    assert report["prediction_mean"] == pred_mean
    assert report["prediction_mean_mae"] == _mean_absolute_deviation(shard_targets, pred_mean)
    assert report["target_mean_mae"] == _mean_absolute_deviation(shard_targets, expected_mean)
    if set(shard_targets) == {0.95}:
        assert report["target_value_mean"] == report["target_value_max"] == 0.95
        assert report["target_mean_mae"] == 0.0
    assert report["target_median_mae"] == _mean_absolute_deviation(
        shard_targets, float(statistics.median(shard_targets))
    )
    pair_sizes = tuple(Counter(cast(str, row["root_id"]) for row in successful).values())
    assert report["value_pair_root_count"] == len(pair_sizes)
    assert report["value_pair_kish_cluster_ess"] == _kish_ess(pair_sizes)
    expected_pearson, expected_reason = _pearson_correlation(preds, shard_targets)
    assert report["pearson_correlation"] == expected_pearson
    assert report["pearson_undefined_reason"] == expected_reason


def test_bounded_fmean_clamps_identical_0_95_labels() -> None:
    values = (0.95,) * 19
    raw = statistics.fmean(values)
    bounded = _bounded_fmean(values)
    assert raw > max(values)
    assert bounded == max(values) == min(values) == 0.95
    assert _mean_absolute_deviation(values, bounded) == 0.0
    stats = {
        "count": len(values),
        "mean": bounded,
        "min": min(values),
        "max": max(values),
        "population_stddev": statistics.pstdev(values),
    }
    assert min(values) <= stats["mean"] <= max(values)


def test_value_metric_helpers_cover_defined_and_undefined_paths() -> None:
    assert _kish_ess(()) is None
    assert _kish_ess((2, 2)) == 2.0
    short_pred = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]
    short_targ = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6]
    assert _pearson_correlation(short_pred, short_targ) == (None, "n_lt_8")
    constant = [0.5] * 8
    varied = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]
    assert _pearson_correlation(varied, constant) == (None, "zero_variance")
    defined_targ = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]
    correlation, reason = _pearson_correlation(varied, defined_targ)
    assert reason is None
    assert correlation == statistics.correlation(varied, defined_targ)
    values = [0.2, 0.4, 0.6]
    assert _mean_absolute_deviation(values, 0.4) == statistics.fmean([0.2, 0.0, 0.2])
    awkward = 0.9388888888888889
    assert float(torch.tensor(awkward, dtype=torch.float32)) != awkward


def test_inference_error_stays_in_model_denominator(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest_path, _teacher = _tiny_puct_dataset(tmp_path)
    student = tmp_path / "student.pt"
    train_beam_clone(manifest_path, student, _smoke_training_config())
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    calls = {"n": 0}
    original = FairCombatPolicyValueNet.forward

    def boom(self: FairCombatPolicyValueNet, batch: BatchedCombatDecision) -> PolicyValueOutput:
        calls["n"] += 1
        if calls["n"] == 1:
            raise RuntimeError("injected inference failure")
        return original(self, batch)

    monkeypatch.setattr(FairCombatPolicyValueNet, "forward", boom)
    report = evaluate_beam_clone(manifest_path, student, split="train")
    assert report["errors"] == 1
    assert report["exact_denominator"] == len(records)
    assert report["always_first_index_denominator"] == len(records)
    assert report["always_first_index_in_max_visit_set_denominator"] == len(records)
    assert report["always_first_index_numerator"] == sum(
        record.chosen_action_index == 0 for record in records
    )
    assert report["always_first_index_in_max_visit_set_numerator"] == sum(
        record.teacher_visit_counts[0] == max(record.teacher_visit_counts) for record in records
    )
    assert (
        report["always_first_index_numerator"]
        == report["always_first_index_in_max_visit_set_numerator"]
    )
    assert cast(int, report["exact_numerator"]) <= len(records) - 1
    assert cast(float, report["accuracy"]) == cast(int, report["exact_numerator"]) / len(records)
    error_rows = [
        row for row in cast(list[dict[str, object]], report["per_record"]) if "error" in row
    ]
    assert len(error_rows) == 1
    assert "selected_action_index" not in error_rows[0]
    assert "injected inference failure" in str(error_rows[0]["error"])
    assert cast(int, report["value_pair_root_count"]) <= cast(int, report["root_count"])


def test_v4_dataset_rejects_mixed_episode_outcome(tmp_path: Path) -> None:
    manifest_path, _teacher = _tiny_puct_dataset(tmp_path)
    records = list(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    counts = Counter(record.episode_id for record in records)
    episode_id = next(episode for episode, count in counts.items() if count >= 2)
    index = next(
        position for position, record in enumerate(records) if record.episode_id == episode_id
    )
    payload = records[index].to_dict()
    outcome = dict(cast(dict[str, object], payload["outcome"]))
    terminal_hp = cast(int, outcome["terminal_hp"])
    outcome["terminal_hp"] = 1 if terminal_hp != 1 else 2
    mutated_outcome = CombatOutcome.from_dict(outcome)
    payload["outcome"] = mutated_outcome.to_dict()
    payload["target_value"] = COMBAT_PROXY_V1.value(mutated_outcome)
    payload["record_id"] = None
    records[index] = SymbolicTrainingRecord.from_dict(payload)
    shard = manifest_path.parent / "train/train.jsonl"
    write_jsonl(shard, records)
    manifest_payload = json.loads(manifest_path.read_text())
    manifest_payload["shard_digest"] = hashlib.sha256(shard.read_bytes()).hexdigest()
    manifest_payload["record_ids"] = [cast(str, record.record_id) for record in records]
    _resign_dataset_manifest(manifest_payload)
    manifest_path.write_text(json.dumps(manifest_payload, sort_keys=True, separators=(",", ":")))
    with pytest.raises(
        ValueError, match="^episode outcome is not identical across decisions$"
    ):
        load_dataset_manifest(manifest_path, requested_split="train")


def test_v4_rejects_noncanonical_episode_ids(tmp_path: Path) -> None:
    manifest_path, _checkpoint = _tiny_puct_dataset(tmp_path)
    records = tuple(read_jsonl(manifest_path.parent / "train/train.jsonl"))
    payload = records[0].to_dict()
    wrong = dict(payload)
    wrong["episode_id"] = "0" * 64
    wrong["record_id"] = None
    with pytest.raises(ValueError, match="does not match canonical root/search/reward identity"):
        SymbolicTrainingRecord.from_dict(wrong)
    expected = canonical_episode_id(
        records[0].root_id,
        records[0].search_config,
        records[0].reward_config_digest,
    )
    assert records[0].episode_id == expected
