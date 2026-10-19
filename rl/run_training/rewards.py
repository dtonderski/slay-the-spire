"""Declared task returns: no targets for invalid/interrupted games or UI farming."""

from dataclasses import dataclass
from typing import Literal

from run_training.progress import RunProgress

Objective = Literal[
    "act1", "act1_binary", "act3", "heart", "heart_progress", "heart_keys_budget"
]
ACT1_BOSS_FLOOR = 16
ACT1_CLEAR_BONUS = 5.0
REWARD_PROTOCOL = "act1_terminal_floor16_plus_clear5_v1_other_objectives_binary"
HEART_REWARD_PROTOCOL = "heart_terminal_floor55_scale3_act2_heart30_v1"
# Declared shaping scale, not an inferred terminal/clear boundary. Native target
# tests: run/reward.rs::corrupt_heart_victory_proceeds_to_true_victory_floor
# reaches Heart combat on 55 and explicit completion on 56 (clipped to 3).
HEART_PROGRESS_FLOOR = 55
HEART_PROGRESS_SCALE = 3.0
ACT_CLEAR_BONUS = 2.0
HEART_CLEAR_BONUS = 30.0
KEY_BONUS = 0.5
ACTION_LIMIT_PENALTY = -20.0
KEYS_BUDGET_PROTOCOL = "heart_floor55x3_act2_key0p5_win30_actionlimit_minus20_v1"


def game_completed(status: str) -> bool:
    return status in ("death", "act1_clear", "act3_clear", "heart_clear")


def reward_protocol(objective: Objective) -> str:
    if objective == "heart_keys_budget":
        return KEYS_BUDGET_PROTOCOL
    if objective == "heart_progress":
        return HEART_REWARD_PROTOCOL
    return "act1_binary_v1" if objective == "act1_binary" else REWARD_PROTOCOL


@dataclass(frozen=True)
class RewardParts:
    progress: float
    clear_bonus: float
    heart_bonus: float = 0.0
    key_bonus: float = 0.0
    action_limit_penalty: float = 0.0

    @property
    def total(self) -> float:
        return (
            self.progress
            + self.clear_bonus
            + self.heart_bonus
            + self.key_bonus
            + self.action_limit_penalty
        )


def succeeded(objective: Objective, status: str) -> bool:
    if objective in ("act1", "act1_binary"):
        return status == "act1_clear"
    if objective == "act3":
        return status in ("act3_clear", "heart_clear")
    if objective in ("heart", "heart_progress", "heart_keys_budget"):
        return status == "heart_clear"
    raise ValueError("Unknown run objective")


def terminal_parts(
    objective: Objective,
    status: str,
    furthest_act1_floor: int,
    *,
    progress: RunProgress | None = None,
) -> RewardParts:
    if furthest_act1_floor < 0:
        raise ValueError("Negative observed floor")
    if objective in ("heart_progress", "heart_keys_budget"):
        valid_statuses = ("death", "act3_clear", "heart_clear")
        if objective == "heart_keys_budget":
            valid_statuses += ("action_limit",)
        if status not in valid_statuses or progress is None:
            raise ValueError(
                "Shaped Heart reward requires a settled task boundary and observed progress"
            )
        if status in ("act3_clear", "heart_clear") and progress.acts_cleared != 3:
            raise ValueError("Final victory missing observed act-clear evidence")
        if status == "heart_clear" and not progress.heart_reached:
            raise ValueError("Heart victory missing observed Heart boundary")
        return RewardParts(
            HEART_PROGRESS_SCALE
            * min(progress.furthest_floor / HEART_PROGRESS_FLOOR, 1.0),
            ACT_CLEAR_BONUS * progress.acts_cleared,
            HEART_CLEAR_BONUS if status == "heart_clear" else 0.0,
            KEY_BONUS
            * sum((progress.ruby_seen, progress.emerald_seen, progress.sapphire_seen))
            if objective == "heart_keys_budget"
            else 0.0,
            ACTION_LIMIT_PENALTY
            if objective == "heart_keys_budget" and status == "action_limit"
            else 0.0,
        )
    if objective == "act1_binary":
        if status not in ("death", "act1_clear"):
            raise ValueError("Binary Act-1 reward requires a completed episode")
        return RewardParts(0.0, float(status == "act1_clear"))
    if objective == "act1":
        if status not in ("death", "act1_clear"):
            raise ValueError("Act-1 reward requires a completed Act-1 episode")
        cleared = status == "act1_clear"
        # An accepted transition into Act 2 completes all Act-1 progress. Floor
        # alone never grants the clear bonus. This does not alter simulator state.
        floor_credit = (
            1.0 if cleared else min(furthest_act1_floor / ACT1_BOSS_FLOOR, 1.0)
        )
        return RewardParts(floor_credit, ACT1_CLEAR_BONUS if cleared else 0.0)
    if objective not in ("act3", "heart") or status not in (
        "death",
        "act3_clear",
        "heart_clear",
    ):
        raise ValueError("Unknown objective or unfinished/invalid terminal boundary")
    return RewardParts(0.0, float(succeeded(objective, status)))
