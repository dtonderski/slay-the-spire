"""Static regression: kind discriminants narrow nested observation fields."""

from sts_sim import Observation, State


def combat_energy(observation: Observation) -> int | None:
    if observation.kind == "combat":
        return observation.screen.player.energy
    return None


def map_current_node(observation: Observation) -> int | None:
    if observation.kind == "map":
        return observation.screen.current_node
    return None


def visible_intent_damage(observation: Observation) -> int | None:
    if observation.kind != "combat":
        return None
    for monster in observation.screen.monsters:
        if monster.intent.visibility == "visible":
            return monster.intent.damage
    return None


def rest_smith_slot(observation: Observation) -> int | None:
    if observation.kind != "rest":
        return None
    for option in observation.screen.options:
        if option.kind == "smith":
            return option.card_slot
    return None


def dark_orb_evoke(observation: Observation) -> int | None:
    if observation.kind != "combat":
        return None
    for slot in observation.screen.orb_slots:
        if slot.orb is not None and slot.orb.type == "dark":
            return slot.orb.evoke
    return None


def live_state_narrowing() -> int | None:
    observation = State.new("HUMAN1").observation()
    if observation.kind == "map":
        return observation.screen.floor
    return None
