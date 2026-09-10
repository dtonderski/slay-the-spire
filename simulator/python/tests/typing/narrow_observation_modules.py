"""Static regression: facade and domain observation imports both narrow."""

from sts_sim.observations import CombatObservation as FacadeCombatObservation
from sts_sim.observations import Observation
from sts_sim.observations.combat import CombatObservation, CombatScreen, VisibleIntent
from sts_sim.observations.common import Card, Relic, RunContext
from sts_sim.observations.screens import MapObservation, RestObservation, RestSmith


def facade_combat_energy(observation: Observation) -> int | None:
    if observation.kind == "combat":
        return observation.screen.player.energy
    return None


def domain_combat_energy(observation: CombatObservation) -> int:
    return observation.screen.player.energy


def domain_combat_known_positions(observation: CombatObservation) -> int:
    if observation.kind == "combat":
        return len(observation.screen.draw_pile.known_positions)
    return 0


def domain_combat_screen_energy(screen: CombatScreen) -> int:
    return screen.player.energy


def domain_visible_intent_damage(intent: VisibleIntent) -> int | None:
    if intent.visibility == "visible":
        return intent.damage
    return None


def domain_context_relic_and_card(context: RunContext) -> int:
    relic: Relic = context.relics[0]
    card: Card = context.deck[0]
    return relic.slot + card.cost


def domain_map_floor(observation: MapObservation) -> int:
    if observation.kind == "map":
        return observation.screen.floor
    return 0


def domain_rest_smith_slot(observation: RestObservation) -> int | None:
    if observation.kind != "rest":
        return None
    for option in observation.screen.options:
        if option.kind == "smith":
            smith: RestSmith = option
            return smith.card_slot
    return None


def facade_class_uses_domain_screen(
    observation: FacadeCombatObservation,
) -> CombatScreen:
    return observation.screen
