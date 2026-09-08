"""Static regression: unknown fields and unnarrowed access must type-error."""

from sts_sim import Observation


def unknown_top_level_field(observation: Observation) -> object:
    return observation.seed


def unknown_nested_field(observation: Observation) -> object:
    return observation.context.hidden_draw_order


def unnarrowed_combat_energy(observation: Observation) -> int:
    return observation.screen.player.energy


def map_screen_is_not_combat(observation: Observation) -> int:
    if observation.kind == "map":
        return observation.screen.player.energy
    return 0


def combat_screen_has_no_owned_relics(observation: Observation) -> object:
    if observation.kind == "combat":
        return observation.screen.relics
    return None


def combat_screen_has_no_context(observation: Observation) -> object:
    if observation.kind == "combat":
        return observation.screen.context
    return None


def combat_screen_has_no_potion_slots(observation: Observation) -> object:
    if observation.kind == "combat":
        return observation.screen.potion_slots
    return None
