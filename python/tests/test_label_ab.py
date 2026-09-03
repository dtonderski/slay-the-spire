from __future__ import annotations

import hashlib
from dataclasses import replace
from pathlib import Path
from types import MappingProxyType
from typing import cast

import pytest
import torch

from sts_sim import FairCombatObservation, RunEnv
from sts_sim.rl import (
    RESULT_KIND,
    TrainingConfig,
    load_label_ab_plan,
    require_paired_treatment_identity,
    require_pairwise_disjoint_cohorts,
    run_label_ab_experiment,
    train_beam_clone,
    verify_label_ab_rerun,
)
from sts_sim.rl.cli import label_ab_main
from sts_sim.rl.data import DatasetManifest, RootEntry, RootManifest
from sts_sim.rl.experiment import load_experiment_predeclaration, write_scientific_artifact
from sts_sim.rl.gameplay import canonical_public_action_descriptors
from sts_sim.rl.label_ab import (
    DESIGNATED_RERUN_ARTIFACTS,
    PUBLISHED_INPUT_DIR,
    _bind_live_source,
    _copied_tree_refs,
    _copy_tree_nofollow,
    _execute_label_ab,
    _plan_for_execution,
    _query_beam_first_step,
    _require_authorization_bindings,
    _require_student_dataset_binding,
    _require_teacher_dataset_binding,
    _require_unpublished_dir,
    _test_execution,
    behavior_policy_index,
    cluster_bootstrap_mean_delta,
    parse_label_ab_plan,
    parse_label_ab_result,
    production_plan_payload,
    require_label_ab_cohorts,
    write_held_out_authorization,
)
from sts_sim.rl.model import CombatModelConfig, FairCombatPolicyValueNet
from sts_sim.rl.provenance import RepositoryVersion, canonical_bytes, capture_repository_version
from sts_sim.rl.tensor import VocabularyBuilder
from sts_sim.rl.training import _model_state_digest, load_training_checkpoint


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _object_map(value: object) -> dict[str, object]:
    assert type(value) is dict
    return cast(dict[str, object], value)


def _object_list(value: object) -> list[object]:
    assert type(value) is list
    return cast(list[object], value)


def _skip_if_dirty() -> None:
    try:
        capture_repository_version(_repo_root())
    except ValueError as error:
        pytest.skip(str(error))


def test_production_plan_rejects_unknown_and_missing_fields() -> None:
    payload = production_plan_payload("a" * 40)
    parse_label_ab_plan(payload)
    extra = dict(payload)
    extra["legacy_treatment_control"] = True
    with pytest.raises(ValueError, match="missing or unknown fields"):
        parse_label_ab_plan(extra)
    missing = dict(payload)
    missing.pop("metrics")
    with pytest.raises(ValueError, match="missing or unknown fields"):
        parse_label_ab_plan(missing)
    mutated = dict(payload)
    training_payload = payload["training"]
    assert isinstance(training_payload, dict)
    training = dict(training_payload)
    training["total_steps"] = 100
    mutated["training"] = training
    unsigned = {key: value for key, value in mutated.items() if key != "plan_digest"}
    mutated["plan_digest"] = hashlib.sha256(canonical_bytes(unsigned)).hexdigest()
    with pytest.raises(ValueError, match="production plan constants mismatch"):
        parse_label_ab_plan(mutated)


def test_plan_to_dict_is_a_deep_copy() -> None:
    plan = parse_label_ab_plan(production_plan_payload("a" * 40))
    thawed = plan.to_dict()
    bootstrap = _object_map(_object_map(thawed["cohorts"])["bootstrap"])
    bootstrap["count"] = 1
    assert _object_map(_object_map(plan.to_dict()["cohorts"])["bootstrap"])["count"] == 2400
    assert isinstance(plan._payload["cohorts"], MappingProxyType)


