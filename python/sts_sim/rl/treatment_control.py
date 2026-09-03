"""Shared training vocabulary for the beam-label versus PUCT-label experiment."""

from __future__ import annotations

import json
import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from .data import (
    DATASET_MANIFEST_VERSION,
    DatasetManifest,
    _sha256_bytes,
    load_dataset_manifest,
)
from .experiment import (
    _raise_if_symlink_ancestor,
    _read_regular_file_bytes,
    write_scientific_artifact,
)
from .records import BEAM_TEACHER_NAME, PUCT_TEACHER_NAME, SymbolicTrainingRecord
from .source_epoch import SOURCE_EPOCH_DIRNAME
from .tensor import Vocabularies, VocabularyBuilder, encoder_contract_digest

SHARED_TRAINING_VOCABULARY_KIND = "shared-training-vocabulary-v1"
SHARED_TRAINING_VOCABULARY_VERSION = 1
_HEX = "0123456789abcdef"
_ARTIFACT_KEYS = frozenset(
    {
        "kind",
        "version",
        "beam_dataset_manifest_digest",
        "puct_dataset_manifest_digest",
        "shared_training_root_manifest_digest",
        "shared_cohort_digest",
        "vocabularies",
        "vocabulary_fingerprint",
        "encoder_contract_digest",
    }
)


def _canonical_bytes(payload: object) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _require_digest(value: object, label: str) -> str:
    if (
        type(value) is not str
        or len(value) != 64
        or any(character not in _HEX for character in value)
    ):
        raise ValueError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _require_regular_file(path: Path, label: str) -> Path:
    _raise_if_symlink_ancestor(path)
    if path.is_symlink():
        raise ValueError(f"{label} must not be a symlink")
    if not path.is_file():
        raise ValueError(f"{label} must be a regular file")
    return path


def _declared_relative_path(value: object, label: str) -> str:
    if type(value) is not str or not value:
        raise TypeError(f"{label} must be a nonempty string")
    relative = Path(value)
    if relative.is_absolute() or any(part in {".", ".."} for part in relative.parts):
        raise ValueError(f"{label} is not a canonical relative path")
    return relative.as_posix()


def _reject_undeclared_dataset_inputs(dataset_root: Path, declared_files: frozenset[str]) -> None:
    if dataset_root.is_symlink():
        raise ValueError("dataset directory must not be a symlink")
    allowed_directories = {""}
    for relative in declared_files:
        parent = Path(relative).parent
        while parent.as_posix() not in {".", ""}:
            allowed_directories.add(parent.as_posix())
            parent = parent.parent
    for dirpath, dirnames, filenames in os.walk(dataset_root, followlinks=False):
        current = Path(dirpath)
        if current.is_symlink():
            raise ValueError("dataset directory contains a symlink")
        relative_dir = current.relative_to(dataset_root).as_posix()
        if relative_dir == ".":
            relative_dir = ""
        for name in dirnames:
            child = current / name
            relative = name if relative_dir == "" else f"{relative_dir}/{name}"
            if child.is_symlink():
                raise ValueError(f"dataset directory contains a symlink: {relative}")
            if relative not in allowed_directories:
                raise ValueError(f"undeclared dataset input: {relative}")
        for name in filenames:
            child = current / name
            relative = name if relative_dir == "" else f"{relative_dir}/{name}"
            if child.is_symlink():
                raise ValueError(f"dataset directory contains a symlink: {relative}")
            if relative not in declared_files:
                raise ValueError(f"undeclared dataset input: {relative}")


def _serialized_vocabularies(value: object) -> dict[str, list[str]]:
    if type(value) is not dict:
        raise TypeError("serialized vocabularies must be an object")
    source = cast(dict[str, object], value)
    if any(type(key) is not str for key in source):
        raise TypeError("serialized vocabulary namespaces must be strings")
    serialized: dict[str, list[str]] = {}
    for namespace, tokens in source.items():
        if type(tokens) is not list:
            raise TypeError("serialized vocabulary tokens must be strings")
        token_list = cast(list[object], tokens)
        if any(type(token) is not str for token in token_list):
            raise TypeError("serialized vocabulary tokens must be strings")
        serialized[namespace] = [cast(str, token) for token in token_list]
    return serialized


