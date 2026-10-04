"""Reference GAE over macro transitions, independent of Torch and simulator state.

Discount and lambda clocks are explicit. For undiscounted terminal win probability,
use discount=1 between decisions and discount=0 at genuine terminals. A collector
cutoff is a boundary with next_value bootstrap, never a synthetic death.
"""

import math
from collections.abc import Sequence
from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class Transition:
    reward: float
    value: float
    next_value: float
    discount: float
    trace_decay: float
    boundary: bool = False
    terminated: bool = False


@dataclass(frozen=True, slots=True)
class Targets:
    advantages: tuple[float, ...]
    returns: tuple[float, ...]


def generalized_advantages(transitions: Sequence[Transition]) -> Targets:
    """Boundary cuts trace recursion but preserves nonterminal bootstrapping.

    A non-boundary successor must have the exported next_value and be present in
    this contiguous trajectory. All records must be validated before learning.
    """
    for index, row in enumerate(transitions):
        if not all(
            math.isfinite(x)
            for x in (
                row.reward,
                row.value,
                row.next_value,
                row.discount,
                row.trace_decay,
            )
        ):
            raise ValueError("Targets require finite inputs")
        if not 0 <= row.discount <= 1 or not 0 <= row.trace_decay <= 1:
            raise ValueError("Discount and trace decay must be in [0, 1]")
        if row.terminated and (
            not row.boundary or row.discount != 0 or row.next_value != 0
        ):
            raise ValueError("Terminals must cut traces and have zero bootstrap")
        if not row.boundary:
            if index + 1 == len(transitions):
                raise ValueError("Final collected transition must mark a boundary")
            if row.next_value != transitions[index + 1].value:
                raise ValueError("Contiguous value predictions do not match")
    advantages = [0.0] * len(transitions)
    running = 0.0
    for index in range(len(transitions) - 1, -1, -1):
        row = transitions[index]
        delta = row.reward + row.discount * row.next_value - row.value
        running = delta + (
            0.0 if row.boundary else row.discount * row.trace_decay * running
        )
        if not math.isfinite(running) or not math.isfinite(running + row.value):
            raise ValueError(
                "Targets overflowed; do not send non-finite labels to learning"
            )
        advantages[index] = running
    return Targets(
        tuple(advantages),
        tuple(
            advantage + row.value
            for advantage, row in zip(advantages, transitions, strict=True)
        ),
    )