def test_load_label_ab_plan_rejects_tiny_on_disk(tmp_path: Path) -> None:
    payload = production_plan_payload("a" * 40)
    training_payload = payload["training"]
    assert isinstance(training_payload, dict)
    training = dict(training_payload)
    training["total_steps"] = 1
    training["minimum_roots"] = 1
    payload["training"] = training
    unsigned = {key: value for key, value in payload.items() if key != "plan_digest"}
    payload["plan_digest"] = hashlib.sha256(canonical_bytes(unsigned)).hexdigest()
    path = tmp_path / "tiny-plan.json"
    write_scientific_artifact(path, canonical_bytes(payload))
    with pytest.raises(ValueError, match="production plan constants mismatch"):
        load_label_ab_plan(path)


def test_cli_validate_plan_rejects_unknown_field(tmp_path: Path) -> None:
    payload = production_plan_payload("a" * 40)
    payload["deleted_compatibility"] = False
    path = tmp_path / "bad-plan.json"
    write_scientific_artifact(path, canonical_bytes(payload))
    assert label_ab_main(["validate-plan", "--plan", str(path)]) == 1


def test_cli_validate_plan_rejects_source_commit_mismatch(tmp_path: Path) -> None:
    payload = production_plan_payload("a" * 40)
    path = tmp_path / "foreign-plan.json"
    write_scientific_artifact(path, canonical_bytes(payload))
    assert label_ab_main(["validate-plan", "--plan", str(path)]) == 1


def test_cli_run_binds_source_commit_before_generating_roots(tmp_path: Path) -> None:
    payload = production_plan_payload("a" * 40)
    plan_path = tmp_path / "plan.json"
    write_scientific_artifact(plan_path, canonical_bytes(payload))
    assert (
        label_ab_main(
            [
                "run",
                "--plan",
                str(plan_path),
                "--work-dir",
                str(tmp_path / "work"),
                "--experiment-dir",
                str(tmp_path / "experiment"),
            ]
        )
        == 1
    )
    assert not (tmp_path / "work" / "bootstrap-roots").exists()


def test_cli_omits_unbound_step_commands() -> None:
    with pytest.raises(SystemExit):
        label_ab_main(["paired-label"])
    with pytest.raises(SystemExit):
        label_ab_main(["train-students"])
    with pytest.raises(SystemExit):
        label_ab_main(["assess"])


def test_public_package_hides_tiny_execution_config() -> None:
    from sts_sim import rl

    assert not hasattr(rl, "label_ab_test_config")
    assert not hasattr(rl, "LabelAbExecutionConfig")
    assert not hasattr(rl, "execution_config_from_plan")
    assert not hasattr(rl, "_test_execution")
    assert not hasattr(rl, "_execute_label_ab")
    assert not hasattr(rl, "_plan_for_execution")


def test_cluster_bootstrap_is_named_and_deterministic() -> None:
    first = cluster_bootstrap_mean_delta((1.0, 0.0, -1.0, 1.0), draws=32, seed=20260903)
    second = cluster_bootstrap_mean_delta((1.0, 0.0, -1.0, 1.0), draws=32, seed=20260903)
    assert first == second
    assert first["stream"] == "cluster_bootstrap_v1"
    assert first["draws"] == 32
    interval = first["percentile_ci_95"]
    assert isinstance(interval, list) and len(interval) == 2
    lower_raw, upper_raw = interval
    assert isinstance(lower_raw, (int, float)) and isinstance(upper_raw, (int, float))
    assert float(lower_raw) <= float(upper_raw)


