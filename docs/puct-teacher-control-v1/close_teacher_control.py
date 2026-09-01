"""Lane-local teacher-control archival close. Does not modify tracked source."""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import sys
from collections import Counter
from pathlib import Path
from typing import Any

WORKTREE = Path("/tmp/sts-puct-scale-final-v2")
LANE = WORKTREE / "target" / "puct-teacher-control-v1"
SCALE_V2 = Path("/home/davton/dev/slay-the-spire/target/puct-distill-scale-v2")
EXPECTED_ROOTS = 565

IMMUTABLE_LANE = {
    "analyze_teacher_control.py": "c575d3000655bae45406b57e268880ebca50032e836d9b3060a00ad0dcba6bdc",
    "bootstrap-cli-attempt.stderr.txt": "25e029243850eec39a4bc50358e5f87c20b81547f1c2803d535fc6ef20461ec8",
    "bootstrap-cli-attempt.stdout.txt": "3a42522290c30927ef959b0a28b50542b94cec50afed5756963cb678495374ef",
    "predeclaration.json": "0c80ebedb90c166f27dccaaf6342f91a3d78c64386e66c31252be4bd04efe71f",
    "predeclaration.sha256": "f65830b37f07eaea8be504cbcce455704aaa06b279c51e8911751340db91ca87",
    "provenance-blocker.json": "13c071e4c46ffb1630d8228ae1740d343464d9c61796f16ccfaf8fae150dca20",
}

IMMUTABLE_SCALE = {
    "development-gameplay.json": "d82a40528f0fa4d3321d9c191cdf712dabd8d6c6a095f5138f8c85bf7cb2ecd6",
    "bootstrap/checkpoint.pt": "5d4535039d957eea7fd281f29d0c1da8ee2f2708395dfe85d90c018be28da889",
    "student/checkpoint.pt": "4ecc701fb5f3d84a07dcd12785efbf19b76bd674468314e1dd288582e3488aa6",
    "roots/root-manifest.json": "a4deecb264ce8bedf865f7f1f34f466c6a409b63b9b5433c438e1b96416c0caf",
}

CHECKPOINT_IDENTITY_KEYS = (
    "source_digest",
    "runtime_identity_digest",
    "root_manifest_digest",
    "cohort_digest",
    "vocabulary_fingerprint",
    "encoder_contract_digest",
    "dataset_manifest_digest",
    "dataset_shard_digest",
    "teacher_search_contract_digest",
    "reward_config_digest",
    "config_digest",
    "global_step",
    "checkpoint_format",
)

FORBIDDEN_SHORTCUTS = (
    "--skip-source-digest",
    "cross-cohort allowance",
    "native rebuild to chase an old digest",
    "checkpoint conversion",
    "root-manifest rewrite",
    "sealed/audit data",
    "retraining",
    "new source arm in this workspace",
)


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_file_digest(path: Path, expected: str, label: str) -> str:
    actual = file_sha256(path)
    if actual != expected:
        raise SystemExit(f"{label} changed: {path} actual={actual} expected={expected}")
    return actual


def verify_immutable_inputs() -> None:
    for name, expected in IMMUTABLE_LANE.items():
        require_file_digest(LANE / name, expected, f"immutable lane artifact {name}")
    for name, expected in IMMUTABLE_SCALE.items():
        require_file_digest(SCALE_V2 / name, expected, f"immutable scale-v2 artifact {name}")


def won(row: dict[str, Any]) -> bool:
    return row["status"] == "won"


def comparable(row: dict[str, Any]) -> bool:
    return row["error"] is None and isinstance(row["terminal_hp"], int)


def classify_status(row: dict[str, Any]) -> str:
    status = row["status"]
    if status not in {"won", "lost", "escaped", "truncated", "error"}:
        raise ValueError(f"unknown episode status {status!r}")
    return status


def arm_accounting(rows: list[dict[str, Any]]) -> dict[str, Any]:
    counts = Counter(classify_status(row) for row in rows)
    denominator = len(rows)
    accounted = sum(counts.values())
    if accounted != denominator:
        raise ValueError("status classification is incomplete")
    return {
        "win_numerator": int(counts["won"]),
        "win_denominator": denominator,
        "not_wins": denominator - int(counts["won"]),
        "lost": int(counts["lost"]),
        "escaped": int(counts["escaped"]),
        "truncations": int(counts["truncated"]),
        "errors": int(counts["error"]),
        "scored_subset_count": sum(1 for row in rows if comparable(row)),
        "scored_subset_is_official_win_denominator": False,
        "truncation_and_error_count_as_not_wins": True,
        "status_counts": {
            "won": int(counts["won"]),
            "lost": int(counts["lost"]),
            "escaped": int(counts["escaped"]),
            "truncated": int(counts["truncated"]),
            "error": int(counts["error"]),
        },
    }


