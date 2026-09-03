from __future__ import annotations

import json
import shutil
from collections.abc import Callable
from pathlib import Path
from typing import cast

import pytest

import sts_sim.rl.data as data_module
from sts_sim import RunEnv
from sts_sim.rl import (
    SHARED_TRAINING_VOCABULARY_KIND,
    SHARED_TRAINING_VOCABULARY_VERSION,
    TrainingConfig,
    VocabularyBuilder,
    encoder_contract_digest,
    generate_beam_dataset,
    generate_legal_roots,
    generate_puct_dataset,
    load_dataset_manifest,
    load_root_manifest,
    load_shared_training_vocabulary,
    publish_shared_training_vocabulary,
    read_jsonl,
    train_beam_clone,
)
from sts_sim.rl.data import DATASET_MANIFEST_VERSION
from sts_sim.rl.puct import PUCT_TEACHER_NAME, puct_clone_episode_payload
from sts_sim.rl.records import SymbolicTrainingRecord
from sts_sim.rl.tensor import Vocabularies


def _smoke_training_config() -> TrainingConfig:
    return TrainingConfig(
        batch_size=2,
        total_steps=1,
        model_width=16,
        model_heads=4,
        model_layers=1,
        feedforward_width=32,
        minimum_roots=1,
        minimum_lineages=1,
    )


def _fit(records: tuple[SymbolicTrainingRecord, ...]) -> Vocabularies:
    builder = VocabularyBuilder()
    for record in records:
        builder.add(record.observation, record.actions)
    return builder.freeze()


def _token_sets(vocabularies: Vocabularies) -> dict[str, set[str]]:
    return {namespace: set(vocab.tokens) for namespace, vocab in vocabularies.namespaces.items()}


def _canonical_bytes(payload: object) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _copy_dataset(manifest_path: Path, destination: Path) -> Path:
    shutil.copytree(manifest_path.parent, destination, symlinks=False)
    return destination / manifest_path.name


def _resign_dataset_manifest(payload: dict[str, object]) -> None:
    payload["manifest_digest"] = data_module._digest_payload(payload, "manifest_digest")