def test_result_parser_rejects_null_plan_digest_and_nested_unknown_fields() -> None:
    plan_digest = "a" * 64
    unsigned = {
        "kind": RESULT_KIND,
        "schema_version": 1,
        "plan_digest": plan_digest,
        "primary": {
            "name": "paired_official_greedy_network_win_rate_delta",
            "delta": 0.0,
            "roots": 2,
            "note": "lost/escaped/truncated/error are nonwins; errors remain in the denominator",
        },
        "secondary": {
            "paired_network_puct_win_rate_delta": 0.0,
            "network_puct_roots": 2,
        },
        "integrity": {
            "nonlearned_arms_identical": True,
            "nonlearned_arms": ["random", "beam", "uniform_prior_constant_value_puct"],
            "promotion_claim": False,
        },
        "bootstrap": {
            "stream": "cluster_bootstrap_v1",
            "draws": 16,
            "seed": 20260903,
            "roots": 2,
            "observed_delta": 0.0,
            "percentile_ci_95": [0.0, 0.0],
        },
        "promotion_claim": False,
    }
    payload = dict(unsigned)
    payload["result_digest"] = hashlib.sha256(canonical_bytes(unsigned)).hexdigest()
    parsed = parse_label_ab_result(payload)
    thawed = parsed.to_dict()
    _object_map(thawed["bootstrap"])["draws"] = 1
    assert _object_map(parsed.to_dict()["bootstrap"])["draws"] == 16
    null_digest: dict[str, object] = dict(payload)
    null_digest["plan_digest"] = None
    unsigned_null = {key: value for key, value in null_digest.items() if key != "result_digest"}
    null_digest["result_digest"] = hashlib.sha256(canonical_bytes(unsigned_null)).hexdigest()
    with pytest.raises((ValueError, TypeError), match="plan_digest"):
        parse_label_ab_result(null_digest)
    extra: dict[str, object] = dict(payload)
    extra_primary = dict(_object_map(payload["primary"]))
    extra_primary["peek"] = True
    extra["primary"] = extra_primary
    extra_unsigned = {key: value for key, value in extra.items() if key != "result_digest"}
    extra["result_digest"] = hashlib.sha256(canonical_bytes(extra_unsigned)).hexdigest()
    with pytest.raises(ValueError, match="missing or unknown fields"):
        parse_label_ab_result(extra)
    mismatched: dict[str, object] = dict(unsigned)
    mismatched_primary = dict(_object_map(unsigned["primary"]))
    mismatched_primary["delta"] = 1.0
    mismatched["primary"] = mismatched_primary
    mismatched_unsigned = {
        key: value for key, value in mismatched.items() if key != "result_digest"
    }
    mismatched["result_digest"] = hashlib.sha256(canonical_bytes(mismatched_unsigned)).hexdigest()
    with pytest.raises(ValueError, match="primary delta does not match bootstrap observed_delta"):
        parse_label_ab_result(mismatched)


def test_pairwise_disjointness_rejects_overlapping_seeds() -> None:
    def manifest(seed: str, root_char: str, bundle: str = "f" * 64) -> RootManifest:
        root_id = root_char * 64
        return RootManifest(
            6,
            "legal_run_policy",
            "sha256_action_policy_v4",
            "a" * 64,
            RepositoryVersion("b" * 40, True),
            0,
            128,
            1,
            "combat-agent-phase2-v1",
            (seed,),
            "c" * 64,
            bundle,
            (
                RootEntry(
                    root_id,
                    "train",
                    "d" * 64,
                    f"train/roots/{root_id}.json",
                    (f"sim-seed:{seed}",),
                    (seed,),
                ),
            ),
            (),
            "e" * 64,
        )

    left = manifest("BOOT0", "1")
    right = manifest("TREAT0", "2")
    require_pairwise_disjoint_cohorts((("bootstrap", left), ("treatment", right)))
    overlap = manifest("BOOT0", "3")
    with pytest.raises(ValueError, match="generation seeds are not disjoint"):
        require_pairwise_disjoint_cohorts((("bootstrap", left), ("treatment", overlap)))
    bundle_mismatch = manifest("HELD0", "4", bundle="0" * 64)
    with pytest.raises(ValueError, match="source-epoch-bundle digests differ"):
        require_pairwise_disjoint_cohorts((("bootstrap", left), ("held_out", bundle_mismatch)))


