from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

import sts_sim.rl.data as data_module
from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import generate_legal_roots, load_root_manifest
from sts_sim.rl.cli import data_main
from sts_sim.rl.data import ROOT_MANIFEST_V4, ROOT_MANIFEST_VERSION


def _resign_root_manifest(payload: dict[str, object]) -> None:
    payload["cohort_digest"] = data_module._cohort_digest(
        requested_seeds=tuple(cast(list[str], payload["requested_seeds"])),
        generator_name=cast(str, payload["generator_name"]),
        generator_version=cast(str, payload["generator_version"]),
        generator_source_digest=cast(str, payload["generator_source_digest"]),
        split_salt=cast(str, payload["split_salt"]),
        ascension=cast(int, payload["ascension"]),
        max_run_steps=cast(int, payload["max_run_steps"]),
        combat_depth=(cast(int, payload["combat_depth"]) if "combat_depth" in payload else None),
    )
    unsigned = dict(payload)
    unsigned.pop("manifest_digest")
    payload["manifest_digest"] = data_module._sha256_bytes(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()
    )


def test_combat_depth_bounds_are_positive_integers(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="combat depth must be a positive integer"):
        generate_legal_roots(tmp_path / "zero", ["BEAMCLONE0"], combat_depth=0)
    with pytest.raises(ValueError, match="combat depth must be a positive integer"):
        generate_legal_roots(tmp_path / "negative", ["BEAMCLONE0"], combat_depth=-1)
    with pytest.raises(ValueError, match="combat depth must be a positive integer"):
        generate_legal_roots(tmp_path / "float", ["BEAMCLONE0"], combat_depth=cast(int, 1.5))


def test_default_depth_one_writes_v5_with_explicit_combat_depth(tmp_path: Path) -> None:
    manifest = generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    payload = json.loads((tmp_path / "roots/root-manifest.json").read_text())
    assert manifest.manifest_version == ROOT_MANIFEST_VERSION
    assert manifest.combat_depth == 1
    assert payload["manifest_version"] == ROOT_MANIFEST_VERSION
    assert payload["combat_depth"] == 1
    assert payload["generator_version"] == "sha256_action_policy_v4"
    assert manifest.roots
    load_root_manifest(tmp_path / "roots/root-manifest.json")


def test_depth_two_generation_is_byte_identical_and_actionable(tmp_path: Path) -> None:
    seeds = ["BEAMCLONE0", "BEAMCLONE12", "BEAMCLONE17"]
    left = generate_legal_roots(tmp_path / "left", seeds, max_run_steps=512, combat_depth=2)
    right = generate_legal_roots(tmp_path / "right", seeds, max_run_steps=512, combat_depth=2)
    assert (tmp_path / "left/root-manifest.json").read_bytes() == (
        tmp_path / "right/root-manifest.json"
    ).read_bytes()
    assert left.manifest_digest == right.manifest_digest
    assert left.combat_depth == 2
    assert left.roots
    assert {exclusion.reason for exclusion in left.exclusions} <= {
        "step_limit",
        "terminal_run",
        "terminal_combat",
        "duplicate_root",
        "cross_split_provenance",
        "withheld_audited_split",
        "generation_error",
    }
    accounted = {root_seed for root in left.roots for root_seed in root.source_seeds}
    accounted.update(exclusion.source_seed for exclusion in left.exclusions)
    assert accounted == set(seeds)
    restored = load_root_manifest(tmp_path / "left/root-manifest.json")
    for root in restored.roots:
        env = RunEnv.from_snapshot((tmp_path / "left" / root.relative_path).read_text())
        decision = env.decision()
        assert isinstance(decision.observation, FairCombatObservation)
        assert decision.observation.phase == "waiting_for_player"
        assert decision.actions
        assert not all(action.kind == "proceed" for action in decision.actions)


def test_cohort_digest_changes_with_combat_depth(tmp_path: Path) -> None:
    seeds = ["BEAMCLONE0", "BEAMCLONE12"]
    first = generate_legal_roots(tmp_path / "depth-1", seeds, max_run_steps=128, combat_depth=1)
    second = generate_legal_roots(tmp_path / "depth-2", seeds, max_run_steps=128, combat_depth=2)
    assert first.cohort_digest != second.cohort_digest
    assert first.combat_depth == 1
    assert second.combat_depth == 2


