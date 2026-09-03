from __future__ import annotations

import ast
import hashlib
import json
import subprocess
from dataclasses import replace
from pathlib import Path
from typing import Literal, cast

import pytest
import torch

from sts_sim import ActionDescriptor, FairCombatObservation, Potion, RunEnv
from sts_sim.rl import (
    BatchedCombatDecision,
    CombatModelConfig,
    CombatOutcome,
    CounterChange,
    FairCombatPolicyValueNet,
    PolicyValueOutput,
    RepositoryVersion,
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    Vocabularies,
    VocabularyBuilder,
    capture_repository_version,
    collate_combat_tensors,
    collate_training_examples,
    fair_observation_digest,
    policy_value_loss,
    read_jsonl,
    rollout_model_combat,
    summarize_rollouts,
    teacher_conflict_report,
    tensorize_combat,
    write_jsonl,
)
from sts_sim.rl.records import BEAM_TEACHER_NAME, RECORD_VERSION, JsonValue, canonical_episode_id
from sts_sim.rl.rewards import COMBAT_PROXY_V1

_BEAM_SEARCH_CONFIG: dict[str, JsonValue] = {
    "depth": 8,
    "width": 24,
    "transition_budget": 32,
    "max_decisions": 512,
    "max_player_turns": 100,
    "deadline": None,
    "replan": "every_public_decision",
    "deduplicate_search_states": True,
}


def _hex(label: str) -> str:
    return hashlib.sha256(label.encode()).hexdigest()


def _rebind_root(
    record: SymbolicTrainingRecord,
    root_id: str,
    **changes: object,
) -> SymbolicTrainingRecord:
    digest_root = root_id if len(root_id) == 64 else _hex(root_id)
    return replace(
        record,
        root_id=digest_root,
        episode_id=canonical_episode_id(
            digest_root, record.search_config, record.reward_config_digest
        ),
        record_id=None,
        **changes,
    )


def _decision() -> tuple[FairCombatObservation, tuple[ActionDescriptor, ...]]:
    decision = RunEnv.combat_fixture().decision()
    assert isinstance(decision.observation, FairCombatObservation)
    return decision.observation, tuple(action.descriptor() for action in decision.actions)


def _vocab(
    observation: FairCombatObservation,
    actions: tuple[ActionDescriptor, ...],
) -> Vocabularies:
    builder = VocabularyBuilder()
    builder.add(observation, actions)
    return builder.freeze()


def _model(vocab: Vocabularies) -> FairCombatPolicyValueNet:
    torch.manual_seed(7)
    return FairCombatPolicyValueNet(
        vocab,
        CombatModelConfig(width=32, heads=4, layers=1, feedforward_width=64),
    ).eval()


class _FixedIndexModel(FairCombatPolicyValueNet):
    def __init__(self, vocab: Vocabularies, index: int) -> None:
        super().__init__(
            vocab,
            CombatModelConfig(width=32, heads=4, layers=1, feedforward_width=64),
        )
        self.index = index

    def forward(self, batch: BatchedCombatDecision) -> PolicyValueOutput:
        base = super().forward(batch)
        logits = torch.full_like(base.logits, -100.0)
        logits[:, self.index] = 100.0
        logits = logits.masked_fill(~batch.action_mask, float("-inf"))
        return PolicyValueOutput(logits, base.value, base.entity_states)


def _outcome() -> CombatOutcome:
    return CombatOutcome(
        status="won",
        terminal_hp=61,
        terminal_max_hp=80,
        hp_change=-19,
        max_hp_change=0,
        gold_change=0,
        potion_slots=(None, None, None),
        counter_changes=(CounterChange("relic", "Ink Bottle", "cards", 0, 1),),
        terminal=True,
        truncated=False,
    )