def test_require_label_ab_cohorts_rejects_wrong_requested_seeds() -> None:
    execution = _test_execution(
        seed_prefix="LABELAB",
        bootstrap_start=0,
        bootstrap_count=1,
        treatment_start=1,
        treatment_count=1,
        held_out_start=2,
        held_out_count=1,
    )

    def manifest(seed: str, root_char: str) -> RootManifest:
        root_id = root_char * 64
        return RootManifest(
            6,
            "legal_run_policy",
            "sha256_action_policy_v4",
            "a" * 64,
            RepositoryVersion("b" * 40, True),
            0,
            128,
            1,
            "combat-agent-phase2-v1",
            (seed,),
            "c" * 64,
            "f" * 64,
            (
                RootEntry(
                    root_id,
                    "train",
                    "d" * 64,
                    f"train/roots/{root_id}.json",
                    (f"sim-seed:{seed}",),
                    (seed,),
                ),
            ),
            (),
            "e" * 64,
        )

    bootstrap = manifest("LABELAB0", "1")
    treatment = manifest("LABELAB1", "2")
    held_out = manifest("WRONG2", "3")
    with pytest.raises(ValueError, match="requested seeds do not match"):
        require_label_ab_cohorts(
            bootstrap=bootstrap,
            treatment=treatment,
            held_out=held_out,
            execution=execution,
        )


def test_beam_query_does_not_mutate_behavior_env() -> None:
    env = RunEnv.combat_fixture()
    decision = env.decision()
    assert isinstance(decision.observation, FairCombatObservation)
    before = env.snapshot().hash
    execution = _test_execution()
    teacher, selected, counts = _query_beam_first_step(env, decision, execution)
    assert env.snapshot().hash == before
    assert teacher[0] == "public_decision_replanning_beam"
    assert sum(counts) == 1
    assert counts[selected] == 1
    index = behavior_policy_index(
        behavior_seed=execution.behavior_seed,
        root_id="a" * 64,
        decision_index=0,
        descriptors=canonical_public_action_descriptors(decision.actions),
    )
    chosen = decision.actions[index]
    assert any(candidate is chosen for candidate in decision.actions)
    env.step(chosen)
    assert env.snapshot().hash != before


def test_resume_rejects_injected_common_init() -> None:
    config = TrainingConfig(
        batch_size=1,
        total_steps=1,
        model_width=16,
        model_heads=4,
        model_layers=1,
        feedforward_width=32,
        minimum_roots=1,
        minimum_lineages=1,
    )
    env = RunEnv.combat_fixture()
    decision = env.decision()
    observation = decision.observation
    assert isinstance(observation, FairCombatObservation)
    builder = VocabularyBuilder()
    builder.add(observation, tuple(action.descriptor() for action in decision.actions))
    vocab = builder.freeze()
    torch.manual_seed(7)
    model = FairCombatPolicyValueNet(
        vocab, CombatModelConfig(width=16, heads=4, layers=1, feedforward_width=32)
    )
    with pytest.raises(ValueError, match="resume cannot inject"):
        train_beam_clone(
            Path("missing.json"),
            Path("missing.pt"),
            config,
            resume=True,
            vocabularies=vocab,
            initial_model_state=model.state_dict(),
        )


