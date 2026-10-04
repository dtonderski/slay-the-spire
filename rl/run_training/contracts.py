"""Minimal fair routing and outcome contracts for full-run research.

No simulator state, seed, revision, or hidden fields enter these functions.
Terminal rewards depend on the declared objective, not merely positive HP.
"""

from dataclasses import dataclass
from typing import Literal

from sts_sim import Action, Decision

Controller = Literal["combat", "macro", "forced", "terminal"]
Outcome = Literal["ongoing", "death", "act3_clear", "heart_clear", "invalid"]
Objective = Literal["act3", "heart"]


@dataclass(frozen=True, slots=True)
class PolicyAction:
    """Decision-local descriptor, deliberately excluding native revision metadata."""

    kind: str
    hand_slot: int | None
    potion_slot: int | None
    option_slot: int | None
    target_slot: int | None
    card_slot: int | None
    node_slot: int | None
    reward_slot: int | None
    shop_slot: int | None

    @classmethod
    def from_action(cls, action: Action) -> "PolicyAction":
        return cls(
            action.kind,
            action.hand_slot,
            action.potion_slot,
            action.option_slot,
            action.target_slot,
            action.card_slot,
            action.node_slot,
            action.reward_slot,
            action.shop_slot,
        )


def outcome(decision: Decision) -> Outcome:
    """Use the explicit public result; never guess victory from HP/act/screen."""
    status = decision.observation.context.outcome
    if status in ("death", "act3_clear", "heart_clear"):
        return status
    if status != "ongoing" or not decision.actions:
        return "invalid"
    return "ongoing"


def controller(decision: Decision) -> Controller:
    status = outcome(decision)
    if status != "ongoing":
        return "terminal"
    if len(decision.actions) == 1:
        return "forced"
    # Potion and selection actions have family='run' even during combat.
    return "combat" if decision.observation.kind == "combat" else "macro"


def terminal_reward(status: Outcome, objective: Objective) -> float:
    if objective not in ("act3", "heart"):
        raise ValueError("Unknown run objective")
    if status not in ("death", "act3_clear", "heart_clear"):
        raise ValueError("Unfinished or invalid runs have no terminal reward")
    return float(
        status == "heart_clear" or (objective == "act3" and status == "act3_clear")
    )