def _record(
    observation: FairCombatObservation,
    actions: tuple[ActionDescriptor, ...],
    *,
    chosen: int = 0,
    visits: tuple[int, ...] | None = None,
    split_group: str = "split-a",
    root_id: str = "fixture-root",
    teacher_pair_id: str | None = None,
    target_value: float | None = 0.75,
    value_target_mask: bool | None = None,
    search_root_mean_value: float | None = None,
) -> SymbolicTrainingRecord:
    counts = visits or tuple(1 if index == chosen else 0 for index in range(len(actions)))
    digest_root = root_id if len(root_id) == 64 else _hex(root_id)
    reward_digest = COMBAT_PROXY_V1.digest
    return SymbolicTrainingRecord(
        observation=observation,
        actions=actions,
        chosen_action_index=chosen,
        chosen_action=actions[chosen],
        teacher_visit_counts=counts,
        target_value=target_value,
        value_target_name=COMBAT_PROXY_V1.name,
        outcome=_outcome(),
        planner_name=BEAM_TEACHER_NAME,
        planner_version="replan_each_public_decision_v2",
        search_config=_BEAM_SEARCH_CONFIG,
        root_id=digest_root,
        split_group_id=split_group,
        teacher_pair_id=teacher_pair_id,
        repository=RepositoryVersion("a" * 40, True),
        observation_digest=fair_observation_digest(observation),
        record_version=RECORD_VERSION,
        root_manifest_digest=_hex("root-manifest"),
        reward_config_digest=reward_digest,
        source_kind="simulator_legal_v1",
        episode_id=canonical_episode_id(digest_root, _BEAM_SEARCH_CONFIG, reward_digest),
        decision_index=0,
        value_target_mask=target_value is not None if value_target_mask is None else value_target_mask,
        record_id=None,
        search_root_mean_value=search_root_mean_value,
    )


def test_model_and_record_modules_have_no_privileged_state_accessor() -> None:
    package = Path(__file__).parents[1] / "sts_sim" / "rl"
    forbidden = {"_native", "full_state", "snapshot", "_handle"}
    for filename in ("model.py", "records.py", "tensor.py"):
        tree = ast.parse((package / filename).read_text())
        referenced = {node.id for node in ast.walk(tree) if isinstance(node, ast.Name)} | {
            node.attr for node in ast.walk(tree) if isinstance(node, ast.Attribute)
        }
        assert forbidden.isdisjoint(referenced), filename


def test_content_embeddings_are_namespace_specific_despite_local_id_collisions() -> None:
    observation, actions = _decision()
    vocab = _vocab(observation, actions)
    model = _model(vocab)
    phase = cast(torch.nn.Embedding, model.content_embeddings["phase"]).weight
    card = cast(torch.nn.Embedding, model.content_embeddings["card"]).weight
    assert phase.data_ptr() != card.data_ptr()
    local_id = 3
    before = card[local_id].detach().clone()
    with torch.no_grad():
        phase[local_id].add_(10.0)
    assert torch.equal(card[local_id], before)


def test_model_scores_only_dynamic_legal_rows_and_masks_padding() -> None:
    observation, actions = _decision()
    vocab = _vocab(observation, actions)
    full = tensorize_combat(observation, actions, vocab)
    short = tensorize_combat(observation, actions[:1], vocab)
    batch = collate_combat_tensors((full, short))
    output = _model(vocab)(batch)

    assert output.logits.shape == batch.action_mask.shape == (2, len(actions))
    assert output.value.shape == (2,)
    assert output.entity_states.shape[:2] == batch.entity_mask.shape
    assert torch.isneginf(output.logits[1, 1:]).all()
    probabilities = torch.softmax(output.logits, dim=-1)
    assert torch.equal(probabilities[1, 1:], torch.zeros_like(probabilities[1, 1:]))
    assert torch.allclose(probabilities.sum(dim=-1), torch.ones(2))


@pytest.mark.parametrize(
    "malformation",
    ("missing", "displaced", "duplicated", "padded", "wrong_parent"),
)
def test_model_rejects_malformed_global_entity_contract(malformation: str) -> None:
    observation, actions = _decision()
    vocab = _vocab(observation, actions)
    batch = collate_combat_tensors((tensorize_combat(observation, actions, vocab),))
    global_kind = vocab.encode("entity_kind", "global")[0]
    entity_kind = batch.entity_kind.clone()
    entity_mask = batch.entity_mask.clone()
    entity_parent = batch.entity_parent.clone()
    if malformation == "missing":
        entity_kind[0, 0] = 0
    elif malformation == "displaced":
        entity_kind[0, 0], entity_kind[0, 1] = (
            entity_kind[0, 1].clone(),
            entity_kind[0, 0].clone(),
        )
    elif malformation == "duplicated":
        entity_kind[0, 1] = global_kind
    elif malformation == "padded":
        entity_mask[0, 0] = False
    elif malformation == "wrong_parent":
        entity_parent[0, 0] = 1
    else:  # pragma: no cover - parametrization is closed above
        raise AssertionError(malformation)
    malformed = replace(
        batch,
        entity_kind=entity_kind,
        entity_mask=entity_mask,
        entity_parent=entity_parent,
    )
    with pytest.raises(ValueError, match="exactly one unpadded global entity"):
        _model(vocab)(malformed)