def test_tiny_end_to_end_paired_label_ab(tmp_path: Path) -> None:
    _skip_if_dirty()
    execution = _test_execution(
        seed_prefix="LABELAB",
        bootstrap_start=0,
        bootstrap_count=3,
        treatment_start=3,
        treatment_count=3,
        held_out_start=6,
        held_out_count=3,
        max_decisions=3,
        max_player_turns=3,
        beam_transition_budget=80,
        simulation_budget=2,
        puct_transition_budget=2,
        bootstrap_draws=16,
    )
    version = capture_repository_version(_repo_root())
    plan = _plan_for_execution(execution, version.git_sha)
    plan_bytes = canonical_bytes(plan.to_dict())
    with pytest.raises(ValueError, match="production plan constants mismatch"):
        parse_label_ab_plan(plan.to_dict())
    leftover = tmp_path / "tiny-plan.json"
    write_scientific_artifact(leftover, plan_bytes)
    with pytest.raises(ValueError, match="production plan constants mismatch"):
        load_label_ab_plan(leftover)
    try:
        payload = _execute_label_ab(
            plan,
            plan_bytes,
            tmp_path / "work",
            tmp_path / "experiment",
            repository=_repo_root(),
        )
    except ValueError as error:
        pytest.skip(str(error))
    assert payload["promotion_claim"] is False
    assert payload["plan_digest"] == plan.plan_digest
    experiment = tmp_path / "experiment"
    declared = load_experiment_predeclaration(experiment / "predeclaration.json")
    assert all(not Path(ref.path).is_absolute() for ref in (*declared.inputs, *declared.outputs))
    assert any(ref.path.startswith("inputs/") for ref in declared.inputs)
    beam_records, puct_records = require_paired_treatment_identity(
        experiment / "inputs/beam-dataset/dataset-manifest.json",
        experiment / "inputs/puct-dataset/dataset-manifest.json",
        bootstrap_manifest=None,
    )
    assert beam_records
    for beam_record, puct_record in zip(beam_records, puct_records, strict=True):
        assert beam_record.observation_digest == puct_record.observation_digest
        assert beam_record.outcome == puct_record.outcome
        assert beam_record.planner_name != puct_record.planner_name
    beam_payload, _, _ = load_training_checkpoint(experiment / "student-beam.pt")
    puct_payload, _, _ = load_training_checkpoint(experiment / "student-puct.pt")
    assert (
        beam_payload["common_initial_model_state_digest"]
        == puct_payload["common_initial_model_state_digest"]
    )
    assert beam_payload["dataset_manifest_digest"] != puct_payload["dataset_manifest_digest"]
    try:
        _execute_label_ab(
            plan,
            plan_bytes,
            tmp_path / "work-rerun",
            tmp_path / "experiment-rerun",
            repository=_repo_root(),
        )
    except ValueError as error:
        pytest.skip(str(error))
    report = verify_label_ab_rerun(reference=experiment, candidate=tmp_path / "experiment-rerun")
    assert report["ok"] is True
    assert "predeclaration.json" in DESIGNATED_RERUN_ARTIFACTS


def test_verify_rerun_compares_designated_bytes_including_mandatory_files(
    tmp_path: Path,
) -> None:
    reference = tmp_path / "reference"
    candidate = tmp_path / "candidate"
    reference.mkdir()
    candidate.mkdir()
    for name in DESIGNATED_RERUN_ARTIFACTS:
        (reference / name).write_bytes(b"identical-bytes")
        (candidate / name).write_bytes(b"identical-bytes")
    (reference / PUBLISHED_INPUT_DIR).mkdir()
    (candidate / PUBLISHED_INPUT_DIR).mkdir()
    report = verify_label_ab_rerun(reference=reference, candidate=candidate)
    assert report["ok"] is True
    assert report["reproduce_experiment_is_identity_verification"] is True
    assert "predeclaration.json" in DESIGNATED_RERUN_ARTIFACTS
    assert "artifact-inventory.sha256" in DESIGNATED_RERUN_ARTIFACTS
    (candidate / "result.json").write_bytes(b"diverged")
    report = verify_label_ab_rerun(reference=reference, candidate=candidate)
    assert report["ok"] is False
    assert "result.json" in _object_list(report["mismatches"])
    (candidate / "extra.json").write_bytes(b"extra")
    report = verify_label_ab_rerun(reference=reference, candidate=candidate)
    assert report["ok"] is False
    assert any(str(item).startswith("membership:") for item in _object_list(report["mismatches"]))