def paired_summary(
    left_rows: list[dict[str, Any]],
    right_rows: list[dict[str, Any]],
    root_ids: list[str],
    left_name: str,
    right_name: str,
) -> dict[str, Any]:
    if len(left_rows) != len(right_rows) or len(left_rows) != len(root_ids):
        raise ValueError("paired arms have different lengths")
    per_root: list[dict[str, Any]] = []
    hp_deltas: list[int] = []
    for root_id, left, right in zip(root_ids, left_rows, right_rows, strict=True):
        left_status = classify_status(left)
        right_status = classify_status(right)
        left_won = won(left)
        right_won = won(right)
        can_compare = comparable(left) and comparable(right)
        hp_delta = int(left["terminal_hp"]) - int(right["terminal_hp"]) if can_compare else None
        if hp_delta is not None:
            hp_deltas.append(hp_delta)
        per_root.append(
            {
                "root_id": root_id,
                "left": left_name,
                "right": right_name,
                "left_status": left_status,
                "right_status": right_status,
                "left_won": left_won,
                "right_won": right_won,
                "win_delta_left_minus_right": int(left_won) - int(right_won),
                "comparable_for_hp": can_compare,
                "hp_delta_left_minus_right": hp_delta,
            }
        )
    return {
        "left": left_name,
        "right": right_name,
        "win_denominator": len(root_ids),
        "left_win_numerator": sum(won(row) for row in left_rows),
        "right_win_numerator": sum(won(row) for row in right_rows),
        "win_numerator_delta_left_minus_right": sum(won(row) for row in left_rows)
        - sum(won(row) for row in right_rows),
        "left_only_wins": sum(won(left) and not won(right) for left, right in zip(left_rows, right_rows, strict=True)),
        "right_only_wins": sum(won(right) and not won(left) for left, right in zip(left_rows, right_rows, strict=True)),
        "both_won": sum(won(left) and won(right) for left, right in zip(left_rows, right_rows, strict=True)),
        "neither_won": sum(
            (not won(left)) and (not won(right)) for left, right in zip(left_rows, right_rows, strict=True)
        ),
        "status_equal": sum(
            left["status"] == right["status"] for left, right in zip(left_rows, right_rows, strict=True)
        ),
        "comparable_hp_delta_left_minus_right": {
            "count": len(hp_deltas),
            "mean": statistics.fmean(hp_deltas) if hp_deltas else None,
            "min": min(hp_deltas) if hp_deltas else None,
            "max": max(hp_deltas) if hp_deltas else None,
            "sum": sum(hp_deltas) if hp_deltas else 0,
            "not_official_win_denominator": True,
        },
        "per_root": per_root,
    }


def identity_payload(path: Path, checkpoint: dict[str, Any]) -> dict[str, Any]:
    payload = {
        "path": str(path),
        "file_sha256": file_sha256(path),
    }
    for key in CHECKPOINT_IDENTITY_KEYS:
        payload[key] = checkpoint[key]
    return payload


def differ(left: object, right: object) -> bool:
    return left != right


def load_checkpoint(path: Path) -> dict[str, Any]:
    import torch

    payload = torch.load(path, map_location="cpu", weights_only=False)
    if type(payload) is not dict:
        raise TypeError(f"{path} is not a mapping checkpoint")
    return payload


def live_identities() -> dict[str, Any]:
    sys.path.insert(0, str(WORKTREE / "python"))
    from sts_sim.rl.training import _digest, _runtime_identity, _source_digest

    runtime = _runtime_identity()
    return {
        "worktree": str(WORKTREE),
        "source_digest": _source_digest(),
        "runtime_identity_digest": _digest(runtime),
        "runtime_identity": runtime,
    }