def test_legacy_v4_root_manifest_round_trips_without_combat_depth(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    path = tmp_path / "roots/root-manifest.json"
    payload = json.loads(path.read_text())
    payload.pop("combat_depth")
    payload["manifest_version"] = ROOT_MANIFEST_V4
    _resign_root_manifest(payload)
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    path.write_bytes(encoded)
    loaded = load_root_manifest(path)
    assert loaded.manifest_version == ROOT_MANIFEST_V4
    assert loaded.combat_depth == 1
    assert "combat_depth" not in json.loads(path.read_text())
    assert path.read_bytes() == encoded
    assert loaded.to_dict()["manifest_version"] == ROOT_MANIFEST_V4
    assert "combat_depth" not in loaded.to_dict()
    assert data_module._canonical_bytes(loaded.to_dict()) == encoded


def test_root_manifest_rejects_unknown_fields_and_v5_without_depth(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    path = tmp_path / "roots/root-manifest.json"
    original = json.loads(path.read_text())

    extra = dict(original)
    extra["unexpected"] = True
    _resign_root_manifest(extra)
    path.write_text(json.dumps(extra, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="unsupported or malformed"):
        load_root_manifest(path)

    missing = dict(original)
    missing.pop("combat_depth")
    _resign_root_manifest(missing)
    path.write_text(json.dumps(missing, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="unsupported or malformed"):
        load_root_manifest(path)

    guessed = dict(original)
    guessed["manifest_version"] = ROOT_MANIFEST_V4
    _resign_root_manifest(guessed)
    path.write_text(json.dumps(guessed, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="unsupported or malformed"):
        load_root_manifest(path)


def test_depth_sampling_preserves_split_and_audited_withholding(tmp_path: Path) -> None:
    seeds = ["BEAMCLONE0", "BEAMCLONE12", "BEAMCLONE17"]
    ordinary = generate_legal_roots(tmp_path / "ordinary", seeds, max_run_steps=512, combat_depth=2)
    assert ordinary.audited_splits_materialized is False
    assert {root.split for root in ordinary.roots} <= {"train", "development"}
    assert all(root.split not in {"sealed_test", "real_trace_audit"} for root in ordinary.roots)
    withheld = {
        exclusion.source_seed
        for exclusion in ordinary.exclusions
        if exclusion.reason == "withheld_audited_split"
    }
    audited = generate_legal_roots(
        tmp_path / "audited",
        seeds,
        max_run_steps=512,
        combat_depth=2,
        materialize_audited_splits=True,
    )
    assert audited.audited_splits_materialized is True
    audited_seeds = {
        seed for root in audited.roots if root.split == "sealed_test" for seed in root.source_seeds
    }
    assert withheld == audited_seeds
    with pytest.raises(PermissionError):
        load_root_manifest(tmp_path / "audited/root-manifest.json")
    load_root_manifest(tmp_path / "audited/root-manifest.json", allow_audited_materialization=True)


def test_step_limit_before_requested_depth_is_typed_and_complete(tmp_path: Path) -> None:
    seeds = ["BEAMCLONE0", "BEAMCLONE12"]
    manifest = generate_legal_roots(tmp_path / "capped", seeds, max_run_steps=1, combat_depth=2)
    assert manifest.roots == ()
    assert {exclusion.source_seed for exclusion in manifest.exclusions} == set(seeds)
    assert {exclusion.reason for exclusion in manifest.exclusions} <= {
        "step_limit",
        "terminal_run",
        "terminal_combat",
        "generation_error",
    }
    assert any(exclusion.reason == "step_limit" for exclusion in manifest.exclusions)
    assert all(
        exclusion.detail == "requested combat depth not reached"
        for exclusion in manifest.exclusions
        if exclusion.reason in {"step_limit", "terminal_run"}
    )


def test_roots_cli_passes_combat_depth(tmp_path: Path) -> None:
    output = tmp_path / "cli-roots"
    assert (
        data_main(
            [
                "roots",
                "--output",
                str(output),
                "--seed-prefix",
                "BEAMCLONE",
                "--start",
                "0",
                "--count",
                "1",
                "--max-run-steps",
                "128",
                "--combat-depth",
                "1",
            ]
        )
        == 0
    )
    payload = json.loads((output / "root-manifest.json").read_text())
    assert payload["combat_depth"] == 1
    assert payload["manifest_version"] == ROOT_MANIFEST_VERSION
