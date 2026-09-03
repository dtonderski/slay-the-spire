from __future__ import annotations

import hashlib
import json
import shutil
from pathlib import Path

import pytest

import sts_sim.rl.data as data_module
from sts_sim.rl.authorization import (
    AUTHORIZATION_KIND,
    AUTHORIZATION_SCHEMA_VERSION,
    MANDATORY_DISJOINTNESS_DIMENSIONS,
    authorization_from_bindings,
    canonical_authorization_bytes,
    load_authorization,
    parse_authorization,
    verify_train_to_evaluation_authorization,
    write_authorization,
)
from sts_sim.rl.data import RootManifest, load_root_manifest
from sts_sim.rl.experiment import normalize_inventory_relative_path
from sts_sim.rl.provenance import RepositoryVersion
from sts_sim.rl.source_epoch import SOURCE_EPOCH_DIRNAME, write_source_epoch_bundle

_BUNDLE = hashlib.sha256(b"source-epoch-bundle-v1-fixture").hexdigest()
_EVALUATION_SEED = 20260902
_EVALUATOR = "network_puct"


def _digest_label(label: str) -> str:
    return hashlib.sha256(label.encode()).hexdigest()


def _lineage_for_split(split: str, nonce: str) -> str:
    for index in range(20_000):
        candidate = f"{nonce}:{index}"
        if data_module._split_for_lineage(candidate) == split:
            return candidate
    raise AssertionError(f"could not find a {split} lineage")


def _canonical_snapshot(payload: object) -> tuple[str, bytes]:
    content = data_module._canonical_bytes(payload)
    return data_module._sha256_bytes(content), content


def _write_root_manifest(
    directory: Path,
    *,
    nonce: str,
    roots: list[tuple[str, str, dict[str, object]]],
    extra_seeds: tuple[str, ...] = (),
    git_sha: str = "a" * 40,
) -> Path:
    """Write a canonical root-manifest. roots are (seed, lineage, snapshot)."""

    directory.mkdir(parents=True, exist_ok=True)
    repository = RepositoryVersion(git_sha, True, None)
    bundle = write_source_epoch_bundle(directory / SOURCE_EPOCH_DIRNAME, repository)
    source_epoch_bundle_digest = bundle.bundle_digest
    generator_source_digest = data_module._sha256_bytes(
        data_module._canonical_bytes(repository.to_dict())
    )
    entries: list[dict[str, object]] = []
    accounted: set[str] = set()
    materialized: list[tuple[str, str, str, bytes]] = []
    for seed, lineage, snapshot_payload in roots:
        root_id, snapshot_bytes = _canonical_snapshot(snapshot_payload)
        materialized.append((root_id, seed, lineage, snapshot_bytes))
    for root_id, seed, lineage, snapshot_bytes in sorted(materialized, key=lambda item: item[0]):
        lineages = (lineage,)
        split = data_module._split_for_lineage(lineage)
        relative_path = f"{split}/roots/{root_id}.json"
        entries.append(
            {
                "root_id": root_id,
                "split": split,
                "split_group_id": data_module._split_group_id(lineages),
                "relative_path": relative_path,
                "lineages": list(lineages),
                "source_seeds": [seed],
            }
        )
        accounted.add(seed)
        snapshot = directory / relative_path
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        snapshot.write_bytes(snapshot_bytes)
    exclusions: list[dict[str, object]] = []
    for seed in extra_seeds:
        exclusions.append({"source_seed": seed, "reason": "synthetic", "detail": nonce})
        accounted.add(seed)
    requested_seeds = tuple(sorted(accounted))
    cohort_digest = data_module._cohort_digest(
        requested_seeds=requested_seeds,
        generator_name=data_module._GENERATOR_NAME,
        generator_version=data_module._GENERATOR_VERSION,
        generator_source_digest=generator_source_digest,
        split_salt=data_module._SPLIT_SALT,
        ascension=0,
        max_run_steps=256,
        combat_depth=1,
    )
    payload: dict[str, object] = {
        "manifest_version": data_module.ROOT_MANIFEST_VERSION,
        "generator_name": data_module._GENERATOR_NAME,
        "generator_version": data_module._GENERATOR_VERSION,
        "generator_source_digest": generator_source_digest,
        "repository": repository.to_dict(),
        "ascension": 0,
        "max_run_steps": 256,
        "combat_depth": 1,
        "split_salt": data_module._SPLIT_SALT,
        "requested_seeds": list(requested_seeds),
        "cohort_digest": cohort_digest,
        "source_epoch_bundle_digest": source_epoch_bundle_digest,
        "roots": entries,
        "exclusions": exclusions,
        "manifest_digest": "0" * 64,
    }
    payload["manifest_digest"] = data_module._digest_payload(payload, "manifest_digest")
    manifest = RootManifest.from_dict(payload)
    path = directory / "root-manifest.json"
    path.write_bytes(data_module._canonical_bytes(manifest.to_dict()))
    return path


