from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import pytest

import sts_sim.rl.data as data_module
from sts_sim import FairCombatObservation, RunEnv
from sts_sim.fair import FairContext, FairRunObservation
from sts_sim.rl import generate_legal_roots, load_root_manifest
from sts_sim.rl.cli import data_main
from sts_sim.rl.data import ROOT_MANIFEST_V4, ROOT_MANIFEST_VERSION, RootManifest
from sts_sim.run import Action, Decision


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
        "hp_zero_player_turn",
        "unmodeled_public_content",
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
    payload["generator_version"] = data_module._GENERATOR_VERSION_V4
    _resign_root_manifest(payload)
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    path.write_bytes(encoded)
    loaded = load_root_manifest(path)
    assert loaded.manifest_version == ROOT_MANIFEST_V4
    assert loaded.combat_depth == 1
    assert loaded.generator_version == data_module._GENERATOR_VERSION_V4
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
        "hp_zero_player_turn",
        "unmodeled_public_content",
        "generation_error",
    }
    assert any(exclusion.reason == "step_limit" for exclusion in manifest.exclusions)
    assert all(
        "reached combat " in exclusion.detail and " of requested depth 2" in exclusion.detail
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


def _restore_combat_observation(root_dir: Path, manifest: RootManifest) -> FairCombatObservation:
    env = RunEnv.from_snapshot((root_dir / manifest.roots[0].relative_path).read_text())
    observation = env.decision().observation
    assert isinstance(observation, FairCombatObservation)
    return observation


def test_depth_two_root_is_later_public_position_than_depth_one(tmp_path: Path) -> None:
    first = generate_legal_roots(
        tmp_path / "depth-1", ["BEAMCLONE0"], max_run_steps=512, combat_depth=1
    )
    second = generate_legal_roots(
        tmp_path / "depth-2", ["BEAMCLONE0"], max_run_steps=512, combat_depth=2
    )
    assert first.roots and second.roots
    assert first.roots[0].root_id != second.roots[0].root_id
    first_obs = _restore_combat_observation(tmp_path / "depth-1", first)
    second_obs = _restore_combat_observation(tmp_path / "depth-2", second)
    assert (second_obs.context.act, second_obs.context.floor) > (
        first_obs.context.act,
        first_obs.context.floor,
    )


def test_root_manifest_key_sets_match_dataclass() -> None:
    assert data_module._ROOT_MANIFEST_V5_KEYS == set(RootManifest.__dataclass_fields__)
    assert data_module._ROOT_MANIFEST_V4_KEYS == data_module._ROOT_MANIFEST_V5_KEYS - {
        "combat_depth"
    }


def test_root_manifest_rejects_non_int_versions_and_cross_schema_generators(
    tmp_path: Path,
) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    path = tmp_path / "roots/root-manifest.json"
    original = json.loads(path.read_text())

    for version in (4.0, 5.0, True, False):
        payload = dict(original)
        payload["manifest_version"] = version
        if version == 4.0:
            payload.pop("combat_depth", None)
            payload["generator_version"] = data_module._GENERATOR_VERSION_V4
        _resign_root_manifest(payload)
        path.write_text(json.dumps(payload, sort_keys=True, separators=(",", ":")))
        with pytest.raises(ValueError, match="unsupported or malformed"):
            load_root_manifest(path)

    v5_with_v3 = dict(original)
    v5_with_v3["generator_version"] = data_module._GENERATOR_VERSION_V4
    _resign_root_manifest(v5_with_v3)
    path.write_text(json.dumps(v5_with_v3, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="generator identity does not match schema version"):
        load_root_manifest(path)

    v4_with_v4 = dict(original)
    v4_with_v4.pop("combat_depth")
    v4_with_v4["manifest_version"] = ROOT_MANIFEST_V4
    v4_with_v4["generator_version"] = data_module._GENERATOR_VERSION
    _resign_root_manifest(v4_with_v4)
    path.write_text(json.dumps(v4_with_v4, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="generator identity does not match schema version"):
        load_root_manifest(path)

    wrong_name = dict(original)
    wrong_name["generator_name"] = "other_policy"
    _resign_root_manifest(wrong_name)
    path.write_text(json.dumps(wrong_name, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="generator identity does not match schema version"):
        load_root_manifest(path)


def _run_kind_combat_decision(actions: tuple[Action, ...]) -> Decision:
    context = FairContext(ascension=0, act=1, floor=1, gold=99)
    observation = FairRunObservation(
        schema_version=1,
        phase="combat",
        kind="map",
        context=context,
        screen={},
    )
    return Decision(
        revision=0,
        phase="combat",
        kind="map",
        observation=observation,
        actions=actions,
    )


def test_combat_boundary_uses_decision_phase_not_observation_kind() -> None:
    env = RunEnv.combat_fixture()
    combat = env.decision()
    assert combat.phase == "combat"
    in_combat, index, entered = data_module._update_combat_boundary(
        in_combat=False, combat_index=0, decision=combat
    )
    assert (in_combat, index, entered) == (True, 1, True)
    run_kind = _run_kind_combat_decision(combat.actions)
    stayed = data_module._update_combat_boundary(in_combat=True, combat_index=1, decision=run_kind)
    assert stayed == (True, 1, False)
    left = Decision(
        revision=combat.revision,
        phase="map",
        kind="map",
        observation=run_kind.observation,
        actions=combat.actions,
    )
    assert data_module._update_combat_boundary(in_combat=True, combat_index=1, decision=left) == (
        False,
        1,
        False,
    )


def test_earlier_non_capturable_combat_with_actions_is_not_aborted() -> None:
    env = RunEnv.combat_fixture()
    combat = env.decision()
    run_kind = _run_kind_combat_decision(combat.actions)
    assert data_module._combat_entry_exclusion(run_kind, combat_index=1, combat_depth=2) is None
    assert (
        data_module._combat_entry_exclusion(run_kind, combat_index=2, combat_depth=2)
        == "terminal_combat"
    )
    empty = _run_kind_combat_decision(())
    assert (
        data_module._combat_entry_exclusion(empty, combat_index=1, combat_depth=2)
        == "terminal_combat"
    )
    assert data_module._is_capturable_combat_decision(combat)
    assert data_module._combat_entry_exclusion(combat, combat_index=1, combat_depth=1) is None


def test_zero_hp_waiting_for_player_is_not_a_capturable_root() -> None:
    state = RunEnv.combat_fixture().full_state()
    combat = cast(dict[str, object], state["combat"])
    player = cast(dict[str, object], combat["player"])
    player["hp"] = 0
    state["player_hp"] = 0
    env = RunEnv.from_state_json_for_debugging(json.dumps(state))
    decision = env.decision()
    assert isinstance(decision.observation, FairCombatObservation)
    assert decision.observation.phase == "waiting_for_player"
    assert decision.observation.player.hp == 0
    assert decision.actions
    assert not data_module._is_capturable_combat_decision(decision)
    assert (
        data_module._combat_entry_exclusion(decision, combat_index=1, combat_depth=1)
        == "hp_zero_player_turn"
    )
    assert (
        data_module._combat_entry_exclusion(decision, combat_index=1, combat_depth=2)
        == "hp_zero_player_turn"
    )


def test_load_root_manifest_accepts_historical_zero_hp_root(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    manifest_path = tmp_path / "roots/root-manifest.json"
    payload = json.loads(manifest_path.read_text())
    root = payload["roots"][0]
    old_path = tmp_path / "roots" / root["relative_path"]
    snapshot = json.loads(old_path.read_text())
    state = cast(dict[str, object], snapshot["state"])
    combat = cast(dict[str, object], state["combat"])
    player = cast(dict[str, object], combat["player"])
    player["hp"] = 0
    state["player_hp"] = 0
    encoded = data_module._canonical_bytes(snapshot)
    new_id = data_module._sha256_bytes(encoded)
    relative_path = f"{root['split']}/roots/{new_id}.json"
    new_path = tmp_path / "roots" / relative_path
    new_path.write_bytes(encoded)
    if new_path != old_path:
        old_path.unlink()
    root["root_id"] = new_id
    root["relative_path"] = relative_path
    _resign_root_manifest(payload)
    manifest_path.write_bytes(data_module._canonical_bytes(payload))
    loaded = load_root_manifest(manifest_path)
    assert loaded.roots[0].root_id == new_id
    restored = RunEnv.from_snapshot(new_path.read_text())
    decision = restored.decision()
    assert isinstance(decision.observation, FairCombatObservation)
    assert decision.observation.player.hp == 0
