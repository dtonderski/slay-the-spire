from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from typing import cast

import pytest
import torch

from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import (
    COMBAT_PROXY_V1,
    CombatModelConfig,
    CombatOutcome,
    FairCombatPolicyValueNet,
    RepositoryVersion,
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    TrainingConfig,
    VocabularyBuilder,
    collate_training_examples,
    evaluate_beam_clone,
    fair_observation_digest,
    generate_beam_dataset,
    generate_legal_roots,
    load_dataset_manifest,
    load_root_manifest,
    policy_value_loss,
    read_jsonl,
    train_beam_clone,
)


def test_reward_is_bounded_survival_dominant_and_masks_truncation() -> None:
    won = CombatOutcome("won", 1, 80, -79, 0, 0, (), (), True, False, 4, 1, None)
    escaped = replace(won, status="escaped", terminal_hp=80, hp_change=0)
    lost = replace(won, status="lost", terminal_hp=0, hp_change=-80)
    truncated = replace(
        won,
        status="truncated",
        terminal=False,
        truncated=True,
        truncation_trigger="accepted_decisions",
    )
    values = [COMBAT_PROXY_V1.value(outcome) for outcome in (won, escaped, lost)]
    assert all(value is not None and -1.0 <= value <= 1.0 for value in values)
    assert cast(float, values[0]) > cast(float, values[1]) > cast(float, values[2])
    assert COMBAT_PROXY_V1.value(truncated) is None
    assert len(COMBAT_PROXY_V1.digest) == 64


def test_native_episode_classifies_terminal_before_next_model_decision() -> None:
    env = RunEnv.combat_fixture()
    state = env.full_state()
    combat = cast(dict[str, object], state["combat"])
    monsters = cast(list[dict[str, object]], combat["monsters"])
    monsters[0]["hp"] = 1
    terminal = RunEnv.from_state_json_for_debugging(__import__("json").dumps(state))
    payload = terminal.beam_clone_episode_payload(
        depth=2, width=4, transition_budget=100, max_decisions=1, max_player_turns=1
    )
    outcome = cast(dict[str, object], payload["outcome"])
    assert outcome["status"] == "won"
    assert outcome["terminal"] is True
    assert outcome["truncated"] is False
    assert outcome["accepted_decisions"] == 1
    assert outcome["truncation_trigger"] is None
    assert len(cast(list[object], payload["steps"])) == 1


def test_native_episode_uses_explicit_truncation_outcome() -> None:
    payload = RunEnv.combat_fixture().beam_clone_episode_payload(
        depth=2, width=4, transition_budget=100, max_decisions=1, max_player_turns=100
    )
    outcome = cast(dict[str, object], payload["outcome"])
    assert outcome["status"] == "truncated"
    assert outcome["terminal"] is False
    assert outcome["truncated"] is True
    assert outcome["accepted_decisions"] == 1
    assert outcome["truncation_trigger"] == "accepted_decisions"
    assert len(cast(list[object], payload["steps"])) == 1


def test_root_and_beam_dataset_generation_is_byte_deterministic(tmp_path: Path) -> None:
    seeds = ["BEAMCLONE0", "BEAMCLONE12"]
    left = generate_legal_roots(tmp_path / "roots-left", seeds, max_run_steps=128)
    right = generate_legal_roots(tmp_path / "roots-right", seeds, max_run_steps=128)
    assert left.manifest_digest == right.manifest_digest
    assert (tmp_path / "roots-left/root-manifest.json").read_bytes() == (
        tmp_path / "roots-right/root-manifest.json"
    ).read_bytes()
    assert {root.split for root in left.roots} == {"train", "development"}
    for root in left.roots:
        assert (tmp_path / "roots-left" / root.relative_path).read_bytes() == (
            tmp_path / "roots-right" / root.relative_path
        ).read_bytes()
    load_root_manifest(tmp_path / "roots-left/root-manifest.json")

    first = generate_beam_dataset(
        tmp_path / "roots-left/root-manifest.json",
        tmp_path / "data-left",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=32,
        max_player_turns=10,
    )
    second = generate_beam_dataset(
        tmp_path / "roots-left/root-manifest.json",
        tmp_path / "data-right",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=32,
        max_player_turns=10,
    )
    assert first.manifest_digest == second.manifest_digest
    assert (tmp_path / "data-left/train.jsonl").read_bytes() == (
        tmp_path / "data-right/train.jsonl"
    ).read_bytes()
    records = tuple(read_jsonl(tmp_path / "data-left/train.jsonl"))
    assert records
    assert all(record.record_version == 2 for record in records)
    assert all(sum(record.teacher_visit_counts) == 1 for record in records)
    assert all(
        record.teacher_visit_counts[record.chosen_action_index] == 1 for record in records
    )
    assert all(
        record.actions[record.chosen_action_index] == record.chosen_action for record in records
    )


