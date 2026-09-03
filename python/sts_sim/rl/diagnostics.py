"""Diagnostics for teacher disagreement and honest binary win/loss calibration."""

from __future__ import annotations

import hashlib
import json
import math
from collections import defaultdict
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from .provenance import canonical_bytes
from .records import SymbolicTrainingRecord, action_descriptor_payload
from .rewards import COMBAT_PROXY_V1

AFFINE_TANH_WIN_PROBABILITY_MAP = "affine_tanh_unit_interval_v1"
COMBAT_PROXY_WIN_LOSS_INTERPRETATION = (
    "terminal combat_proxy_v1 is a composite survival-dominant score with disjoint "
    "win/escape/loss bands and within-band resource terms. This diagnostic treats the "
    "tanh value-head output as a binary win/loss discriminator via an affine map onto "
    "[0, 1]. It does not evaluate or claim calibration of within-outcome HP, gold, or "
    "potion resolution."
)
_TERMINAL_STATUSES = frozenset({"won", "lost", "escaped"})


@dataclass(frozen=True, slots=True)
class TeacherConflictGroup:
    teacher_pair_id: str
    observation_digest: str
    record_count: int
    action_count: int
    mean_policy_entropy: float
    mixture_entropy: float
    jensen_shannon_divergence: float


@dataclass(frozen=True, slots=True)
class WinLossScore:
    identity: str
    predicted_value: float
    won: bool
    status: str


def _entropy(probabilities: list[float]) -> float:
    return -sum(value * math.log(value) for value in probabilities if value > 0.0)


def teacher_conflict_report(
    records: list[SymbolicTrainingRecord],
) -> tuple[TeacherConflictGroup, ...]:
    """Compare labels only within explicitly paired hidden-equivalent roots.

    Actual use requires generating hidden-equivalent roots and running the
    privileged teacher once on each. Natural trajectory digest collisions are
    not a substitute for that paired-root experiment.
    """

    grouped: defaultdict[str, list[SymbolicTrainingRecord]] = defaultdict(list)
    for record in records:
        if record.teacher_pair_id is not None:
            grouped[record.teacher_pair_id].append(record)
    result: list[TeacherConflictGroup] = []
    for pair_id, group in sorted(grouped.items()):
        root_ids = [record.root_id for record in group]
        if len(group) < 2:
            raise ValueError("teacher pair must contain at least two distinct roots")
        if len(root_ids) != len(set(root_ids)):
            raise ValueError("teacher pair must contain exactly one policy per root")
        if len(set(root_ids)) < 2:
            raise ValueError("teacher pair must contain at least two distinct roots")
        digests = {record.observation_digest for record in group}
        if len(digests) != 1:
            raise ValueError("teacher pair observations do not share one fair digest")
        ordered_actions = [
            tuple(canonical_bytes(action_descriptor_payload(action)).decode() for action in record.actions)
            for record in group
        ]
        if any(actions != ordered_actions[0] for actions in ordered_actions[1:]):
            raise ValueError("teacher pair legal action descriptors differ")
        policies: list[list[float]] = []
        for record in group:
            total = sum(record.teacher_visit_counts)
            policies.append([count / total for count in record.teacher_visit_counts])
        mixture = [
            sum(policy[i] for policy in policies) / len(policies)
            for i in range(len(ordered_actions[0]))
        ]
        mean_entropy = sum(_entropy(policy) for policy in policies) / len(policies)
        mixture_entropy = _entropy(mixture)
        result.append(
            TeacherConflictGroup(
                teacher_pair_id=pair_id,
                observation_digest=next(iter(digests)),
                record_count=len(group),
                action_count=len(ordered_actions[0]),
                mean_policy_entropy=mean_entropy,
                mixture_entropy=mixture_entropy,
                jensen_shannon_divergence=max(0.0, mixture_entropy - mean_entropy),
            )
        )
    return tuple(result)


