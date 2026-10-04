"""Declared terminal objectives; no reward for UI actions or incomplete runs."""

from dataclasses import dataclass
from typing import Literal

Objective = Literal["act1", "act3", "heart"]
ACT1_BOSS_FLOOR = 16
ACT1_CLEAR_BONUS = 5.0
REWARD_PROTOCOL = "act1_terminal_floor16_plus_clear5_v1_other_objectives_binary"


@dataclass(frozen=True)
class RewardParts:
    progress: float
    clear_bonus: float

    @property
    def total(self) -> float:
        return self.progress + self.clear_bonus


def succeeded(objective: Objective, status: str) -> bool:
    if objective == "act1":
        return status == "act1_clear"
    if objective == "act3":
        return status in ("act3_clear", "heart_clear")
    if objective == "heart":
        return status == "heart_clear"
    raise ValueError("Unknown run objective")


def terminal_parts(
    objective: Objective, status: str, furthest_act1_floor: int
) -> RewardParts:
    if furthest_act1_floor < 0:
        raise ValueError("Negative observed floor")
    if objective == "act1":
        if status not in ("death", "act1_clear"):
            raise ValueError("Act-1 reward requires a completed Act-1 episode")
        cleared = status == "act1_clear"
        # An accepted transition into Act 2 completes all Act-1 progress. Floor
        # alone never grants the clear bonus. This does not alter simulator state.
        progress = 1.0 if cleared else min(furthest_act1_floor / ACT1_BOSS_FLOOR, 1.0)
        return RewardParts(progress, ACT1_CLEAR_BONUS if cleared else 0.0)
    if objective not in ("act3", "heart") or status not in (
        "death",
        "act3_clear",
        "heart_clear",
    ):
        raise ValueError("Unknown objective or unfinished/invalid terminal boundary")
    return RewardParts(0.0, float(succeeded(objective, status)))