def load_root_manifest_identities(path: Path) -> dict[str, Any]:
    manifest = json.loads(path.read_text())
    development = [row for row in manifest["roots"] if row["split"] == "development"]
    train = [row for row in manifest["roots"] if row["split"] == "train"]
    return {
        "path": str(path),
        "file_sha256": file_sha256(path),
        "manifest_digest": manifest.get("manifest_digest") or manifest.get("digest"),
        "cohort_digest": manifest["cohort_digest"],
        "development_roots": len(development),
        "train_roots": len(train),
        "requested_seed_count": len(manifest.get("requested_seeds") or []),
    }


def recompute_report_digest(report: dict[str, Any]) -> str:
    body = dict(report)
    body.pop("report_digest", None)
    return digest(body)


def extract_arm_rows(report: dict[str, Any], policy: str) -> list[dict[str, Any]]:
    return [row["policies"][policy] for row in report["per_root"]]


def build_apprentice_report(gameplay: dict[str, Any]) -> dict[str, Any]:
    per_root = gameplay["per_root"]
    if len(per_root) != EXPECTED_ROOTS:
        raise SystemExit(f"development report root count {len(per_root)} != {EXPECTED_ROOTS}")
    if gameplay["materialized_split_root_count"] != EXPECTED_ROOTS:
        raise SystemExit("materialized_split_root_count is not 565")
    root_ids = [row["root_id"] for row in per_root]
    if root_ids != gameplay["root_ids"]:
        raise SystemExit("per_root order disagrees with root_ids")
    arms = {name: extract_arm_rows(gameplay, name) for name in ("random", "beam", "network", "puct")}
    accounting = {name: arm_accounting(rows) for name, rows in arms.items()}
    for name, computed in accounting.items():
        reported = gameplay["aggregates"][name]
        if computed["win_denominator"] != EXPECTED_ROOTS:
            raise SystemExit(f"{name} win_denominator is not 565")
        if computed["win_numerator"] != reported["win_numerator"]:
            raise SystemExit(f"{name} win_numerator disagrees with frozen aggregates")
        if computed["errors"] != reported["errors"]:
            raise SystemExit(f"{name} error count disagrees with frozen aggregates")
        if computed["truncations"] != reported["truncations"]:
            raise SystemExit(f"{name} truncation count disagrees with frozen aggregates")
        if computed["lost"] != reported["lost"]:
            raise SystemExit(f"{name} lost count disagrees with frozen aggregates")
    report: dict[str, Any] = {
        "kind": "apprentice_vs_expert_v1",
        "report_version": 1,
        "diagnostic_only": True,
        "promotion_claim": False,
        "selection_claim": False,
        "epoch_over_epoch_claim": False,
        "comparison_kind": "apprentice_versus_current_expert_on_the_same_565_roots",
        "not_epoch_over_epoch": True,
        "source_report": {
            "path": str(SCALE_V2 / "development-gameplay.json"),
            "file_sha256": IMMUTABLE_SCALE["development-gameplay.json"],
            "report_digest": gameplay["report_digest"],
            "recomputed_report_digest": recompute_report_digest(gameplay),
            "split": gameplay["split"],
            "evaluation_seed": gameplay["evaluation_seed"],
            "checkpoint_file_digest": gameplay["checkpoint_file_digest"],
            "source_digest": gameplay["source_digest"],
            "runtime_identity_digest": gameplay["runtime_identity_digest"],
            "vocabulary_fingerprint": gameplay["vocabulary_fingerprint"],
            "encoder_contract_digest": gameplay["encoder_contract_digest"],
            "root_manifest_digest": gameplay["root_manifest_digest"],
            "cohort_digest": gameplay["cohort_digest"],
        },
        "win_denominator": EXPECTED_ROOTS,
        "denominator_rule": (
            "Official win denominator is all 565 development roots. "
            "status==won is a win; lost, escaped, truncated, and error are not-wins. "
            "The comparable/scored HP subset is not the official win denominator."
        ),
        "arms": accounting,
        "paired": {
            "network_minus_beam": paired_summary(
                arms["network"], arms["beam"], root_ids, "network", "beam"
            ),
            "puct_minus_beam": paired_summary(arms["puct"], arms["beam"], root_ids, "puct", "beam"),
            "network_minus_random": paired_summary(
                arms["network"], arms["random"], root_ids, "network", "random"
            ),
            "puct_minus_random": paired_summary(
                arms["puct"], arms["random"], root_ids, "puct", "random"
            ),
        },
        "primary_comparisons": ["network_minus_beam", "puct_minus_beam"],
        "secondary_diagnostics": ["network_minus_random", "puct_minus_random"],
    }
    if report["source_report"]["report_digest"] != report["source_report"]["recomputed_report_digest"]:
        raise SystemExit("development-gameplay.json report_digest does not recompute")
    for pair in report["paired"].values():
        if pair["win_denominator"] != EXPECTED_ROOTS:
            raise SystemExit("paired comparison dropped roots from the official denominator")
    report["report_digest"] = digest(report)
    return report