def _disjoint_pair(tmp_path: Path) -> tuple[Path, Path, RootManifest, RootManifest]:
    train_lineage = _lineage_for_split("train", "train-lineage")
    eval_lineage = _lineage_for_split("train", "eval-lineage")
    assert train_lineage != eval_lineage
    training_path = _write_root_manifest(
        tmp_path / "training",
        nonce="train",
        roots=[("TRAIN-SEED", train_lineage, {"fixture": "train"})],
    )
    evaluation_path = _write_root_manifest(
        tmp_path / "evaluation",
        nonce="eval",
        roots=[("EVAL-SEED", eval_lineage, {"fixture": "eval"})],
    )
    training = load_root_manifest(training_path, verify_roots=False)
    evaluation = load_root_manifest(evaluation_path, verify_roots=False)
    return training_path, evaluation_path, training, evaluation


def _write_bound_authorization(
    path: Path,
    training: RootManifest,
    evaluation: RootManifest,
    *,
    names: tuple[str, ...] = ("beam", "network_puct", "random"),
    bundle: str | None = None,
    evaluation_seed: int = _EVALUATION_SEED,
) -> Path:
    authorization = authorization_from_bindings(
        training_root_manifest_digest=training.manifest_digest,
        training_cohort_digest=training.cohort_digest,
        evaluation_root_manifest_digest=evaluation.manifest_digest,
        evaluation_cohort_digest=evaluation.cohort_digest,
        source_epoch_bundle_digest=training.source_epoch_bundle_digest if bundle is None else bundle,
        evaluation_seed=evaluation_seed,
        authorized_evaluator_names=names,
    )
    write_authorization(path, authorization)
    return path


def _verify(
    tmp_path: Path,
    training_path: Path,
    evaluation_path: Path,
    training: RootManifest,
    evaluation: RootManifest,
) -> None:
    auth_path = tmp_path / "authorization.json"
    _write_bound_authorization(auth_path, training, evaluation)
    verify_train_to_evaluation_authorization(
        auth_path,
        training_root_manifest_path=training_path,
        evaluation_root_manifest_path=evaluation_path,
        expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
        evaluation_seed=_EVALUATION_SEED,
        requested_evaluator_name=_EVALUATOR,
    )


def test_round_trip_is_canonical_and_unknown_fields_are_rejected(
    tmp_path: Path,
) -> None:
    training_path, evaluation_path, training, evaluation = _disjoint_pair(tmp_path)
    authorization = authorization_from_bindings(
        training_root_manifest_digest=training.manifest_digest,
        training_cohort_digest=training.cohort_digest,
        evaluation_root_manifest_digest=evaluation.manifest_digest,
        evaluation_cohort_digest=evaluation.cohort_digest,
        source_epoch_bundle_digest=training.source_epoch_bundle_digest,
        evaluation_seed=_EVALUATION_SEED,
        authorized_evaluator_names=("random", "beam"),
    )
    assert authorization.kind == AUTHORIZATION_KIND
    assert authorization.schema_version == AUTHORIZATION_SCHEMA_VERSION
    assert authorization.mandatory_disjointness_dimensions == MANDATORY_DISJOINTNESS_DIMENSIONS
    assert authorization.authorized_evaluator_names == ("beam", "random")
    first = tmp_path / "authorization.json"
    second = tmp_path / "authorization-again.json"
    write_authorization(first, authorization)
    write_authorization(second, load_authorization(first))
    assert first.read_bytes() == second.read_bytes()
    assert first.read_bytes() == canonical_authorization_bytes(authorization)
    parsed = json.loads(first.read_text())
    parsed["root_ids_disjoint"] = True
    with pytest.raises(ValueError, match="unknown fields"):
        parse_authorization(parsed)
    first.write_text(json.dumps(parsed, indent=2))
    with pytest.raises(ValueError, match="not canonical|unknown fields"):
        load_authorization(first)
    del parsed["root_ids_disjoint"]
    proof = verify_train_to_evaluation_authorization(
        second,
        training_root_manifest_path=training_path,
        evaluation_root_manifest_path=evaluation_path,
        expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
        evaluation_seed=_EVALUATION_SEED,
        requested_evaluator_name="beam",
    )
    assert proof.training.root_ids.isdisjoint(proof.evaluation.root_ids)
    assert proof.training.seeds.isdisjoint(proof.evaluation.seeds)
    assert proof.training.lineages.isdisjoint(proof.evaluation.lineages)
    for manifest_path, manifest in (
        (training_path, training),
        (evaluation_path, evaluation),
    ):
        for root in manifest.roots:
            snapshot = (manifest_path.parent / root.relative_path).read_bytes()
            assert hashlib.sha256(snapshot).hexdigest() == root.root_id