def test_hand_and_action_permutation_is_policy_equivariant_and_value_invariant() -> None:
    observation, actions = _decision()
    count = len(observation.hand)
    slot_map = {old: count - 1 - old for old in range(count)}
    hand = tuple(replace(item, slot=slot_map[item.slot]) for item in reversed(observation.hand))
    remapped = tuple(
        replace(action, hand_slot=None if action.hand_slot is None else slot_map[action.hand_slot])
        for action in actions
    )
    variant = replace(observation, hand=hand)
    vocab = _vocab(observation, actions)
    model = _model(vocab)
    left = model(collate_combat_tensors((tensorize_combat(observation, actions, vocab),)))
    right = model(collate_combat_tensors((tensorize_combat(variant, remapped, vocab),)))
    assert torch.allclose(left.logits, right.logits, atol=1e-6)
    assert torch.allclose(left.value, right.value, atol=1e-6)

    reversed_output = model(
        collate_combat_tensors((tensorize_combat(observation, actions[::-1], vocab),))
    )
    assert torch.allclose(left.logits.flip(1), reversed_output.logits, atol=1e-6)
    assert torch.allclose(left.value, reversed_output.value, atol=1e-6)


def _init_git_fixture(repo: Path) -> None:
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    subprocess.run(
        ["git", "-C", str(repo), "config", "user.email", "test@example.com"], check=True
    )
    subprocess.run(["git", "-C", str(repo), "config", "user.name", "Test"], check=True)


def _commit_fixture(repo: Path) -> None:
    subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
    subprocess.run(["git", "-C", str(repo), "commit", "-qm", "fixture"], check=True)


def test_repository_capture_refuses_dirty_by_default_and_hashes_untracked_content(
    tmp_path: Path,
) -> None:
    _init_git_fixture(tmp_path)
    (tmp_path / "tracked.txt").write_text("tracked")
    _commit_fixture(tmp_path)
    assert capture_repository_version(tmp_path).clean

    untracked = tmp_path / "new.txt"
    untracked.write_text("one")
    with pytest.raises(ValueError, match="dirty"):
        capture_repository_version(tmp_path)
    first = capture_repository_version(tmp_path, allow_dirty=True)
    untracked.write_text("two")
    second = capture_repository_version(tmp_path, allow_dirty=True)
    assert not first.clean
    assert first.dirty_diff_digest != second.dirty_diff_digest


def test_repository_capture_detects_deleted_tracked_file(tmp_path: Path) -> None:
    _init_git_fixture(tmp_path)
    tracked = tmp_path / "tracked.txt"
    tracked.write_text("tracked")
    _commit_fixture(tmp_path)

    tracked.unlink()
    with pytest.raises(ValueError, match="dirty"):
        capture_repository_version(tmp_path)
    assert not capture_repository_version(tmp_path, allow_dirty=True).clean


def test_repository_capture_detects_staged_content_hidden_by_clean_worktree(
    tmp_path: Path,
) -> None:
    _init_git_fixture(tmp_path)
    tracked = tmp_path / "tracked.txt"
    tracked.write_text("head")
    _commit_fixture(tmp_path)

    tracked.write_text("staged")
    subprocess.run(["git", "-C", str(tmp_path), "add", "tracked.txt"], check=True)
    tracked.write_text("head")
    with pytest.raises(ValueError, match="dirty"):
        capture_repository_version(tmp_path)
    dirty = capture_repository_version(tmp_path, allow_dirty=True)
    assert not dirty.clean
    assert dirty.dirty_diff_digest is not None


def test_repository_capture_detects_mode_change_when_git_ignores_file_modes(
    tmp_path: Path,
) -> None:
    _init_git_fixture(tmp_path)
    tracked = tmp_path / "tracked.sh"
    tracked.write_text("#!/bin/sh\n")
    tracked.chmod(0o644)
    _commit_fixture(tmp_path)
    subprocess.run(
        ["git", "-C", str(tmp_path), "config", "core.fileMode", "false"], check=True
    )
    assert capture_repository_version(tmp_path).clean

    tracked.chmod(0o755)
    assert not subprocess.run(
        ["git", "-C", str(tmp_path), "status", "--porcelain"],
        check=True,
        capture_output=True,
    ).stdout
    with pytest.raises(ValueError, match="dirty"):
        capture_repository_version(tmp_path)
    assert not capture_repository_version(tmp_path, allow_dirty=True).clean