def _records_from_jsonl_bytes(content: bytes) -> tuple[SymbolicTrainingRecord, ...]:
    records: list[SymbolicTrainingRecord] = []
    for line_number, line in enumerate(content.decode("utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            records.append(SymbolicTrainingRecord.from_dict(json.loads(line)))
        except (KeyError, TypeError, ValueError) as error:
            raise ValueError(f"invalid symbolic record at line {line_number}") from error
    return tuple(records)


def _fit_union(
    beam_records: Sequence[SymbolicTrainingRecord],
    puct_records: Sequence[SymbolicTrainingRecord],
) -> Vocabularies:
    builder = VocabularyBuilder()
    for record in (*beam_records, *puct_records):
        builder.add(record.observation, record.actions)
    return builder.freeze()


def _require_beam_dataset(manifest: DatasetManifest) -> None:
    if manifest.split != "train":
        raise ValueError("shared training vocabulary requires train-split datasets")
    if manifest.manifest_version != DATASET_MANIFEST_VERSION:
        raise ValueError("dataset must use the current dataset manifest version")
    if manifest.teacher_name != BEAM_TEACHER_NAME:
        raise ValueError("beam dataset must use the public replanning-beam teacher")


def _require_puct_dataset(manifest: DatasetManifest) -> None:
    if manifest.split != "train":
        raise ValueError("shared training vocabulary requires train-split datasets")
    if manifest.manifest_version != DATASET_MANIFEST_VERSION:
        raise ValueError("dataset must use the current dataset manifest version")
    if manifest.teacher_name != PUCT_TEACHER_NAME:
        raise ValueError("PUCT dataset must use the privileged PUCT teacher")


def _require_shared_training_pair(beam: DatasetManifest, puct: DatasetManifest) -> None:
    _require_beam_dataset(beam)
    _require_puct_dataset(puct)
    if beam.cohort_digest != puct.cohort_digest:
        raise ValueError("beam and PUCT datasets do not share a cohort digest")
    if beam.root_manifest_digest != puct.root_manifest_digest:
        raise ValueError("beam and PUCT datasets do not share a training root-manifest digest")
    if beam.roots != puct.roots:
        raise ValueError("beam and PUCT datasets do not share realized training-root membership")


def _load_completed_dataset(
    path: Path, *, role: str
) -> tuple[DatasetManifest, tuple[SymbolicTrainingRecord, ...]]:
    _require_regular_file(path, f"{role} dataset manifest")
    raw = json.loads(_read_regular_file_bytes(path))
    if type(raw) is not dict:
        raise TypeError(f"{role} dataset manifest must be an object")
    source = cast(dict[str, object], raw)
    dataset_root = path.parent
    declared = frozenset(
        {
            path.name,
            _declared_relative_path(source.get("shard_path"), f"{role} shard path"),
            _declared_relative_path(source.get("root_manifest_path"), f"{role} root manifest path"),
        }
    )
    root_manifest_rel = _declared_relative_path(
        source.get("root_manifest_path"), f"{role} root manifest path"
    )
    bundle_dir = dataset_root / Path(root_manifest_rel).parent / SOURCE_EPOCH_DIRNAME
    bundle_files: set[str] = set()
    if bundle_dir.exists():
        for child in bundle_dir.rglob("*"):
            if child.is_file():
                bundle_files.add(child.relative_to(dataset_root).as_posix())
    declared = declared | bundle_files
    for relative in declared:
        _require_regular_file(dataset_root / relative, f"{role} dataset file {relative}")
    _reject_undeclared_dataset_inputs(dataset_root, declared)
    manifest = load_dataset_manifest(path, requested_split="train")
    shard_bytes = _read_regular_file_bytes(dataset_root / manifest.shard_path)
    if _sha256_bytes(shard_bytes) != manifest.shard_digest:
        raise ValueError("dataset shard digest is invalid")
    records = _records_from_jsonl_bytes(shard_bytes)
    return manifest, records


def _artifact_payload(
    beam: DatasetManifest,
    puct: DatasetManifest,
    vocabularies: Vocabularies,
) -> dict[str, object]:
    return {
        "kind": SHARED_TRAINING_VOCABULARY_KIND,
        "version": SHARED_TRAINING_VOCABULARY_VERSION,
        "beam_dataset_manifest_digest": beam.manifest_digest,
        "puct_dataset_manifest_digest": puct.manifest_digest,
        "shared_training_root_manifest_digest": beam.root_manifest_digest,
        "shared_cohort_digest": beam.cohort_digest,
        "vocabularies": vocabularies.to_dict(),
        "vocabulary_fingerprint": vocabularies.fingerprint,
        "encoder_contract_digest": encoder_contract_digest(vocabularies),
    }


@dataclass(frozen=True, slots=True)
class SharedTrainingVocabulary:
    kind: str
    version: int
    beam_dataset_manifest_digest: str
    puct_dataset_manifest_digest: str
    shared_training_root_manifest_digest: str
    shared_cohort_digest: str
    vocabularies: Vocabularies
    vocabulary_fingerprint: str
    encoder_contract_digest: str

    def to_dict(self) -> dict[str, object]:
        return _artifact_payload_from_self(self)


def _artifact_payload_from_self(artifact: SharedTrainingVocabulary) -> dict[str, object]:
    return {
        "kind": artifact.kind,
        "version": artifact.version,
        "beam_dataset_manifest_digest": artifact.beam_dataset_manifest_digest,
        "puct_dataset_manifest_digest": artifact.puct_dataset_manifest_digest,
        "shared_training_root_manifest_digest": artifact.shared_training_root_manifest_digest,
        "shared_cohort_digest": artifact.shared_cohort_digest,
        "vocabularies": artifact.vocabularies.to_dict(),
        "vocabulary_fingerprint": artifact.vocabulary_fingerprint,
        "encoder_contract_digest": artifact.encoder_contract_digest,
    }


def _from_validated(
    payload: Mapping[str, object], vocabularies: Vocabularies
) -> SharedTrainingVocabulary:
    return SharedTrainingVocabulary(
        cast(str, payload["kind"]),
        cast(int, payload["version"]),
        cast(str, payload["beam_dataset_manifest_digest"]),
        cast(str, payload["puct_dataset_manifest_digest"]),
        cast(str, payload["shared_training_root_manifest_digest"]),
        cast(str, payload["shared_cohort_digest"]),
        vocabularies,
        cast(str, payload["vocabulary_fingerprint"]),
        cast(str, payload["encoder_contract_digest"]),
    )


def load_shared_training_vocabulary(
    artifact_path: Path,
    *,
    beam_manifest_path: Path,
    puct_manifest_path: Path,
) -> SharedTrainingVocabulary:
    """Load a published artifact after recomputing vocabulary and encoder digests."""

    _require_regular_file(artifact_path, "shared training vocabulary")
    content = _read_regular_file_bytes(artifact_path)
    raw = json.loads(content)
    if type(raw) is not dict:
        raise TypeError("shared training vocabulary must be an object")
    payload = cast(dict[str, object], raw)
    if set(payload) != _ARTIFACT_KEYS:
        raise ValueError("shared training vocabulary has missing or unknown fields")
    if payload["kind"] != SHARED_TRAINING_VOCABULARY_KIND:
        raise ValueError("unsupported shared training vocabulary kind")
    if (
        type(payload["version"]) is not int
        or payload["version"] != SHARED_TRAINING_VOCABULARY_VERSION
    ):
        raise ValueError("unsupported shared training vocabulary version")
    for key in (
        "beam_dataset_manifest_digest",
        "puct_dataset_manifest_digest",
        "shared_training_root_manifest_digest",
        "shared_cohort_digest",
        "vocabulary_fingerprint",
        "encoder_contract_digest",
    ):
        _require_digest(payload[key], key.replace("_", " "))
    if content != _canonical_bytes(payload):
        raise ValueError("shared training vocabulary is not canonical")
    stored = Vocabularies.from_dict(_serialized_vocabularies(payload["vocabularies"]))
    if stored.to_dict() != payload["vocabularies"]:
        raise ValueError("serialized vocabularies are not canonical")
    recomputed_fingerprint = stored.fingerprint
    recomputed_encoder = encoder_contract_digest(stored)
    if payload["vocabulary_fingerprint"] != recomputed_fingerprint:
        raise ValueError("vocabulary fingerprint does not match serialized vocabularies")
    if payload["encoder_contract_digest"] != recomputed_encoder:
        raise ValueError("encoder contract digest does not match serialized vocabularies")
    beam, beam_records = _load_completed_dataset(beam_manifest_path, role="beam")
    puct, puct_records = _load_completed_dataset(puct_manifest_path, role="puct")
    _require_shared_training_pair(beam, puct)
    if payload["beam_dataset_manifest_digest"] != beam.manifest_digest:
        raise ValueError("beam dataset manifest digest mismatch")
    if payload["puct_dataset_manifest_digest"] != puct.manifest_digest:
        raise ValueError("PUCT dataset manifest digest mismatch")
    if payload["shared_training_root_manifest_digest"] != beam.root_manifest_digest:
        raise ValueError("shared training root-manifest digest mismatch")
    if payload["shared_cohort_digest"] != beam.cohort_digest:
        raise ValueError("shared cohort digest mismatch")
    rebuilt = _fit_union(beam_records, puct_records)
    if stored.to_dict() != rebuilt.to_dict():
        raise ValueError("serialized vocabularies do not match the union of declared shards")
    if rebuilt.fingerprint != recomputed_fingerprint:
        raise ValueError("vocabulary fingerprint does not match the union of declared shards")
    if encoder_contract_digest(rebuilt) != recomputed_encoder:
        raise ValueError("encoder contract digest does not match the union of declared shards")
    return _from_validated(payload, rebuilt)


def publish_shared_training_vocabulary(
    beam_manifest_path: Path,
    puct_manifest_path: Path,
    output_path: Path,
) -> SharedTrainingVocabulary:
    """Publish one vocabulary from the union of completed beam and PUCT datasets."""

    beam, beam_records = _load_completed_dataset(beam_manifest_path, role="beam")
    puct, puct_records = _load_completed_dataset(puct_manifest_path, role="puct")
    _require_shared_training_pair(beam, puct)
    vocabularies = _fit_union(beam_records, puct_records)
    payload = _artifact_payload(beam, puct, vocabularies)
    write_scientific_artifact(output_path, _canonical_bytes(payload))
    return _from_validated(payload, vocabularies)
