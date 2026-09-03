from __future__ import annotations

import hashlib
import json
import math
import multiprocessing
import subprocess
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path
from typing import cast

import pytest

from sts_sim.rl.cli import experiment_main
from sts_sim.rl.diagnostics import (
    WinLossScore,
    affine_tanh_win_probability,
    binary_win_loss_calibration,
    calibrate_combat_proxy_win_loss,
)
from sts_sim.rl.experiment import (
    ARTIFACT_INVENTORY_NAME,
    PREDECLARATION_KIND,
    UNDECLARED_POLICY_REPORT_ONLY,
    UNDECLARED_POLICY_STRICT,
    ArtifactIntegrityError,
    ExperimentReproductionError,
    is_mutable_synchronization_path,
    load_experiment_predeclaration,
    normalize_inventory_relative_path,
    parse_sha256sum_inventory,
    reproduce_experiment,
    verify_artifact_integrity,
    write_artifact_inventory,
    write_scientific_artifact,
)

FIXTURE_DIR = Path(__file__).resolve().parent / "fixtures" / "experiment_infra"


def _init_git_fixture(repo: Path) -> None:
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    subprocess.run(["git", "-C", str(repo), "config", "user.email", "test@example.com"], check=True)
    subprocess.run(["git", "-C", str(repo), "config", "user.name", "Test"], check=True)


def _commit_fixture(repo: Path) -> str:
    subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
    subprocess.run(["git", "-C", str(repo), "commit", "-qm", "fixture"], check=True)
    completed = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def _empty_environment() -> dict[str, object]:
    return {
        "runtime_identity_digest": None,
        "encoder_contract_digest": None,
        "vocabulary_fingerprint": None,
        "source_digest": None,
        "cohort_digest": None,
        "root_manifest_digest": None,
        "dataset_manifest_digest": None,
        "checkpoint_file_digest": None,
        "checkpoint_config_digest": None,
    }


def _v1_predeclaration(
    source_commit: str,
    *,
    inputs: list[dict[str, object]] | None = None,
    outputs: list[dict[str, object]] | None = None,
    environment: dict[str, object] | None = None,
) -> dict[str, object]:
    return {
        "kind": PREDECLARATION_KIND,
        "schema_version": 1,
        "name": "unit-experiment",
        "source_commit": source_commit,
        "source_worktree_must_be_clean": True,
        "promotion_claim": False,
        "consumed_evidence_policy": {
            "sealed_test": False,
            "real_trace_audit": False,
            "development_only_for_assessment": True,
        },
        "inputs": [] if inputs is None else inputs,
        "outputs": [] if outputs is None else outputs,
        "environment": _empty_environment() if environment is None else environment,
    }


def _write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False),
        encoding="utf-8",
    )


def _load_fixture(name: str) -> dict[str, object]:
    payload = json.loads((FIXTURE_DIR / name).read_text(encoding="utf-8"))
    return cast(dict[str, object], payload)


def _write_scientific_worker(payload: tuple[str, bytes]) -> tuple[str, str]:
    path, content = payload
    try:
        digest = write_scientific_artifact(Path(path), content)
    except ValueError as error:
        return ("error", str(error))
    return ("ok", digest)


def test_mutable_path_classification_is_timing_only() -> None:
    assert is_mutable_synchronization_path(Path("development-gameplay.time.txt"))
    assert not is_mutable_synchronization_path(Path("wandb/offline-run-1/run.wandb"))
    assert not is_mutable_synchronization_path(Path("wandb/wandb/latest-run"))
    assert not is_mutable_synchronization_path(Path("predeclaration.json"))
    assert not is_mutable_synchronization_path(Path("roots/root-manifest.json"))
    assert not is_mutable_synchronization_path(Path("student/checkpoint.pt"))


def test_write_scientific_artifact_is_idempotent_and_rejects_mutation(tmp_path: Path) -> None:
    path = tmp_path / "report.json"
    first = write_scientific_artifact(path, b'{"ok":true}')
    second = write_scientific_artifact(path, b'{"ok":true}')
    assert first == second
    with pytest.raises(ValueError, match="refusing to mutate scientific artifact"):
        write_scientific_artifact(path, b'{"ok":false}')
    assert path.read_bytes() == b'{"ok":true}'