def test_repository_digest_tracks_symlink_target_changes_independently(tmp_path: Path) -> None:
    _init_git_fixture(tmp_path)
    for name in ("target-a", "target-b", "target-c"):
        (tmp_path / name).write_text(name)
    link = tmp_path / "link"
    link.symlink_to("target-a")
    _commit_fixture(tmp_path)
    assert capture_repository_version(tmp_path).clean

    link.unlink()
    link.symlink_to("target-b")
    first = capture_repository_version(tmp_path, allow_dirty=True)
    link.unlink()
    link.symlink_to("target-c")
    second = capture_repository_version(tmp_path, allow_dirty=True)
    assert first.dirty_diff_digest != second.dirty_diff_digest


def test_symbolic_jsonl_round_trip_and_on_demand_dataset(tmp_path: Path) -> None:
    observation, actions = _decision()
    record = _record(observation, actions)
    path = tmp_path / "records.jsonl"
    write_jsonl(path, (record,))
    assert "entity_scalars" not in path.read_text()
    loaded = tuple(read_jsonl(path))
    assert loaded == (record,)

    vocab = _vocab(observation, actions)
    dataset = SymbolicCombatDataset(loaded, vocab)
    example = dataset[0]
    batch = collate_training_examples((example,))
    assert batch.policy_target.shape == (1, len(actions))
    assert batch.value_target.tolist() == pytest.approx([0.75])
    assert batch.outcomes == (_outcome(),)


def test_schema_one_symbolic_payload_is_rejected() -> None:
    observation, actions = _decision()
    payload = _record(observation, actions).to_dict()
    observation_payload = cast(dict[str, object], payload["observation"])
    observation_payload["schema_version"] = 1
    with pytest.raises(ValueError, match="unsupported fair observation schema"):
        SymbolicTrainingRecord.from_dict(payload)


def test_tensorizer_rejects_nonfinite_and_float32_overflow_scalars() -> None:
    observation, actions = _decision()
    vocab = _vocab(observation, actions)
    nonfinite = replace(observation, player=replace(observation.player, hp=float("nan")))
    overflow = replace(observation, player=replace(observation.player, hp=10**1000))
    with pytest.raises(ValueError, match="finite"):
        tensorize_combat(nonfinite, actions, vocab)
    with pytest.raises(ValueError, match="representable"):
        tensorize_combat(overflow, actions, vocab)


def test_huge_visit_counts_normalize_before_float32_conversion() -> None:
    observation, actions = _decision()
    huge = 10**1000
    record = _record(observation, actions, visits=(huge, huge, 0, 0))
    vocab = _vocab(observation, actions)
    policy = SymbolicCombatDataset((record,), vocab)[0].policy_target
    assert torch.isfinite(policy).all()
    assert policy.tolist() == pytest.approx([0.5, 0.5, 0.0, 0.0])