def test_run_rejects_foreign_plan_without_creating_roots(tmp_path: Path) -> None:
    payload = production_plan_payload("a" * 40)
    plan_path = tmp_path / "plan.json"
    write_scientific_artifact(plan_path, canonical_bytes(payload))
    work = tmp_path / "work"
    experiment = tmp_path / "experiment"
    with pytest.raises(ValueError, match="plan.source_commit|clean source worktree|dirty"):
        run_label_ab_experiment(plan_path, work, experiment)
    assert not (work / "bootstrap-roots").exists()


def test_bind_live_source_rejects_foreign_commit() -> None:
    plan = parse_label_ab_plan(production_plan_payload("a" * 40))
    with pytest.raises(ValueError, match="plan.source_commit|clean source worktree|dirty"):
        _bind_live_source(plan, repository=_repo_root())


def test_copied_input_tree_refs_are_relative_and_stable(tmp_path: Path) -> None:
    payload = b'{"kind":"fixture","value":1}\n'
    for name in ("work-a", "work-b"):
        source = tmp_path / name / "roots"
        _require_unpublished_dir(source)
        write_scientific_artifact(source / "root-manifest.json", payload)
        nested = source / "train" / "roots"
        _require_unpublished_dir(nested)
        write_scientific_artifact(nested / ("a" * 64 + ".json"), payload)
    first = tmp_path / "exp-a"
    second = tmp_path / "exp-b"
    _require_unpublished_dir(first)
    _require_unpublished_dir(second)
    _copy_tree_nofollow(tmp_path / "work-a" / "roots", first / "inputs" / "bootstrap")
    _copy_tree_nofollow(tmp_path / "work-b" / "roots", second / "inputs" / "bootstrap")
    left = _copied_tree_refs(
        first, "inputs/bootstrap", "root-manifest.json", "bootstrap_root_manifest"
    )
    right = _copied_tree_refs(
        second, "inputs/bootstrap", "root-manifest.json", "bootstrap_root_manifest"
    )
    assert [ref.path for ref in left] == [ref.path for ref in right]
    assert [ref.sha256 for ref in left] == [ref.sha256 for ref in right]
    assert all(not Path(ref.path).is_absolute() for ref in left)
    assert {ref.role for ref in left} == {"bootstrap_root_manifest", "input_tree_member"}


def test_nested_output_dir_refuses_symlink(tmp_path: Path) -> None:
    target = tmp_path / "target"
    target.mkdir()
    link = tmp_path / "link"
    link.symlink_to(target)
    with pytest.raises(ValueError, match="symlink"):
        _require_unpublished_dir(link)


def test_authorization_rebind_rejects_wrong_treatment_manifest(tmp_path: Path) -> None:
    def manifest(seed: str, root_char: str, digest: str) -> RootManifest:
        root_id = root_char * 64
        return RootManifest(
            6,
            "legal_run_policy",
            "sha256_action_policy_v4",
            "a" * 64,
            RepositoryVersion("b" * 40, True),
            0,
            128,
            1,
            "combat-agent-phase2-v1",
            (seed,),
            "c" * 64,
            "f" * 64,
            (
                RootEntry(
                    root_id,
                    "train",
                    "d" * 64,
                    f"train/roots/{root_id}.json",
                    (f"sim-seed:{seed}",),
                    (seed,),
                ),
            ),
            (),
            digest,
        )

    treatment = manifest("LABELAB0", "1", "e" * 64)
    held_out = manifest("LABELAB1", "2", "1" * 64)
    other = manifest("LABELAB2", "3", "2" * 64)
    path = tmp_path / "authorization.json"
    write_held_out_authorization(
        path,
        treatment_manifest=treatment,
        held_out_manifest=held_out,
        evaluation_seed=20260903,
    )
    execution = _test_execution()
    _require_authorization_bindings(
        path, treatment=treatment, held_out=held_out, execution=execution
    )
    with pytest.raises(ValueError, match="not bound to the treatment"):
        _require_authorization_bindings(
            path, treatment=other, held_out=held_out, execution=execution
        )