def test_concurrent_identical_writes_are_exclusive_and_idempotent(tmp_path: Path) -> None:
    path = tmp_path / "artifact.bin"
    content = b"same-bytes"
    jobs = [(str(path), content)] * 16
    with ProcessPoolExecutor(
        max_workers=8, mp_context=multiprocessing.get_context("spawn")
    ) as pool:
        results = list(pool.map(_write_scientific_worker, jobs))
    assert all(status == "ok" for status, _digest in results)
    assert len({digest for _status, digest in results}) == 1
    assert path.read_bytes() == content


def test_concurrent_conflicting_writes_leave_a_single_winner(tmp_path: Path) -> None:
    path = tmp_path / "artifact.bin"
    payloads = [(str(path), b"alpha"), (str(path), b"beta")] * 8
    with ProcessPoolExecutor(
        max_workers=8, mp_context=multiprocessing.get_context("spawn")
    ) as pool:
        results = list(pool.map(_write_scientific_worker, payloads))
    winners = [digest for status, digest in results if status == "ok"]
    errors = [message for status, message in results if status == "error"]
    assert winners
    assert errors
    assert path.read_bytes() in {b"alpha", b"beta"}
    assert hashlib.sha256(path.read_bytes()).hexdigest() in set(winners)


def test_inventory_path_normalization_rejects_escape_and_duplicates() -> None:
    assert normalize_inventory_relative_path("./kept.json") == "kept.json"
    assert normalize_inventory_relative_path("dir/kept.json") == "dir/kept.json"
    with pytest.raises(ValueError, match="absolute"):
        normalize_inventory_relative_path("/tmp/kept.json")
    with pytest.raises(ValueError, match=r"\.\."):
        normalize_inventory_relative_path("../kept.json")
    with pytest.raises(ValueError, match="misleading separators"):
        normalize_inventory_relative_path("dir//kept.json")
    with pytest.raises(ValueError, match="posix separators"):
        normalize_inventory_relative_path("dir\\kept.json")
    with pytest.raises(ValueError, match="more than once"):
        parse_sha256sum_inventory(f"{'a' * 64}  ./kept.json\n{'b' * 64}  kept.json\n")


def test_verify_rejects_absolute_and_traversal_inventory_paths(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "kept.json", {"ok": True})
    write_artifact_inventory(experiment)
    inventory = experiment / ARTIFACT_INVENTORY_NAME
    inventory.write_text(f"{'a' * 64}  /etc/passwd\n", encoding="utf-8")
    with pytest.raises(ValueError, match="absolute"):
        verify_artifact_integrity(experiment)
    inventory.write_text(f"{'a' * 64}  ./../escape.json\n", encoding="utf-8")
    with pytest.raises(ValueError, match=r"\.\."):
        verify_artifact_integrity(experiment)