def test_overlapping_root_ids_are_rejected_when_seeds_and_lineages_are_disjoint(
    tmp_path: Path,
) -> None:
    shared: dict[str, object] = {"fixture": "shared-root"}
    training_path = _write_root_manifest(
        tmp_path / "training",
        nonce="train",
        roots=[("TRAIN-SEED", _lineage_for_split("train", "train-lineage"), shared)],
    )
    evaluation_path = _write_root_manifest(
        tmp_path / "evaluation",
        nonce="eval",
        roots=[("EVAL-SEED", _lineage_for_split("train", "eval-lineage"), shared)],
    )
    training = load_root_manifest(training_path, verify_roots=False)
    evaluation = load_root_manifest(evaluation_path, verify_roots=False)
    assert set(training.requested_seeds).isdisjoint(evaluation.requested_seeds)
    assert {root.lineages for root in training.roots}.isdisjoint(
        {root.lineages for root in evaluation.roots}
    )
    with pytest.raises(ValueError, match="root IDs are not disjoint"):
        _verify(tmp_path, training_path, evaluation_path, training, evaluation)


def test_overlapping_generation_seeds_are_rejected_when_ids_and_lineages_are_disjoint(
    tmp_path: Path,
) -> None:
    training_path = _write_root_manifest(
        tmp_path / "training",
        nonce="train",
        roots=[
            (
                "SHARED-SEED",
                _lineage_for_split("train", "train-lineage"),
                {"fixture": "train-shared-seed"},
            )
        ],
        extra_seeds=("TRAIN-ONLY",),
    )
    evaluation_path = _write_root_manifest(
        tmp_path / "evaluation",
        nonce="eval",
        roots=[
            (
                "SHARED-SEED",
                _lineage_for_split("train", "eval-lineage"),
                {"fixture": "eval-shared-seed"},
            )
        ],
        extra_seeds=("EVAL-ONLY",),
    )
    training = load_root_manifest(training_path, verify_roots=False)
    evaluation = load_root_manifest(evaluation_path, verify_roots=False)
    assert {root.root_id for root in training.roots}.isdisjoint(
        {root.root_id for root in evaluation.roots}
    )
    assert {root.lineages for root in training.roots}.isdisjoint(
        {root.lineages for root in evaluation.roots}
    )
    with pytest.raises(ValueError, match="generation seeds are not disjoint"):
        _verify(tmp_path, training_path, evaluation_path, training, evaluation)


def test_overlapping_lineages_are_rejected_when_ids_and_seeds_are_disjoint(
    tmp_path: Path,
) -> None:
    shared_lineage = _lineage_for_split("train", "shared-lineage")
    training_path = _write_root_manifest(
        tmp_path / "training",
        nonce="train",
        roots=[("TRAIN-SEED", shared_lineage, {"fixture": "train-shared-lineage"})],
    )
    evaluation_path = _write_root_manifest(
        tmp_path / "evaluation",
        nonce="eval",
        roots=[("EVAL-SEED", shared_lineage, {"fixture": "eval-shared-lineage"})],
    )
    training = load_root_manifest(training_path, verify_roots=False)
    evaluation = load_root_manifest(evaluation_path, verify_roots=False)
    assert {root.root_id for root in training.roots}.isdisjoint(
        {root.root_id for root in evaluation.roots}
    )
    assert set(training.requested_seeds).isdisjoint(evaluation.requested_seeds)
    with pytest.raises(ValueError, match="lineages are not disjoint"):
        _verify(tmp_path, training_path, evaluation_path, training, evaluation)