def test_policy_loss_remains_finite_with_padded_actions() -> None:
    observation, actions = _decision()
    vocab = _vocab(observation, actions)
    batch = collate_combat_tensors(
        (
            tensorize_combat(observation, actions, vocab),
            tensorize_combat(observation, actions[:1], vocab),
        )
    )
    model = _model(vocab)
    output = model(batch)
    policy = torch.zeros_like(output.logits)
    policy[0, : len(actions)] = 1.0 / len(actions)
    policy[1, 0] = 1.0
    loss = policy_value_loss(output, policy, torch.zeros(2), batch.action_mask)
    assert torch.isfinite(loss)
    loss.backward()
    assert all(
        parameter.grad is None or torch.isfinite(parameter.grad).all()
        for parameter in model.parameters()
    )

    invalid_policy = policy.clone()
    invalid_policy[0, 0] = float("nan")
    with pytest.raises(ValueError, match="finite and nonnegative"):
        policy_value_loss(output, invalid_policy, torch.zeros(2), batch.action_mask)
    invalid_policy = policy.clone()
    invalid_policy[0, 0] = -1.0
    with pytest.raises(ValueError, match="finite and nonnegative"):
        policy_value_loss(output, invalid_policy, torch.zeros(2), batch.action_mask)
    padded_policy = policy.clone()
    padded_policy[1, 1] = 1.0
    with pytest.raises(ValueError, match="padded"):
        policy_value_loss(output, padded_policy, torch.zeros(2), batch.action_mask)
    with pytest.raises(ValueError, match=r"\[-1, 1\]"):
        policy_value_loss(output, policy, torch.tensor([0.0, 2.0]), batch.action_mask)
    with pytest.raises(ValueError, match=r"\[-1, 1\]"):
        policy_value_loss(output, policy, torch.tensor([0.0, float("nan")]), batch.action_mask)
    bad_logits = output.logits.clone()
    bad_logits[0, 0] = float("inf")
    with pytest.raises(ValueError, match="legal policy logits"):
        policy_value_loss(
            PolicyValueOutput(bad_logits, output.value, output.entity_states),
            policy,
            torch.zeros(2),
            batch.action_mask,
        )
    with pytest.raises(ValueError, match="model values"):
        policy_value_loss(
            PolicyValueOutput(
                output.logits, torch.tensor([0.0, float("nan")]), output.entity_states
            ),
            policy,
            torch.zeros(2),
            batch.action_mask,
        )


def test_one_optimizer_step_uses_symbolic_policy_and_value_targets() -> None:
    observation, actions = _decision()
    record = _record(observation, actions, visits=(4, 2, 1, 1))
    vocab = _vocab(observation, actions)
    batch = collate_training_examples((SymbolicCombatDataset((record,), vocab)[0],))
    model = _model(vocab)
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-3)
    before = {name: parameter.detach().clone() for name, parameter in model.named_parameters()}
    output = model(batch.decision)
    loss = policy_value_loss(
        output,
        batch.policy_target,
        batch.value_target,
        batch.decision.action_mask,
    )
    optimizer.zero_grad()
    loss.backward()
    optimizer.step()
    assert torch.isfinite(loss)
    assert any(
        not torch.equal(before[name], parameter) for name, parameter in model.named_parameters()
    )


def test_rollout_classifies_terminal_on_cap_smoke_escape_and_initial_noncombat() -> None:
    lethal_env = RunEnv.combat_fixture()
    state = lethal_env.full_state()
    combat = cast(dict[str, object], state["combat"])
    monsters = cast(list[dict[str, object]], combat["monsters"])
    monsters[0]["hp"] = 1
    lethal_env = RunEnv.from_state_json_for_debugging(json.dumps(state))
    lethal_decision = lethal_env.decision()
    assert lethal_decision.actions[0].kind == "play_hand_slot"
    lethal_observation = cast(FairCombatObservation, lethal_decision.observation)
    lethal_actions = tuple(action.descriptor() for action in lethal_decision.actions)
    lethal_vocab = _vocab(lethal_observation, lethal_actions)
    lethal_model = _FixedIndexModel(lethal_vocab, 0)
    terminal = rollout_model_combat(
        lethal_env,
        lethal_model,
        lethal_vocab,
        generator_seed=0,
        max_steps=1,
    )
    assert terminal.terminal and terminal.status == "won" and terminal.steps == 1
    assert lethal_model.training

    smoke_env = RunEnv.combat_fixture()
    smoke_env.add_potion(Potion.SMOKE_BOMB)
    smoke_decision = smoke_env.decision()
    smoke_index = next(
        index
        for index, action in enumerate(smoke_decision.actions)
        if action.kind == "use_potion_slot"
    )
    smoke_observation = cast(FairCombatObservation, smoke_decision.observation)
    smoke_actions = tuple(action.descriptor() for action in smoke_decision.actions)
    smoke_vocab = _vocab(smoke_observation, smoke_actions)
    escaped = rollout_model_combat(
        smoke_env,
        _FixedIndexModel(smoke_vocab, smoke_index),
        smoke_vocab,
        generator_seed=0,
        max_steps=1,
    )
    assert escaped.terminal and escaped.status == "escaped" and escaped.steps == 1

    with pytest.raises(TypeError, match="initially active combat"):
        rollout_model_combat(
            RunEnv.map_fixture(),
            _model(smoke_vocab),
            smoke_vocab,
            generator_seed=0,
            max_steps=1,
        )