def build_void_assessment(
    bootstrap: dict[str, Any],
    student: dict[str, Any],
    live: dict[str, Any],
    manifest: dict[str, Any],
    gameplay: dict[str, Any],
    bootstrap_train: dict[str, Any],
    student_train: dict[str, Any],
) -> dict[str, Any]:
    vocab_match = bootstrap["vocabulary_fingerprint"] == student["vocabulary_fingerprint"]
    encoder_match = bootstrap["encoder_contract_digest"] == student["encoder_contract_digest"]
    if vocab_match and encoder_match:
        raise SystemExit("vocabulary and encoder identities unexpectedly match; stopping")
    mismatches = {
        "checkpoint_file_sha256": differ(bootstrap["file_sha256"], student["file_sha256"]),
        "source_digest_bootstrap_vs_student": differ(
            bootstrap["source_digest"], student["source_digest"]
        ),
        "source_digest_checkpoint_vs_live": differ(student["source_digest"], live["source_digest"]),
        "runtime_identity_digest_bootstrap_vs_student": differ(
            bootstrap["runtime_identity_digest"], student["runtime_identity_digest"]
        ),
        "runtime_identity_digest_checkpoint_vs_live": differ(
            student["runtime_identity_digest"], live["runtime_identity_digest"]
        ),
        "root_manifest_digest": differ(
            bootstrap["root_manifest_digest"], student["root_manifest_digest"]
        ),
        "cohort_digest": differ(bootstrap["cohort_digest"], student["cohort_digest"]),
        "vocabulary_fingerprint": not vocab_match,
        "encoder_contract_digest": not encoder_match,
        "dataset_manifest_digest": differ(
            bootstrap["dataset_manifest_digest"], student["dataset_manifest_digest"]
        ),
        "teacher_search_contract_digest": differ(
            bootstrap["teacher_search_contract_digest"],
            student["teacher_search_contract_digest"],
        ),
    }
    report: dict[str, Any] = {
        "kind": "puct_teacher_control_v1_void_comparison_assessment",
        "report_version": 1,
        "diagnostic_only": True,
        "promotion_claim": False,
        "selection_claim": False,
        "epoch_over_epoch_claim": False,
        "scientifically_uncontrolled": True,
        "unexecutable_under_declared_envelope": True,
        "finding": (
            "The planned bootstrap-versus-student comparison is scientifically uncontrolled "
            "and unexecutable under the declared envelope."
        ),
        "no_epoch_over_epoch_teacher_control_result": (
            "No epoch-over-epoch teacher-control result exists from scale-v2 artifacts."
        ),
        "identities": {
            "bootstrap_checkpoint": bootstrap,
            "student_checkpoint": student,
            "live_worktree": {
                "worktree": live["worktree"],
                "source_digest": live["source_digest"],
                "runtime_identity_digest": live["runtime_identity_digest"],
            },
            "evaluation_root_manifest": manifest,
            "student_development_gameplay": {
                "path": str(SCALE_V2 / "development-gameplay.json"),
                "file_sha256": IMMUTABLE_SCALE["development-gameplay.json"],
                "report_digest": gameplay["report_digest"],
                "source_digest": gameplay["source_digest"],
                "runtime_identity_digest": gameplay["runtime_identity_digest"],
                "vocabulary_fingerprint": gameplay["vocabulary_fingerprint"],
                "encoder_contract_digest": gameplay["encoder_contract_digest"],
                "root_manifest_digest": gameplay["root_manifest_digest"],
                "cohort_digest": gameplay["cohort_digest"],
                "materialized_split_root_count": gameplay["materialized_split_root_count"],
            },
            "bootstrap_train_output": {
                "path": str(SCALE_V2 / "bootstrap-train-output.json"),
                "file_sha256": file_sha256(SCALE_V2 / "bootstrap-train-output.json"),
                "vocabulary_fingerprint": bootstrap_train["vocabulary_fingerprint"],
                "encoder_contract_digest": bootstrap_train["encoder_contract_digest"],
                "runtime_identity_digest": bootstrap_train["runtime_identity_digest"],
            },
            "student_train_output": {
                "path": str(SCALE_V2 / "student-train-output.json"),
                "file_sha256": file_sha256(SCALE_V2 / "student-train-output.json"),
                "vocabulary_fingerprint": student_train["vocabulary_fingerprint"],
                "encoder_contract_digest": student_train["encoder_contract_digest"],
                "runtime_identity_digest": student_train["runtime_identity_digest"],
            },
        },
        "mismatch_flags": mismatches,
        "mismatch_classes": {
            "missing_artifact_preservation": [
                {
                    "id": "native_source_digest_not_reproducible",
                    "class": "missing_artifact_preservation",
                    "checkpoint_source_digest": student["source_digest"],
                    "live_source_digest": live["source_digest"],
                    "note": (
                        "Both frozen checkpoints attest source_digest "
                        f"{student['source_digest']}. The current worktree live source_digest is "
                        f"{live['source_digest']}. The native extension was rebuilt after student "
                        "gameplay; no on-disk native binary reproduces the checkpoint source digest "
                        "when paired with a clean ec0d6c8a repository payload. This is a missing "
                        "preservation/attestation failure, not a reason to skip source-digest checks."
                    ),
                }
            ],
            "legitimate_cohort_and_contract_mismatch": [
                {
                    "id": "root_manifest_and_cohort",
                    "class": "legitimate_cohort_and_contract_mismatch",
                    "bootstrap_root_manifest_digest": bootstrap["root_manifest_digest"],
                    "student_root_manifest_digest": student["root_manifest_digest"],
                    "bootstrap_cohort_digest": bootstrap["cohort_digest"],
                    "student_cohort_digest": student["cohort_digest"],
                    "evaluation_root_manifest_digest": manifest["manifest_digest"],
                    "note": (
                        "Bootstrap was trained on a different root cohort/manifest than the "
                        "scale-v2 student and the 565-root development evaluation. "
                        "evaluate_matched_puct_gameplay requires exact root-manifest digest "
                        "equality for development and would still abort after native identity "
                        "was restored."
                    ),
                },
                {
                    "id": "vocabulary_and_encoder_contract",
                    "class": "legitimate_cohort_and_contract_mismatch",
                    "bootstrap_vocabulary_fingerprint": bootstrap["vocabulary_fingerprint"],
                    "student_vocabulary_fingerprint": student["vocabulary_fingerprint"],
                    "bootstrap_encoder_contract_digest": bootstrap["encoder_contract_digest"],
                    "student_encoder_contract_digest": student["encoder_contract_digest"],
                    "note": (
                        "Vocabulary fingerprint and encoder contract digest differ between the "
                        "delayed-sealed bootstrap and the scale-v2 student. A shared-contract "
                        "epoch-over-epoch comparison is therefore uncontrolled even if rollout "
                        "could be forced."
                    ),
                },
                {
                    "id": "training_dataset_contract",
                    "class": "legitimate_cohort_and_contract_mismatch",
                    "bootstrap_dataset_manifest_digest": bootstrap["dataset_manifest_digest"],
                    "student_dataset_manifest_digest": student["dataset_manifest_digest"],
                    "bootstrap_teacher_search_contract_digest": bootstrap[
                        "teacher_search_contract_digest"
                    ],
                    "student_teacher_search_contract_digest": student[
                        "teacher_search_contract_digest"
                    ],
                    "note": (
                        "Bootstrap and student were not trained under the same dataset or teacher "
                        "search contract. That mismatch is independent of missing native binary "
                        "preservation."
                    ),
                },
            ],
        },
        "shared_but_insufficient": {
            "checkpoint_source_digest_bootstrap_equals_student": bootstrap["source_digest"]
            == student["source_digest"],
            "checkpoint_runtime_identity_digest_bootstrap_equals_student": bootstrap[
                "runtime_identity_digest"
            ]
            == student["runtime_identity_digest"],
            "note": (
                "Matching checkpoint source/runtime identities with each other does not make the "
                "comparison executable against the live worktree, and does not repair cohort, "
                "vocabulary, or encoder mismatch."
            ),
        },
        "envelope_seams_that_would_abort": [
            "python/sts_sim/rl/gameplay.py:evaluate_matched_puct_gameplay source_digest",
            "python/sts_sim/rl/gameplay.py:evaluate_matched_puct_gameplay root_manifest_digest",
            "python/sts_sim/rl/training.py checkpoint vocabulary_fingerprint / encoder_contract_digest",
        ],
        "would_still_block_if_native_restored": True,
        "artifacts_not_created": ["bootstrap-gameplay.json", "teacher-control-report.json"],
        "forbidden_shortcuts_not_taken": list(FORBIDDEN_SHORTCUTS),
        "frozen_evidence_unchanged": True,
    }
    if (
        bootstrap_train["vocabulary_fingerprint"] != bootstrap["vocabulary_fingerprint"]
        or bootstrap_train["encoder_contract_digest"] != bootstrap["encoder_contract_digest"]
        or student_train["vocabulary_fingerprint"] != student["vocabulary_fingerprint"]
        or student_train["encoder_contract_digest"] != student["encoder_contract_digest"]
    ):
        raise SystemExit("train-output JSON disagrees with checkpoint identity fields")
    report["report_digest"] = digest(report)
    return report


