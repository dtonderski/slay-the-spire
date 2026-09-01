"""Portable archival verifier for the scale-v2 teacher-control close.

Committed JSON under this directory is static evidence. This script checks that
evidence. It does not require /tmp worktrees, scale-v2 checkpoints, or torch.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import sys
from collections import Counter
from pathlib import Path
from typing import Any

ARCHIVE_DIR = Path(__file__).resolve().parent
EXPECTED_ROOTS = 565
CLEAN_TERMINAL = frozenset({"won", "lost", "escaped"})
ALL_STATUSES = frozenset({"won", "lost", "escaped", "truncated", "error"})
VOID_NAME = "void-comparison-assessment.json"
APPRENTICE_NAME = "apprentice-vs-expert-report.json"
NEXT_EPOCH_NAME = "next-epoch-control-predeclaration.json"
TRACKED_ARCHIVE_DECISION = (
    "Track the archival JSON and this verifier under docs/puct-teacher-control-v1/. "
    "The original lane was target-only and never-merge. These files are reviewable "
    "evidence, not a gameplay source change and not an epoch-over-epoch result."
)


def canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode()


def digest(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def recompute_report_digest(report: dict[str, Any]) -> str:
    body = dict(report)
    body.pop("report_digest", None)
    return digest(body)


def won(row: dict[str, Any]) -> bool:
    return row["status"] == "won"


def classify_status(row: dict[str, Any]) -> str:
    status = row["status"]
    if status not in ALL_STATUSES:
        raise ValueError(f"unknown episode status {status!r}")
    return status


def non_error_hp_comparable(row: dict[str, Any]) -> bool:
    return row["error"] is None and isinstance(row["terminal_hp"], int)


def clean_terminal_hp_comparable(row: dict[str, Any]) -> bool:
    return classify_status(row) in CLEAN_TERMINAL and non_error_hp_comparable(row)


def hp_summary(deltas: list[int]) -> dict[str, Any]:
    return {
        "count": len(deltas),
        "mean": statistics.fmean(deltas) if deltas else None,
        "min": min(deltas) if deltas else None,
        "max": max(deltas) if deltas else None,
        "sum": sum(deltas) if deltas else 0,
        "not_official_win_denominator": True,
    }


def arm_accounting(rows: list[dict[str, Any]]) -> dict[str, Any]:
    counts = Counter(classify_status(row) for row in rows)
    denominator = len(rows)
    if sum(counts.values()) != denominator:
        raise ValueError("status classification is incomplete")
    return {
        "win_numerator": int(counts["won"]),
        "win_denominator": denominator,
        "not_wins": denominator - int(counts["won"]),
        "lost": int(counts["lost"]),
        "escaped": int(counts["escaped"]),
        "truncations": int(counts["truncated"]),
        "errors": int(counts["error"]),
        "non_error_hp_count": sum(1 for row in rows if non_error_hp_comparable(row)),
        "clean_terminal_hp_count": sum(
            1 for row in rows if clean_terminal_hp_comparable(row)
        ),
        "non_error_hp_includes_truncated": True,
        "clean_terminal_hp_excludes_truncated_and_error": True,
        "hp_subsets_are_not_official_win_denominator": True,
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
    non_error_deltas: list[int] = []
    clean_deltas: list[int] = []
    for root_id, left, right in zip(root_ids, left_rows, right_rows, strict=True):
        left_status = classify_status(left)
        right_status = classify_status(right)
        left_won = won(left)
        right_won = won(right)
        non_error = non_error_hp_comparable(left) and non_error_hp_comparable(right)
        clean = clean_terminal_hp_comparable(left) and clean_terminal_hp_comparable(
            right
        )
        non_error_delta = (
            int(left["terminal_hp"]) - int(right["terminal_hp"]) if non_error else None
        )
        clean_delta = (
            int(left["terminal_hp"]) - int(right["terminal_hp"]) if clean else None
        )
        if non_error_delta is not None:
            non_error_deltas.append(non_error_delta)
        if clean_delta is not None:
            clean_deltas.append(clean_delta)
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
                "non_error_hp_comparable": non_error,
                "clean_terminal_hp_comparable": clean,
                "non_error_hp_delta_left_minus_right": non_error_delta,
                "clean_terminal_hp_delta_left_minus_right": clean_delta,
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
        "left_only_wins": sum(
            won(left) and not won(right)
            for left, right in zip(left_rows, right_rows, strict=True)
        ),
        "right_only_wins": sum(
            won(right) and not won(left)
            for left, right in zip(left_rows, right_rows, strict=True)
        ),
        "both_won": sum(
            won(left) and won(right)
            for left, right in zip(left_rows, right_rows, strict=True)
        ),
        "neither_won": sum(
            (not won(left)) and (not won(right))
            for left, right in zip(left_rows, right_rows, strict=True)
        ),
        "status_equal": sum(
            left["status"] == right["status"]
            for left, right in zip(left_rows, right_rows, strict=True)
        ),
        "non_error_hp_delta_left_minus_right": hp_summary(non_error_deltas),
        "clean_terminal_hp_delta_left_minus_right": hp_summary(clean_deltas),
        "per_root": per_root,
    }


def _search_cutoff() -> dict[str, Any]:
    return {
        "rule": "first exhausted bound stops search",
        "simulation_budget": 64,
        "transition_budget": 64,
        "both_budgets_must_be_positive": True,
        "episode_not_wins": {
            "max_decisions": 128,
            "max_player_turns": 40,
            "truncated_and_error_are_not_wins": True,
        },
    }


def _terminal_value_semantics() -> dict[str, Any]:
    return {
        "contract": "combat_proxy_v1",
        "applied_to": ["won", "lost", "escaped"],
        "truncated_has_no_combat_proxy_value": True,
        "loss_value": -1.0,
        "win_base": 0.75,
        "escape_base": 0.25,
        "resource_clip": 0.20,
        "value_range": [-1.0, 1.0],
    }


def puct_search_arms() -> dict[str, Any]:
    cutoff = _search_cutoff()
    terminals = _terminal_value_semantics()
    restore = "independent_from_identical_evaluation_roots"
    return {
        "random": {"restore": restore, "uses_checkpoint": False},
        "beam": {"restore": restore, "uses_checkpoint": False},
        "network": {"restore": restore, "uses_checkpoint": True},
        "uniform_prior_network_value_puct": {
            "restore": restore,
            "uses_checkpoint": True,
            "role": "policy-prior ablation, not an unguided baseline",
            "prior": {
                "kind": "uniform",
                "over": "legal public actions",
            },
            "leaf_value": {
                "kind": "learned_value_head",
                "range": [-1.0, 1.0],
                "note": "uses the checkpoint value head; this is not unguided PUCT",
            },
            "terminal_values": terminals,
            "search_cutoff": cutoff,
            "equal_transition_budget_with_other_puct_arms": True,
        },
        "uniform_prior_constant_value_puct": {
            "restore": restore,
            "uses_checkpoint": False,
            "role": "equal-budget unguided-search arm",
            "prior": {
                "kind": "uniform",
                "over": "legal public actions",
            },
            "leaf_value": {
                "kind": "constant_nonlearned",
                "value": 0.0,
                "note": "no network prior and no network value; only combat_proxy_v1 at true terminals",
            },
            "terminal_values": terminals,
            "search_cutoff": cutoff,
            "equal_transition_budget_with_other_puct_arms": True,
        },
        "network_puct": {
            "restore": restore,
            "uses_checkpoint": True,
            "prior": {
                "kind": "learned_policy_head",
                "over": "legal public actions",
            },
            "leaf_value": {
                "kind": "learned_value_head",
                "range": [-1.0, 1.0],
            },
            "terminal_values": terminals,
            "search_cutoff": cutoff,
            "equal_transition_budget_with_other_puct_arms": True,
        },
    }


def build_next_epoch_predeclaration(void: dict[str, Any]) -> dict[str, Any]:
    bootstrap = void["identities"]["bootstrap_checkpoint"]
    student = void["identities"]["student_checkpoint"]
    live = void["identities"]["live_worktree"]
    shared_source = bootstrap["source_digest"] == student["source_digest"]
    shared_runtime = (
        bootstrap["runtime_identity_digest"] == student["runtime_identity_digest"]
    )
    report: dict[str, Any] = {
        "kind": "puct_next_epoch_control_predeclaration",
        "report_version": 2,
        "diagnostic_only": False,
        "promotion_claim": False,
        "selection_claim": False,
        "applies_to": "a future frozen source epoch, not scale-v2 artifacts",
        "scale_v2_cannot_satisfy_this_predeclaration": True,
        "artifact_tracking_decision": TRACKED_ARCHIVE_DECISION,
        "provenance_fields_are_distinct": True,
        "experiments": {
            "controlled_label_treatment_ab": {
                "causal_claim": True,
                "purpose": (
                    "Isolate the teacher-label treatment. The baseline student is trained on "
                    "beam labels; the treatment student is trained on PUCT labels."
                ),
                "training_provenance": {
                    "must_be_identical": [
                        "initialization",
                        "architecture",
                        "training_root_manifest_digest",
                        "training_cohort_digest",
                        "training_budget",
                        "encoder_contract_digest",
                        "vocabulary_fingerprint",
                        "source_native_bundle",
                        "runtime_identity_digest",
                    ],
                    "may_differ": ["label_teacher", "dataset_manifest_digest"],
                    "note": (
                        "Training manifests may differ only because the label teacher differs. "
                        "Training roots, cohort, encoder, vocabulary, init, budget, and native "
                        "bundle must match."
                    ),
                },
                "evaluation_provenance": {
                    "must_be_identical": [
                        "fresh_held_out_evaluation_root_manifest_digest",
                        "evaluation_cohort_digest",
                        "evaluation_seed",
                        "source_native_bundle",
                        "encoder_contract_digest",
                        "vocabulary_fingerprint",
                    ],
                    "must_be_disjoint_from_training_roots": True,
                    "note": (
                        "Both students are evaluated on the same fresh held-out roots. "
                        "Evaluation provenance is not the training manifest."
                    ),
                },
                "students": {
                    "beam_label_student": {"label_teacher": "beam"},
                    "puct_label_student": {"label_teacher": "privileged_puct"},
                },
            },
            "system_iteration_comparison": {
                "causal_claim": False,
                "purpose": (
                    "Compare the previous agent to the new student. Useful, but not a pure "
                    "causal estimate of the label treatment."
                ),
                "training_provenance": {
                    "may_differ": [
                        "training_root_manifest_digest",
                        "training_cohort_digest",
                        "dataset_manifest_digest",
                        "label_teacher",
                        "training_budget",
                        "initialization",
                    ],
                    "note": (
                        "Ordinary expert iteration trains the new student on newly generated "
                        "labels, so training manifests naturally differ from the previous agent."
                    ),
                },
                "evaluation_provenance": {
                    "must_be_identical": [
                        "fresh_held_out_evaluation_root_manifest_digest",
                        "evaluation_cohort_digest",
                        "evaluation_seed",
                        "source_native_bundle",
                    ],
                    "must_be_disjoint_from_both_training_sets": True,
                    "note": "Previous agent and new student run on the same fresh held-out roots.",
                },
            },
        },
        "search_arms": puct_search_arms(),
        "independent_restore_from_identical_evaluation_roots": True,
        "selection": {
            "split": "held_out_evaluation_only",
            "development_selection_only": True,
            "sealed_or_audit_guidance": False,
            "allow_audited_split": False,
        },
        "provenance_enforcement": {
            "skip_source_digest": False,
            "evaluate_on_training_roots": False,
            "native_rebuild_to_chase_old_digest": False,
            "checkpoint_conversion": False,
            "root_manifest_rewrite": False,
            "exact_native_binary_must_be_archived_with_the_source_epoch": True,
        },
        "denominator_rule": (
            "Every arm keeps the full held-out evaluation-root denominator. Truncation and "
            "error are not-wins and remain explicitly classified. Non-error HP and clean "
            "terminal HP subsets are not the official win denominator."
        ),
        "scale_v2_identity_facts": {
            "bootstrap_and_student_share_checkpoint_source_digest": shared_source,
            "bootstrap_and_student_share_checkpoint_runtime_identity_digest": shared_runtime,
            "checkpoint_source_digest": student["source_digest"],
            "live_source_digest": live["source_digest"],
            "bootstrap_training_root_manifest_digest": bootstrap[
                "root_manifest_digest"
            ],
            "student_training_root_manifest_digest": student["root_manifest_digest"],
            "bootstrap_vocabulary_fingerprint": bootstrap["vocabulary_fingerprint"],
            "student_vocabulary_fingerprint": student["vocabulary_fingerprint"],
            "bootstrap_encoder_contract_digest": bootstrap["encoder_contract_digest"],
            "student_encoder_contract_digest": student["encoder_contract_digest"],
            "note": (
                "The two frozen checkpoints share source and runtime identities with each "
                "other. Both differ from the later rebuilt native runtime. They also differ "
                "from each other in training cohort, vocabulary, and encoder contract."
            ),
        },
        "forbidden_shortcuts": [
            "--skip-source-digest",
            "treating training-manifest equality as the evaluation control",
            "calling uniform-prior network-value PUCT an unguided baseline",
            "native rebuild to chase an old digest",
            "checkpoint conversion",
            "root-manifest rewrite",
            "sealed/audit data",
        ],
    }
    report["report_digest"] = digest(report)
    return report


def upgrade_arm_accounting(arm: dict[str, Any]) -> dict[str, Any]:
    counts = arm["status_counts"]
    denominator = int(arm["win_denominator"])
    errors = int(counts["error"])
    truncated = int(counts["truncated"])
    upgraded = dict(arm)
    upgraded.pop("scored_subset_count", None)
    upgraded.pop("scored_subset_is_official_win_denominator", None)
    upgraded["non_error_hp_count"] = denominator - errors
    upgraded["clean_terminal_hp_count"] = denominator - errors - truncated
    upgraded["non_error_hp_includes_truncated"] = True
    upgraded["clean_terminal_hp_excludes_truncated_and_error"] = True
    upgraded["hp_subsets_are_not_official_win_denominator"] = True
    return upgraded


def upgrade_paired(pair: dict[str, Any]) -> dict[str, Any]:
    per_root: list[dict[str, Any]] = []
    non_error_deltas: list[int] = []
    clean_deltas: list[int] = []
    for row in pair["per_root"]:
        left_status = row["left_status"]
        right_status = row["right_status"]
        old_delta = row.get("hp_delta_left_minus_right")
        if old_delta is None:
            old_delta = row.get("non_error_hp_delta_left_minus_right")
        non_error = bool(
            row.get("comparable_for_hp", row.get("non_error_hp_comparable", False))
        )
        clean = (
            left_status in CLEAN_TERMINAL
            and right_status in CLEAN_TERMINAL
            and old_delta is not None
        )
        if non_error and old_delta is not None:
            non_error_deltas.append(int(old_delta))
        if clean:
            clean_deltas.append(int(old_delta))
        per_root.append(
            {
                "root_id": row["root_id"],
                "left": row["left"],
                "right": row["right"],
                "left_status": left_status,
                "right_status": right_status,
                "left_won": row["left_won"],
                "right_won": row["right_won"],
                "win_delta_left_minus_right": row["win_delta_left_minus_right"],
                "non_error_hp_comparable": non_error,
                "clean_terminal_hp_comparable": clean,
                "non_error_hp_delta_left_minus_right": int(old_delta)
                if non_error
                else None,
                "clean_terminal_hp_delta_left_minus_right": int(old_delta)
                if clean
                else None,
            }
        )
    upgraded = {
        key: value
        for key, value in pair.items()
        if key
        not in {
            "per_root",
            "comparable_hp_delta_left_minus_right",
            "non_error_hp_delta_left_minus_right",
            "clean_terminal_hp_delta_left_minus_right",
        }
    }
    upgraded["non_error_hp_delta_left_minus_right"] = hp_summary(non_error_deltas)
    upgraded["clean_terminal_hp_delta_left_minus_right"] = hp_summary(clean_deltas)
    upgraded["per_root"] = per_root
    return upgraded


def upgrade_apprentice_report(report: dict[str, Any]) -> dict[str, Any]:
    body = dict(report)
    body.pop("report_digest", None)
    body["report_version"] = 2
    body["denominator_rule"] = (
        "Official win denominator is all 565 development roots. status==won is a win; "
        "lost, escaped, truncated, and error are not-wins. Non-error HP includes truncated "
        "episodes and is not the official win denominator. Clean terminal HP is "
        "won/lost/escaped only."
    )
    body["artifact_tracking_decision"] = TRACKED_ARCHIVE_DECISION
    body["arms"] = {
        name: upgrade_arm_accounting(arm) for name, arm in body["arms"].items()
    }
    body["paired"] = {
        name: upgrade_paired(pair) for name, pair in body["paired"].items()
    }
    body["report_digest"] = digest(body)
    return body


def upgrade_void_assessment(report: dict[str, Any]) -> dict[str, Any]:
    body = dict(report)
    body.pop("report_digest", None)
    body["report_version"] = 2
    identities = body["identities"]
    bootstrap = identities["bootstrap_checkpoint"]
    student = identities["student_checkpoint"]
    live = identities["live_worktree"]
    body["identity_summary"] = {
        "bootstrap_and_student_share_checkpoint_source_digest": bootstrap[
            "source_digest"
        ]
        == student["source_digest"],
        "bootstrap_and_student_share_checkpoint_runtime_identity_digest": bootstrap[
            "runtime_identity_digest"
        ]
        == student["runtime_identity_digest"],
        "both_checkpoints_differ_from_live_native_source_digest": student[
            "source_digest"
        ]
        != live["source_digest"],
        "both_checkpoints_differ_from_live_runtime_identity_digest": student[
            "runtime_identity_digest"
        ]
        != live["runtime_identity_digest"],
        "they_differ_from_each_other_in": [
            "file_sha256",
            "root_manifest_digest",
            "cohort_digest",
            "vocabulary_fingerprint",
            "encoder_contract_digest",
            "dataset_manifest_digest",
            "teacher_search_contract_digest",
        ],
        "they_do_not_differ_from_each_other_in": [
            "source_digest",
            "runtime_identity_digest",
        ],
        "note": (
            "The two frozen checkpoints share source and runtime identities. The live "
            "rebuilt native runtime matches neither checkpoint. Cohort, vocabulary, and "
            "encoder mismatch between checkpoints is independent of the missing native "
            "binary."
        ),
    }
    body["artifact_tracking_decision"] = TRACKED_ARCHIVE_DECISION
    body["shared_but_insufficient"]["note"] = (
        "The two checkpoints share source_digest and runtime_identity_digest with each "
        "other. That shared checkpoint identity does not match the later rebuilt native "
        "runtime, and it does not repair cohort, vocabulary, or encoder mismatch."
    )
    body["report_digest"] = digest(body)
    return body


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text())
    if type(payload) is not dict:
        raise TypeError(f"{path} is not an object")
    return payload


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def verify_apprentice(report: dict[str, Any]) -> None:
    require(
        report["promotion_claim"] is False, "apprentice promotion_claim must be false"
    )
    require(
        report["selection_claim"] is False, "apprentice selection_claim must be false"
    )
    require(
        report["epoch_over_epoch_claim"] is False,
        "apprentice epoch claim must be false",
    )
    require(
        report["win_denominator"] == EXPECTED_ROOTS,
        "apprentice win_denominator must be 565",
    )
    require(
        report["report_digest"] == recompute_report_digest(report),
        "apprentice report_digest does not recompute",
    )
    for name, arm in report["arms"].items():
        require(
            arm["win_denominator"] == EXPECTED_ROOTS,
            f"{name} win_denominator must be 565",
        )
        require(
            arm["hp_subsets_are_not_official_win_denominator"] is True,
            f"{name} HP subset must not be the official denominator",
        )
        counted = (
            arm["status_counts"]["won"]
            + arm["status_counts"]["lost"]
            + arm["status_counts"]["escaped"]
            + arm["status_counts"]["truncated"]
            + arm["status_counts"]["error"]
        )
        require(counted == EXPECTED_ROOTS, f"{name} status counts must cover 565 roots")
        require(
            arm["win_numerator"] == arm["status_counts"]["won"],
            f"{name} wins must equal status==won",
        )
        require(
            arm["non_error_hp_count"] == EXPECTED_ROOTS - arm["errors"],
            f"{name} non-error HP count is wrong",
        )
        require(
            arm["clean_terminal_hp_count"]
            == EXPECTED_ROOTS - arm["errors"] - arm["truncations"],
            f"{name} clean terminal HP count is wrong",
        )
    required_pairs = {
        "network_minus_beam",
        "puct_minus_beam",
        "network_minus_random",
        "puct_minus_random",
    }
    require(required_pairs <= set(report["paired"]), "missing paired comparisons")
    for name, pair in report["paired"].items():
        require(pair["win_denominator"] == EXPECTED_ROOTS, f"{name} dropped roots")
        require(len(pair["per_root"]) == EXPECTED_ROOTS, f"{name} per_root is not 565")
        reconstructed_left = sum(1 for row in pair["per_root"] if row["left_won"])
        reconstructed_right = sum(1 for row in pair["per_root"] if row["right_won"])
        require(
            reconstructed_left == pair["left_win_numerator"],
            f"{name} left wins disagree with per_root",
        )
        require(
            reconstructed_right == pair["right_win_numerator"],
            f"{name} right wins disagree with per_root",
        )
        non_error_count = sum(
            1 for row in pair["per_root"] if row["non_error_hp_comparable"]
        )
        clean_count = sum(
            1 for row in pair["per_root"] if row["clean_terminal_hp_comparable"]
        )
        require(
            non_error_count == pair["non_error_hp_delta_left_minus_right"]["count"],
            f"{name} non-error HP count disagrees",
        )
        require(
            clean_count == pair["clean_terminal_hp_delta_left_minus_right"]["count"],
            f"{name} clean terminal HP count disagrees",
        )
        require(
            clean_count <= non_error_count,
            f"{name} clean terminal HP must be a subset of non-error HP",
        )
        for row in pair["per_root"]:
            if row["left_status"] == "truncated" or row["right_status"] == "truncated":
                require(
                    row["clean_terminal_hp_comparable"] is False,
                    f"{name} truncated row marked clean terminal",
                )


def verify_void(report: dict[str, Any]) -> None:
    require(report["promotion_claim"] is False, "void promotion_claim must be false")
    require(report["selection_claim"] is False, "void selection_claim must be false")
    require(report["epoch_over_epoch_claim"] is False, "void epoch claim must be false")
    require(
        report["scientifically_uncontrolled"] is True,
        "void must call the comparison uncontrolled",
    )
    require(
        report["unexecutable_under_declared_envelope"] is True,
        "void must call the comparison unexecutable",
    )
    require(
        report["report_digest"] == recompute_report_digest(report),
        "void report_digest does not recompute",
    )
    summary = report["identity_summary"]
    require(
        summary["bootstrap_and_student_share_checkpoint_source_digest"] is True,
        "checkpoints must share source_digest",
    )
    require(
        summary["bootstrap_and_student_share_checkpoint_runtime_identity_digest"]
        is True,
        "checkpoints must share runtime_identity_digest",
    )
    require(
        summary["both_checkpoints_differ_from_live_native_source_digest"] is True,
        "live native source digest must differ from checkpoints",
    )
    require(
        "missing_artifact_preservation" in report["mismatch_classes"],
        "missing preservation class",
    )
    require(
        "legitimate_cohort_and_contract_mismatch" in report["mismatch_classes"],
        "cohort/contract class",
    )
    bootstrap = report["identities"]["bootstrap_checkpoint"]
    student = report["identities"]["student_checkpoint"]
    require(
        bootstrap["vocabulary_fingerprint"] != student["vocabulary_fingerprint"],
        "vocabulary unexpectedly matches",
    )
    require(
        bootstrap["encoder_contract_digest"] != student["encoder_contract_digest"],
        "encoder unexpectedly matches",
    )


def verify_next_epoch(report: dict[str, Any]) -> None:
    require(
        report["promotion_claim"] is False, "next-epoch promotion_claim must be false"
    )
    require(
        report["selection_claim"] is False, "next-epoch selection_claim must be false"
    )
    require(
        report["report_digest"] == recompute_report_digest(report),
        "next-epoch report_digest does not recompute",
    )
    require(
        report["provenance_fields_are_distinct"] is True,
        "training/eval provenance must be distinct",
    )
    experiments = report["experiments"]
    require(
        "controlled_label_treatment_ab" in experiments,
        "missing label-treatment A/B experiment",
    )
    require(
        "system_iteration_comparison" in experiments,
        "missing system-iteration experiment",
    )
    ab_experiment = experiments["controlled_label_treatment_ab"]
    require(
        ab_experiment["causal_claim"] is True, "label A/B must be the causal experiment"
    )
    require(
        "training_root_manifest_digest"
        in ab_experiment["training_provenance"]["must_be_identical"],
        "A/B must share training roots",
    )
    require(
        "fresh_held_out_evaluation_root_manifest_digest"
        in ab_experiment["evaluation_provenance"]["must_be_identical"],
        "A/B must share held-out evaluation roots",
    )
    iteration = experiments["system_iteration_comparison"]
    require(
        iteration["causal_claim"] is False,
        "system iteration must not claim pure causality",
    )
    require(
        "training_root_manifest_digest"
        in iteration["training_provenance"]["may_differ"],
        "system iteration must allow different training manifests",
    )
    arms = report["search_arms"]
    ablation = arms["uniform_prior_network_value_puct"]
    unguided = arms["uniform_prior_constant_value_puct"]
    require(
        ablation["leaf_value"]["kind"] == "learned_value_head",
        "prior ablation must use learned value",
    )
    require(
        unguided["leaf_value"]["kind"] == "constant_nonlearned",
        "unguided arm must use a nonlearned leaf value",
    )
    require(unguided["leaf_value"]["value"] == 0.0, "unguided leaf value must be 0.0")
    require(
        unguided["uses_checkpoint"] is False, "unguided arm must not use a checkpoint"
    )
    require(
        unguided["terminal_values"]["truncated_has_no_combat_proxy_value"] is True,
        "truncated terminals must not receive combat_proxy_v1",
    )
    require(
        unguided["search_cutoff"]["rule"] == "first exhausted bound stops search",
        "unguided cutoff is unspecified",
    )
    facts = report["scale_v2_identity_facts"]
    require(
        facts["bootstrap_and_student_share_checkpoint_source_digest"] is True,
        "shared source",
    )
    require(
        facts["bootstrap_and_student_share_checkpoint_runtime_identity_digest"] is True,
        "shared runtime",
    )


def verify_archive(archive_dir: Path) -> None:
    void = load_json(archive_dir / VOID_NAME)
    apprentice = load_json(archive_dir / APPRENTICE_NAME)
    next_epoch = load_json(archive_dir / NEXT_EPOCH_NAME)
    verify_void(void)
    verify_apprentice(apprentice)
    verify_next_epoch(next_epoch)
    print("verify ok")


def rewrite_archive(archive_dir: Path) -> dict[str, str]:
    void = upgrade_void_assessment(load_json(archive_dir / VOID_NAME))
    apprentice = upgrade_apprentice_report(load_json(archive_dir / APPRENTICE_NAME))
    next_epoch = build_next_epoch_predeclaration(void)
    outputs = {
        VOID_NAME: void,
        APPRENTICE_NAME: apprentice,
        NEXT_EPOCH_NAME: next_epoch,
    }
    written: dict[str, str] = {}
    for name, payload in outputs.items():
        path = archive_dir / name
        path.write_bytes(canonical_bytes(payload))
        written[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    verify_archive(archive_dir)
    return written


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
    assert accounting["non_error_hp_count"] == 3
    assert accounting["clean_terminal_hp_count"] == 2
    paired = paired_summary(network, beam, roots, "network", "beam")
    assert paired["win_denominator"] == 3
    assert paired["non_error_hp_delta_left_minus_right"]["count"] == 3
    assert paired["clean_terminal_hp_delta_left_minus_right"]["count"] == 2
    assert paired["per_root"][1]["left_status"] == "truncated"
    assert paired["per_root"][1]["left_won"] is False
    assert paired["per_root"][1]["clean_terminal_hp_comparable"] is False
    assert paired["per_root"][1]["non_error_hp_comparable"] is True
    errored = [_toy_row("error", None, "boom"), _toy_row("won", 5), _toy_row("won", 5)]
    errored_accounting = arm_accounting(errored)
    assert errored_accounting["errors"] == 1
    assert errored_accounting["non_error_hp_count"] == 2
    assert errored_accounting["clean_terminal_hp_count"] == 2
    payload = {"x": 1, "promotion_claim": False}
    assert digest(payload) == digest({"promotion_claim": False, "x": 1})
    print("self-test ok")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "--verify",
        action="store_true",
        help="read-only check of committed reports in --archive-dir",
    )
    parser.add_argument(
        "--rewrite-archive",
        action="store_true",
        help="upgrade committed reports in --archive-dir, then verify",
    )
    parser.add_argument("--archive-dir", type=Path, default=ARCHIVE_DIR)
    args = parser.parse_args()
    if args.self_test:
        self_test()
        if not args.verify and not args.rewrite_archive:
            return 0
    if args.rewrite_archive:
        written = rewrite_archive(args.archive_dir)
        print(json.dumps(written, indent=2, sort_keys=True))
        return 0
    if args.verify:
        verify_archive(args.archive_dir)
        return 0
    parser.error("pass --self-test, --verify, and/or --rewrite-archive")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