def test_inventory_skips_timing_and_rejects_scientific_tamper(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    scientific = experiment / "report.json"
    timing = experiment / "run.time.txt"
    _write_json(scientific, {"name": "demo"})
    timing.write_text("1.23\n", encoding="utf-8")
    inventory = write_artifact_inventory(experiment)
    text = inventory.read_text(encoding="utf-8")
    assert "report.json" in text
    assert "run.time.txt" not in text
    report = verify_artifact_integrity(experiment)
    assert report.ok
    assert report.checked == 1
    assert report.undeclared_policy == UNDECLARED_POLICY_REPORT_ONLY
    timing.write_text("9.99\n", encoding="utf-8")
    assert verify_artifact_integrity(experiment).ok
    listed_timing = experiment / ARTIFACT_INVENTORY_NAME
    listed_timing.write_text(
        listed_timing.read_text(encoding="utf-8") + f"{'b' * 64}  ./run.time.txt\n",
        encoding="utf-8",
    )
    skipped = verify_artifact_integrity(experiment)
    assert skipped.ok
    assert "run.time.txt" in skipped.skipped_mutable
    scientific.write_text('{"name":"tampered"}', encoding="utf-8")
    with pytest.raises(ArtifactIntegrityError, match="report.json") as error:
        verify_artifact_integrity(experiment)
    mismatch = error.value.report.mismatches[0]
    assert mismatch.relative_path == "report.json"
    assert mismatch.actual_sha256 != mismatch.declared_sha256


def test_inventory_rejects_missing_declared_scientific_file(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    path = experiment / "kept.json"
    _write_json(path, {"keep": True})
    write_artifact_inventory(experiment)
    gone = experiment / "gone.json"
    _write_json(gone, {"gone": True})
    inventory = experiment / ARTIFACT_INVENTORY_NAME
    digest = "d" * 64
    inventory.write_text(inventory.read_text(encoding="utf-8") + f"{digest}  ./gone.json\n")
    gone.unlink()
    with pytest.raises(ArtifactIntegrityError, match="gone.json") as error:
        verify_artifact_integrity(experiment)
    assert "gone.json" in error.value.report.missing


def test_inventory_refuses_symlinks_including_parent_links(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    payload = experiment / "report.json"
    _write_json(payload, {"ok": True})
    write_artifact_inventory(experiment)
    linked = experiment / "linked.json"
    linked.symlink_to(payload)
    with pytest.raises(ValueError, match="symlink"):
        write_artifact_inventory(experiment)
    with pytest.raises(ArtifactIntegrityError, match="symlink") as error:
        verify_artifact_integrity(experiment)
    assert "linked.json" in error.value.report.symlink_violations

    nested = tmp_path / "nested"
    real_parent = nested / "real"
    real_parent.mkdir(parents=True)
    _write_json(real_parent / "kept.json", {"ok": True})
    (nested / "parent").symlink_to(real_parent)
    with pytest.raises(ValueError, match="symlink"):
        write_artifact_inventory(nested)


def test_undeclared_wandb_symlink_is_a_violation(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "kept.json", {"ok": True})
    write_artifact_inventory(experiment)
    run = experiment / "wandb" / "wandb" / "offline-run-1"
    run.mkdir(parents=True)
    (run / "run.wandb").write_bytes(b"x")
    (experiment / "wandb" / "wandb" / "latest-run").symlink_to(run)
    with pytest.raises(ValueError, match="symlink"):
        write_artifact_inventory(experiment)
    with pytest.raises(ArtifactIntegrityError, match="symlink"):
        verify_artifact_integrity(experiment)


def test_legacy_undeclared_files_are_report_only(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "kept.json", {"keep": True})
    write_artifact_inventory(experiment)
    _write_json(experiment / "extra.json", {"extra": True})
    report = verify_artifact_integrity(experiment)
    assert report.ok
    assert report.undeclared_policy == UNDECLARED_POLICY_REPORT_ONLY
    assert report.undeclared_scientific == ("extra.json",)


def test_v1_undeclared_files_fail_closed(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "predeclaration.json", _v1_predeclaration("a" * 40))
    write_artifact_inventory(experiment)
    _write_json(experiment / "extra.json", {"extra": True})
    with pytest.raises(ArtifactIntegrityError, match="undeclared") as error:
        verify_artifact_integrity(experiment)
    assert error.value.report.undeclared_policy == UNDECLARED_POLICY_STRICT
    assert error.value.report.undeclared_scientific == ("extra.json",)


def test_predeclaration_v1_rejects_unknown_fields_and_promotion(tmp_path: Path) -> None:
    _init_git_fixture(tmp_path)
    (tmp_path / "tracked.txt").write_text("x", encoding="utf-8")
    sha = _commit_fixture(tmp_path)
    payload = _v1_predeclaration(sha)
    payload["extra"] = True
    path = tmp_path / "predeclaration.json"
    _write_json(path, payload)
    with pytest.raises(ValueError, match="missing or unknown fields"):
        load_experiment_predeclaration(path)
    payload.pop("extra")
    payload["promotion_claim"] = True
    _write_json(path, payload)
    with pytest.raises(ExperimentReproductionError, match="promotion"):
        load_experiment_predeclaration(path)
    payload["promotion_claim"] = False
    policy = cast(dict[str, object], payload["consumed_evidence_policy"])
    policy["sealed_test"] = True
    _write_json(path, payload)
    with pytest.raises(ExperimentReproductionError, match="sealed/audit"):
        load_experiment_predeclaration(path)


def test_reproduce_experiment_records_identities_and_rejects_commit_mismatch(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "repo"
    experiment = tmp_path / "exp"
    _init_git_fixture(repo)
    (repo / "tracked.txt").write_text("source", encoding="utf-8")
    sha = _commit_fixture(repo)
    checkpoint = experiment / "checkpoint.pt"
    checkpoint.parent.mkdir(parents=True)
    checkpoint.write_bytes(b"ckpt")
    root_manifest = experiment / "roots" / "root-manifest.json"
    _write_json(root_manifest, {"manifest_digest": "e" * 64, "cohort_digest": "f" * 64})
    declared = _v1_predeclaration(
        sha,
        inputs=[
            {
                "role": "checkpoint",
                "path": str(checkpoint),
                "sha256": hashlib.sha256(b"ckpt").hexdigest(),
            },
            {
                "role": "root_manifest",
                "path": str(root_manifest),
                "sha256": hashlib.sha256(root_manifest.read_bytes()).hexdigest(),
            },
        ],
        environment={
            **_empty_environment(),
            "checkpoint_file_digest": hashlib.sha256(b"ckpt").hexdigest(),
            "root_manifest_digest": "e" * 64,
            "cohort_digest": "f" * 64,
        },
    )
    predeclaration = experiment / "predeclaration.json"
    _write_json(predeclaration, declared)
    report = reproduce_experiment(predeclaration, repository=repo, experiment_dir=experiment)
    assert report["ok"] is True
    assert report["source_commit"] == sha
    assert report["consumed_sealed_or_audit_evidence"] is False
    environment = cast(dict[str, object], report["environment"])
    validation = cast(dict[str, object], environment["validation"])
    fields = cast(dict[str, dict[str, object]], validation["fields"])
    assert fields["checkpoint_file_digest"]["status"] == "independently_hashed"
    assert fields["root_manifest_digest"]["status"] == "artifact_attested"
    assert fields["cohort_digest"]["status"] == "artifact_attested"
    assert fields["source_digest"]["status"] == "not_declared"
    identities = cast(list[dict[str, object]], report["input_identities"])
    declared_digests = cast(dict[str, object], identities[1]["declared_digests"])
    assert declared_digests["manifest_digest"] == "e" * 64
    declared["source_commit"] = "0" * 40
    _write_json(predeclaration, declared)
    with pytest.raises(ExperimentReproductionError, match="source commit mismatch"):
        reproduce_experiment(predeclaration, repository=repo, experiment_dir=experiment)
    (repo / "tracked.txt").write_text("dirty", encoding="utf-8")
    declared["source_commit"] = sha
    _write_json(predeclaration, declared)
    with pytest.raises(ExperimentReproductionError, match="dirty"):
        reproduce_experiment(predeclaration, repository=repo, experiment_dir=experiment)


def test_reproduce_experiment_rejects_input_digest_mismatch(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    experiment = tmp_path / "exp"
    _init_git_fixture(repo)
    (repo / "tracked.txt").write_text("source", encoding="utf-8")
    sha = _commit_fixture(repo)
    artifact = experiment / "roots" / "root-manifest.json"
    _write_json(artifact, {"manifest_digest": "1" * 64})
    declared = _v1_predeclaration(
        sha,
        inputs=[
            {
                "role": "root_manifest",
                "path": "roots/root-manifest.json",
                "sha256": "2" * 64,
            }
        ],
    )
    predeclaration = experiment / "predeclaration.json"
    _write_json(predeclaration, declared)
    with pytest.raises(ExperimentReproductionError, match="digest mismatch"):
        reproduce_experiment(predeclaration, repository=repo, experiment_dir=experiment)


def test_reproduce_experiment_rejects_fabricated_environment_identities(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    experiment = tmp_path / "exp"
    _init_git_fixture(repo)
    (repo / "tracked.txt").write_text("source", encoding="utf-8")
    sha = _commit_fixture(repo)
    artifact = experiment / "kept.json"
    _write_json(artifact, {"ok": True})
    fabricated: dict[str, object] = {key: "f" * 64 for key in _empty_environment()}
    declared = _v1_predeclaration(
        sha,
        inputs=[
            {
                "role": "checkpoint",
                "path": "kept.json",
                "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
            }
        ],
        environment=fabricated,
    )
    predeclaration = experiment / "predeclaration.json"
    _write_json(predeclaration, declared)
    with pytest.raises(ExperimentReproductionError, match="environment identity"):
        reproduce_experiment(predeclaration, repository=repo, experiment_dir=experiment)


def test_reproduce_rejects_self_attested_source_digest(tmp_path: Path) -> None:
    from sts_sim.rl.training import _source_digest

    repo = tmp_path / "repo"
    experiment = tmp_path / "exp"
    _init_git_fixture(repo)
    (repo / "tracked.txt").write_text("source", encoding="utf-8")
    sha = _commit_fixture(repo)
    fabricated = "f" * 64
    artifact = experiment / "report.json"
    _write_json(artifact, {"source_digest": fabricated, "runtime_identity_digest": fabricated})
    declared = _v1_predeclaration(
        sha,
        inputs=[
            {
                "role": "evaluation_report",
                "path": "report.json",
                "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
            }
        ],
        environment={**_empty_environment(), "source_digest": fabricated},
    )
    predeclaration = experiment / "predeclaration.json"
    _write_json(predeclaration, declared)
    with pytest.raises(ExperimentReproductionError, match="environment identity"):
        reproduce_experiment(predeclaration, repository=repo, experiment_dir=experiment)
    declared["environment"] = {**_empty_environment(), "source_digest": _source_digest()}
    _write_json(predeclaration, declared)
    report = reproduce_experiment(predeclaration, repository=repo, experiment_dir=experiment)
    environment = cast(dict[str, object], report["environment"])
    validation = cast(dict[str, object], environment["validation"])
    fields = cast(dict[str, dict[str, object]], validation["fields"])
    assert fields["source_digest"]["status"] == "matched_live"
    assert fields["source_digest"]["observed_artifact"] == [fabricated]


def test_unknown_predeclaration_kind_fails_closed(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    experiment = tmp_path / "exp"
    _init_git_fixture(repo)
    (repo / "tracked.txt").write_text("source", encoding="utf-8")
    sha = _commit_fixture(repo)
    predeclaration = experiment / "predeclaration.json"
    _write_json(predeclaration, {"name": "legacy-run", "source_commit": sha})
    with pytest.raises(ValueError, match="unsupported experiment predeclaration"):
        reproduce_experiment(predeclaration, repository=repo, experiment_dir=experiment)


def test_affine_win_probability_and_exact_brier() -> None:
    assert affine_tanh_win_probability(-1.0) == 0.0
    assert affine_tanh_win_probability(1.0) == 1.0
    assert affine_tanh_win_probability(0.0) == 0.5
    with pytest.raises(ValueError, match="finite"):
        affine_tanh_win_probability(math.inf)
    perfect = binary_win_loss_calibration(
        (
            WinLossScore("win", 1.0, True, "won"),
            WinLossScore("loss", -1.0, False, "lost"),
        ),
        unit="labeled_decision",
    )
    assert perfect["scored_denominator"] == 2
    assert perfect["subset_win_rate"] == 0.5
    assert perfect["base_rate_scope"] == "scored_subset"
    assert perfect["brier_score"] == 0.0
    assert perfect["log_loss_clipped_numerator"] == 2
    assert cast(float, perfect["log_loss"]) < 1e-10
    chance = binary_win_loss_calibration(
        (
            WinLossScore("a", 0.0, True, "won"),
            WinLossScore("b", 0.0, False, "lost"),
        ),
        unit="labeled_decision",
    )
    assert chance["brier_score"] == 0.25
    assert chance["discrimination"] == "no_better_than_base_rate"


def test_calibration_official_denominators_and_coverage_bounds() -> None:
    static = _load_fixture("static_win_loss.json")
    gameplay = _load_fixture("gameplay_win_loss.json")
    report = calibrate_combat_proxy_win_loss(static_report=static, gameplay_report=gameplay)
    assert report["kind"] == "combat_proxy_v1_binary_win_loss_calibration"
    assert report["report_version"] == 4
    inputs = cast(list[dict[str, object]], report["inputs"])
    assert [item["role"] for item in inputs] == ["static_report", "gameplay_report"]
    for item in inputs:
        digest = item["sha256"]
        assert type(digest) is str
        assert len(digest) == 64
    assert report["within_outcome_resolution"] == "not_evaluated"
    assert "does not evaluate or claim calibration of within-outcome" in str(
        report["interpretation"]
    )
    labeled = cast(dict[str, object], report["labeled_decision"])
    labeled_accounting = cast(dict[str, object], labeled["accounting"])
    assert labeled["official_denominator"] == 5
    assert labeled["official_win_numerator"] == 2
    assert labeled["scored_denominator"] == 4
    assert labeled["missing_prediction_numerator"] == 1
    assert labeled_accounting["included_truncated"] == 1
    assert labeled_accounting["included_error"] == 1
    assert labeled_accounting["missing_nonfinite"] == 1
    assert (
        cast(int, labeled_accounting["scored"])
        + cast(int, labeled_accounting["missing_prediction"])
        == 5
    )
    assert labeled["subset_win_rate"] == 0.25
    assert labeled["official_win_rate"] == 0.4
    assert labeled["brier_score_coverage_best"] != labeled["brier_score_coverage_worst"]
    gameplay_metrics = cast(dict[str, object], report["gameplay_root"])
    gameplay_accounting = cast(dict[str, object], gameplay_metrics["accounting"])
    assert gameplay_metrics["official_denominator"] == 4
    assert gameplay_metrics["official_win_numerator"] == 1
    assert gameplay_metrics["scored_denominator"] == 3
    assert gameplay_metrics["missing_prediction_numerator"] == 1
    assert gameplay_accounting["included_truncated"] == 1
    assert gameplay_accounting["requested_root_source"] == "root_ids"
    join = cast(dict[str, object], gameplay_accounting["join"])
    assert join["rule"] == "unproven_v4_file_order"
    assert "cannot prove first-decision identity" in str(join["limitation"])
    chosen = {
        row["root_id"]: row["record_id"]
        for row in cast(list[dict[str, object]], join["chosen_joins"])
    }
    assert chosen["root-a"] == "win"
    assert chosen["root-c"] == "trunc"
    bins = cast(list[dict[str, object]], gameplay_metrics["reliability_bins"])
    assert sum(cast(int, row["actual_win_denominator"]) for row in bins) == 3
    assert (
        report["report_digest"]
        == calibrate_combat_proxy_win_loss(static_report=static, gameplay_report=gameplay)[
            "report_digest"
        ]
    )


def test_gameplay_join_uses_minimum_decision_index_when_present() -> None:
    static = _load_fixture("static_first_decision.json")
    gameplay = _load_fixture("gameplay_first_decision.json")
    report = calibrate_combat_proxy_win_loss(static_report=static, gameplay_report=gameplay)
    gameplay_metrics = cast(dict[str, object], report["gameplay_root"])
    accounting = cast(dict[str, object], gameplay_metrics["accounting"])
    join = cast(dict[str, object], accounting["join"])
    assert join["rule"] == "min_decision_index"
    assert join["limitation"] is None
    chosen = cast(list[dict[str, object]], join["chosen_joins"])
    assert chosen == [
        {"root_id": "root-a", "record_id": "first", "decision_index": 0},
    ]
    assert gameplay_metrics["predicted_value_mean"] == 0.1


def test_cli_verify_and_calibrate(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "report.json", {"ok": True})
    write_artifact_inventory(experiment)
    assert experiment_main(["verify", "--experiment-dir", str(experiment)]) == 0
    verify_payload = json.loads(capsys.readouterr().out)
    assert verify_payload["ok"] is True
    static = {
        "per_record": [
            {
                "record_id": "r1",
                "root_id": "root-1",
                "status": "won",
                "truncated": False,
                "value_target_mask": True,
                "predicted_value": 0.9,
            },
            {
                "record_id": "r2",
                "root_id": "root-2",
                "status": "lost",
                "truncated": False,
                "value_target_mask": True,
                "predicted_value": -0.9,
            },
        ]
    }
    static_path = tmp_path / "static.json"
    output = tmp_path / "calibration.json"
    _write_json(static_path, static)
    assert (
        experiment_main(["calibrate", "--static", str(static_path), "--output", str(output)]) == 0
    )
    capsys.readouterr()
    written = json.loads(output.read_text(encoding="utf-8"))
    assert written["primary_unit"] == "labeled_decision"
    assert written["discrimination"] == "positive"
    calibrate_inputs = cast(list[dict[str, object]], written["inputs"])
    assert calibrate_inputs[0]["path"] == str(static_path)
    assert calibrate_inputs[0]["sha256"] == hashlib.sha256(static_path.read_bytes()).hexdigest()
    static["per_record"][0]["predicted_value"] = 0.1
    _write_json(static_path, static)
    assert (
        experiment_main(["calibrate", "--static", str(static_path), "--output", str(output)]) == 1
    )
    mutate_payload = json.loads(capsys.readouterr().out)
    assert mutate_payload["ok"] is False
    assert "refusing to mutate" in str(mutate_payload["error"])


def test_write_scientific_artifact_refuses_symlinked_parent(tmp_path: Path) -> None:
    outside = tmp_path / "outside"
    outside.mkdir()
    experiment = tmp_path / "exp"
    experiment.mkdir()
    linked = experiment / "linked"
    linked.symlink_to(outside)
    with pytest.raises(ValueError, match="symlink parent"):
        write_scientific_artifact(linked / "artifact.json", b"secret")
    assert not (outside / "artifact.json").exists()


def test_wandb_parent_directory_symlink_is_a_violation(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "kept.json", {"ok": True})
    outside = tmp_path / "wandb-outside"
    run = outside / "offline-run-1"
    run.mkdir(parents=True)
    (run / "run.wandb").write_bytes(b"x")
    (experiment / "wandb").symlink_to(outside)
    with pytest.raises(ValueError, match="symlink"):
        write_artifact_inventory(experiment)
    digest = hashlib.sha256((experiment / "kept.json").read_bytes()).hexdigest()
    (experiment / ARTIFACT_INVENTORY_NAME).write_text(f"{digest}  ./kept.json\n", encoding="utf-8")
    with pytest.raises(ArtifactIntegrityError, match="symlink"):
        verify_artifact_integrity(experiment)


def test_verify_rejects_inventory_outside_experiment(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "kept.json", {"ok": True})
    write_artifact_inventory(experiment)
    outside = tmp_path / "outside.sha256"
    outside.write_text((experiment / ARTIFACT_INVENTORY_NAME).read_text(encoding="utf-8"))
    with pytest.raises(ValueError, match="under the experiment"):
        verify_artifact_integrity(experiment, inventory_path=outside)


def test_verify_rejects_lexically_escaped_inventory(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "kept.json", {"ok": True})
    write_artifact_inventory(experiment)
    outside = tmp_path / "outside"
    outside.write_text((experiment / ARTIFACT_INVENTORY_NAME).read_text(encoding="utf-8"))
    with pytest.raises(ValueError, match="under the experiment"):
        verify_artifact_integrity(experiment, inventory_path=experiment / ".." / "outside")


def test_malformed_v1_predeclaration_does_not_downgrade_to_legacy(tmp_path: Path) -> None:
    experiment = tmp_path / "exp"
    _write_json(experiment / "kept.json", {"ok": True})
    payload = _v1_predeclaration("a" * 40)
    payload["extra"] = True
    _write_json(experiment / "predeclaration.json", payload)
    write_artifact_inventory(experiment)
    _write_json(experiment / "extra.json", {"extra": True})
    with pytest.raises(ValueError, match="missing or unknown fields"):
        verify_artifact_integrity(experiment)


def test_calibration_rejects_unknown_status_and_extra_roots() -> None:
    static = {
        "per_record": [
            {
                "record_id": "r1",
                "root_id": "root-a",
                "status": "weird",
                "truncated": False,
                "predicted_value": 0.1,
            }
        ]
    }
    with pytest.raises(ValueError, match="unknown terminal status"):
        calibrate_combat_proxy_win_loss(static_report=static)
    error_only = {
        "per_record": [
            {
                "record_id": "r1",
                "root_id": "root-a",
                "status": "error",
                "truncated": False,
                "predicted_value": 0.1,
            }
        ]
    }
    report = calibrate_combat_proxy_win_loss(static_report=error_only)
    labeled = cast(dict[str, object], report["labeled_decision"])
    accounting = cast(dict[str, object], labeled["accounting"])
    assert accounting["included_error"] == 1
    assert labeled["official_win_numerator"] == 0
    indexed = {
        "per_record": [
            {
                "record_id": "later",
                "root_id": "root-a",
                "status": "won",
                "truncated": False,
                "predicted_value": 0.9,
                "decision_index": 1,
            }
        ]
    }
    gameplay = {
        "root_ids": ["root-a"],
        "per_root": [
            {
                "root_id": "root-a",
                "policies": {"network": {"status": "won", "error": None}},
            }
        ],
    }
    with pytest.raises(ValueError, match="decision_index 0"):
        calibrate_combat_proxy_win_loss(static_report=indexed, gameplay_report=gameplay)
    extra_root = {
        "root_ids": ["root-a"],
        "per_root": [
            {
                "root_id": "root-a",
                "policies": {"network": {"status": "won", "error": None}},
            },
            {
                "root_id": "root-extra",
                "policies": {"network": {"status": "lost", "error": None}},
            },
        ],
    }
    valid_static = {
        "per_record": [
            {
                "record_id": "first",
                "root_id": "root-a",
                "status": "won",
                "truncated": False,
                "predicted_value": 0.1,
                "decision_index": 0,
            }
        ]
    }
    with pytest.raises(ValueError, match="absent from root_ids"):
        calibrate_combat_proxy_win_loss(static_report=valid_static, gameplay_report=extra_root)


def test_calibration_rejects_payload_path_byte_disagreement(tmp_path: Path) -> None:
    analyzed = {
        "per_record": [
            {
                "record_id": "r1",
                "root_id": "root-a",
                "status": "won",
                "truncated": False,
                "predicted_value": 0.9,
            }
        ]
    }
    on_disk = {
        "per_record": [
            {
                "record_id": "r1",
                "root_id": "root-a",
                "status": "lost",
                "truncated": False,
                "predicted_value": -0.9,
            }
        ]
    }
    static_path = tmp_path / "static.json"
    _write_json(static_path, on_disk)
    raw = static_path.read_bytes()
    with pytest.raises(ValueError, match="does not match the hashed bytes"):
        calibrate_combat_proxy_win_loss(
            static_report=analyzed,
            static_path=static_path,
            static_bytes=raw,
        )
    with pytest.raises(ValueError, match="parsed bytes"):
        calibrate_combat_proxy_win_loss(static_report=on_disk, static_path=static_path)
    parsed = json.loads(raw)
    report = calibrate_combat_proxy_win_loss(
        static_report=parsed,
        static_path=static_path,
        static_bytes=raw,
    )
    inputs = cast(list[dict[str, object]], report["inputs"])
    assert inputs[0]["sha256"] == hashlib.sha256(raw).hexdigest()
    assert inputs[0]["path"] == str(static_path)