def build_next_epoch_predeclaration(
    bootstrap: dict[str, Any], student: dict[str, Any], live: dict[str, Any]
) -> dict[str, Any]:
    report: dict[str, Any] = {
        "kind": "puct_next_epoch_control_predeclaration",
        "report_version": 1,
        "diagnostic_only": False,
        "promotion_claim": False,
        "selection_claim": False,
        "applies_to": "a future frozen source epoch, not scale-v2 artifacts",
        "scale_v2_cannot_satisfy_this_predeclaration": True,
        "required_equalities_for_all_compared_checkpoints": {
            "source_native_bundle": True,
            "root_cohort": True,
            "root_manifest_digest": True,
            "encoder_contract_digest": True,
            "vocabulary_fingerprint": True,
            "runtime_identity_digest": True,
        },
        "required_pair": {
            "pre_iteration_baseline": True,
            "post_distillation_student": True,
            "created_under_the_same_contract": True,
        },
        "causal_attribution_requires": {
            "same_initialization_policy": True,
            "same_training_budget_policy": True,
        },
        "search_arms": {
            "random": {"restore": "independent_from_identical_roots"},
            "beam": {"restore": "independent_from_identical_roots"},
            "network": {"restore": "independent_from_identical_roots", "uses_checkpoint": True},
            "uniform_prior_puct": {
                "restore": "independent_from_identical_roots",
                "role": "equal-budget unguided-search arm",
                "prior": "uniform",
                "equal_transition_budget_with_network_puct": True,
            },
            "network_puct": {
                "restore": "independent_from_identical_roots",
                "uses_checkpoint": True,
                "equal_transition_budget_with_uniform_prior_puct": True,
            },
        },
        "independent_restore_from_identical_roots": True,
        "selection": {
            "split": "development_only",
            "sealed_or_audit_guidance": False,
            "allow_audited_split": False,
        },
        "provenance_enforcement": {
            "skip_source_digest": False,
            "cross_cohort_allowance": False,
            "native_rebuild_to_chase_old_digest": False,
            "checkpoint_conversion": False,
            "root_manifest_rewrite": False,
            "exact_native_binary_must_be_archived_with_the_source_epoch": True,
        },
        "denominator_rule": (
            "Every arm keeps the full development-root denominator. Truncation and error are "
            "not-wins and remain explicitly classified. The scored HP subset is not the official "
            "win denominator."
        ),
        "why_scale_v2_is_invalid_under_this_predeclaration": {
            "bootstrap_vocabulary_fingerprint": bootstrap["vocabulary_fingerprint"],
            "student_vocabulary_fingerprint": student["vocabulary_fingerprint"],
            "bootstrap_encoder_contract_digest": bootstrap["encoder_contract_digest"],
            "student_encoder_contract_digest": student["encoder_contract_digest"],
            "bootstrap_root_manifest_digest": bootstrap["root_manifest_digest"],
            "student_root_manifest_digest": student["root_manifest_digest"],
            "checkpoint_source_digest": student["source_digest"],
            "live_source_digest": live["source_digest"],
        },
        "forbidden_shortcuts": list(FORBIDDEN_SHORTCUTS),
    }
    report["report_digest"] = digest(report)
    return report


