from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import cast

import pytest
import torch

from sts_sim import Action, ActionDescriptor, Decision, FairCombatObservation, RunEnv
from sts_sim.rl import (
    CombatModelConfig,
    FairCombatPolicyValueNet,
    PolicyEpisode,
    Vocabularies,
    VocabularyBuilder,
    aggregate_paired_differences,
    aggregate_policy_metrics,
    evaluate_matched_puct_gameplay,
    evaluate_matched_puct_roots,
    gameplay,
    random_policy_index,
    select_greedy_action,
)
from sts_sim.rl.gameplay import canonical_public_action_descriptors


def _end_turn(decision: Decision) -> Action:
    for action in decision.actions:
        if action.kind == "end_turn":
            return action
    return decision.actions[0]


def _fixture_snapshot() -> tuple[str, bytes]:
    snapshot = RunEnv.combat_fixture().snapshot()
    snapshot_bytes = snapshot.json.encode()
    return hashlib.sha256(snapshot_bytes).hexdigest(), snapshot_bytes


def test_canonical_root_bytes_restore_by_state_hash_not_wire_byte_round_trip() -> None:
    snapshot = RunEnv.combat_fixture().snapshot()
    canonical_bytes = json.dumps(
        json.loads(snapshot.json), sort_keys=True, separators=(",", ":")
    ).encode()
    root_id = hashlib.sha256(canonical_bytes).hexdigest()
    left = gameplay._restore_independently(canonical_bytes, root_id)
    right = gameplay._restore_independently(canonical_bytes, root_id)
    assert left.snapshot().hash == right.snapshot().hash
    assert left.snapshot().json.encode() != canonical_bytes


def _tiny_policy_net() -> tuple[RunEnv, FairCombatPolicyValueNet, Vocabularies]:
    env = RunEnv.combat_fixture()
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
    return env, model, vocabularies


def test_random_policy_index_golden_descriptor_hash() -> None:
    descriptors = canonical_public_action_descriptors(
        (ActionDescriptor(family="combat", kind="end_turn"),)
    )
    payload = [7, "root-id", 0, descriptors]
    digest = hashlib.sha256(
        json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    )
    assert digest.hexdigest() == "bb9162d8fa97c14415e236e3c309761d76359f51718467f66afd9b94e6c67779"
    assert random_policy_index(
        evaluation_seed=7,
        root_id="root-id",
        accepted_decision_index=0,
        descriptors=descriptors,
    ) == int.from_bytes(digest.digest()[:8], "big") % len(descriptors)


def test_greedy_returns_original_sidecar_action() -> None:
    env, model, vocabularies = _tiny_policy_net()
    decision = env.decision()
    selected = select_greedy_action(decision, model, vocabularies)
    assert any(candidate is selected for candidate in decision.actions)


def test_public_caps_match_native_truncation_semantics() -> None:
    def choose(decision: Decision, accepted_decision_index: int) -> Action:
        del accepted_decision_index
        return _end_turn(decision)

    decisions = gameplay._capped_public_episode(
        RunEnv.combat_fixture(),
        max_decisions=1,
        max_player_turns=100,
        choose=choose,
    )
    assert decisions.status == "truncated"
    assert decisions.accepted_decisions == 1
    assert decisions.truncation_trigger == "accepted_decisions"
    assert type(decisions.terminal_hp) is int

    turns = gameplay._capped_public_episode(
        RunEnv.combat_fixture(),
        max_decisions=512,
        max_player_turns=1,
        choose=choose,
    )
    assert turns.status == "truncated"
    assert turns.truncation_trigger == "player_turns"
    assert turns.player_turns > 1

    native = RunEnv.combat_fixture().beam_clone_episode_payload(
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=1,
        max_player_turns=100,
    )
    outcome = cast(dict[str, object], native["outcome"])
    assert outcome["status"] == "truncated"
    assert outcome["accepted_decisions"] == 1
    assert outcome["truncation_trigger"] == "accepted_decisions"


