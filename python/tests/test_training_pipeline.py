from __future__ import annotations

import hashlib
import json
from collections.abc import Sequence
from dataclasses import replace
from pathlib import Path
from typing import cast

import pytest
import torch

import sts_sim.rl.data as data_module
import sts_sim.rl.training as training_module
from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import (
    COMBAT_PROXY_V1,
    BatchedTrainingExamples,
    CombatModelConfig,
    CombatOutcome,
    FairCombatPolicyValueNet,
    PolicyValueOutput,
    RepositoryVersion,
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    TensorizedTrainingExample,
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


def test_native_episode_classifies_restored_terminal_roots_before_search_and_not_cleanup_as_win() -> (
    None
):
    state = RunEnv.combat_fixture().full_state()
    combat = cast(dict[str, object], state["combat"])
    player = cast(dict[str, object], combat["player"])
    combat["phase"] = "Lost"
    player["hp"] = 0
    state["player_hp"] = 0
    lost = RunEnv.from_state_json_for_debugging(json.dumps(state))
    payload = lost.beam_clone_episode_payload(
        depth=2, width=4, transition_budget=100, max_decisions=1, max_player_turns=1
    )
    outcome = cast(dict[str, object], payload["outcome"])
    assert outcome["status"] == "lost"
    assert outcome["accepted_decisions"] == 0
    assert outcome["terminal"] is True
    assert cast(list[object], payload["steps"]) == []
    assert lost.step(lost.decision().actions[0]).combat_outcome is None

    won_state = RunEnv.combat_fixture().full_state()
    won_combat = cast(dict[str, object], won_state["combat"])
    won_monsters = cast(list[dict[str, object]], won_combat["monsters"])
    won_combat["phase"] = "Won"
    won_monsters[0]["hp"] = 0
    won_monsters[0]["alive"] = False
    won = RunEnv.from_state_json_for_debugging(json.dumps(won_state))
    won_payload = won.beam_clone_episode_payload(
        depth=2, width=4, transition_budget=100, max_decisions=1, max_player_turns=1
    )
    assert cast(dict[str, object], won_payload["outcome"])["status"] == "won"
    assert cast(list[object], won_payload["steps"]) == []


def test_native_episode_uses_explicit_truncation_outcome() -> None:
    payload = RunEnv.combat_fixture().beam_clone_episode_payload(
        depth=2, width=4, transition_budget=100, max_decisions=1, max_player_turns=100
    )
    outcome = cast(dict[str, object], payload["outcome"])
    assert outcome["status"] == "truncated"
    assert outcome["terminal"] is False
    assert outcome["truncated"] is True
    assert outcome["accepted_decisions"] == 1
    assert outcome["player_turns"] == 2
    assert outcome["truncation_trigger"] == "accepted_decisions"
    assert len(cast(list[object], payload["steps"])) == 1


def test_native_episode_counts_conclude_as_a_forced_turn_source() -> None:
    state = RunEnv.combat_fixture().full_state()
    combat = cast(dict[str, object], state["combat"])
    player = cast(dict[str, object], combat["player"])
    player["energy"] = 3
    monsters = cast(list[dict[str, object]], combat["monsters"])
    monsters[0]["hp"] = 100
    monsters[0]["max_hp"] = 100
    monsters[0]["intent"] = "Stun"
    piles = cast(dict[str, object], combat["piles"])
    piles["hand"] = [
        {
            "id": 100,
            "content_id": 1_915_755_234_499,
            "temp_cost": None,
            "combat_only": False,
        }
    ]
    env = RunEnv.from_state_json_for_debugging(json.dumps(state))
    payload = env.beam_clone_episode_payload(
        depth=2, width=4, transition_budget=100, max_decisions=1, max_player_turns=100
    )
    step = cast(dict[str, object], cast(list[object], payload["steps"])[0])
    choices = cast(list[dict[str, object]], step["choices"])
    assert choices[cast(int, step["selected_index"])]["kind"] == "play_hand_slot"
    outcome = cast(dict[str, object], payload["outcome"])
    assert outcome["player_turns"] == 2
    assert outcome["truncation_trigger"] == "accepted_decisions"


def _fail_native_episodes(monkeypatch: pytest.MonkeyPatch, failing_root_ids: set[str]) -> None:
    native_run_env = RunEnv

    class _EpisodeEnv:
        def __init__(self, env: RunEnv, root_id: str) -> None:
            self._env = env
            self._root_id = root_id

        def __getattr__(self, name: str) -> object:
            return getattr(self._env, name)

        def beam_clone_episode_payload(
            self,
            *,
            depth: int,
            width: int,
            transition_budget: int,
            max_decisions: int,
            max_player_turns: int,
            deduplicate_search_states: bool = True,
        ) -> dict[str, object]:
            if self._root_id in failing_root_ids:
                raise RuntimeError("Burning Pact requires exactly one card")
            return self._env.beam_clone_episode_payload(
                depth=depth,
                width=width,
                transition_budget=transition_budget,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
                deduplicate_search_states=deduplicate_search_states,
            )

    class _FailingRunEnv:
        @staticmethod
        def from_snapshot(snapshot: str) -> _EpisodeEnv:
            root_id = hashlib.sha256(snapshot.encode()).hexdigest()
            return _EpisodeEnv(native_run_env.from_snapshot(snapshot), root_id)

    monkeypatch.setattr(data_module, "RunEnv", _FailingRunEnv)


def _resign_dataset_manifest(payload: dict[str, object]) -> None:
    unsigned = dict(payload)
    unsigned.pop("manifest_digest")
    payload["manifest_digest"] = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def _resign_root_manifest(payload: dict[str, object]) -> None:
    payload["cohort_digest"] = data_module._cohort_digest(
        requested_seeds=tuple(cast(list[str], payload["requested_seeds"])),
        generator_name=cast(str, payload["generator_name"]),
        generator_version=cast(str, payload["generator_version"]),
        generator_source_digest=cast(str, payload["generator_source_digest"]),
        split_salt=cast(str, payload["split_salt"]),
        ascension=cast(int, payload["ascension"]),
        max_run_steps=cast(int, payload["max_run_steps"]),
    )
    _resign_dataset_manifest(payload)


def test_root_and_beam_dataset_generation_is_byte_deterministic(tmp_path: Path) -> None:
    seeds = ["BEAMCLONE0", "BEAMCLONE12"]
    left = generate_legal_roots(tmp_path / "roots-left", seeds, max_run_steps=128)
    right = generate_legal_roots(tmp_path / "roots-right", seeds, max_run_steps=128)
    assert left.manifest_digest == right.manifest_digest
    assert (tmp_path / "roots-left/root-manifest.json").read_bytes() == (
        tmp_path / "roots-right/root-manifest.json"
    ).read_bytes()
    assert left.cohort_digest == right.cohort_digest
    assert left.requested_seeds == ("BEAMCLONE0", "BEAMCLONE12")
    assert left.split_salt == "combat-agent-phase2-v1"
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
    assert first.cohort_digest == left.cohort_digest
    assert first.teacher_search_contract_digest == second.teacher_search_contract_digest
    assert first.root_manifest_path == "provenance/root-manifest.json"
    assert (tmp_path / "data-left/provenance/root-manifest.json").read_bytes() == (
        tmp_path / "roots-left/root-manifest.json"
    ).read_bytes()
    assert (tmp_path / "data-left/train/train.jsonl").read_bytes() == (
        tmp_path / "data-right/train/train.jsonl"
    ).read_bytes()
    records = tuple(read_jsonl(tmp_path / "data-left/train/train.jsonl"))
    assert records
    assert all(record.record_version == 2 for record in records)
    assert all(sum(record.teacher_visit_counts) == 1 for record in records)
    assert all(record.teacher_visit_counts[record.chosen_action_index] == 1 for record in records)
    assert all(
        record.actions[record.chosen_action_index] == record.chosen_action for record in records
    )


def test_dataset_generation_excludes_failed_root_and_continues_with_complete_accounting(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root_manifest = generate_legal_roots(
        tmp_path / "roots", ["BEAMCLONE0", "BEAMCLONE1"], max_run_steps=128
    )
    train_roots = tuple(root for root in root_manifest.roots if root.split == "train")
    assert len(train_roots) == 2
    failed_root = train_roots[0]
    successful_root = train_roots[1]
    _fail_native_episodes(monkeypatch, {failed_root.root_id})

    manifest = generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "data",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )

    assert [root.root_id for root in manifest.roots] == [successful_root.root_id]
    assert len(manifest.exclusions) == 1
    exclusion = manifest.exclusions[0]
    assert exclusion.root_id == failed_root.root_id
    assert exclusion.reason == "native_episode_error"
    assert exclusion.detail == "Burning Pact requires exactly one card"
    records = tuple(read_jsonl(tmp_path / "data/train/train.jsonl"))
    assert records
    assert {record.root_id for record in records} == {successful_root.root_id}

    manifest_path = tmp_path / "data/dataset-manifest.json"
    original = cast(dict[str, object], json.loads(manifest_path.read_text()))
    incomplete = dict(original)
    incomplete["exclusions"] = []
    _resign_dataset_manifest(incomplete)
    manifest_path.write_text(json.dumps(incomplete, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="accounting is incomplete"):
        load_dataset_manifest(manifest_path, requested_split="train")

    overlapping = json.loads(json.dumps(original))
    cast(list[dict[str, object]], overlapping["exclusions"]).append(
        {
            "root_id": successful_root.root_id,
            "reason": "native_episode_error",
            "detail": "tampered overlap",
        }
    )
    cast(list[dict[str, object]], overlapping["exclusions"]).sort(
        key=lambda item: cast(str, item["root_id"])
    )
    _resign_dataset_manifest(overlapping)
    manifest_path.write_text(json.dumps(overlapping, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="overlaps membership and exclusion"):
        load_dataset_manifest(manifest_path, requested_split="train")


def test_dataset_generation_all_failed_roots_publishes_nothing(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root_manifest = generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    train_root = next(root for root in root_manifest.roots if root.split == "train")
    _fail_native_episodes(monkeypatch, {train_root.root_id})
    output = tmp_path / "data"

    with pytest.raises(RuntimeError, match="all 1 train roots failed.*no dataset was published"):
        generate_beam_dataset(
            tmp_path / "roots/root-manifest.json",
            output,
            split="train",
            depth=2,
            width=4,
            transition_budget=100,
            max_decisions=4,
            max_player_turns=3,
        )
    assert not output.exists() or not any(output.iterdir())


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
        repository=RepositoryVersion("a" * 40, True),
        observation_digest=fair_observation_digest(observation),
        value_target_mask=False,
    )
    builder = VocabularyBuilder()
    builder.add(observation, actions)
    vocab = builder.freeze()
    batch = collate_training_examples((SymbolicCombatDataset((legacy,), vocab)[0],))
    assert not hasattr(batch.decision, "records")
    assert batch.records == (legacy,)  # diagnostics only; never passed to the model
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

    logits = torch.tensor([[0.2, -0.1], [0.4, 0.3]], requires_grad=True)
    action_mask = torch.ones_like(logits, dtype=torch.bool)
    policy = torch.tensor([[1.0, 0.0], [0.0, 1.0]])
    entities = torch.empty(0)
    masked_values = torch.tensor([-0.9, 0.8], requires_grad=True)
    all_masked = torch.zeros(2, dtype=torch.bool)
    first_loss = policy_value_loss(
        PolicyValueOutput(logits, masked_values, entities),
        policy,
        torch.zeros(2),
        action_mask,
        all_masked,
    )
    changed_values = torch.tensor([0.1, -0.2], requires_grad=True)
    second_loss = policy_value_loss(
        PolicyValueOutput(logits, changed_values, entities),
        policy,
        torch.zeros(2),
        action_mask,
        all_masked,
    )
    assert torch.equal(first_loss, second_loss)
    first_loss.backward(retain_graph=True)
    assert masked_values.grad is not None
    assert torch.equal(masked_values.grad, torch.zeros_like(masked_values))

    mixed_values = torch.tensor([0.5, -0.8], requires_grad=True)
    mixed_loss = policy_value_loss(
        PolicyValueOutput(logits, mixed_values, entities),
        policy,
        torch.tensor([0.0, 0.0]),
        action_mask,
        torch.tensor([True, False]),
    )
    mixed_loss.backward()
    assert mixed_values.grad is not None
    assert mixed_values.grad[0] != 0
    assert mixed_values.grad[1] == 0


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
        minimum_roots=1,
        minimum_lineages=1,
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
    malformed = tmp_path / "malformed.pt"
    invalid = dict(right)
    invalid["cursor"] = (cast(int, invalid["cursor"]) + 1) % len(cast(list[int], invalid["order"]))
    torch.save(invalid, malformed)
    with pytest.raises(ValueError, match="cursor/global step"):
        train_beam_clone(tmp_path / "train/dataset-manifest.json", malformed, config, resume=True)
    with pytest.raises(ValueError, match="cursor/global step"):
        evaluate_beam_clone(
            tmp_path / "development/dataset-manifest.json", malformed, split="development"
        )

    wrong_runtime = tmp_path / "wrong-runtime.pt"
    runtime_mutation = dict(right)
    runtime_identity = json.loads(json.dumps(runtime_mutation["runtime_identity"]))
    runtime_identity["python"]["implementation"] = "different-runtime"
    runtime_mutation["runtime_identity"] = runtime_identity
    runtime_mutation["runtime_identity_digest"] = hashlib.sha256(
        json.dumps(runtime_identity, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    torch.save(runtime_mutation, wrong_runtime)
    with pytest.raises(ValueError, match="runtime"):
        train_beam_clone(
            tmp_path / "train/dataset-manifest.json", wrong_runtime, config, resume=True
        )
    with pytest.raises(ValueError, match="runtime identity"):
        evaluate_beam_clone(
            tmp_path / "development/dataset-manifest.json", wrong_runtime, split="development"
        )

    first = evaluate_beam_clone(
        tmp_path / "development/dataset-manifest.json", resumed, split="development"
    )
    second = evaluate_beam_clone(
        tmp_path / "development/dataset-manifest.json", resumed, split="development"
    )
    assert first == second
    assert first["report_version"] == 3
    exact_numerator = cast(int, first["exact_numerator"])
    exact_denominator = cast(int, first["exact_denominator"])
    truncated_numerator = cast(int, first["truncated_numerator"])
    truncated_denominator = cast(int, first["truncated_denominator"])
    nontruncated_numerator = cast(int, first["nontruncated_numerator"])
    nontruncated_denominator = cast(int, first["nontruncated_denominator"])
    assert exact_denominator == len(cast(list[object], first["per_record"]))
    assert first["accuracy"] == exact_numerator / exact_denominator
    assert truncated_numerator + nontruncated_numerator == exact_numerator
    assert truncated_denominator + nontruncated_denominator == exact_denominator
    assert first["errors"] == 0
    assert first["records"] == exact_denominator
    assert first["correct"] == exact_numerator
    rows = [cast(dict[str, object], row) for row in cast(list[object], first["per_record"])]
    truncated_root_ids = {cast(str, row["root_id"]) for row in rows if row["truncated"] is True}
    assert first["truncated_root_count"] == len(truncated_root_ids)
    assert first["value_mae_rows"] == sum(
        1 for row in rows if row["value_target_mask"] is True and "error" not in row
    )
    assert all("error" not in row for row in rows)
    for row in cast(list[object], first["per_record"]):
        payload = cast(dict[str, object], row)
        assert {
            "record_id",
            "root_id",
            "status",
            "truncated",
            "value_target_mask",
            "target_value",
            "selected_action_index",
            "teacher_action_index",
            "correct",
            "predicted_value",
        } <= set(payload)
    for key in (
        "checkpoint_file_digest",
        "checkpoint_model_state_digest",
        "checkpoint_config_digest",
        "source_digest",
        "runtime_identity_digest",
        "vocabulary_fingerprint",
        "encoder_contract_digest",
        "reward_config_digest",
        "checkpoint_training_root_manifest_digest",
        "checkpoint_training_cohort_digest",
        "teacher_search_contract_digest",
        "root_manifest_digest",
        "cohort_digest",
        "dataset_manifest_digest",
        "dataset_shard_digest",
    ):
        assert len(cast(str, first[key])) == 64
    wrong_cohort = tmp_path / "wrong-cohort.pt"
    cohort_mutation = dict(right)
    cohort_mutation["cohort_digest"] = "a" * 64
    torch.save(cohort_mutation, wrong_cohort)
    with pytest.raises(ValueError, match="cohort_digest"):
        train_beam_clone(
            tmp_path / "train/dataset-manifest.json", wrong_cohort, config, resume=True
        )
    wrong_teacher = tmp_path / "wrong-teacher.pt"
    teacher_mutation = dict(right)
    teacher_mutation["teacher_search_contract_digest"] = "b" * 64
    torch.save(teacher_mutation, wrong_teacher)
    with pytest.raises(ValueError, match="teacher_search_contract_digest"):
        train_beam_clone(
            tmp_path / "train/dataset-manifest.json", wrong_teacher, config, resume=True
        )


def test_sealed_roots_are_withheld_by_default_and_audited_materialization_is_explicit(
    tmp_path: Path,
) -> None:
    ordinary = generate_legal_roots(tmp_path / "ordinary-roots", ["BEAMCLONE17"], max_run_steps=128)
    assert ordinary.audited_splits_materialized is False
    assert ordinary.roots == ()
    assert list((tmp_path / "ordinary-roots").rglob("*.json")) == [
        tmp_path / "ordinary-roots/root-manifest.json"
    ]

    audited = generate_legal_roots(
        tmp_path / "audited-roots",
        ["BEAMCLONE17"],
        max_run_steps=128,
        materialize_audited_splits=True,
    )
    assert audited.audited_splits_materialized is True
    assert ordinary.cohort_digest == audited.cohort_digest
    assert ordinary.manifest_digest != audited.manifest_digest
    assert ordinary.requested_seeds == audited.requested_seeds == ("BEAMCLONE17",)
    assert {root.split for root in audited.roots} == {"sealed_test"}
    assert all(root.relative_path.startswith("sealed_test/roots/") for root in audited.roots)
    generate_beam_dataset(
        tmp_path / "audited-roots/root-manifest.json",
        tmp_path / "sealed",
        split="sealed_test",
        allow_audited_split=True,
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    with pytest.raises(PermissionError, match="explicit access"):
        load_root_manifest(tmp_path / "audited-roots/root-manifest.json")
    with pytest.raises(PermissionError, match="explicit audited access"):
        load_dataset_manifest(
            tmp_path / "sealed/dataset-manifest.json", requested_split="sealed_test"
        )


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


def test_explicit_audited_evaluation_requires_matching_cohort_and_teacher_search(
    tmp_path: Path,
) -> None:
    seeds = ["BEAMCLONE17", "BEAMCLONE0"]
    ordinary = generate_legal_roots(tmp_path / "train-roots", seeds, max_run_steps=128)
    reversed_order = generate_legal_roots(
        tmp_path / "train-roots-reversed", list(reversed(seeds)), max_run_steps=128
    )
    assert ordinary.requested_seeds == ("BEAMCLONE0", "BEAMCLONE17")
    assert ordinary.cohort_digest == reversed_order.cohort_digest
    train = generate_beam_dataset(
        tmp_path / "train-roots/root-manifest.json",
        tmp_path / "train",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    audited = generate_legal_roots(
        tmp_path / "audited-roots",
        seeds,
        max_run_steps=128,
        materialize_audited_splits=True,
    )
    sealed = generate_beam_dataset(
        tmp_path / "audited-roots/root-manifest.json",
        tmp_path / "sealed",
        split="sealed_test",
        allow_audited_split=True,
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    assert (
        ordinary.cohort_digest
        == audited.cohort_digest
        == train.cohort_digest
        == sealed.cohort_digest
    )
    assert train.root_manifest_digest != sealed.root_manifest_digest
    assert train.teacher_search_contract_digest == sealed.teacher_search_contract_digest
    checkpoint = tmp_path / "checkpoint.pt"
    train_beam_clone(tmp_path / "train/dataset-manifest.json", checkpoint, _smoke_training_config())
    with pytest.raises(PermissionError, match="explicit audited access"):
        evaluate_beam_clone(
            tmp_path / "sealed/dataset-manifest.json", checkpoint, split="sealed_test"
        )
    report = evaluate_beam_clone(
        tmp_path / "sealed/dataset-manifest.json",
        checkpoint,
        split="sealed_test",
        allow_audited_split=True,
    )
    assert report["report_version"] == 3
    assert all(
        "error" not in cast(dict[str, object], row)
        for row in cast(list[object], report["per_record"])
    )
    assert report["checkpoint_training_root_manifest_digest"] == train.root_manifest_digest
    assert report["root_manifest_digest"] == sealed.root_manifest_digest
    assert (
        report["checkpoint_training_cohort_digest"]
        == report["cohort_digest"]
        == train.cohort_digest
    )
    assert report["teacher_search_contract_digest"] == train.teacher_search_contract_digest

    generate_legal_roots(
        tmp_path / "disjoint-roots",
        ["BEAMCLONE4"],
        max_run_steps=128,
        materialize_audited_splits=True,
    )
    disjoint = generate_beam_dataset(
        tmp_path / "disjoint-roots/root-manifest.json",
        tmp_path / "disjoint",
        split="sealed_test",
        allow_audited_split=True,
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    assert disjoint.cohort_digest != train.cohort_digest
    with pytest.raises(ValueError, match="disjoint cohort"):
        evaluate_beam_clone(
            tmp_path / "disjoint/dataset-manifest.json",
            checkpoint,
            split="sealed_test",
            allow_audited_split=True,
        )

    mismatched = generate_beam_dataset(
        tmp_path / "audited-roots/root-manifest.json",
        tmp_path / "sealed-mismatch",
        split="sealed_test",
        allow_audited_split=True,
        depth=3,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    assert mismatched.cohort_digest == train.cohort_digest
    assert mismatched.teacher_search_contract_digest != train.teacher_search_contract_digest
    with pytest.raises(ValueError, match="teacher/search contract"):
        evaluate_beam_clone(
            tmp_path / "sealed-mismatch/dataset-manifest.json",
            checkpoint,
            split="sealed_test",
            allow_audited_split=True,
        )


def test_generation_refuses_nonempty_output_directories(tmp_path: Path) -> None:
    root_output = tmp_path / "roots"
    generate_legal_roots(root_output, ["BEAMCLONE0"], max_run_steps=128)
    stale_root = root_output / "sealed_test/roots/stale.json"
    stale_root.parent.mkdir(parents=True)
    stale_root.write_text("stale")
    with pytest.raises(ValueError, match="output directory must be empty"):
        generate_legal_roots(root_output, ["BEAMCLONE0"], max_run_steps=128)
    assert stale_root.read_text() == "stale"

    dataset_output = tmp_path / "data"
    generate_beam_dataset(
        root_output / "root-manifest.json",
        dataset_output,
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )
    stale_shard = dataset_output / "stale.jsonl"
    stale_shard.write_text("stale")
    with pytest.raises(ValueError, match="output directory must be empty"):
        generate_beam_dataset(root_output / "root-manifest.json", dataset_output, split="train")
    assert stale_shard.read_text() == "stale"


def test_dataset_loader_resolves_named_root_manifest_membership(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "data",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )
    manifest_path = tmp_path / "data/dataset-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["roots"][0]["root_id"] = "f" * 64
    unsigned = dict(manifest)
    unsigned.pop("manifest_digest")
    manifest["manifest_digest"] = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    manifest_path.write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="named root manifest"):
        load_dataset_manifest(manifest_path, requested_split="train")


def test_manifest_formats_have_no_compatibility_shim(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    root_path = tmp_path / "roots/root-manifest.json"
    root_payload = json.loads(root_path.read_text())
    root_payload["manifest_version"] = 3
    _resign_dataset_manifest(root_payload)
    root_path.write_text(json.dumps(root_payload, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="unsupported or malformed"):
        load_root_manifest(root_path)

    generate_legal_roots(tmp_path / "fresh-roots", ["BEAMCLONE0"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "fresh-roots/root-manifest.json",
        tmp_path / "data",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )
    dataset_path = tmp_path / "data/dataset-manifest.json"
    dataset_payload = json.loads(dataset_path.read_text())
    dataset_payload["manifest_version"] = 4
    _resign_dataset_manifest(dataset_payload)
    dataset_path.write_text(json.dumps(dataset_payload, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="unsupported or malformed"):
        load_dataset_manifest(dataset_path, requested_split="train")


def test_root_manifest_rejects_unused_claimed_seed_and_decoupled_generator_source(
    tmp_path: Path,
) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    root_path = tmp_path / "roots/root-manifest.json"
    original = json.loads(root_path.read_text())

    unused = dict(original)
    unused["requested_seeds"] = sorted([*cast(list[str], unused["requested_seeds"]), "UNUSEDSEED"])
    _resign_root_manifest(unused)
    root_path.write_text(json.dumps(unused, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="requested seed accounting is incomplete"):
        load_root_manifest(root_path)

    decoupled = dict(original)
    decoupled["generator_source_digest"] = "e" * 64
    _resign_root_manifest(decoupled)
    root_path.write_text(json.dumps(decoupled, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="generator source digest does not match repository"):
        load_root_manifest(root_path)


def test_injected_evaluation_error_is_denominator_only(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0", "BEAMCLONE12"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "train",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "development",
        split="development",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    checkpoint = tmp_path / "checkpoint.pt"
    train_beam_clone(tmp_path / "train/dataset-manifest.json", checkpoint, _smoke_training_config())
    records = tuple(read_jsonl(tmp_path / "development/development/development.jsonl"))
    failed = records[0]
    original_collate = training_module.collate_training_examples
    calls = {"count": 0}

    def failing_collate(
        items: Sequence[TensorizedTrainingExample],
    ) -> BatchedTrainingExamples:
        if calls["count"] == 0:
            calls["count"] += 1
            raise RuntimeError("injected evaluation failure")
        calls["count"] += 1
        return original_collate(items)

    monkeypatch.setattr(training_module, "collate_training_examples", failing_collate)
    report = evaluate_beam_clone(
        tmp_path / "development/dataset-manifest.json", checkpoint, split="development"
    )
    rows = [cast(dict[str, object], row) for row in cast(list[object], report["per_record"])]
    error_rows = [row for row in rows if "error" in row]
    success_rows = [row for row in rows if "error" not in row]
    assert report["errors"] == 1
    assert len(error_rows) == 1
    assert error_rows[0]["record_id"] == failed.record_id
    assert error_rows[0]["error"] == "injected evaluation failure"
    assert error_rows[0]["teacher_action_index"] == failed.chosen_action_index
    assert error_rows[0]["truncated"] is failed.outcome.truncated
    assert error_rows[0]["value_target_mask"] is failed.value_target_mask
    assert "correct" not in error_rows[0]
    assert "predicted_value" not in error_rows[0]
    assert "selected_action_index" not in error_rows[0]
    assert report["records"] == len(records) == report["exact_denominator"]
    assert (
        report["correct"]
        == report["exact_numerator"]
        == sum(1 for row in success_rows if row["correct"] is True)
    )
    truncated_rows = [row for row in rows if row["truncated"] is True]
    nontruncated_rows = [row for row in rows if row["truncated"] is False]
    assert report["truncated_denominator"] == len(truncated_rows)
    assert report["nontruncated_denominator"] == len(nontruncated_rows)
    if failed.outcome.truncated:
        assert cast(int, report["truncated_denominator"]) >= 1
        assert report["truncated_numerator"] == sum(
            1 for row in success_rows if row["truncated"] is True and row["correct"] is True
        )
    else:
        assert cast(int, report["nontruncated_denominator"]) >= 1
        assert report["nontruncated_numerator"] == sum(
            1 for row in success_rows if row["truncated"] is False and row["correct"] is True
        )
    assert report["value_mae_rows"] == sum(
        1 for row in success_rows if row["value_target_mask"] is True
    )
    if failed.value_target_mask:
        assert (
            report["value_mae_rows"]
            == sum(1 for row in rows if row["value_target_mask"] is True) - 1
        )


def test_dataset_loader_rejects_cohort_and_teacher_search_digest_mismatch(
    tmp_path: Path,
) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "data",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )
    manifest_path = tmp_path / "data/dataset-manifest.json"
    original = json.loads(manifest_path.read_text())
    cohort = dict(original)
    cohort["cohort_digest"] = "c" * 64
    _resign_dataset_manifest(cohort)
    manifest_path.write_text(json.dumps(cohort, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="cohort digest"):
        load_dataset_manifest(manifest_path, requested_split="train")

    teacher = dict(original)
    teacher["teacher_search_contract_digest"] = "d" * 64
    _resign_dataset_manifest(teacher)
    manifest_path.write_text(json.dumps(teacher, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="teacher/search contract digest"):
        load_dataset_manifest(manifest_path, requested_split="train")


def test_dataset_loader_rejects_policy_chosen_action_contradiction(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "data",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )
    shard = tmp_path / "data/train/train.jsonl"
    payloads = [json.loads(line) for line in shard.read_text().splitlines()]
    first = payloads[0]
    chosen = cast(int, first["chosen_action_index"])
    replacement = next(index for index in range(len(first["actions"])) if index != chosen)
    first["teacher_visit_counts"] = [0] * len(first["actions"])
    first["teacher_visit_counts"][replacement] = 1
    first["record_id"] = None
    changed = SymbolicTrainingRecord.from_dict(first)
    payloads[0] = changed.to_dict()
    content = b"".join(
        json.dumps(item, sort_keys=True, separators=(",", ":")).encode() + b"\n"
        for item in payloads
    )
    shard.write_bytes(content)
    manifest_path = tmp_path / "data/dataset-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["shard_digest"] = hashlib.sha256(content).hexdigest()
    manifest["record_ids"][0] = changed.record_id
    unsigned = dict(manifest)
    unsigned.pop("manifest_digest")
    manifest["manifest_digest"] = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    manifest_path.write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="one-hot"):
        load_dataset_manifest(manifest_path, requested_split="train")


def test_training_refuses_corpus_below_default_floor(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "data",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )
    with pytest.raises(ValueError, match="below configured minimums"):
        train_beam_clone(
            tmp_path / "data/dataset-manifest.json",
            tmp_path / "checkpoint.pt",
            TrainingConfig(total_steps=1),
        )


def test_dataset_loader_recomputes_value_targets_and_record_identity_is_substantive(
    tmp_path: Path,
) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "data",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=32,
        max_player_turns=10,
    )
    shard = tmp_path / "data/train/train.jsonl"
    payloads = [json.loads(line) for line in shard.read_text().splitlines()]
    original_id = payloads[0]["record_id"]
    assert payloads[0]["value_target_mask"] is True
    payloads[0]["target_value"] = 0.0
    payloads[0]["record_id"] = None
    changed = SymbolicTrainingRecord.from_dict(payloads[0])
    assert changed.record_id != original_id
    payloads[0] = changed.to_dict()
    content = b"".join(
        json.dumps(item, sort_keys=True, separators=(",", ":")).encode() + b"\n"
        for item in payloads
    )
    shard.write_bytes(content)
    manifest_path = tmp_path / "data/dataset-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["shard_digest"] = hashlib.sha256(content).hexdigest()
    manifest["record_ids"][0] = changed.record_id
    unsigned = dict(manifest)
    unsigned.pop("manifest_digest")
    manifest["manifest_digest"] = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    manifest_path.write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError, match="value target"):
        load_dataset_manifest(manifest_path, requested_split="train")


def test_v2_record_rejects_forbidden_search_state_keys(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "data",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )
    shard = tmp_path / "data/train/train.jsonl"
    payload = json.loads(shard.read_text().splitlines()[0])
    payload["search_config"]["snapshot"] = {"rng_state": "private"}
    with pytest.raises(ValueError, match="search config"):
        SymbolicTrainingRecord.from_dict(payload)