def test_rollout_selected_index_applies_exact_sidecar_action_result() -> None:
    env = RunEnv.combat_fixture()
    decision = env.decision()
    observation = cast(FairCombatObservation, decision.observation)
    actions = tuple(action.descriptor() for action in decision.actions)
    end_index = next(
        index for index, action in enumerate(decision.actions) if action.kind == "end_turn"
    )
    vocab = _vocab(observation, actions)
    result = rollout_model_combat(
        env,
        _FixedIndexModel(vocab, end_index),
        vocab,
        generator_seed=0,
        max_steps=1,
    )
    after = cast(FairCombatObservation, env.observation())
    assert result.selected_action_indices == (end_index,)
    assert result.truncated and env.revision == 1
    assert after.player.hp < observation.player.hp


def test_capped_model_rollouts_use_sidecar_and_report_distribution() -> None:
    observation, actions = _decision()
    vocab = _vocab(observation, actions)
    model = _model(vocab)
    results = [
        rollout_model_combat(
            RunEnv.combat_fixture(),
            model,
            vocab,
            generator_seed=seed,
            max_steps=40,
        )
        for seed in range(4)
    ]
    capped = rollout_model_combat(
        RunEnv.combat_fixture(),
        model,
        vocab,
        generator_seed=99,
        max_steps=1,
    )
    distribution = summarize_rollouts([*results, capped])
    assert distribution.runs == 5
    assert distribution.terminal + distribution.truncated == 5
    assert capped.truncated and capped.steps == 1
    assert all(result.steps <= 40 for result in results)
    assert all(len(result.selected_action_indices) == result.steps for result in results)


def test_teacher_conflict_uses_explicit_distinct_paired_public_roots() -> None:
    observation, actions = _decision()
    first = _record(
        observation,
        actions,
        chosen=0,
        visits=(10, 0, 0, 0),
        root_id="hidden-root-a",
        teacher_pair_id="pair-1",
    )
    second = _record(
        observation,
        actions,
        chosen=1,
        visits=(0, 10, 0, 0),
        root_id="hidden-root-b",
        teacher_pair_id="pair-1",
    )
    unrelated = _rebind_root(first, "natural-root", teacher_pair_id=None)
    report = teacher_conflict_report([first, second, unrelated])
    assert len(report) == 1
    assert report[0].teacher_pair_id == "pair-1"
    assert report[0].record_count == 2
    assert report[0].jensen_shannon_divergence == pytest.approx(torch.log(torch.tensor(2.0)).item())

    reversed_actions = second.actions[::-1]
    with pytest.raises(ValueError, match="legal action"):
        teacher_conflict_report(
            [
                first,
                replace(
                    second,
                    actions=reversed_actions,
                    chosen_action=reversed_actions[1],
                    chosen_action_index=1,
                    record_id=None,
                ),
            ]
        )
    with pytest.raises(ValueError, match="one policy per root"):
        teacher_conflict_report([first, _rebind_root(second, first.root_id)])
    changed_observation = replace(
        observation,
        player=replace(observation.player, hp=observation.player.hp - 1),
    )
    changed = _record(
        changed_observation,
        actions,
        root_id="hidden-root-c",
        teacher_pair_id="pair-1",
    )
    with pytest.raises(ValueError, match="fair digest"):
        teacher_conflict_report([first, changed])
    with pytest.raises(ValueError, match="at least two"):
        teacher_conflict_report([first])


def test_structured_outcome_rejects_invalid_hp_potions_flags_and_counters() -> None:
    outcome = _outcome()
    with pytest.raises(ValueError, match="terminal HP"):
        replace(outcome, terminal_hp=81)
    with pytest.raises(TypeError, match="potion slots"):
        replace(outcome, potion_slots=(3,))
    with pytest.raises(TypeError, match="potion slots must be a tuple"):
        replace(outcome, potion_slots=cast(tuple[str | None, ...], [None]))
    with pytest.raises(TypeError, match="counter changes must be a tuple"):
        replace(outcome, counter_changes=cast(tuple[CounterChange, ...], []))
    with pytest.raises(TypeError, match="contain CounterChange"):
        replace(outcome, counter_changes=cast(tuple[CounterChange, ...], ("bad",)))
    with pytest.raises(ValueError, match="flags"):
        replace(outcome, terminal=False)
    with pytest.raises(ValueError, match="zero HP"):
        replace(outcome, status="lost", terminal_hp=1)
    with pytest.raises(ValueError, match="owner kind"):
        CounterChange(
            cast(Literal["card", "relic"], "player"),
            "Player",
            "cards",
            0,
            1,
        )
    with pytest.raises(TypeError, match="owner kind must be a string"):
        CounterChange(cast(Literal["card", "relic"], 1), "Player", "cards", 0, 1)
    with pytest.raises(TypeError, match="keys must be strings"):
        CounterChange("card", cast(str, 1), "cards", 0, 1)
    with pytest.raises(ValueError, match="keys must be nonempty"):
        CounterChange("relic", "Ink Bottle", "", 0, 1)
    with pytest.raises(TypeError, match="values must be integers"):
        CounterChange("relic", "Ink Bottle", "cards", cast(int, True), 1)


