"""Train-to-evaluation-authorization-v1 parser, verifier, and held-out gate.

Booleans stored on disk are never proof; every digest and disjointness check is
recomputed from the loaded manifests. Same-cohort development evaluation of the
training root manifest needs no authorization. A different root manifest requires
an authorization proving disjoint lineages, root IDs, and seeds plus matching
source-epoch-bundle identity. Sealed and audit splits are never loadable.
"""

from __future__ import annotations

import json
import stat
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from .data import (
    _LOADABLE_SPLITS,
    RootManifest,
    _require_canonical_root_snapshot,
    parse_root_manifest,
)
from .experiment import (
    _absolute_without_follow,
    _lstat,
    _read_regular_file_bytes,
    normalize_inventory_relative_path,
    resolve_inventory_path,
    write_scientific_artifact,
)
from .provenance import canonical_bytes, require_digest, sha256_bytes
from .source_epoch import SOURCE_EPOCH_DIRNAME, load_source_epoch_bundle, verify_loaded_native_bytes

AUTHORIZATION_KIND = "train-to-evaluation-authorization-v1"
AUTHORIZATION_SCHEMA_VERSION = 1
MANDATORY_DISJOINTNESS_DIMENSIONS = ("lineages", "root_ids", "seeds")
AUTHORIZED_EVALUATOR_NAMES = frozenset(
    {
        "beam",
        "beam_clone",
        "network",
        "network_puct",
        "random",
        "uniform_prior_constant_value_puct",
        "uniform_prior_network_value_puct",
    }
)
_AUTHORIZATION_KEYS = frozenset(
    {
        "kind",
        "schema_version",
        "training_root_manifest_digest",
        "training_cohort_digest",
        "evaluation_root_manifest_digest",
        "evaluation_cohort_digest",
        "source_epoch_bundle_digest",
        "evaluation_seed",
        "authorized_evaluator_names",
        "mandatory_disjointness_dimensions",
        "authorization_digest",
    }
)


def _canonical_bytes(payload: object) -> bytes:
    return canonical_bytes(payload)


def _sha256_bytes(payload: bytes) -> str:
    return sha256_bytes(payload)