def test_initial_hp_zero_is_lost_like_native() -> None:
    state = RunEnv.combat_fixture().full_state()
    combat = cast(dict[str, object], state["combat"])
    player = cast(dict[str, object], combat["player"])
    player["hp"] = 0
    state["player_hp"] = 0
    env = RunEnv.from_state_json_for_debugging(json.dumps(state))
    episode = gameplay.rollout_random_policy(
        env,
        evaluation_seed=0,
        root_id="hp-zero",
        max_decisions=8,
        max_player_turns=8,
    )
    assert episode.status == "lost"
    assert episode.accepted_decisions == 0
    assert episode.player_turns == 1
    assert episode.terminal_hp == 0
    native = RunEnv.from_state_json_for_debugging(json.dumps(state)).beam_clone_episode_payload(
        depth=2, width=4, transition_budget=100, max_decisions=1, max_player_turns=1
    )
    assert cast(dict[str, object], native["outcome"])["status"] == "lost"


def test_injected_error_and_truncation_keep_denominator_and_partial_metrics(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def choose(decision: Decision, accepted_decision_index: int) -> Action:
        if accepted_decision_index >= 1:
            raise ValueError("injected failure")
        return _end_turn(decision)

    failed = gameplay._capped_public_episode(
        RunEnv.combat_fixture(),
        max_decisions=8,
        max_player_turns=8,
        choose=choose,
    )
    assert failed.status == "error"
    assert failed.accepted_decisions == 1
    assert failed.player_turns >= 1
    assert type(failed.terminal_hp) is int
    assert failed.error == "injected failure"

    root_hp = gameplay._detached_player_hp(RunEnv.combat_fixture())

    def boom(
        self: RunEnv,
        *,
        depth: int,
        width: int,
        transition_budget: int,
        max_decisions: int,
        max_player_turns: int,
        deduplicate_search_states: bool,
    ) -> dict[str, object]:
        del self, depth, width, transition_budget, max_decisions, max_player_turns
        del deduplicate_search_states
        raise RuntimeError("beam setup failed")

    monkeypatch.setattr(RunEnv, "beam_clone_episode_payload", boom)
    beam_failed = gameplay.rollout_beam_policy(
        RunEnv.combat_fixture(),
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=1,
        max_player_turns=100,
        deduplicate_search_states=True,
    )
    assert beam_failed.status == "error"
    assert beam_failed.accepted_decisions == 0
    assert beam_failed.player_turns == 1
    assert beam_failed.terminal_hp == root_hp

    truncated = PolicyEpisode("truncated", 3, 4, 7, truncation_trigger="accepted_decisions")
    won = PolicyEpisode("won", 2, 2, 10)
    metrics = aggregate_policy_metrics((won, failed, truncated))
    assert metrics["win_numerator"] == 1
    assert metrics["win_denominator"] == 3
    assert metrics["errors"] == 1
    assert metrics["truncations"] == 1
    assert cast(dict[str, object], metrics["accepted_decisions"])["count"] == 3
    terminal_hp = cast(dict[str, object], metrics["terminal_hp"])
    assert terminal_hp["count"] == 2
    assert terminal_hp["sum"] == 17

    crashed = PolicyEpisode("error", 0, 1, 999, error="injected overflow")
    crashed_metrics = aggregate_policy_metrics((won, crashed))
    assert crashed_metrics["win_denominator"] == 2
    assert crashed_metrics["errors"] == 1
    assert cast(dict[str, object], crashed_metrics["terminal_hp"])["count"] == 1
    assert cast(dict[str, object], crashed_metrics["terminal_hp"])["sum"] == 10

    paired = aggregate_paired_differences(
        (won, failed),
        (PolicyEpisode("lost", 5, 3, 1), PolicyEpisode("won", 1, 1, 9)),
    )
    per_root = cast(list[dict[str, object]], paired["per_root"])
    assert per_root[0]["errored"] is False
    assert per_root[0]["accepted_decision_delta"] == 3
    assert per_root[0]["hp_delta"] == -9
    assert per_root[1]["errored"] is True
    assert per_root[1]["left_status"] == "error"
    assert per_root[1]["right_status"] == "won"
    assert per_root[1]["accepted_decision_delta"] is None
    assert per_root[1]["hp_delta"] is None
    assert cast(dict[str, object], paired["accepted_decision_delta"])["count"] == 1
    assert cast(dict[str, object], paired["hp_delta"])["count"] == 1

    inflated = aggregate_paired_differences((won,), (crashed,))
    inflated_root = cast(list[dict[str, object]], inflated["per_root"])[0]
    assert inflated_root["errored"] is True
    assert inflated_root["hp_delta"] is None
    assert inflated_root["accepted_decision_delta"] is None
    assert cast(dict[str, object], inflated["hp_delta"])["count"] == 0


def test_unsorted_and_duplicate_matched_roots_are_rejected() -> None:
    _, model, vocabularies = _tiny_policy_net()
    with pytest.raises(ValueError, match="canonically ordered"):
        evaluate_matched_puct_roots(
            split_roots=(("b", b"x"), ("a", b"x")),
            evaluation_seed=0,
            model=model,
            vocabularies=vocabularies,
            transition_budget=100,
            simulation_budget=8,
            c_puct=1.5,
            beam_depth=2,
            beam_width=4,
            max_decisions=1,
            max_player_turns=100,
            deduplicate_search_states=True,
        )
    with pytest.raises(ValueError, match="duplicate matched root ID"):
        evaluate_matched_puct_roots(
            split_roots=(("a", b"x"), ("a", b"y")),
            evaluation_seed=0,
            model=model,
            vocabularies=vocabularies,
            transition_budget=100,
            simulation_budget=8,
            c_puct=1.5,
            beam_depth=2,
            beam_width=4,
            max_decisions=1,
            max_player_turns=100,
            deduplicate_search_states=True,
        )


def test_sealed_split_evaluation_fails_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    seen: list[Path] = []

    def fake_load(path: Path, *, verify_roots: bool = True) -> object:
        del verify_roots
        seen.append(path)
        raise RuntimeError("stop-after-load")

    monkeypatch.setattr(gameplay, "load_root_manifest", fake_load)
    roots = tmp_path / "root-manifest.json"
    checkpoint = tmp_path / "checkpoint.pt"
    roots.write_text("{}", encoding="utf-8")
    checkpoint.write_bytes(b"x")
    with pytest.raises(RuntimeError, match="stop-after-load"):
        evaluate_matched_puct_gameplay(roots, checkpoint, split="development")
    assert seen == [roots]
    with pytest.raises(PermissionError, match="sealed and audit splits are not available"):
        evaluate_matched_puct_gameplay(roots, checkpoint, split="sealed_test")
    assert seen == [roots]


def test_matched_roots_report_is_deterministically_serializable() -> None:
    root_id, snapshot_bytes = _fixture_snapshot()
    _, model, vocabularies = _tiny_policy_net()
    first = evaluate_matched_puct_roots(
        split_roots=((root_id, snapshot_bytes),),
        evaluation_seed=0,
        model=model,
        vocabularies=vocabularies,
        transition_budget=100,
        simulation_budget=8,
        c_puct=1.5,
        beam_depth=2,
        beam_width=4,
        max_decisions=1,
        max_player_turns=100,
        deduplicate_search_states=True,
    )
    second = evaluate_matched_puct_roots(
        split_roots=((root_id, snapshot_bytes),),
        evaluation_seed=0,
        model=model,
        vocabularies=vocabularies,
        transition_budget=100,
        simulation_budget=8,
        c_puct=1.5,
        beam_depth=2,
        beam_width=4,
        max_decisions=1,
        max_player_turns=100,
        deduplicate_search_states=True,
    )
    encoded = json.dumps(first, sort_keys=True, separators=(",", ":"), allow_nan=False)
    assert first == second
    assert json.loads(encoded) == first
    assert encoded == json.dumps(json.loads(encoded), sort_keys=True, separators=(",", ":"))