def test_digest_and_caller_bindings_are_recomputed(tmp_path: Path) -> None:
    training_path, evaluation_path, training, evaluation = _disjoint_pair(tmp_path)
    auth_path = tmp_path / "authorization.json"
    _write_bound_authorization(auth_path, training, evaluation)
    with pytest.raises(ValueError, match="source-epoch-bundle digest"):
        verify_train_to_evaluation_authorization(
            auth_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=_digest_label("other-bundle"),
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )
    with pytest.raises(ValueError, match="evaluation seed"):
        verify_train_to_evaluation_authorization(
            auth_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED + 1,
            requested_evaluator_name=_EVALUATOR,
        )
    with pytest.raises(ValueError, match="not authorized"):
        verify_train_to_evaluation_authorization(
            auth_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name="beam_clone",
        )
    tampered = authorization_from_bindings(
        training_root_manifest_digest=_digest_label("missing-training"),
        training_cohort_digest=training.cohort_digest,
        evaluation_root_manifest_digest=evaluation.manifest_digest,
        evaluation_cohort_digest=evaluation.cohort_digest,
        source_epoch_bundle_digest=training.source_epoch_bundle_digest,
        evaluation_seed=_EVALUATION_SEED,
        authorized_evaluator_names=("network_puct",),
    )
    tampered_path = tmp_path / "tampered.json"
    write_authorization(tampered_path, tampered)
    with pytest.raises(ValueError, match="training root-manifest digest"):
        verify_train_to_evaluation_authorization(
            tampered_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )


def test_same_artifact_and_empty_cohort_are_rejected(tmp_path: Path) -> None:
    training_path, evaluation_path, training, evaluation = _disjoint_pair(tmp_path)
    auth_path = tmp_path / "authorization.json"
    _write_bound_authorization(auth_path, training, evaluation)
    with pytest.raises(ValueError, match="same artifact"):
        verify_train_to_evaluation_authorization(
            auth_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=training_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )
    copied_dir = tmp_path / "copied"
    shutil.copytree(training_path.parent, copied_dir)
    copied = copied_dir / "root-manifest.json"
    with pytest.raises(ValueError, match="same artifact"):
        verify_train_to_evaluation_authorization(
            auth_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=copied,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )
    with pytest.raises(ValueError, match="same artifact"):
        authorization_from_bindings(
            training_root_manifest_digest=training.manifest_digest,
            training_cohort_digest=training.cohort_digest,
            evaluation_root_manifest_digest=training.manifest_digest,
            evaluation_cohort_digest=training.cohort_digest,
            source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            authorized_evaluator_names=("network_puct",),
        )
    empty_path = _write_root_manifest(
        tmp_path / "empty",
        nonce="empty",
        roots=[],
        extra_seeds=("EMPTY-SEED",),
    )
    empty = load_root_manifest(empty_path, verify_roots=False)
    empty_auth = tmp_path / "empty-auth.json"
    disjoint_eval = load_root_manifest(evaluation_path, verify_roots=False)
    _write_bound_authorization(empty_auth, empty, disjoint_eval)
    with pytest.raises(ValueError, match="cohort is empty"):
        verify_train_to_evaluation_authorization(
            empty_auth,
            training_root_manifest_path=empty_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )


def test_malformed_identities_duplicates_and_incomplete_dimensions() -> None:
    payload: dict[str, object] = {
        "kind": AUTHORIZATION_KIND,
        "schema_version": AUTHORIZATION_SCHEMA_VERSION,
        "training_root_manifest_digest": _digest_label("train"),
        "training_cohort_digest": _digest_label("train-cohort"),
        "evaluation_root_manifest_digest": _digest_label("eval"),
        "evaluation_cohort_digest": _digest_label("eval-cohort"),
        "source_epoch_bundle_digest": _BUNDLE,
        "evaluation_seed": _EVALUATION_SEED,
        "authorized_evaluator_names": ["beam", "random"],
        "mandatory_disjointness_dimensions": list(MANDATORY_DISJOINTNESS_DIMENSIONS),
        "authorization_digest": "0" * 64,
    }
    payload["authorization_digest"] = hashlib.sha256(
        json.dumps(
            {key: value for key, value in payload.items() if key != "authorization_digest"},
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode()
    ).hexdigest()
    parse_authorization(payload)
    malformed = dict(payload)
    malformed["training_root_manifest_digest"] = "AB" * 32
    malformed["authorization_digest"] = payload["authorization_digest"]
    with pytest.raises(ValueError, match="lowercase SHA-256 digest"):
        parse_authorization(malformed)
    duplicated = dict(payload)
    duplicated["authorized_evaluator_names"] = ["beam", "beam"]
    with pytest.raises(ValueError, match="duplicate entry"):
        parse_authorization(duplicated)
    unknown_name = dict(payload)
    unknown_name["authorized_evaluator_names"] = ["beam", "puct"]
    unknown_name["authorization_digest"] = "0" * 64
    with pytest.raises(ValueError, match="unauthorized evaluator name"):
        parse_authorization(unknown_name)
    incomplete = dict(payload)
    incomplete["mandatory_disjointness_dimensions"] = ["lineages", "root_ids"]
    incomplete["authorization_digest"] = "0" * 64
    with pytest.raises(ValueError, match="mandatory disjointness dimensions"):
        parse_authorization(incomplete)
    bool_seed = dict(payload)
    bool_seed["evaluation_seed"] = True
    with pytest.raises(TypeError, match="evaluation seed"):
        parse_authorization(bool_seed)
    bool_version = dict(payload)
    bool_version["schema_version"] = True
    with pytest.raises(TypeError, match="schema version"):
        parse_authorization(bool_version)
    float_version = dict(payload)
    float_version["schema_version"] = 1.0
    with pytest.raises(TypeError, match="schema version"):
        parse_authorization(float_version)


def test_symlinks_and_path_escapes_are_rejected(tmp_path: Path) -> None:
    training_path, evaluation_path, training, evaluation = _disjoint_pair(tmp_path)
    auth_path = tmp_path / "authorization.json"
    _write_bound_authorization(auth_path, training, evaluation)
    link = tmp_path / "authorization.link.json"
    link.symlink_to(auth_path)
    with pytest.raises(ValueError, match="must not be a symlink"):
        load_authorization(link)
    with pytest.raises(ValueError, match="must not be a symlink"):
        verify_train_to_evaluation_authorization(
            link,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )
    root = next(iter(training.roots))
    snapshot = training_path.parent / root.relative_path
    outside = tmp_path / "escaped.json"
    outside.write_bytes(b'{"escaped":true}')
    snapshot.unlink()
    snapshot.symlink_to(outside)
    with pytest.raises(ValueError, match="must not be a symlink"):
        verify_train_to_evaluation_authorization(
            auth_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )
    with pytest.raises(ValueError, match=r"\.\."):
        normalize_inventory_relative_path("../escaped.json")
    with pytest.raises(ValueError, match="absolute"):
        normalize_inventory_relative_path("/etc/passwd")


def test_sealed_split_roots_are_rejected_from_manifests(tmp_path: Path) -> None:
    path = _write_root_manifest(
        tmp_path / "sealed",
        nonce="sealed",
        roots=[
            (
                "SEALED-SEED",
                _lineage_for_split("train", "sealed-lineage"),
                {"fixture": "sealed"},
            )
        ],
    )
    payload = json.loads(path.read_text())
    root = dict(payload["roots"][0])
    root["split"] = "sealed_test"
    root["relative_path"] = f"sealed_test/roots/{root['root_id']}.json"
    payload["roots"] = [root]
    payload["manifest_digest"] = data_module._digest_payload(payload, "manifest_digest")
    path.write_bytes(data_module._canonical_bytes(payload))
    with pytest.raises(ValueError, match="sealed or unknown split"):
        load_root_manifest(path, verify_roots=False)


def test_missing_or_mismatched_snapshots_are_rejected(tmp_path: Path) -> None:
    training_path, evaluation_path, training, evaluation = _disjoint_pair(tmp_path)
    auth_path = tmp_path / "authorization.json"
    _write_bound_authorization(auth_path, training, evaluation)
    snapshot = training_path.parent / next(iter(training.roots)).relative_path
    snapshot.unlink()
    with pytest.raises(ValueError, match="root snapshot is missing"):
        verify_train_to_evaluation_authorization(
            auth_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )
    training_path, evaluation_path, training, evaluation = _disjoint_pair(tmp_path / "mismatch")
    auth_path = tmp_path / "mismatch-authorization.json"
    _write_bound_authorization(auth_path, training, evaluation)
    snapshot = training_path.parent / next(iter(training.roots)).relative_path
    snapshot.write_bytes(data_module._canonical_bytes({"tampered": True}))
    with pytest.raises(ValueError, match="is not canonical"):
        verify_train_to_evaluation_authorization(
            auth_path,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )


def test_missing_or_mismatched_source_epoch_bundles_fail_closed(tmp_path: Path) -> None:
    train_lineage = _lineage_for_split("train", "train-lineage")
    eval_lineage = _lineage_for_split("train", "eval-lineage")
    missing_field = _write_root_manifest(
        tmp_path / "missing-training",
        nonce="train",
        roots=[("TRAIN-SEED", train_lineage, {"fixture": "train"})],
    )
    payload = json.loads(missing_field.read_text())
    payload.pop("source_epoch_bundle_digest")
    with pytest.raises(ValueError, match="unsupported or malformed"):
        RootManifest.from_dict(payload)
    bound_training = _write_root_manifest(
        tmp_path / "bound-training",
        nonce="bound-train",
        roots=[("TRAIN-SEED", train_lineage, {"fixture": "train-bound"})],
        git_sha="a" * 40,
    )
    other_eval = _write_root_manifest(
        tmp_path / "other-eval",
        nonce="other-eval",
        roots=[("EVAL-SEED", eval_lineage, {"fixture": "eval-other"})],
        git_sha="b" * 40,
    )
    training = load_root_manifest(bound_training, verify_roots=False)
    evaluation = load_root_manifest(other_eval, verify_roots=False)
    assert training.source_epoch_bundle_digest != evaluation.source_epoch_bundle_digest
    mismatch_auth = tmp_path / "mismatch-bundle.json"
    _write_bound_authorization(mismatch_auth, training, evaluation)
    with pytest.raises(ValueError, match="source-epoch-bundle digests differ"):
        verify_train_to_evaluation_authorization(
            mismatch_auth,
            training_root_manifest_path=bound_training,
            evaluation_root_manifest_path=other_eval,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )
    training_path, evaluation_path, training, evaluation = _disjoint_pair(tmp_path / "missing-dir")
    missing_auth = tmp_path / "missing-dir.json"
    _write_bound_authorization(missing_auth, training, evaluation)
    shutil.rmtree(evaluation_path.parent / SOURCE_EPOCH_DIRNAME)
    with pytest.raises(ValueError, match="source-epoch-bundle"):
        verify_train_to_evaluation_authorization(
            missing_auth,
            training_root_manifest_path=training_path,
            evaluation_root_manifest_path=evaluation_path,
            expected_source_epoch_bundle_digest=training.source_epoch_bundle_digest,
            evaluation_seed=_EVALUATION_SEED,
            requested_evaluator_name=_EVALUATOR,
        )


def test_integer_lineages_are_rejected_as_malformed_identities(tmp_path: Path) -> None:
    path = _write_root_manifest(
        tmp_path / "roots",
        nonce="lineage-type",
        roots=[
            (
                "TRAIN-SEED",
                _lineage_for_split("train", "typed-lineage"),
                {"fixture": "typed"},
            )
        ],
    )
    payload = json.loads(path.read_text())
    integer_lineages = dict(payload)
    integer_lineages["roots"] = [dict(payload["roots"][0], lineages=[1])]
    with pytest.raises(TypeError, match="root lineages"):
        RootManifest.from_dict(integer_lineages)
    empty_lineage = dict(payload)
    empty_lineage["roots"] = [dict(payload["roots"][0], lineages=[""])]
    with pytest.raises(TypeError, match="root lineages"):
        RootManifest.from_dict(empty_lineage)