def _toy_row(status: str, hp: int | None, error: str | None = None) -> dict[str, Any]:
    return {
        "status": status,
        "accepted_decisions": 1,
        "player_turns": 1,
        "terminal_hp": hp,
        "error": error,
        "truncation_trigger": "max_decisions" if status == "truncated" else None,
    }


def self_test() -> None:
    roots = ["a", "b", "c"]
    network = [_toy_row("won", 10), _toy_row("truncated", 4), _toy_row("lost", 0)]
    beam = [_toy_row("won", 8), _toy_row("won", 7), _toy_row("lost", 0)]
    accounting = arm_accounting(network)
    assert accounting["win_denominator"] == 3
    assert accounting["win_numerator"] == 1
    assert accounting["truncations"] == 1
    assert accounting["not_wins"] == 2
    assert accounting["scored_subset_count"] == 3
    assert accounting["scored_subset_is_official_win_denominator"] is False
    paired = paired_summary(network, beam, roots, "network", "beam")
    assert paired["win_denominator"] == 3
    assert paired["left_win_numerator"] == 1
    assert paired["right_win_numerator"] == 2
    assert paired["win_numerator_delta_left_minus_right"] == -1
    assert paired["per_root"][1]["left_status"] == "truncated"
    assert paired["per_root"][1]["left_won"] is False
    assert paired["per_root"][1]["right_won"] is True
    errored = [_toy_row("error", None, "boom"), _toy_row("won", 5), _toy_row("won", 5)]
    errored_accounting = arm_accounting(errored)
    assert errored_accounting["errors"] == 1
    assert errored_accounting["win_numerator"] == 2
    assert errored_accounting["win_denominator"] == 3
    payload = {"x": 1, "promotion_claim": False}
    assert digest(payload) == digest({"promotion_claim": False, "x": 1})
    matching = {
        "vocabulary_fingerprint": "aa",
        "encoder_contract_digest": "bb",
        "file_sha256": "1",
        "source_digest": "s",
        "runtime_identity_digest": "r",
        "root_manifest_digest": "m",
        "cohort_digest": "c",
        "dataset_manifest_digest": "d",
        "teacher_search_contract_digest": "t",
    }
    try:
        build_void_assessment(
            matching,
            dict(matching),
            {"worktree": "w", "source_digest": "s2", "runtime_identity_digest": "r2"},
            {"manifest_digest": "m", "cohort_digest": "c"},
            {"report_digest": "g", "source_digest": "s", "runtime_identity_digest": "r",
             "vocabulary_fingerprint": "aa", "encoder_contract_digest": "bb",
             "root_manifest_digest": "m", "cohort_digest": "c",
             "materialized_split_root_count": 565},
            matching,
            matching,
        )
    except SystemExit as error:
        assert "unexpectedly match" in str(error)
    else:
        raise AssertionError("matching vocabulary/encoder must stop")
    print("self-test ok")