def _require_mapping(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        raise TypeError(f"{label} must be an object")
    result = cast(dict[str, object], value)
    if any(type(key) is not str for key in result):
        raise TypeError(f"{label} keys must be strings")
    return result


def _require_digest(value: object, label: str) -> str:
    return require_digest(value, label)


def _require_int(value: object, label: str) -> int:
    if type(value) is not int:
        raise TypeError(f"{label} must be an integer")
    return value


def _require_string_list(value: object, label: str) -> tuple[str, ...]:
    if type(value) is not list:
        raise TypeError(f"{label} must be an array")
    items = tuple(cast(list[object], value))
    if not items:
        raise ValueError(f"{label} must be nonempty")
    names: list[str] = []
    seen: set[str] = set()
    for item in items:
        if type(item) is not str or not item:
            raise TypeError(f"{label} entries must be nonempty strings")
        if item in seen:
            raise ValueError(f"{label} contains a duplicate entry")
        seen.add(item)
        names.append(item)
    if tuple(names) != tuple(sorted(names)):
        raise ValueError(f"{label} are not canonically ordered")
    return tuple(names)


def _digest_payload(payload: dict[str, object], digest_key: str) -> str:
    unsigned = dict(payload)
    unsigned.pop(digest_key, None)
    return _sha256_bytes(_canonical_bytes(unsigned))


def _reject_symlink(path: Path, label: str) -> None:
    current = _absolute_without_follow(path)
    while True:
        info = _lstat(current)
        if info is not None and stat.S_ISLNK(info.st_mode):
            raise ValueError(f"{label} must not be a symlink: {current}")
        parent = current.parent
        if parent == current:
            return
        current = parent


@dataclass(frozen=True, slots=True)
class TrainToEvaluationAuthorization:
    kind: str
    schema_version: int
    training_root_manifest_digest: str
    training_cohort_digest: str
    evaluation_root_manifest_digest: str
    evaluation_cohort_digest: str
    source_epoch_bundle_digest: str
    evaluation_seed: int
    authorized_evaluator_names: tuple[str, ...]
    mandatory_disjointness_dimensions: tuple[str, ...]
    authorization_digest: str

    def to_dict(self) -> dict[str, object]:
        return {
            "kind": self.kind,
            "schema_version": self.schema_version,
            "training_root_manifest_digest": self.training_root_manifest_digest,
            "training_cohort_digest": self.training_cohort_digest,
            "evaluation_root_manifest_digest": self.evaluation_root_manifest_digest,
            "evaluation_cohort_digest": self.evaluation_cohort_digest,
            "source_epoch_bundle_digest": self.source_epoch_bundle_digest,
            "evaluation_seed": self.evaluation_seed,
            "authorized_evaluator_names": list(self.authorized_evaluator_names),
            "mandatory_disjointness_dimensions": list(self.mandatory_disjointness_dimensions),
            "authorization_digest": self.authorization_digest,
        }


@dataclass(frozen=True, slots=True)
class CohortIdentities:
    root_ids: frozenset[str]
    seeds: frozenset[str]
    lineages: frozenset[str]


@dataclass(frozen=True, slots=True)
class HeldOutAuthorizationProof:
    authorization: TrainToEvaluationAuthorization
    requested_evaluator_name: str
    training_file_digest: str
    evaluation_file_digest: str
    training: CohortIdentities
    evaluation: CohortIdentities


def parse_authorization(payload: object) -> TrainToEvaluationAuthorization:
    source = _require_mapping(payload, "authorization")
    if set(source) != _AUTHORIZATION_KEYS:
        raise ValueError("authorization has missing or unknown fields")
    if source["kind"] != AUTHORIZATION_KIND:
        raise ValueError("unsupported authorization kind")
    schema_version = _require_int(source["schema_version"], "authorization schema version")
    if schema_version != AUTHORIZATION_SCHEMA_VERSION:
        raise ValueError("unsupported authorization schema version")
    names = _require_string_list(source["authorized_evaluator_names"], "authorized evaluator names")
    unknown = [name for name in names if name not in AUTHORIZED_EVALUATOR_NAMES]
    if unknown:
        raise ValueError(f"unauthorized evaluator name: {unknown[0]}")
    dimensions = _require_string_list(
        source["mandatory_disjointness_dimensions"],
        "mandatory disjointness dimensions",
    )
    if dimensions != MANDATORY_DISJOINTNESS_DIMENSIONS:
        raise ValueError("mandatory disjointness dimensions must be root_ids, seeds, and lineages")
    training_manifest = _require_digest(
        source["training_root_manifest_digest"], "training root-manifest digest"
    )
    evaluation_manifest = _require_digest(
        source["evaluation_root_manifest_digest"], "evaluation root-manifest digest"
    )
    training_cohort = _require_digest(source["training_cohort_digest"], "training cohort digest")
    evaluation_cohort = _require_digest(
        source["evaluation_cohort_digest"], "evaluation cohort digest"
    )
    if training_manifest == evaluation_manifest:
        raise ValueError("training and evaluation manifests are the same artifact")
    authorization = TrainToEvaluationAuthorization(
        AUTHORIZATION_KIND,
        AUTHORIZATION_SCHEMA_VERSION,
        training_manifest,
        training_cohort,
        evaluation_manifest,
        evaluation_cohort,
        _require_digest(source["source_epoch_bundle_digest"], "source-epoch-bundle digest"),
        _require_int(source["evaluation_seed"], "evaluation seed"),
        names,
        dimensions,
        _require_digest(source["authorization_digest"], "authorization digest"),
    )
    expected_digest = _digest_payload(authorization.to_dict(), "authorization_digest")
    if authorization.authorization_digest != expected_digest:
        raise ValueError("authorization digest is invalid")
    return authorization


def authorization_from_bindings(
    *,
    training_root_manifest_digest: str,
    training_cohort_digest: str,
    evaluation_root_manifest_digest: str,
    evaluation_cohort_digest: str,
    source_epoch_bundle_digest: str,
    evaluation_seed: int,
    authorized_evaluator_names: Sequence[str],
) -> TrainToEvaluationAuthorization:
    payload: dict[str, object] = {
        "kind": AUTHORIZATION_KIND,
        "schema_version": AUTHORIZATION_SCHEMA_VERSION,
        "training_root_manifest_digest": training_root_manifest_digest,
        "training_cohort_digest": training_cohort_digest,
        "evaluation_root_manifest_digest": evaluation_root_manifest_digest,
        "evaluation_cohort_digest": evaluation_cohort_digest,
        "source_epoch_bundle_digest": source_epoch_bundle_digest,
        "evaluation_seed": evaluation_seed,
        "authorized_evaluator_names": sorted(authorized_evaluator_names),
        "mandatory_disjointness_dimensions": list(MANDATORY_DISJOINTNESS_DIMENSIONS),
        "authorization_digest": "0" * 64,
    }
    payload["authorization_digest"] = _digest_payload(payload, "authorization_digest")
    return parse_authorization(payload)


def load_authorization(path: Path) -> TrainToEvaluationAuthorization:
    _reject_symlink(path, "authorization")
    content = _read_regular_file_bytes(path)
    try:
        payload = json.loads(content)
    except json.JSONDecodeError as error:
        raise ValueError("authorization is not JSON") from error
    authorization = parse_authorization(payload)
    if content != _canonical_bytes(authorization.to_dict()):
        raise ValueError("authorization is not canonical")
    return authorization


def write_authorization(path: Path, authorization: TrainToEvaluationAuthorization) -> str:
    parsed = parse_authorization(authorization.to_dict())
    return write_scientific_artifact(path, _canonical_bytes(parsed.to_dict()))


def _load_cohort_manifest(path: Path, label: str) -> tuple[RootManifest, str]:
    _reject_symlink(path, f"{label} root manifest")
    content = _read_regular_file_bytes(path)
    file_digest = _sha256_bytes(content)
    manifest = parse_root_manifest(content)
    if not manifest.roots:
        raise ValueError(f"{label} cohort is empty")
    parent = _absolute_without_follow(path).parent
    seen_ids: set[str] = set()
    seen_seeds: set[str] = set()
    seen_lineages: set[str] = set()
    for root in manifest.roots:
        if root.root_id in seen_ids:
            raise ValueError(f"{label} cohort has a duplicate root ID")
        seen_ids.add(root.root_id)
        for seed in root.source_seeds:
            if seed in seen_seeds:
                raise ValueError(f"{label} cohort has a duplicate generation seed")
            seen_seeds.add(seed)
        for lineage in root.lineages:
            if type(lineage) is not str or not lineage:
                raise TypeError(f"{label} lineages must be nonempty strings")
            if lineage in seen_lineages:
                raise ValueError(f"{label} cohort has a duplicate lineage")
            seen_lineages.add(lineage)
        relative = normalize_inventory_relative_path(root.relative_path)
        snapshot = resolve_inventory_path(parent, relative)
        info = _lstat(snapshot)
        if info is None:
            raise ValueError(f"{label} root snapshot is missing: {snapshot}")
        if stat.S_ISLNK(info.st_mode):
            raise ValueError(f"{label} root path must not be a symlink: {snapshot}")
        if not stat.S_ISREG(info.st_mode):
            raise ValueError(f"{label} root path must be a regular file: {snapshot}")
        _reject_symlink(snapshot.parent, f"{label} root path")
        _require_canonical_root_snapshot(_read_regular_file_bytes(snapshot), root.root_id)
    bundle_dir = parent / SOURCE_EPOCH_DIRNAME
    bundle = load_source_epoch_bundle(bundle_dir)
    verify_loaded_native_bytes(bundle)
    if bundle.bundle_digest != manifest.source_epoch_bundle_digest:
        raise ValueError(f"{label} source-epoch-bundle digest does not match the root manifest")
    return manifest, file_digest


def _cohort_identities(manifest: RootManifest) -> CohortIdentities:
    return CohortIdentities(
        root_ids=frozenset(root.root_id for root in manifest.roots),
        seeds=frozenset(manifest.requested_seeds),
        lineages=frozenset(lineage for root in manifest.roots for lineage in root.lineages),
    )


def _require_disjoint(left: frozenset[str], right: frozenset[str], dimension: str) -> None:
    overlap = left & right
    if overlap:
        raise ValueError(f"training and evaluation {dimension} are not disjoint")


def verify_train_to_evaluation_authorization(
    authorization_path: Path,
    *,
    training_root_manifest_path: Path,
    evaluation_root_manifest_path: Path,
    expected_source_epoch_bundle_digest: str,
    evaluation_seed: int,
    requested_evaluator_name: str,
) -> HeldOutAuthorizationProof:
    """Load both root manifests and recompute every bound identity and disjointness check."""

    if type(evaluation_seed) is not int:
        raise TypeError("evaluation seed must be an integer")
    if type(requested_evaluator_name) is not str or not requested_evaluator_name:
        raise TypeError("requested evaluator name must be a nonempty string")
    expected_bundle = _require_digest(
        expected_source_epoch_bundle_digest, "expected source-epoch-bundle digest"
    )
    authorization = load_authorization(authorization_path)
    if requested_evaluator_name not in authorization.authorized_evaluator_names:
        raise ValueError("evaluator is not authorized")
    if evaluation_seed != authorization.evaluation_seed:
        raise ValueError("evaluation seed does not match authorization")
    if authorization.source_epoch_bundle_digest != expected_bundle:
        raise ValueError("source-epoch-bundle digest does not match the caller digest")

    training_abs = _absolute_without_follow(training_root_manifest_path)
    evaluation_abs = _absolute_without_follow(evaluation_root_manifest_path)
    if training_abs == evaluation_abs:
        raise ValueError("training and evaluation manifests are the same artifact")

    training_manifest, training_file_digest = _load_cohort_manifest(
        training_root_manifest_path, "training"
    )
    evaluation_manifest, evaluation_file_digest = _load_cohort_manifest(
        evaluation_root_manifest_path, "evaluation"
    )
    if training_file_digest == evaluation_file_digest:
        raise ValueError("training and evaluation manifests are the same artifact")
    if training_manifest.manifest_digest == evaluation_manifest.manifest_digest:
        raise ValueError("training and evaluation manifests are the same artifact")
    if training_manifest.manifest_digest != authorization.training_root_manifest_digest:
        raise ValueError("training root-manifest digest does not match authorization")
    if training_manifest.cohort_digest != authorization.training_cohort_digest:
        raise ValueError("training cohort digest does not match authorization")
    if evaluation_manifest.manifest_digest != authorization.evaluation_root_manifest_digest:
        raise ValueError("evaluation root-manifest digest does not match authorization")
    if evaluation_manifest.cohort_digest != authorization.evaluation_cohort_digest:
        raise ValueError("evaluation cohort digest does not match authorization")
    if (
        training_manifest.source_epoch_bundle_digest
        != evaluation_manifest.source_epoch_bundle_digest
    ):
        raise ValueError("training and evaluation source-epoch-bundle digests differ")
    if training_manifest.source_epoch_bundle_digest != expected_bundle:
        raise ValueError("training source-epoch-bundle digest does not match the caller digest")
    if evaluation_manifest.source_epoch_bundle_digest != expected_bundle:
        raise ValueError("evaluation source-epoch-bundle digest does not match the caller digest")
    if authorization.source_epoch_bundle_digest != expected_bundle:
        raise ValueError("source-epoch-bundle digest does not match the caller digest")

    training_identities = _cohort_identities(training_manifest)
    evaluation_identities = _cohort_identities(evaluation_manifest)
    _require_disjoint(training_identities.root_ids, evaluation_identities.root_ids, "root IDs")
    _require_disjoint(training_identities.seeds, evaluation_identities.seeds, "generation seeds")
    _require_disjoint(training_identities.lineages, evaluation_identities.lineages, "lineages")
    return HeldOutAuthorizationProof(
        authorization=authorization,
        requested_evaluator_name=requested_evaluator_name,
        training_file_digest=training_file_digest,
        evaluation_file_digest=evaluation_file_digest,
        training=training_identities,
        evaluation=evaluation_identities,
    )


def require_held_out_evaluation(
    *,
    training_root_manifest_digest: str,
    training_cohort_digest: str,
    evaluation_manifest: RootManifest,
    evaluation_root_manifest_path: Path,
    evaluation_split: str,
    evaluation_seed: int,
    requested_evaluator_names: Sequence[str],
    authorization_path: Path | None = None,
    training_root_manifest_path: Path | None = None,
) -> HeldOutAuthorizationProof | None:
    """Same-cohort loadable splits pass; cross-manifest eval requires authorization."""

    if evaluation_split not in _LOADABLE_SPLITS:
        raise PermissionError("sealed and audit splits are not available for evaluation")
    if not requested_evaluator_names:
        raise ValueError("held-out evaluation must name at least one evaluator")
    unknown = [name for name in requested_evaluator_names if name not in AUTHORIZED_EVALUATOR_NAMES]
    if unknown:
        raise ValueError(f"unauthorized evaluator name: {unknown[0]}")
    if training_root_manifest_digest == evaluation_manifest.manifest_digest:
        if training_cohort_digest != evaluation_manifest.cohort_digest:
            raise ValueError("evaluation disjoint cohort")
        if authorization_path is not None or training_root_manifest_path is not None:
            raise ValueError("same-cohort evaluation must not include an authorization")
        return None
    if authorization_path is None or training_root_manifest_path is None:
        raise ValueError("cross-manifest evaluation requires train-to-evaluation authorization")
    proof = verify_train_to_evaluation_authorization(
        authorization_path,
        training_root_manifest_path=training_root_manifest_path,
        evaluation_root_manifest_path=evaluation_root_manifest_path,
        expected_source_epoch_bundle_digest=evaluation_manifest.source_epoch_bundle_digest,
        evaluation_seed=evaluation_seed,
        requested_evaluator_name=requested_evaluator_names[0],
    )
    authorized = frozenset(proof.authorization.authorized_evaluator_names)
    missing = [name for name in requested_evaluator_names if name not in authorized]
    if missing:
        raise ValueError(f"evaluator is not authorized: {missing[0]}")
    return proof


def canonical_authorization_bytes(authorization: TrainToEvaluationAuthorization) -> bytes:
    return _canonical_bytes(parse_authorization(authorization.to_dict()).to_dict())
