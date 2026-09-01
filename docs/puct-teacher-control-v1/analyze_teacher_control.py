from __future__ import annotations

import argparse
import hashlib
import json
import statistics
from pathlib import Path
from typing import Any


def canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode()


def digest(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def won(row: dict[str, object]) -> bool:
    return row["status"] == "won"


def comparable(row: dict[str, object]) -> bool:
    return row["error"] is None and isinstance(row["terminal_hp"], int)


def compare_arms(
    bootstrap_rows: list[dict[str, object]], student_rows: list[dict[str, object]]
) -> dict[str, object]:
    if len(bootstrap_rows) != len(student_rows):
        raise ValueError("arm lengths differ")
    hp_deltas: list[int] = []
    bootstrap_only_wins = 0
    student_only_wins = 0
    exact_episode_rows = 0
    status_equal = 0
    for bootstrap, student in zip(bootstrap_rows, student_rows, strict=True):
        bootstrap_win = won(bootstrap)
        student_win = won(student)
        bootstrap_only_wins += bootstrap_win and not student_win
        student_only_wins += student_win and not bootstrap_win
        status_equal += bootstrap["status"] == student["status"]
        exact_episode_rows += bootstrap == student
        if comparable(bootstrap) and comparable(student):
            hp_deltas.append(
                int(student["terminal_hp"]) - int(bootstrap["terminal_hp"])
            )
    return {
        "roots": len(bootstrap_rows),
        "bootstrap_win_numerator": sum(won(row) for row in bootstrap_rows),
        "student_win_numerator": sum(won(row) for row in student_rows),
        "win_denominator": len(bootstrap_rows),
        "student_only_wins": student_only_wins,
        "bootstrap_only_wins": bootstrap_only_wins,
        "status_equal": status_equal,
        "exact_episode_rows": exact_episode_rows,
        "comparable_hp_delta_student_minus_bootstrap": {
            "count": len(hp_deltas),
            "mean": statistics.fmean(hp_deltas) if hp_deltas else None,
            "min": min(hp_deltas) if hp_deltas else None,
            "max": max(hp_deltas) if hp_deltas else None,
            "sum": sum(hp_deltas),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bootstrap", type=Path, required=True)
    parser.add_argument("--student", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    bootstrap = json.loads(args.bootstrap.read_text())
    student = json.loads(args.student.read_text())

    fixed = (
        "split",
        "evaluation_seed",
        "c_puct",
        "simulation_budget",
        "transition_budget",
        "beam_depth",
        "beam_width",
        "max_decisions",
        "max_player_turns",
        "deduplicate_search_states",
        "root_manifest_digest",
        "cohort_digest",
        "source_digest",
        "materialized_split_root_count",
        "requested_seed_count",
        "requested_seeds",
        "root_ids",
    )
    for key in fixed:
        if bootstrap[key] != student[key]:
            raise ValueError(f"control reports disagree on fixed field {key}")
    bootstrap_per_root = bootstrap["per_root"]
    student_per_root = student["per_root"]
    if len(bootstrap_per_root) != len(student_per_root):
        raise ValueError("control reports have different root counts")
    if [row["root_id"] for row in bootstrap_per_root] != [
        row["root_id"] for row in student_per_root
    ]:
        raise ValueError("control reports have different root ordering")

    checkpoint_independent_equal: dict[str, bool] = {}
    for policy in ("random", "beam"):
        checkpoint_independent_equal[policy] = all(
            left["policies"][policy] == right["policies"][policy]
            for left, right in zip(bootstrap_per_root, student_per_root, strict=True)
        )
    if not all(checkpoint_independent_equal.values()):
        raise ValueError("checkpoint-independent policy did not reproduce exactly")

    arms: dict[str, Any] = {}
    for policy in ("network", "puct"):
        bootstrap_rows = [row["policies"][policy] for row in bootstrap_per_root]
        student_rows = [row["policies"][policy] for row in student_per_root]
        arms[policy] = compare_arms(bootstrap_rows, student_rows)

    report: dict[str, Any] = {
        "report_version": 1,
        "kind": "posthoc_teacher_control_v1",
        "diagnostic_only": True,
        "promotion_claim": False,
        "selection_claim": False,
        "audited_evidence_consumed": False,
        "bootstrap_report_digest": bootstrap["report_digest"],
        "student_report_digest": student["report_digest"],
        "bootstrap_checkpoint_file_digest": bootstrap["checkpoint_file_digest"],
        "student_checkpoint_file_digest": student["checkpoint_file_digest"],
        "development_roots": bootstrap["materialized_split_root_count"],
        "checkpoint_independent_policy_rows_byte_equivalent": checkpoint_independent_equal,
        "bootstrap_to_student": arms,
        "interpretation_rule": {
            "both_positive": "student greedy and student PUCT each have more wins than bootstrap counterparts",
            "greedy_only": "distillation improved the fair policy but not the next privileged teacher",
            "puct_only": "search improved but fair policy distillation did not",
            "neither": "do not scale another iteration before revisiting targets or search",
        },
        "interpretation": (
            "both_positive"
            if arms["network"]["student_win_numerator"]
            > arms["network"]["bootstrap_win_numerator"]
            and arms["puct"]["student_win_numerator"]
            > arms["puct"]["bootstrap_win_numerator"]
            else "greedy_only"
            if arms["network"]["student_win_numerator"]
            > arms["network"]["bootstrap_win_numerator"]
            else "puct_only"
            if arms["puct"]["student_win_numerator"]
            > arms["puct"]["bootstrap_win_numerator"]
            else "neither"
        ),
        "caveat": "This is a post-hoc diagnostic on an already-consumed development cohort and cannot select or promote a checkpoint.",
    }
    report["report_digest"] = digest(report)
    args.output.write_bytes(canonical_bytes(report))
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