def test_dataset_rejects_empty_and_out_of_range_targets() -> None:
    observation, actions = _decision()
    first = _record(observation, actions)
    vocab = _vocab(observation, actions)
    with pytest.raises(ValueError, match="nonempty"):
        SymbolicCombatDataset((), vocab)
    with pytest.raises(ValueError, match=r"\[-1, 1\]"):
        replace(first, target_value=1.1)
    with pytest.raises(TypeError, match="nonempty"):
        replace(first, value_target_name="")


def test_training_record_parsing_is_strict_and_mappings_are_deep_frozen() -> None:
    observation, actions = _decision()
    record = _record(observation, actions)
    nested = cast(dict[str, object], record.to_dict()["search_config"])
    nested["depth"] = 99
    assert cast(dict[str, object], record.to_dict()["search_config"])["depth"] == 8
    with pytest.raises(TypeError):
        cast(dict[str, object], record.search_config)["new"] = 1

    payload = record.to_dict()
    payload["extra"] = 1
    with pytest.raises(ValueError, match="missing or unknown"):
        SymbolicTrainingRecord.from_dict(payload)
    payload = record.to_dict()
    cast(dict[str, object], cast(list[object], payload["actions"])[0])["family"] = 7
    with pytest.raises(TypeError, match="action family"):
        SymbolicTrainingRecord.from_dict(payload)
    payload = record.to_dict()
    cast(dict[str, object], cast(list[object], payload["actions"])[0])["hand_slot"] = None
    with pytest.raises(TypeError, match="hand_slot"):
        SymbolicTrainingRecord.from_dict(payload)
    payload = record.to_dict()
    cast(dict[str, object], payload["outcome"])["terminal"] = 1
    with pytest.raises(TypeError, match="boolean"):
        SymbolicTrainingRecord.from_dict(payload)
    payload = record.to_dict()
    cast(dict[str, object], payload["repository"])["extra"] = True
    with pytest.raises(ValueError, match="repository"):
        SymbolicTrainingRecord.from_dict(payload)
    payload = record.to_dict()
    payload["teacher_visit_counts"] = [1.0, 0, 0, 0]
    with pytest.raises(TypeError, match="teacher visit count"):
        SymbolicTrainingRecord.from_dict(payload)
    payload = record.to_dict()
    payload["target_value"] = "0.5"
    with pytest.raises(TypeError, match="number"):
        SymbolicTrainingRecord.from_dict(payload)
    payload = record.to_dict()
    cast(dict[str, object], payload["observation"])["future_hidden_field"] = 1
    with pytest.raises(ValueError, match="canonical"):
        SymbolicTrainingRecord.from_dict(payload)
    payload = record.to_dict()
    player = cast(
        dict[str, object],
        cast(dict[str, object], payload["observation"])["player"],
    )
    player["hp"] = True
    with pytest.raises(TypeError, match="scalar type"):
        SymbolicTrainingRecord.from_dict(payload)


def test_training_record_rejects_misaligned_choice_and_counts() -> None:
    observation, actions = _decision()
    record = _record(observation, actions)
    with pytest.raises(ValueError, match="descriptor"):
        replace(record, chosen_action=actions[1])
    with pytest.raises(ValueError, match="align"):
        replace(record, teacher_visit_counts=(1,))
    payload = record.to_dict()
    payload["observation_digest"] = "bad"
    payload["record_id"] = None
    with pytest.raises(ValueError, match="digest"):
        SymbolicTrainingRecord.from_dict(payload)