def write_reports() -> dict[str, str]:
    verify_immutable_inputs()
    bootstrap_ckpt = identity_payload(
        SCALE_V2 / "bootstrap" / "checkpoint.pt",
        load_checkpoint(SCALE_V2 / "bootstrap" / "checkpoint.pt"),
    )
    student_ckpt = identity_payload(
        SCALE_V2 / "student" / "checkpoint.pt",
        load_checkpoint(SCALE_V2 / "student" / "checkpoint.pt"),
    )
    live = live_identities()
    manifest = load_root_manifest_identities(SCALE_V2 / "roots" / "root-manifest.json")
    gameplay = json.loads((SCALE_V2 / "development-gameplay.json").read_text())
    bootstrap_train = json.loads((SCALE_V2 / "bootstrap-train-output.json").read_text())
    student_train = json.loads((SCALE_V2 / "student-train-output.json").read_text())
    void = build_void_assessment(
        bootstrap_ckpt, student_ckpt, live, manifest, gameplay, bootstrap_train, student_train
    )
    apprentice = build_apprentice_report(gameplay)
    next_epoch = build_next_epoch_predeclaration(bootstrap_ckpt, student_ckpt, live)
    outputs = {
        "void-comparison-assessment.json": void,
        "apprentice-vs-expert-report.json": apprentice,
        "next-epoch-control-predeclaration.json": next_epoch,
    }
    written: dict[str, str] = {}
    for name, payload in outputs.items():
        path = LANE / name
        path.write_bytes(canonical_bytes(payload))
        written[name] = file_sha256(path)
    verify_immutable_inputs()
    return written


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.write:
        written = write_reports()
        print(json.dumps(written, indent=2, sort_keys=True))
        return 0
    parser.error("pass --self-test and/or --write")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