@pytest.fixture(scope="module")
def paired_label_datasets(tmp_path_factory: pytest.TempPathFactory) -> dict[str, Path]:
    root = tmp_path_factory.mktemp("shared-vocab")
    generate_legal_roots(root / "roots", ["BEAMCLONE0", "BEAMCLONE12"], max_run_steps=128)
    roots = root / "roots/root-manifest.json"
    generate_beam_dataset(
        roots,
        root / "beam",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    checkpoint = root / "teacher.pt"
    train_beam_clone(root / "beam/dataset-manifest.json", checkpoint, _smoke_training_config())
    generate_puct_dataset(
        roots,
        root / "puct",
        checkpoint,
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=2,
        max_player_turns=3,
    )
    return {
        "beam": root / "beam/dataset-manifest.json",
        "puct": root / "puct/dataset-manifest.json",
        "checkpoint": checkpoint,
        "roots": roots,
    }


def test_current_record_and_manifest_versions_are_strict() -> None:
    assert DATASET_MANIFEST_VERSION == 7


def test_shared_vocabulary_unions_beam_and_puct_observations(
    paired_label_datasets: dict[str, Path], tmp_path: Path
) -> None:
    beam = load_dataset_manifest(paired_label_datasets["beam"], requested_split="train")
    puct = load_dataset_manifest(paired_label_datasets["puct"], requested_split="train")
    assert beam.manifest_version == DATASET_MANIFEST_VERSION
    assert puct.manifest_version == DATASET_MANIFEST_VERSION
    assert beam.teacher_name != PUCT_TEACHER_NAME
    assert puct.teacher_name == PUCT_TEACHER_NAME
    assert beam.manifest_digest != puct.manifest_digest
    assert beam.root_manifest_digest == puct.root_manifest_digest
    assert beam.cohort_digest == puct.cohort_digest

    beam_records = tuple(read_jsonl(paired_label_datasets["beam"].parent / beam.shard_path))
    puct_records = tuple(read_jsonl(paired_label_datasets["puct"].parent / puct.shard_path))
    expected = _fit((*beam_records, *puct_records))
    beam_only = _fit(beam_records)
    puct_only = _fit(puct_records)

    artifact = publish_shared_training_vocabulary(
        paired_label_datasets["beam"],
        paired_label_datasets["puct"],
        tmp_path / "shared-training-vocabulary.json",
    )
    loaded = load_shared_training_vocabulary(
        tmp_path / "shared-training-vocabulary.json",
        beam_manifest_path=paired_label_datasets["beam"],
        puct_manifest_path=paired_label_datasets["puct"],
    )
    assert artifact == loaded
    assert artifact.kind == SHARED_TRAINING_VOCABULARY_KIND
    assert artifact.version == SHARED_TRAINING_VOCABULARY_VERSION
    assert artifact.beam_dataset_manifest_digest == beam.manifest_digest
    assert artifact.puct_dataset_manifest_digest == puct.manifest_digest
    assert artifact.shared_training_root_manifest_digest == beam.root_manifest_digest
    assert artifact.shared_cohort_digest == beam.cohort_digest
    assert artifact.vocabulary_fingerprint == expected.fingerprint
    assert artifact.encoder_contract_digest == encoder_contract_digest(expected)
    assert artifact.vocabularies.to_dict() == expected.to_dict()
    union_tokens = _token_sets(artifact.vocabularies)
    for namespace, tokens in union_tokens.items():
        assert tokens == _token_sets(beam_only)[namespace] | _token_sets(puct_only)[namespace]


def test_shared_vocabulary_is_byte_identical_and_idempotent(
    paired_label_datasets: dict[str, Path], tmp_path: Path
) -> None:
    first_path = tmp_path / "left.json"
    second_path = tmp_path / "right.json"
    first = publish_shared_training_vocabulary(
        paired_label_datasets["beam"], paired_label_datasets["puct"], first_path
    )
    second = publish_shared_training_vocabulary(
        paired_label_datasets["beam"], paired_label_datasets["puct"], second_path
    )
    assert first_path.read_bytes() == second_path.read_bytes() == _canonical_bytes(first.to_dict())
    assert first == second
    again = publish_shared_training_vocabulary(
        paired_label_datasets["beam"], paired_label_datasets["puct"], first_path
    )
    assert again == first
    assert first_path.read_bytes() == second_path.read_bytes()


def test_shared_vocabulary_rejects_different_content_at_the_same_path(
    paired_label_datasets: dict[str, Path], tmp_path: Path
) -> None:
    path = tmp_path / "shared-training-vocabulary.json"
    publish_shared_training_vocabulary(
        paired_label_datasets["beam"], paired_label_datasets["puct"], path
    )
    path.write_bytes(b'{"kind":"other"}')
    with pytest.raises(ValueError, match="refusing to mutate scientific artifact"):
        publish_shared_training_vocabulary(
            paired_label_datasets["beam"], paired_label_datasets["puct"], path
        )


def test_shared_vocabulary_rejects_tampered_inputs(
    paired_label_datasets: dict[str, Path], tmp_path: Path
) -> None:
    original = publish_shared_training_vocabulary(
        paired_label_datasets["beam"],
        paired_label_datasets["puct"],
        tmp_path / "shared-training-vocabulary.json",
    )

    modified = _copy_dataset(paired_label_datasets["beam"], tmp_path / "modified-shard")
    shard = tmp_path / "modified-shard/train/train.jsonl"
    shard.write_bytes(shard.read_bytes() + b"\n")
    with pytest.raises(ValueError, match="shard digest"):
        publish_shared_training_vocabulary(
            modified, paired_label_datasets["puct"], tmp_path / "from-modified-shard.json"
        )

    undeclared = _copy_dataset(paired_label_datasets["puct"], tmp_path / "undeclared")
    (tmp_path / "undeclared/extra.json").write_text("{}", encoding="utf-8")
    with pytest.raises(ValueError, match="undeclared dataset input"):
        publish_shared_training_vocabulary(
            paired_label_datasets["beam"], undeclared, tmp_path / "from-undeclared.json"
        )

    linked = _copy_dataset(paired_label_datasets["beam"], tmp_path / "linked")
    shard_link = tmp_path / "linked/train/train.jsonl"
    real_shard = tmp_path / "linked/train/real.jsonl"
    shard_link.rename(real_shard)
    shard_link.symlink_to(real_shard.name)
    with pytest.raises(ValueError, match="symlink"):
        publish_shared_training_vocabulary(
            linked, paired_label_datasets["puct"], tmp_path / "from-symlink.json"
        )

    artifact_path = tmp_path / "tampered-artifact.json"
    payload = json.loads((tmp_path / "shared-training-vocabulary.json").read_text(encoding="utf-8"))
    payload["vocabulary_fingerprint"] = "0" * 64
    artifact_path.write_bytes(_canonical_bytes(payload))
    with pytest.raises(ValueError, match="vocabulary fingerprint"):
        load_shared_training_vocabulary(
            artifact_path,
            beam_manifest_path=paired_label_datasets["beam"],
            puct_manifest_path=paired_label_datasets["puct"],
        )

    encoder_tampered = tmp_path / "tampered-encoder.json"
    payload = json.loads((tmp_path / "shared-training-vocabulary.json").read_text(encoding="utf-8"))
    payload["encoder_contract_digest"] = "1" * 64
    encoder_tampered.write_bytes(_canonical_bytes(payload))
    with pytest.raises(ValueError, match="encoder contract digest"):
        load_shared_training_vocabulary(
            encoder_tampered,
            beam_manifest_path=paired_label_datasets["beam"],
            puct_manifest_path=paired_label_datasets["puct"],
        )

    noncanonical = tmp_path / "noncanonical-vocab.json"
    payload = json.loads((tmp_path / "shared-training-vocabulary.json").read_text(encoding="utf-8"))
    vocabularies = cast(dict[str, list[str]], payload["vocabularies"])
    vocabularies["monster"] = list(reversed(vocabularies["monster"]))
    noncanonical.write_bytes(_canonical_bytes(payload))
    with pytest.raises(ValueError, match="vocabulary"):
        load_shared_training_vocabulary(
            noncanonical,
            beam_manifest_path=paired_label_datasets["beam"],
            puct_manifest_path=paired_label_datasets["puct"],
        )

    pretty = tmp_path / "pretty.json"
    pretty.write_text(
        json.dumps(original.to_dict(), indent=2, sort_keys=True),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="not canonical"):
        load_shared_training_vocabulary(
            pretty,
            beam_manifest_path=paired_label_datasets["beam"],
            puct_manifest_path=paired_label_datasets["puct"],
        )


def test_shared_vocabulary_rejects_mismatched_cohorts(
    paired_label_datasets: dict[str, Path], tmp_path: Path
) -> None:
    generate_legal_roots(
        tmp_path / "other-roots",
        [f"OTHER{index}" for index in range(8)],
        max_run_steps=128,
    )
    generate_puct_dataset(
        tmp_path / "other-roots/root-manifest.json",
        tmp_path / "other-puct",
        paired_label_datasets["checkpoint"],
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=2,
        max_player_turns=3,
    )
    beam = load_dataset_manifest(paired_label_datasets["beam"], requested_split="train")
    other = load_dataset_manifest(
        tmp_path / "other-puct/dataset-manifest.json", requested_split="train"
    )
    assert beam.cohort_digest != other.cohort_digest
    assert beam.root_manifest_digest != other.root_manifest_digest
    with pytest.raises(ValueError, match="do not share a cohort digest"):
        publish_shared_training_vocabulary(
            paired_label_datasets["beam"],
            tmp_path / "other-puct/dataset-manifest.json",
            tmp_path / "mismatched.json",
        )


def test_shared_vocabulary_rejects_swapped_teacher_roles(
    paired_label_datasets: dict[str, Path], tmp_path: Path
) -> None:
    with pytest.raises(ValueError, match="beam dataset"):
        publish_shared_training_vocabulary(
            paired_label_datasets["puct"],
            paired_label_datasets["beam"],
            tmp_path / "swapped.json",
        )


def test_shared_vocabulary_rejects_non_integer_version(
    paired_label_datasets: dict[str, Path], tmp_path: Path
) -> None:
    publish_shared_training_vocabulary(
        paired_label_datasets["beam"],
        paired_label_datasets["puct"],
        tmp_path / "shared-training-vocabulary.json",
    )
    payload = json.loads((tmp_path / "shared-training-vocabulary.json").read_text(encoding="utf-8"))
    for label, version in (("bool", True), ("float", 1.0)):
        tampered = dict(payload)
        tampered["version"] = version
        path = tmp_path / f"version-{label}.json"
        path.write_bytes(_canonical_bytes(tampered))
        with pytest.raises(ValueError, match="unsupported shared training vocabulary version"):
            load_shared_training_vocabulary(
                path,
                beam_manifest_path=paired_label_datasets["beam"],
                puct_manifest_path=paired_label_datasets["puct"],
            )


def test_shared_vocabulary_rejects_mismatched_root_membership(
    paired_label_datasets: dict[str, Path], tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0", "BEAMCLONE1"], max_run_steps=128)
    roots = tmp_path / "roots/root-manifest.json"
    train_ids = [root.root_id for root in load_root_manifest(roots).roots if root.split == "train"]
    assert len(train_ids) == 2
    generate_beam_dataset(
        roots,
        tmp_path / "beam",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    original = puct_clone_episode_payload
    calls = {"count": 0}

    def boom(env: RunEnv, evaluator: Callable[[str], str], **kwargs: object) -> dict[str, object]:
        calls["count"] += 1
        del kwargs
        if calls["count"] == 1:
            raise RuntimeError("injected PUCT labeling failure")
        return original(
            env,
            evaluator,
            c_puct=1.5,
            simulation_budget=4,
            transition_budget=4,
            max_decisions=2,
            max_player_turns=3,
            leaf_cache="exact_state",
        )

    monkeypatch.setattr("sts_sim.rl.puct_data.puct_clone_episode_payload", boom)
    generate_puct_dataset(
        roots,
        tmp_path / "puct",
        paired_label_datasets["checkpoint"],
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=2,
        max_player_turns=3,
    )
    beam = load_dataset_manifest(tmp_path / "beam/dataset-manifest.json", requested_split="train")
    puct = load_dataset_manifest(tmp_path / "puct/dataset-manifest.json", requested_split="train")
    assert beam.cohort_digest == puct.cohort_digest
    assert beam.root_manifest_digest == puct.root_manifest_digest
    assert beam.roots != puct.roots
    with pytest.raises(ValueError, match="realized training-root membership"):
        publish_shared_training_vocabulary(
            tmp_path / "beam/dataset-manifest.json",
            tmp_path / "puct/dataset-manifest.json",
            tmp_path / "mismatched-membership.json",
        )


def test_shared_vocabulary_rejects_old_puct_manifest_versions(
    paired_label_datasets: dict[str, Path], tmp_path: Path
) -> None:
    converted = _copy_dataset(paired_label_datasets["puct"], tmp_path / "puct-old")
    payload = json.loads(converted.read_text(encoding="utf-8"))
    payload["manifest_version"] = 6
    _resign_dataset_manifest(payload)
    converted.write_bytes(data_module._canonical_bytes(payload))
    with pytest.raises(ValueError, match="unsupported or malformed"):
        load_dataset_manifest(converted, requested_split="train")
    with pytest.raises(ValueError, match="unsupported or malformed"):
        publish_shared_training_vocabulary(
            paired_label_datasets["beam"], converted, tmp_path / "shared-old.json"
        )