def _canonical_bytes(payload: object) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _require_mapping(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        raise TypeError(f"{label} must be an object")
    result = cast(dict[str, object], value)
    if any(type(key) is not str for key in result):
        raise TypeError(f"{label} keys must be strings")
    return result


def _require_list(value: object, label: str) -> list[object]:
    if type(value) is not list:
        raise TypeError(f"{label} must be an array")
    return cast(list[object], value)


def _require_string(value: object, label: str) -> str:
    if type(value) is not str or not value:
        raise TypeError(f"{label} must be a nonempty string")
    return value


def affine_tanh_win_probability(predicted_value: float) -> float:
    """Map a tanh-bounded combat_proxy_v1 score onto [0, 1]. Not a fitted calibrator."""

    if type(predicted_value) is not float and type(predicted_value) is not int:
        raise TypeError("predicted value must be numeric")
    value = float(predicted_value)
    if not math.isfinite(value):
        raise ValueError("predicted value must be finite")
    clipped = min(1.0, max(-1.0, value))
    return (clipped + 1.0) / 2.0


def _finite_predicted_value(value: object) -> float | None:
    if type(value) not in {int, float}:
        return None
    converted = float(cast(int | float, value))
    if not math.isfinite(converted):
        return None
    return converted


def _equal_count_bins(
    scores: Sequence[WinLossScore], bin_count: int
) -> tuple[tuple[WinLossScore, ...], ...]:
    if bin_count <= 0:
        raise ValueError("bin_count must be positive")
    count = len(scores)
    if count == 0:
        return ()
    actual_bins = min(bin_count, count)
    ordered = tuple(sorted(scores, key=lambda item: (item.predicted_value, item.identity)))
    bins: list[tuple[WinLossScore, ...]] = []
    for index in range(actual_bins):
        start = (index * count) // actual_bins
        end = ((index + 1) * count) // actual_bins
        bins.append(ordered[start:end])
    return tuple(bins)


def binary_win_loss_calibration(
    observations: Sequence[WinLossScore],
    *,
    unit: str,
    bin_count: int = 10,
    log_loss_epsilon: float = 1e-12,
) -> dict[str, object]:
    """Brier/log-loss reliability for binary win/loss with explicit denominators."""

    if type(unit) is not str or not unit:
        raise TypeError("calibration unit must be a nonempty string")
    if type(log_loss_epsilon) not in {int, float} or not math.isfinite(float(log_loss_epsilon)):
        raise ValueError("log-loss epsilon must be finite")
    epsilon = float(log_loss_epsilon)
    if not 0.0 < epsilon < 0.5:
        raise ValueError("log-loss epsilon must be in (0, 0.5)")
    identities = [item.identity for item in observations]
    if len(identities) != len(set(identities)):
        raise ValueError("calibration observations must have unique identities")
    for item in observations:
        if not math.isfinite(item.predicted_value):
            raise ValueError("calibration observations must be finite")
        if type(item.status) is not str or not item.status:
            raise ValueError("calibration status must be a nonempty string")
        if item.won != (item.status == "won"):
            raise ValueError("won flag must match status == 'won'")
    scored = len(observations)
    if scored == 0:
        return {
            "unit": unit,
            "probability_map": AFFINE_TANH_WIN_PROBABILITY_MAP,
            "scored_denominator": 0,
            "win_numerator": 0,
            "subset_win_rate": None,
            "base_rate": None,
            "base_rate_scope": "scored_subset",
            "predicted_value_mean": None,
            "predicted_win_probability_mean": None,
            "brier_score": None,
            "brier_base_rate_reference": None,
            "brier_constant_half_reference": None,
            "brier_skill_vs_base_rate": None,
            "brier_undefined_reason": "no_scored_observations",
            "log_loss": None,
            "log_loss_epsilon": epsilon,
            "log_loss_clipped_numerator": 0,
            "ece": None,
            "reliability_bins": [],
            "discrimination": "undefined",
        }
    probabilities = [affine_tanh_win_probability(item.predicted_value) for item in observations]
    outcomes = [1.0 if item.won else 0.0 for item in observations]
    win_numerator = int(sum(outcomes))
    base_rate = win_numerator / scored
    brier = sum(
        (prob - outcome) ** 2 for prob, outcome in zip(probabilities, outcomes, strict=True)
    )
    brier /= scored
    brier_base = sum((base_rate - outcome) ** 2 for outcome in outcomes) / scored
    brier_half = sum((0.5 - outcome) ** 2 for outcome in outcomes) / scored
    if brier_base > 0.0:
        skill: float | None = 1.0 - (brier / brier_base)
        skill_reason = None
    elif brier == 0.0:
        skill = None
        skill_reason = "degenerate_base_rate_perfect_predictions"
    else:
        skill = None
        skill_reason = "degenerate_base_rate"
    clipped_count = 0
    log_loss_total = 0.0
    for prob, outcome in zip(probabilities, outcomes, strict=True):
        clipped = min(1.0 - epsilon, max(epsilon, prob))
        if clipped != prob:
            clipped_count += 1
        log_loss_total += -(outcome * math.log(clipped) + (1.0 - outcome) * math.log(1.0 - clipped))
    bins = _equal_count_bins(observations, bin_count)
    reliability: list[dict[str, object]] = []
    ece = 0.0
    for index, group in enumerate(bins):
        group_probs = [affine_tanh_win_probability(item.predicted_value) for item in group]
        group_wins = sum(item.won for item in group)
        denominator = len(group)
        predicted_mean = sum(group_probs) / denominator
        actual_rate = group_wins / denominator
        ece += (denominator / scored) * abs(actual_rate - predicted_mean)
        values = [item.predicted_value for item in group]
        reliability.append(
            {
                "bin_index": index,
                "count": denominator,
                "predicted_value_min": min(values),
                "predicted_value_max": max(values),
                "predicted_value_mean": sum(values) / denominator,
                "predicted_win_probability_mean": predicted_mean,
                "actual_win_numerator": group_wins,
                "actual_win_denominator": denominator,
                "actual_win_rate": actual_rate,
                "reliability_gap": predicted_mean - actual_rate,
            }
        )
    if skill is None:
        discrimination = "undefined"
    elif skill <= 0.0:
        discrimination = "no_better_than_base_rate"
    else:
        discrimination = "positive"
    return {
        "unit": unit,
        "probability_map": AFFINE_TANH_WIN_PROBABILITY_MAP,
        "scored_denominator": scored,
        "win_numerator": win_numerator,
        "subset_win_rate": base_rate,
        "base_rate": base_rate,
        "base_rate_scope": "scored_subset",
        "predicted_value_mean": sum(item.predicted_value for item in observations) / scored,
        "predicted_win_probability_mean": sum(probabilities) / scored,
        "brier_score": brier,
        "brier_base_rate_reference": brier_base,
        "brier_constant_half_reference": brier_half,
        "brier_skill_vs_base_rate": skill,
        "brier_undefined_reason": skill_reason,
        "log_loss": log_loss_total / scored,
        "log_loss_epsilon": epsilon,
        "log_loss_clipped_numerator": clipped_count,
        "ece": ece,
        "reliability_bins": reliability,
        "discrimination": discrimination,
    }


def _empty_official_accounting(rows: int) -> dict[str, object]:
    return {
        "rows": rows,
        "official_denominator": rows,
        "official_win_numerator": 0,
        "official_not_win_numerator": 0,
        "scored": 0,
        "missing_prediction": 0,
        "included_truncated": 0,
        "included_error": 0,
        "missing_error": 0,
        "missing_truncated": 0,
        "missing_nonfinite": 0,
        "unknown_status": 0,
    }


def _official_outcome(
    *,
    status: str,
    truncated: bool,
    error: bool,
) -> tuple[bool, str]:
    if error:
        return False, "error"
    if truncated or status == "truncated":
        return False, "truncated"
    if status == "won":
        return True, "won"
    if status in _TERMINAL_STATUSES:
        return False, status
    return False, "unknown"


def _reconcile_official_accounting(accounting: dict[str, object]) -> None:
    rows = cast(int, accounting["rows"])
    official = cast(int, accounting["official_denominator"])
    wins = cast(int, accounting["official_win_numerator"])
    not_wins = cast(int, accounting["official_not_win_numerator"])
    scored = cast(int, accounting["scored"])
    missing = cast(int, accounting["missing_prediction"])
    if official != rows:
        raise ValueError("official denominator must equal requested rows")
    if wins + not_wins != official:
        raise ValueError("official wins and not-wins must sum to the official denominator")
    if scored + missing != official:
        raise ValueError(
            "scored and missing-prediction counts must sum to the official denominator"
        )


def _coverage_bounds(
    metrics: Mapping[str, object],
    accounting: Mapping[str, object],
) -> dict[str, object]:
    official_n = cast(int, accounting["official_denominator"])
    scored_n = cast(int, accounting["scored"])
    missing = cast(int, accounting["missing_prediction"])
    official_wins = cast(int, accounting["official_win_numerator"])
    epsilon = cast(float, metrics["log_loss_epsilon"])
    brier = metrics["brier_score"]
    log_loss = metrics["log_loss"]
    if official_n == 0:
        best_brier: float | None = None
        worst_brier: float | None = None
        best_log_loss: float | None = None
        worst_log_loss: float | None = None
        official_win_rate: float | None = None
        scored_coverage: float | None = None
    else:
        scored_brier_total = 0.0 if brier is None else float(cast(float, brier)) * scored_n
        scored_log_total = 0.0 if log_loss is None else float(cast(float, log_loss)) * scored_n
        best_brier = scored_brier_total / official_n
        worst_brier = (scored_brier_total + float(missing)) / official_n
        best_log_loss = (scored_log_total + missing * (-math.log(1.0 - epsilon))) / official_n
        worst_log_loss = (scored_log_total + missing * (-math.log(epsilon))) / official_n
        official_win_rate = official_wins / official_n
        scored_coverage = scored_n / official_n
    return {
        "official_denominator": official_n,
        "official_win_numerator": official_wins,
        "official_not_win_numerator": accounting["official_not_win_numerator"],
        "official_win_rate": official_win_rate,
        "scored_coverage": scored_coverage,
        "missing_prediction_numerator": missing,
        "brier_score_coverage_best": best_brier,
        "brier_score_coverage_worst": worst_brier,
        "log_loss_coverage_best": best_log_loss,
        "log_loss_coverage_worst": worst_log_loss,
        "subset_win_rate_note": (
            "subset_win_rate/base_rate are scored-subset rates, not the official win rate"
        ),
        "official_win_rate_note": (
            "official_win_rate uses every requested unit; truncations and errors are not-wins"
        ),
    }


def _decision_index(row: Mapping[str, object], label: str) -> int | None:
    if "decision_index" not in row:
        return None
    value = row["decision_index"]
    if type(value) is not int or value < 0:
        raise TypeError(f"{label}.decision_index must be a nonnegative integer")
    return value


def combat_proxy_observations_from_static_report(
    report: Mapping[str, object],
) -> tuple[tuple[WinLossScore, ...], dict[str, object]]:
    rows = _require_list(report.get("per_record"), "static per_record")
    scores: list[WinLossScore] = []
    accounting = _empty_official_accounting(len(rows))
    seen: set[str] = set()
    for index, raw in enumerate(rows):
        row = _require_mapping(raw, f"static per_record[{index}]")
        identity = _require_string(row.get("record_id"), f"static per_record[{index}].record_id")
        if identity in seen:
            raise ValueError(f"duplicate static record_id {identity}")
        seen.add(identity)
        status = _require_string(row.get("status"), f"static per_record[{index}].status")
        truncated = row.get("truncated") is True or status == "truncated"
        error = status == "error" or row.get("error") is not None
        if status not in _TERMINAL_STATUSES and status not in {"truncated", "error"}:
            raise ValueError(f"unknown terminal status: {status}")
        won, scoring_status = _official_outcome(status=status, truncated=truncated, error=error)
        if scoring_status == "unknown":
            raise ValueError(f"unknown terminal status: {status}")
        if won:
            accounting["official_win_numerator"] = (
                cast(int, accounting["official_win_numerator"]) + 1
            )
        else:
            accounting["official_not_win_numerator"] = (
                cast(int, accounting["official_not_win_numerator"]) + 1
            )
        predicted = _finite_predicted_value(row.get("predicted_value"))
        if predicted is None:
            accounting["missing_prediction"] = cast(int, accounting["missing_prediction"]) + 1
            if error:
                accounting["missing_error"] = cast(int, accounting["missing_error"]) + 1
            elif truncated or status == "truncated":
                accounting["missing_truncated"] = cast(int, accounting["missing_truncated"]) + 1
            else:
                accounting["missing_nonfinite"] = cast(int, accounting["missing_nonfinite"]) + 1
            continue
        if error:
            accounting["included_error"] = cast(int, accounting["included_error"]) + 1
        if truncated or status == "truncated":
            accounting["included_truncated"] = cast(int, accounting["included_truncated"]) + 1
        scores.append(WinLossScore(identity, predicted, won, scoring_status))
    accounting["scored"] = len(scores)
    _reconcile_official_accounting(accounting)
    return tuple(scores), accounting


def _static_first_decision_joins(
    static: Mapping[str, object],
) -> tuple[dict[str, dict[str, object]], dict[str, object]]:
    static_rows = _require_list(static.get("per_record"), "static per_record")
    parsed: list[dict[str, object]] = []
    index_present = 0
    for index, raw in enumerate(static_rows):
        row = _require_mapping(raw, f"static per_record[{index}]")
        decision_index = _decision_index(row, f"static per_record[{index}]")
        if decision_index is not None:
            index_present += 1
        parsed.append(
            {
                "file_order": index,
                "record_id": _require_string(
                    row.get("record_id"), f"static per_record[{index}].record_id"
                ),
                "root_id": _require_string(
                    row.get("root_id"), f"static per_record[{index}].root_id"
                ),
                "predicted": _finite_predicted_value(row.get("predicted_value")),
                "decision_index": decision_index,
            }
        )
    if parsed and index_present not in {0, len(parsed)}:
        raise ValueError("static per_record decision_index is incomplete")
    proven = index_present == len(parsed) and bool(parsed)
    chosen: dict[str, dict[str, object]] = {}
    for item in parsed:
        root_id = cast(str, item["root_id"])
        current = chosen.get(root_id)
        if current is None:
            chosen[root_id] = item
            continue
        if proven:
            current_index = cast(int, current["decision_index"])
            candidate_index = cast(int, item["decision_index"])
            if candidate_index < current_index:
                chosen[root_id] = item
            elif candidate_index == current_index:
                raise ValueError(
                    f"ambiguous first-decision identity for root {root_id}: "
                    "multiple records share the minimum decision_index"
                )
            continue
        if cast(int, item["file_order"]) < cast(int, current["file_order"]):
            chosen[root_id] = item
    if proven:
        for root_id, item in chosen.items():
            if item["decision_index"] != 0:
                raise ValueError(f"root {root_id} first-decision identity is not decision_index 0")
    audit = {
        "rule": "min_decision_index" if proven else "unproven_v4_file_order",
        "limitation": None
        if proven
        else (
            "static per_record rows do not include decision_index; join uses the first matching "
            "root_id in file order and cannot prove first-decision identity"
        ),
        "chosen_joins": [
            {
                "root_id": root_id,
                "record_id": item["record_id"],
                "decision_index": item["decision_index"],
            }
            for root_id, item in sorted(chosen.items())
        ],
    }
    return chosen, audit


def combat_proxy_observations_from_gameplay_report(
    gameplay: Mapping[str, object],
    static: Mapping[str, object],
    *,
    policy: str = "network",
) -> tuple[tuple[WinLossScore, ...], dict[str, object]]:
    if type(policy) is not str or not policy:
        raise TypeError("gameplay policy must be a nonempty string")
    joins, join_audit = _static_first_decision_joins(static)
    per_root = _require_list(gameplay.get("per_root"), "gameplay per_root")
    per_root_by_id: dict[str, dict[str, object]] = {}
    for index, raw in enumerate(per_root):
        row = _require_mapping(raw, f"gameplay per_root[{index}]")
        root_id = _require_string(row.get("root_id"), f"gameplay per_root[{index}].root_id")
        if root_id in per_root_by_id:
            raise ValueError(f"duplicate gameplay root_id {root_id}")
        per_root_by_id[root_id] = row
    root_ids_value = gameplay.get("root_ids")
    if root_ids_value is None:
        requested = [
            _require_string(
                _require_mapping(raw, f"gameplay per_root[{index}]").get("root_id"),
                f"gameplay per_root[{index}].root_id",
            )
            for index, raw in enumerate(per_root)
        ]
        requested_source = "per_root_file_order"
    else:
        requested_list = _require_list(root_ids_value, "gameplay root_ids")
        requested = [
            _require_string(item, f"gameplay root_ids[{index}]")
            for index, item in enumerate(requested_list)
        ]
        requested_source = "root_ids"
        if len(requested) != len(set(requested)):
            raise ValueError("gameplay root_ids must be unique")
        extra = sorted(set(per_root_by_id) - set(requested))
        if extra:
            raise ValueError(
                "gameplay per_root contains roots absent from root_ids: " + ", ".join(extra)
            )
        missing_roots = [root_id for root_id in requested if root_id not in per_root_by_id]
        if missing_roots:
            raise ValueError("gameplay root_ids missing per_root rows: " + ", ".join(missing_roots))
    accounting = _empty_official_accounting(len(requested))
    accounting["requested_root_source"] = requested_source
    accounting["predicted_value_aggregation"] = join_audit["rule"]
    accounting["join"] = join_audit
    materialized = gameplay.get("materialized_split_root_count")
    if type(materialized) is int and materialized != len(requested):
        raise ValueError("materialized_split_root_count does not match requested roots")
    scores: list[WinLossScore] = []
    for root_id in requested:
        row = per_root_by_id.get(root_id)
        join = joins.get(root_id)
        predicted = None if join is None else cast(float | None, join["predicted"])
        if row is None:
            won = False
            scoring_status = "missing_root"
            error = True
            truncated = False
        else:
            policies = _require_mapping(row.get("policies"), f"gameplay root {root_id} policies")
            if policy not in policies:
                raise ValueError(f"gameplay report is missing policy {policy}")
            episode = _require_mapping(policies[policy], f"gameplay root {root_id} policy {policy}")
            status = _require_string(episode.get("status"), f"gameplay {policy} status")
            truncated = status == "truncated"
            error = status == "error" or episode.get("error") is not None
            if status not in _TERMINAL_STATUSES and status not in {"truncated", "error"}:
                raise ValueError(f"unknown terminal status: {status}")
            won, scoring_status = _official_outcome(status=status, truncated=truncated, error=error)
            if scoring_status == "unknown":
                raise ValueError(f"unknown terminal status: {status}")
        if won:
            accounting["official_win_numerator"] = (
                cast(int, accounting["official_win_numerator"]) + 1
            )
        else:
            accounting["official_not_win_numerator"] = (
                cast(int, accounting["official_not_win_numerator"]) + 1
            )
        if predicted is None:
            accounting["missing_prediction"] = cast(int, accounting["missing_prediction"]) + 1
            if error:
                accounting["missing_error"] = cast(int, accounting["missing_error"]) + 1
            elif truncated:
                accounting["missing_truncated"] = cast(int, accounting["missing_truncated"]) + 1
            else:
                accounting["missing_nonfinite"] = cast(int, accounting["missing_nonfinite"]) + 1
            continue
        if error:
            accounting["included_error"] = cast(int, accounting["included_error"]) + 1
        if truncated:
            accounting["included_truncated"] = cast(int, accounting["included_truncated"]) + 1
        scores.append(WinLossScore(root_id, predicted, won, scoring_status))
    accounting["scored"] = len(scores)
    _reconcile_official_accounting(accounting)
    return tuple(scores), accounting


def _json_object_from_bytes(raw: bytes, label: str) -> dict[str, object]:
    try:
        loaded = json.loads(raw)
    except (TypeError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"{label} is not valid JSON") from error
    return _require_mapping(loaded, label)


def _require_payload_matches_bytes(payload: Mapping[str, object], raw: bytes, label: str) -> None:
    parsed = _json_object_from_bytes(raw, label)
    if parsed != dict(payload):
        raise ValueError(f"{label} payload does not match the hashed bytes")


def _input_report_identity(
    payload: Mapping[str, object],
    *,
    role: str,
    path: Path | None,
    raw_bytes: bytes | None,
) -> dict[str, object]:
    if path is not None and raw_bytes is None:
        raise ValueError(f"{role} path requires the exact parsed bytes")
    if raw_bytes is not None:
        _require_payload_matches_bytes(payload, raw_bytes, role)
        sha256 = hashlib.sha256(raw_bytes).hexdigest()
        path_text: str | None = str(path) if path is not None else None
    else:
        sha256 = hashlib.sha256(_canonical_bytes(dict(payload))).hexdigest()
        path_text = None
    report_digest = payload.get("report_digest")
    return {
        "role": role,
        "path": path_text,
        "sha256": sha256,
        "report_digest": report_digest if type(report_digest) is str else None,
    }


def calibrate_combat_proxy_win_loss(
    *,
    static_report: Mapping[str, object] | None = None,
    gameplay_report: Mapping[str, object] | None = None,
    policy: str = "network",
    bin_count: int = 10,
    static_path: Path | None = None,
    gameplay_path: Path | None = None,
    static_bytes: bytes | None = None,
    gameplay_bytes: bytes | None = None,
) -> dict[str, object]:
    """Read-only binary win/loss calibration over static and/or gameplay reports."""

    if static_report is None:
        raise ValueError("combat_proxy_v1 calibration requires a static evaluation report")
    if gameplay_path is not None and gameplay_report is None:
        raise ValueError("gameplay_path requires gameplay_report")
    if gameplay_bytes is not None and gameplay_report is None:
        raise ValueError("gameplay_bytes require gameplay_report")
    static_payload = _require_mapping(dict(static_report), "static_report")
    if static_path is not None and static_bytes is None:
        raise ValueError("static_report path requires the exact parsed bytes")
    if static_bytes is not None:
        _require_payload_matches_bytes(static_payload, static_bytes, "static_report")
        static_payload = _json_object_from_bytes(static_bytes, "static_report")
    labeled, labeled_accounting = combat_proxy_observations_from_static_report(static_payload)
    labeled_metrics = binary_win_loss_calibration(
        labeled, unit="labeled_decision", bin_count=bin_count
    )
    labeled_bundle = {
        "accounting": labeled_accounting,
        **labeled_metrics,
        **_coverage_bounds(labeled_metrics, labeled_accounting),
    }
    gameplay_bundle: dict[str, object] | None = None
    if gameplay_report is not None:
        gameplay_payload = _require_mapping(dict(gameplay_report), "gameplay_report")
        if gameplay_path is not None and gameplay_bytes is None:
            raise ValueError("gameplay_report path requires the exact parsed bytes")
        if gameplay_bytes is not None:
            _require_payload_matches_bytes(gameplay_payload, gameplay_bytes, "gameplay_report")
            gameplay_payload = _json_object_from_bytes(gameplay_bytes, "gameplay_report")
        gameplay_scores, gameplay_accounting = combat_proxy_observations_from_gameplay_report(
            gameplay_payload,
            static_payload,
            policy=policy,
        )
        gameplay_metrics = binary_win_loss_calibration(
            gameplay_scores, unit="gameplay_root", bin_count=bin_count
        )
        gameplay_bundle = {
            "accounting": gameplay_accounting,
            **gameplay_metrics,
            **_coverage_bounds(gameplay_metrics, gameplay_accounting),
        }
    else:
        gameplay_payload = None
    primary = gameplay_bundle if gameplay_bundle is not None else labeled_bundle
    inputs = [
        _input_report_identity(
            static_payload,
            role="static_report",
            path=static_path,
            raw_bytes=static_bytes,
        ),
    ]
    if gameplay_payload is not None:
        inputs.append(
            _input_report_identity(
                gameplay_payload,
                role="gameplay_report",
                path=gameplay_path,
                raw_bytes=gameplay_bytes,
            )
        )
    report: dict[str, object] = {
        "kind": "combat_proxy_v1_binary_win_loss_calibration",
        "report_version": 4,
        "value_target_name": COMBAT_PROXY_V1.name,
        "probability_map": AFFINE_TANH_WIN_PROBABILITY_MAP,
        "interpretation": COMBAT_PROXY_WIN_LOSS_INTERPRETATION,
        "within_outcome_resolution": "not_evaluated",
        "win_band_lower": COMBAT_PROXY_V1.win_base - COMBAT_PROXY_V1.resource_clip,
        "escape_band_center": COMBAT_PROXY_V1.escape_base,
        "loss_value": COMBAT_PROXY_V1.loss_value,
        "policy": policy if gameplay_report is not None else None,
        "primary_unit": primary["unit"],
        "discrimination": primary["discrimination"],
        "inputs": inputs,
        "labeled_decision": labeled_bundle,
        "gameplay_root": gameplay_bundle,
    }
    report["report_digest"] = hashlib.sha256(_canonical_bytes(report)).hexdigest()
    return report
