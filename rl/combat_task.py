"""Shared combat-only objective and action restriction; no simulator rule changes."""

from sts_sim import Decision, Observation, PotionKey


def action_indices(decision: Decision) -> list[int]:
    """Keep native candidate indices, excluding Smoke Bomb use (including newly generated bombs)."""
    return [
        index
        for index, action in enumerate(decision.actions)
        if not (
            action.kind == "use_potion_slot"
            and action.potion_slot is not None
            and decision.observation.context.potion_slots[action.potion_slot].content_key == PotionKey.SMOKE_BOMB
        )
    ]


def combat_outcome(observation: Observation) -> bool | None:
    """Recognize completion without taking postcombat actions; unexpected screens fail closed."""
    if observation.kind == "combat":
        if observation.screen.phase == "won":
            return True
        if observation.screen.phase == "lost":
            return False
        return None
    if observation.phase == "reward":
        return True
    if observation.kind == "complete" and observation.context.player_hp <= 0:
        return False
    raise RuntimeError(f"Unexpected screen after combat: {observation.kind}/{observation.phase}")


def terminal_reward(observation: Observation, starting_max_hp: int) -> float | None:
    """Terminal HP / starting max HP; defeat is zero, unfinished is None."""
    if starting_max_hp <= 0:
        raise ValueError("Starting max HP must be positive")
    won = combat_outcome(observation)
    if won is None:
        return None
    return observation.context.player_hp / starting_max_hp if won else 0.0