def test_truncated_value_rows_are_masked_from_loss() -> None:
    decision = RunEnv.combat_fixture().decision()
    observation = cast(FairCombatObservation, decision.observation)
    actions = tuple(action.descriptor() for action in decision.actions)
    outcome = CombatOutcome(
        "truncated",
        observation.player.hp,
        observation.player.max_hp,
        0,
        0,
        0,
        tuple(slot.content_key for slot in observation.potion_slots),
        (),
        False,
        True,
        512,
        2,
        "accepted_decisions",
    )
    legacy = SymbolicTrainingRecord(
        observation=observation,
        actions=actions,
        chosen_action_index=0,
        chosen_action=actions[0],
        teacher_visit_counts=(1, *([0] * (len(actions) - 1))),
        target_value=None,
        value_target_name="combat_proxy_v1",
        outcome=outcome,
        planner_name="test",
        planner_version="1",
        search_config={},
        root_id="legacy",
        split_group_id="legacy",
        teacher_pair_id=None,
        repository=RepositoryVersion("abc", True),
        observation_digest=fair_observation_digest(observation),
        value_target_mask=False,
    )
    builder = VocabularyBuilder()
    builder.add(observation, actions)
    vocab = builder.freeze()
    batch = collate_training_examples((SymbolicCombatDataset((legacy,), vocab)[0],))
    model = FairCombatPolicyValueNet(
        vocab, CombatModelConfig(width=16, heads=4, layers=1, feedforward_width=32)
    )
    output = model(batch.decision)
    loss = policy_value_loss(
        output,
        batch.policy_target,
        batch.value_target,
        batch.decision.action_mask,
        batch.value_target_mask,
    )
    assert torch.isfinite(loss)
    loss.backward()


def _assert_nested_equal(left: object, right: object) -> None:
    if isinstance(left, torch.Tensor):
        assert isinstance(right, torch.Tensor) and torch.equal(left, right)
    elif isinstance(left, dict):
        assert isinstance(right, dict)
        left_map = cast(dict[object, object], left)
        right_map = cast(dict[object, object], right)
        assert left_map.keys() == right_map.keys()
        for key in left_map:
            _assert_nested_equal(left_map[key], right_map[key])
    elif isinstance(left, (list, tuple)):
        assert isinstance(right, type(left))
        left_items = cast(list[object] | tuple[object, ...], left)
        right_items = cast(list[object] | tuple[object, ...], right)
        assert len(left_items) == len(right_items)
        for a, b in zip(left_items, right_items):
            _assert_nested_equal(a, b)
    else:
        assert left == right


def test_training_resume_and_development_report_are_deterministic(tmp_path: Path) -> None:
    roots = generate_legal_roots(
        tmp_path / "roots", ["BEAMCLONE0", "BEAMCLONE12"], max_run_steps=128
    )
    train = generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "train",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=32,
        max_player_turns=10,
    )
    development = generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "development",
        split="development",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=32,
        max_player_turns=10,
    )
    assert train.root_manifest_digest == development.root_manifest_digest == roots.manifest_digest
    config = TrainingConfig(
        seed=11,
        batch_size=2,
        total_steps=4,
        model_width=16,
        model_heads=4,
        model_layers=1,
        feedforward_width=32,
    )
    uninterrupted = tmp_path / "uninterrupted.pt"
    resumed = tmp_path / "resumed.pt"
    train_beam_clone(tmp_path / "train/dataset-manifest.json", uninterrupted, config)
    train_beam_clone(
        tmp_path / "train/dataset-manifest.json",
        resumed,
        config,
        stop_after_steps=2,
    )
    train_beam_clone(tmp_path / "train/dataset-manifest.json", resumed, config, resume=True)
    left = torch.load(uninterrupted, map_location="cpu", weights_only=False)
    right = torch.load(resumed, map_location="cpu", weights_only=False)
    for key in (
        "model_state",
        "optimizer_state",
        "scheduler_state",
        "global_step",
        "cursor",
        "order",
        "torch_rng_state",
    ):
        _assert_nested_equal(left[key], right[key])
    first = evaluate_beam_clone(
        tmp_path / "development/dataset-manifest.json", resumed, split="development"
    )
    second = evaluate_beam_clone(
        tmp_path / "development/dataset-manifest.json", resumed, split="development"
    )
    assert first == second
    assert first["errors"] == 0


def test_sealed_dataset_access_fails_closed(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE17"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "sealed",
        split="sealed_test",
        allow_audited_split=True,
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    with pytest.raises(PermissionError, match="explicit audited access"):
        load_dataset_manifest(
            tmp_path / "sealed/dataset-manifest.json", requested_split="sealed_test"
        )
