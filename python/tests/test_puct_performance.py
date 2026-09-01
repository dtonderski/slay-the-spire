from __future__ import annotations

import json
import os
import time
from pathlib import Path
from typing import cast

import pytest
import torch

from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import (
    CombatModelConfig,
    FairCombatPolicyValueNet,
    Vocabularies,
    VocabularyBuilder,
    puct_clone_episode_payload,
    puct_search_payload,
    select_puct_action,
)
from sts_sim.rl.provenance import capture_repository_version
from sts_sim.rl.puct import PUCT_TEACHER_VERSION, network_leaf_evaluator

_ROOTS = os.environ.get("STS_PUCT_PERF_ROOTS")
_REPORT_DIR = os.environ.get("STS_PUCT_PERF_REPORT_DIR")


def _uniform_evaluator(request_json: str) -> str:
    request = json.loads(request_json)
    observation = request["batch"][0]["observation"]
    choices = request["batch"][0]["choices"]
    encoded = json.dumps(observation, sort_keys=True, separators=(",", ":"))
    fingerprint = 0xCBF29CE484222325
    for byte in encoded.encode():
        fingerprint ^= byte
        fingerprint = (fingerprint * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    value = ((fingerprint % 1999) / 999.5) - 1.0
    priors = []
    for index, choice in enumerate(choices):
        mixed = fingerprint ^ (index + 1)
        choice_bytes = json.dumps(choice, sort_keys=True, separators=(",", ":")).encode()
        for byte in choice_bytes:
            mixed ^= byte
            mixed = (mixed * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
        priors.append(float((mixed % 89) + 1))
    return json.dumps(
        {
            "schema": "fair_leaf_batch_v1",
            "batch": [{"priors": priors, "value": value}],
        }
    )


def _tiny_policy_net(env: RunEnv) -> tuple[FairCombatPolicyValueNet, Vocabularies]:
    decision = env.decision()
    observation = decision.observation
    assert isinstance(observation, FairCombatObservation)
    descriptors = tuple(action.descriptor() for action in decision.actions)
    builder = VocabularyBuilder()
    builder.add(observation, descriptors)
    vocabularies = builder.freeze()
    torch.manual_seed(7)
    model = FairCombatPolicyValueNet(
        vocabularies,
        CombatModelConfig(width=32, heads=4, layers=1, feedforward_width=64),
    ).eval()
    return model, vocabularies


def _decision_core(payload: dict[str, object]) -> dict[str, object]:
    return {
        "selected_index": payload["selected_index"],
        "visits": payload["visits"],
        "priors": payload["priors"],
        "value": payload["value"],
        "transitions": payload["transitions"],
        "completed_simulations": payload["completed_simulations"],
        "stop_reason": payload["stop_reason"],
        "choices": payload["choices"],
        "teacher_name": payload["teacher_name"],
        "teacher_version": payload["teacher_version"],
    }


def test_search_preserves_root_bytes_and_teacher_version() -> None:
    env = RunEnv.combat_fixture()
    before = env.snapshot()
    first = puct_search_payload(env, _uniform_evaluator, simulation_budget=8, transition_budget=8)
    second = puct_search_payload(env, _uniform_evaluator, simulation_budget=8, transition_budget=8)
    after = env.snapshot()
    assert before.hash == after.hash
    assert before.json == after.json
    assert first == second
    assert first["teacher_version"] == PUCT_TEACHER_VERSION == "synchronous_batch1_v3"
    assert first["teacher_name"] == "privileged_puct"
    decision = env.decision()
    model, vocabularies = _tiny_policy_net(env)
    selected = select_puct_action(
        env,
        decision,
        model,
        vocabularies,
        simulation_budget=4,
        transition_budget=4,
    )
    assert any(candidate is selected for candidate in decision.actions)
    assert env.snapshot().hash == before.hash


def test_teacher_exact_state_cache_matches_off_except_eval_count() -> None:
    env = RunEnv.combat_fixture()
    before = env.snapshot()
    off = puct_search_payload(
        env,
        _uniform_evaluator,
        simulation_budget=64,
        transition_budget=64,
        leaf_cache="off",
    )
    on = puct_search_payload(
        env,
        _uniform_evaluator,
        simulation_budget=64,
        transition_budget=64,
        leaf_cache="exact_state",
    )
    default = puct_search_payload(
        env,
        _uniform_evaluator,
        simulation_budget=64,
        transition_budget=64,
    )
    after = env.snapshot()
    assert before.hash == after.hash
    assert before.json == after.json
    assert _decision_core(off) == _decision_core(on) == _decision_core(default)
    assert default["leaf_evaluations"] == off["leaf_evaluations"]
    assert on["leaf_evaluations"] <= off["leaf_evaluations"]


def test_clone_episode_keeps_independent_root_bytes_and_turn_caps() -> None:
    env = RunEnv.combat_fixture()
    before = env.snapshot()
    payload = puct_clone_episode_payload(
        env,
        _uniform_evaluator,
        simulation_budget=4,
        transition_budget=4,
        max_decisions=1,
        max_player_turns=1,
    )
    off = puct_clone_episode_payload(
        env,
        _uniform_evaluator,
        simulation_budget=4,
        transition_budget=4,
        max_decisions=1,
        max_player_turns=1,
        leaf_cache="off",
    )
    on = puct_clone_episode_payload(
        env,
        _uniform_evaluator,
        simulation_budget=4,
        transition_budget=4,
        max_decisions=1,
        max_player_turns=1,
        leaf_cache="exact_state",
    )
    after = env.snapshot()
    assert before.hash == after.hash
    assert before.json == after.json
    outcome = cast(dict[str, object], payload["outcome"])
    assert outcome["status"] == "truncated"
    assert outcome["truncation_trigger"] == "accepted_decisions"
    assert payload["teacher_version"] == "synchronous_batch1_v3"
    assert payload == off
    assert payload["outcome"] == on["outcome"]
    assert payload["steps"] == on["steps"]


def _runtime_source_epoch() -> dict[str, object]:
    repo = Path(__file__).resolve().parents[2]
    return capture_repository_version(repo, allow_dirty=True).to_dict()


@pytest.mark.skipif(
    not _REPORT_DIR,
    reason="set STS_PUCT_PERF_REPORT_DIR to emit fixture network timings",
)
def test_network_leaf_is_the_binding_hot_path_on_fixture() -> None:
    env = RunEnv.combat_fixture()
    model, vocabularies = _tiny_policy_net(env)
    evaluator = network_leaf_evaluator(model, vocabularies)
    started = time.perf_counter()
    off = puct_search_payload(
        env,
        evaluator,
        simulation_budget=64,
        transition_budget=64,
        leaf_cache="off",
    )
    off_s = time.perf_counter() - started
    started = time.perf_counter()
    on = puct_search_payload(
        env,
        evaluator,
        simulation_budget=64,
        transition_budget=64,
        leaf_cache="exact_state",
    )
    on_s = time.perf_counter() - started
    assert _decision_core(off) == _decision_core(on)
    assert off["teacher_version"] == on["teacher_version"] == "synchronous_batch1_v3"
    assert off["completed_simulations"] == 64
    report_dir = Path(_REPORT_DIR)
    report_dir.mkdir(parents=True, exist_ok=True)
    (report_dir / "python-fixture-timings.json").write_text(
        json.dumps(
            {
                "source_epoch": _runtime_source_epoch(),
                "simulation_budget": 64,
                "transition_budget": 64,
                "network_leaf_off_s": off_s,
                "network_leaf_exact_state_s": on_s,
                "network_leaf_evaluations_off": off["leaf_evaluations"],
                "network_leaf_evaluations_exact_state": on["leaf_evaluations"],
                "exact_state_eval_savings": off["leaf_evaluations"] - on["leaf_evaluations"],
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )


def _preselect_development_combat_roots(
    manifest: dict[str, object], manifest_path: Path, limit: int
) -> list[dict[str, object]]:
    selected: list[dict[str, object]] = []
    for entry in cast(list[dict[str, object]], manifest["roots"]):
        if entry["split"] != "development":
            continue
        snapshot_text = (manifest_path.parent / cast(str, entry["relative_path"])).read_text()
        env = RunEnv.from_snapshot(snapshot_text)
        if env.phase != "combat":
            continue
        observation = env.observation()
        if not isinstance(observation, FairCombatObservation):
            continue
        # Match `classify_combat_state`: terminal combat includes hp <= 0 even
        # when the fair observation phase is still waiting_for_player.
        if observation.phase in ("won", "lost") or observation.player.hp <= 0:
            continue
        selected.append(entry)
        if len(selected) == limit:
            break
    assert (
        len(selected) == limit
    ), f"expected {limit} development combat roots, got {len(selected)}"
    return selected


@pytest.mark.skipif(
    not _ROOTS,
    reason="set STS_PUCT_PERF_ROOTS to a root-manifest.json",
)
def test_twenty_development_roots_are_byte_and_decision_stable() -> None:
    manifest_path = Path(_ROOTS)
    assert manifest_path.is_file(), f"STS_PUCT_PERF_ROOTS is not a file: {manifest_path}"
    manifest = json.loads(manifest_path.read_text())
    development = _preselect_development_combat_roots(manifest, manifest_path, 20)
    rows: list[dict[str, object]] = []
    for entry in development:
        snapshot_text = (manifest_path.parent / cast(str, entry["relative_path"])).read_text()
        first = RunEnv.from_snapshot(snapshot_text)
        second = RunEnv.from_snapshot(snapshot_text)
        assert first.snapshot().hash == second.snapshot().hash
        assert first.snapshot().json == second.snapshot().json
        before = first.snapshot()
        off = puct_search_payload(
            first,
            _uniform_evaluator,
            simulation_budget=64,
            transition_budget=64,
            leaf_cache="off",
        )
        on = puct_search_payload(
            first,
            _uniform_evaluator,
            simulation_budget=64,
            transition_budget=64,
            leaf_cache="exact_state",
        )
        after = first.snapshot()
        assert before.hash == after.hash
        assert before.json == after.json
        assert _decision_core(off) == _decision_core(on)
        assert off["teacher_version"] == "synchronous_batch1_v3"
        assert sum(cast(list[int], off["visits"])) == off["completed_simulations"]
        rows.append(
            {
                "root_id": entry["root_id"],
                "selected_index": off["selected_index"],
                "visits": off["visits"],
                "value": off["value"],
                "transitions": off["transitions"],
                "completed_simulations": off["completed_simulations"],
                "leaf_evaluations_off": off["leaf_evaluations"],
                "leaf_evaluations_exact_state": on["leaf_evaluations"],
                "restore_hash": before.hash,
            }
        )
    assert len(rows) == 20
    if _REPORT_DIR:
        report_dir = Path(_REPORT_DIR)
        report_dir.mkdir(parents=True, exist_ok=True)
        (report_dir / "python-root-equivalence.json").write_text(
            json.dumps(
                {
                    "source_epoch": _runtime_source_epoch(),
                    "root_count": len(rows),
                    "roots": rows,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n"
        )
