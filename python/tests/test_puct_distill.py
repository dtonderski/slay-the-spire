from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from typing import cast

import pytest
import torch

from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import (
    CombatModelConfig,
    FairCombatPolicyValueNet,
    TrainingConfig,
    Vocabularies,
    VocabularyBuilder,
    evaluate_matched_puct_gameplay,
    evaluate_matched_puct_roots,
    generate_beam_dataset,
    generate_legal_roots,
    generate_puct_dataset,
    load_dataset_manifest,
    puct_clone_episode_payload,
    read_jsonl,
    train_beam_clone,
)
from sts_sim.rl.cli import data_main
from sts_sim.rl.data import DATASET_MANIFEST_VERSION
from sts_sim.rl.puct import FAIR_LEAF_BATCH_SCHEMA, PUCT_TEACHER_NAME, network_leaf_evaluator
from sts_sim.rl.puct_data import AuthoritativeRootMutationError
from sts_sim.rl.records import COMBAT_PROXY_VALUE_TARGET_NAME
from sts_sim.rl.rewards import COMBAT_PROXY_V1, CombatRewardConfig
from sts_sim.rl.training import TRAINING_CHECKPOINT_FORMAT


def _uniform_evaluator(request_json: str) -> str:
    request = json.loads(request_json)
    assert request["schema"] == FAIR_LEAF_BATCH_SCHEMA
    choices = request["batch"][0]["choices"]
    encoded = json.dumps(request["batch"][0])
    for field in ("card_id", "monster_id", "content_id", "rng"):
        assert field not in encoded
    return json.dumps(
        {
            "schema": FAIR_LEAF_BATCH_SCHEMA,
            "batch": [{"priors": [1.0] * len(choices), "value": 0.25}],
        }
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


def _resign_dataset_manifest(payload: dict[str, object]) -> None:
    unsigned = dict(payload)
    unsigned.pop("manifest_digest")
    payload["manifest_digest"] = hashlib.sha256(
        json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def test_puct_clone_episode_is_deterministic_and_leaves_the_root_untouched() -> None:
    env = RunEnv.combat_fixture()
    before = env.snapshot().hash
    first = puct_clone_episode_payload(
        env,
        _uniform_evaluator,
        simulation_budget=4,
        transition_budget=4,
        max_decisions=1,
        max_player_turns=100,
    )
    second = puct_clone_episode_payload(
        env,
        _uniform_evaluator,
        simulation_budget=4,
        transition_budget=4,
        max_decisions=1,
        max_player_turns=100,
    )
    assert first == second
    assert env.snapshot().hash == before
    assert first["teacher_name"] == PUCT_TEACHER_NAME
    outcome = cast(dict[str, object], first["outcome"])
    assert outcome["status"] == "truncated"
    assert outcome["truncation_trigger"] == "accepted_decisions"
    assert outcome["accepted_decisions"] == 1
    step = cast(dict[str, object], cast(list[object], first["steps"])[0])
    visits = cast(list[int], step["visits"])
    assert sum(visits) == step["completed_simulations"]
    assert cast(int, step["transitions"]) <= 4
    selected = cast(int, step["selected_index"])
    assert visits[selected] == max(visits)
    assert all(value < visits[selected] for value in visits[:selected])
    assert abs(cast(float, step["value"]) - 0.25) < 1e-12


def test_puct_clone_episode_classifies_initial_terminal_without_search() -> None:
    state = RunEnv.combat_fixture().full_state()
    combat = cast(dict[str, object], state["combat"])
    player = cast(dict[str, object], combat["player"])
    combat["phase"] = "Lost"
    player["hp"] = 0
    state["player_hp"] = 0
    env = RunEnv.from_state_json_for_debugging(json.dumps(state))
    payload = puct_clone_episode_payload(
        env,
        _uniform_evaluator,
        simulation_budget=4,
        transition_budget=4,
        max_decisions=8,
        max_player_turns=8,
    )
    assert payload["steps"] == []
    assert cast(dict[str, object], payload["outcome"])["status"] == "lost"
    assert cast(dict[str, object], payload["outcome"])["accepted_decisions"] == 0


def test_puct_clone_episode_network_leaf_stays_fair() -> None:
    env, model, vocabularies = _tiny_policy_net()
    payload = puct_clone_episode_payload(
        env,
        network_leaf_evaluator(model, vocabularies),
        simulation_budget=2,
        transition_budget=2,
        max_decisions=1,
        max_player_turns=100,
    )
    assert payload["steps"]
    step = cast(dict[str, object], cast(list[object], payload["steps"])[0])
    encoded = json.dumps(step["observation"])
    assert "card_id" not in encoded
    assert "monster_id" not in encoded


def _beam_train_checkpoint(tmp_path: Path) -> tuple[Path, Path]:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0", "BEAMCLONE12"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "beam-train",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    checkpoint = tmp_path / "teacher.pt"
    train_beam_clone(
        tmp_path / "beam-train/dataset-manifest.json", checkpoint, _smoke_training_config()
    )
    return tmp_path / "roots/root-manifest.json", checkpoint


def test_puct_dataset_generation_is_deterministic_and_v7(
    tmp_path: Path,
) -> None:
    roots, checkpoint = _beam_train_checkpoint(tmp_path)
    first = generate_puct_dataset(
        roots,
        tmp_path / "puct-left",
        checkpoint,
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=2,
        max_player_turns=3,
    )
    second = generate_puct_dataset(
        roots,
        tmp_path / "puct-right",
        checkpoint,
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=2,
        max_player_turns=3,
    )
    assert first.manifest_digest == second.manifest_digest
    assert first.manifest_version == DATASET_MANIFEST_VERSION
    assert first.teacher_name == PUCT_TEACHER_NAME
    records = tuple(read_jsonl(tmp_path / "puct-left/train/train.jsonl"))
    assert records
    assert all(record.record_version == 4 for record in records)
    assert all(record.value_target_name == COMBAT_PROXY_VALUE_TARGET_NAME for record in records)
    assert all(record.search_root_mean_value is not None for record in records)
    assert all(sum(record.teacher_visit_counts) > 0 for record in records)
    for record in records:
        expected = COMBAT_PROXY_V1.value(record.outcome)
        assert record.target_value == expected
        assert record.value_target_mask is (expected is not None)
    loaded = load_dataset_manifest(
        tmp_path / "puct-left/dataset-manifest.json", requested_split="train"
    )
    assert loaded.manifest_digest == first.manifest_digest


def test_old_dataset_manifest_versions_fail_closed(tmp_path: Path) -> None:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    beam = generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "beam",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=4,
        max_player_turns=3,
    )
    assert beam.manifest_version == DATASET_MANIFEST_VERSION
    load_dataset_manifest(tmp_path / "beam/dataset-manifest.json", requested_split="train")
    roots, checkpoint = _beam_train_checkpoint(tmp_path / "puct-setup")
    puct = generate_puct_dataset(
        roots,
        tmp_path / "puct",
        checkpoint,
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=1,
        max_player_turns=3,
    )
    assert puct.manifest_version == DATASET_MANIFEST_VERSION
    records = tuple(read_jsonl(tmp_path / "puct/train/train.jsonl"))
    truncated = [record for record in records if record.outcome.truncated]
    assert truncated
    assert all(
        record.value_target_mask is False
        and record.target_value is None
        and record.search_root_mean_value is not None
        for record in truncated
    )
    manifest_path = tmp_path / "puct/dataset-manifest.json"
    payload = json.loads(manifest_path.read_text())
    payload["manifest_version"] = 6
    _resign_dataset_manifest(payload)
    manifest_path.write_text(json.dumps(payload, sort_keys=True, separators=(",", ":")))
    with pytest.raises(ValueError):
        load_dataset_manifest(manifest_path, requested_split="train")


def test_puct_labeling_refuses_checkpoint_source_mismatch(tmp_path: Path) -> None:
    roots, checkpoint = _beam_train_checkpoint(tmp_path)
    payload = torch.load(checkpoint, map_location="cpu", weights_only=False)
    payload["source_digest"] = "a" * 64
    bad = tmp_path / "bad.pt"
    torch.save(payload, bad)
    with pytest.raises(ValueError, match="source digest"):
        generate_puct_dataset(
            roots,
            tmp_path / "puct",
            bad,
            split="train",
            simulation_budget=4,
            transition_budget=4,
            max_decisions=1,
            max_player_turns=3,
        )
    assert not (tmp_path / "puct").exists() or not any((tmp_path / "puct").iterdir())


def test_puct_dataset_excludes_failed_root_and_keeps_complete_accounting(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    roots, checkpoint = _beam_train_checkpoint(tmp_path)
    root_manifest = json.loads(roots.read_text())
    train_ids = [
        cast(str, root["root_id"])
        for root in cast(list[dict[str, object]], root_manifest["roots"])
        if root["split"] == "train"
    ]
    assert len(train_ids) >= 1
    failed = train_ids[0]
    original = puct_clone_episode_payload
    calls = {"count": 0}

    def boom(env: RunEnv, evaluator: object, **kwargs: object) -> dict[str, object]:
        calls["count"] += 1
        assert kwargs.get("leaf_cache") == "exact_state"
        del evaluator, kwargs
        if calls["count"] == 1:
            raise RuntimeError("injected PUCT labeling failure")
        return original(
            env,
            _uniform_evaluator,
            simulation_budget=4,
            transition_budget=4,
            max_decisions=1,
            max_player_turns=3,
        )

    monkeypatch.setattr("sts_sim.rl.puct_data.puct_clone_episode_payload", boom)
    if len(train_ids) == 1:
        with pytest.raises(RuntimeError, match="no dataset was published"):
            generate_puct_dataset(
                roots,
                tmp_path / "puct",
                checkpoint,
                split="train",
                simulation_budget=4,
                transition_budget=4,
                max_decisions=1,
                max_player_turns=3,
            )
        return
    manifest = generate_puct_dataset(
        roots,
        tmp_path / "puct",
        checkpoint,
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=1,
        max_player_turns=3,
    )
    assert failed not in {root.root_id for root in manifest.roots}
    assert any(exclusion.root_id == failed for exclusion in manifest.exclusions)


def test_puct_training_writes_current_checkpoint_format(tmp_path: Path) -> None:
    roots, checkpoint = _beam_train_checkpoint(tmp_path)
    generate_puct_dataset(
        roots,
        tmp_path / "puct-train",
        checkpoint,
        split="train",
        simulation_budget=4,
        transition_budget=4,
        max_decisions=1,
        max_player_turns=3,
    )
    student_path = tmp_path / "student.pt"
    train_beam_clone(
        tmp_path / "puct-train/dataset-manifest.json",
        student_path,
        _smoke_training_config(),
    )
    student = torch.load(student_path, map_location="cpu", weights_only=False)
    assert student["checkpoint_format"] == TRAINING_CHECKPOINT_FORMAT
    assert student["source_epoch_bundle_digest"]


def test_six_policy_matched_roots_restore_independently_and_keep_errors() -> None:
    env, model, vocabularies = _tiny_policy_net()
    snapshot = env.snapshot()
    snapshot_bytes = snapshot.json.encode()
    root_id = hashlib.sha256(snapshot_bytes).hexdigest()
    report = evaluate_matched_puct_roots(
        split_roots=((root_id, snapshot_bytes),),
        evaluation_seed=0,
        model=model,
        vocabularies=vocabularies,
        transition_budget=8,
        simulation_budget=8,
        c_puct=1.5,
        beam_depth=2,
        beam_width=4,
        max_decisions=1,
        max_player_turns=100,
        deduplicate_search_states=True,
    )
    policies = cast(
        dict[str, object], cast(list[dict[str, object]], report["per_root"])[0]["policies"]
    )
    assert set(policies) == {
        "random",
        "network",
        "beam",
        "network_puct",
        "uniform_prior_network_value_puct",
        "uniform_prior_constant_value_puct",
    }
    aggregates = cast(dict[str, dict[str, object]], report["aggregates"])
    for name in (
        "random",
        "network",
        "beam",
        "network_puct",
        "uniform_prior_network_value_puct",
        "uniform_prior_constant_value_puct",
    ):
        row = aggregates[name]
        assert row["win_denominator"] == 1
        counted = (
            cast(int, row["errors"])
            + cast(int, row["truncations"])
            + cast(int, row["win_numerator"])
            + cast(int, row["lost"])
            + cast(int, row["escaped"])
        )
        assert counted == 1


def test_cli_puct_label_wires_generate(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    captured: dict[str, object] = {}

    def fake_generate(*args: object, **kwargs: object) -> object:
        captured["args"] = args
        captured["kwargs"] = kwargs

        class _Manifest:
            def to_dict(self) -> dict[str, object]:
                return {"ok": True}

        return _Manifest()

    monkeypatch.setattr("sts_sim.rl.cli.generate_puct_dataset", fake_generate)
    data_main(
        [
            "puct-label",
            "--roots",
            str(tmp_path / "roots.json"),
            "--output",
            str(tmp_path / "out"),
            "--checkpoint",
            str(tmp_path / "ckpt.pt"),
            "--split",
            "train",
            "--simulation-budget",
            "8",
            "--transition-budget",
            "8",
        ]
    )
    kwargs = cast(dict[str, object], captured["kwargs"])
    assert kwargs["simulation_budget"] == 8
    assert kwargs["transition_budget"] == 8


def test_puct_labeling_root_mutation_hard_aborts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    roots, checkpoint = _beam_train_checkpoint(tmp_path)
    original = puct_clone_episode_payload

    def mutate(
        env: RunEnv,
        evaluator: Callable[[str], str],
        *,
        c_puct: float = 1.5,
        simulation_budget: int = 64,
        transition_budget: int = 64,
        max_decisions: int = 512,
        max_player_turns: int = 100,
        reward_config: CombatRewardConfig | None = None,
        leaf_cache: str | None = None,
    ) -> dict[str, object]:
        decision = env.decision()
        env.step(decision.actions[0])
        return original(
            env,
            evaluator,
            c_puct=c_puct,
            simulation_budget=simulation_budget,
            transition_budget=transition_budget,
            max_decisions=max_decisions,
            max_player_turns=max_player_turns,
            reward_config=reward_config,
            leaf_cache=leaf_cache,
        )

    monkeypatch.setattr("sts_sim.rl.puct_data.puct_clone_episode_payload", mutate)
    output = tmp_path / "puct"
    with pytest.raises(AuthoritativeRootMutationError, match="mutated restored root"):
        generate_puct_dataset(
            roots,
            output,
            checkpoint,
            split="train",
            simulation_budget=4,
            transition_budget=4,
            max_decisions=1,
            max_player_turns=3,
        )
    assert not output.exists() or not any(output.iterdir())


def test_six_policy_rejects_invalid_search_config() -> None:
    missing = Path("missing-roots.json")
    checkpoint = Path("missing-checkpoint.pt")
    with pytest.raises(ValueError, match="beam_depth must be a positive integer"):
        evaluate_matched_puct_gameplay(missing, checkpoint, beam_depth=0)
    with pytest.raises(ValueError, match="beam_width must be a positive integer"):
        evaluate_matched_puct_gameplay(missing, checkpoint, beam_width=0)
    with pytest.raises(ValueError, match="simulation_budget must be a positive integer"):
        evaluate_matched_puct_gameplay(missing, checkpoint, simulation_budget=0)
    with pytest.raises(ValueError, match="transition_budget must be a positive integer"):
        evaluate_matched_puct_gameplay(missing, checkpoint, transition_budget=0)
    with pytest.raises(ValueError, match="c_puct must be finite and positive"):
        evaluate_matched_puct_gameplay(missing, checkpoint, c_puct=0.0)
    with pytest.raises(TypeError, match="deduplicate_search_states must be boolean"):
        evaluate_matched_puct_gameplay(
            missing, checkpoint, deduplicate_search_states=cast(bool, "yes")
        )


def test_six_policy_keeps_overflow_errors_in_the_denominator(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    env, model, vocabularies = _tiny_policy_net()
    snapshot = env.snapshot()
    snapshot_bytes = snapshot.json.encode()
    root_id = hashlib.sha256(snapshot_bytes).hexdigest()

    def boom(*_args: object, **_kwargs: object) -> None:
        raise OverflowError("injected overflow")

    monkeypatch.setattr("sts_sim.rl.puct.rollout_puct_policy", boom)
    report = evaluate_matched_puct_roots(
        split_roots=((root_id, snapshot_bytes),),
        evaluation_seed=0,
        model=model,
        vocabularies=vocabularies,
        transition_budget=8,
        simulation_budget=8,
        c_puct=1.5,
        beam_depth=2,
        beam_width=4,
        max_decisions=1,
        max_player_turns=100,
        deduplicate_search_states=True,
    )
    policies = cast(
        dict[str, dict[str, object]],
        cast(list[dict[str, object]], report["per_root"])[0]["policies"],
    )
    assert policies["network_puct"]["status"] == "error"
    assert policies["network_puct"]["error"] == "injected overflow"
    assert type(policies["network_puct"]["terminal_hp"]) is int
    aggregates = cast(dict[str, dict[str, object]], report["aggregates"])
    assert aggregates["network_puct"]["errors"] == 1
    assert aggregates["network_puct"]["win_denominator"] == 1
    counted = (
        cast(int, aggregates["network_puct"]["errors"])
        + cast(int, aggregates["network_puct"]["truncations"])
        + cast(int, aggregates["network_puct"]["win_numerator"])
        + cast(int, aggregates["network_puct"]["lost"])
        + cast(int, aggregates["network_puct"]["escaped"])
    )
    assert counted == 1
    assert cast(dict[str, object], aggregates["network_puct"]["terminal_hp"])["count"] == 0
    paired = cast(dict[str, dict[str, object]], report["paired"])
    for name in ("network_puct_network", "network_puct_beam"):
        pair = paired[name]
        per_root = cast(list[dict[str, object]], pair["per_root"])
        assert len(per_root) == 1
        assert per_root[0]["errored"] is True
        assert per_root[0]["right_status"] == "error"
        assert per_root[0]["hp_delta"] is None
        assert per_root[0]["accepted_decision_delta"] is None
        assert cast(dict[str, object], pair["hp_delta"])["count"] == 0
        assert pair["roots"] == 1


def test_cli_puct_label_subprocess_writes_v7_manifest(tmp_path: Path) -> None:
    roots, checkpoint = _beam_train_checkpoint(tmp_path)
    output = tmp_path / "puct-cli"
    completed = subprocess.run(
        [
            sys.executable,
            "-c",
            (
                "from sts_sim.rl.cli import data_main; "
                "raise SystemExit(data_main(__import__('sys').argv[1:]))"
            ),
            "puct-label",
            "--roots",
            str(roots),
            "--output",
            str(output),
            "--checkpoint",
            str(checkpoint),
            "--split",
            "train",
            "--simulation-budget",
            "4",
            "--transition-budget",
            "4",
            "--max-decisions",
            "1",
            "--max-player-turns",
            "3",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    manifest = load_dataset_manifest(output / "dataset-manifest.json", requested_split="train")
    assert manifest.manifest_version == DATASET_MANIFEST_VERSION
    payload = json.loads(completed.stdout)
    assert payload["manifest_version"] == DATASET_MANIFEST_VERSION
    assert payload["teacher_name"] == PUCT_TEACHER_NAME
