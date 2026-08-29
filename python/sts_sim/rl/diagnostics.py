"""Diagnostics for privileged-teacher disagreement on paired fair roots."""

from __future__ import annotations

import math
from collections import defaultdict
from dataclasses import dataclass

from .records import SymbolicTrainingRecord, _canonical_json, action_descriptor_payload


@dataclass(frozen=True, slots=True)
class TeacherConflictGroup:
    teacher_pair_id: str
    observation_digest: str
    record_count: int
    action_count: int
    mean_policy_entropy: float
    mixture_entropy: float
    jensen_shannon_divergence: float


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
            tuple(_canonical_json(action_descriptor_payload(action)) for action in record.actions)
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