def test_student_checkpoint_binding_rejects_wrong_dataset_manifest() -> None:
    treatment = RootManifest(
        6,
        "legal_run_policy",
        "sha256_action_policy_v4",
        "a" * 64,
        RepositoryVersion("b" * 40, True),
        0,
        128,
        1,
        "combat-agent-phase2-v1",
        ("LABELAB0",),
        "c" * 64,
        "f" * 64,
        (
            RootEntry(
                "1" * 64,
                "train",
                "d" * 64,
                "train/roots/" + "1" * 64 + ".json",
                ("sim-seed:LABELAB0",),
                ("LABELAB0",),
            ),
        ),
        (),
        "e" * 64,
    )
    dataset = DatasetManifest(
        1,
        "provenance/root-manifest.json",
        "a" * 64,
        treatment.manifest_digest,
        treatment.cohort_digest,
        (),
        (),
        "train",
        {},
        "b" * 64,
        "beam",
        "1",
        "c" * 64,
        "simulator_legal_v1",
        {},
        RepositoryVersion("d" * 40, True),
        "train/train.jsonl",
        "e" * 64,
        0,
        (),
        "f" * 64,
    )
    payload: dict[str, object] = {
        "dataset_manifest_digest": dataset.manifest_digest,
        "dataset_shard_digest": dataset.shard_digest,
        "cohort_digest": treatment.cohort_digest,
        "root_manifest_digest": treatment.manifest_digest,
        "source_epoch_bundle_digest": treatment.source_epoch_bundle_digest,
    }
    _require_student_dataset_binding(payload, dataset, treatment, "beam student")
    payload["dataset_manifest_digest"] = "0" * 64
    with pytest.raises(ValueError, match="not bound to its dataset manifest"):
        _require_student_dataset_binding(payload, dataset, treatment, "beam student")
    payload["dataset_manifest_digest"] = dataset.manifest_digest
    payload["dataset_shard_digest"] = "0" * 64
    with pytest.raises(ValueError, match="not bound to its dataset shard"):
        _require_student_dataset_binding(payload, dataset, treatment, "beam student")


def test_teacher_checkpoint_binding_rejects_label_time_identity_mismatch() -> None:
    model_state = {"weight": torch.tensor([1.0])}
    payload: dict[str, object] = {
        "model_state": model_state,
        "config_digest": "1" * 64,
        "source_digest": "2" * 64,
        "runtime_identity_digest": "3" * 64,
        "vocabulary_fingerprint": "4" * 64,
        "encoder_contract_digest": "5" * 64,
    }
    file_digest = "6" * 64
    search_config: dict[str, object] = {
        "checkpoint_file_digest": file_digest,
        "checkpoint_model_state_digest": _model_state_digest(model_state),
        "checkpoint_config_digest": payload["config_digest"],
        "source_digest": payload["source_digest"],
        "runtime_identity_digest": payload["runtime_identity_digest"],
        "vocabulary_fingerprint": payload["vocabulary_fingerprint"],
        "encoder_contract_digest": payload["encoder_contract_digest"],
    }
    dataset = DatasetManifest(
        1,
        "provenance/root-manifest.json",
        "a" * 64,
        "b" * 64,
        "c" * 64,
        (),
        (),
        "train",
        {},
        "d" * 64,
        "privileged_puct",
        "synchronous_batch1_v3",
        "e" * 64,
        "simulator_legal_v1",
        search_config,
        RepositoryVersion("f" * 40, True),
        "train/train.jsonl",
        "7" * 64,
        0,
        (),
        "8" * 64,
    )
    _require_teacher_dataset_binding(payload, file_digest, dataset)
    mismatched = replace(
        dataset,
        search_config={**search_config, "checkpoint_file_digest": "9" * 64},
    )
    with pytest.raises(ValueError, match="label-time checkpoint_file_digest"):
        _require_teacher_dataset_binding(payload, file_digest, mismatched)
