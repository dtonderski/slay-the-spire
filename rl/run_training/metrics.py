"""Public behavior diagnostics only: never model inputs, rewards, or RNG draws."""

import math
from dataclasses import dataclass, replace

from sts_sim import Observation

from run_training.contracts import PolicyAction

# Half-open intervals except the last: [0,25), [25,50), [50,75), [75,100].
HP_BANDS = ("hp_00_25", "hp_25_50", "hp_50_75", "hp_75_100")
TRACKED_ACTIONS = (
    "take_card_reward",
    "close_card_reward",
    "take_singing_bowl_reward",
    "buy_shop_card",
    "buy_shop_relic",
    "buy_shop_potion",
    "use_potion_slot",
    "discard_potion_slot",
    "enter_shop",
    "leave_shop",
    "rest_heal",
    "rest_open_smith",
    "rest_recall",
    "choose_boss_relic_reward",
)


@dataclass(frozen=True)
class BehaviorStats:
    heal_opportunities: tuple[int, ...] = (0, 0, 0, 0)
    heals: tuple[int, ...] = (0, 0, 0, 0)
    heal_probability_sum: tuple[float, ...] = (0.0, 0.0, 0.0, 0.0)
    map_nodes_visited: int = 0
    elite_nodes_visited: int = 0
    action_counts: tuple[tuple[str, int], ...] = ()
    shop_floors_seen: tuple[tuple[int, int], ...] = ()
    shop_reentries: int = 0

    def accepted(
        self,
        observation: Observation,
        action: PolicyAction,
        heal_probability: float | None,
    ) -> "BehaviorStats":
        """Call only after a successful step, with the PRE-action public observation.

        heal_probability=None means healing was not available, not zero preference.
        Reopening menus can yield multiple rest decisions at one campfire; this is
        deliberately a decision-level rate, not a unique-campfire count.
        """
        result = self
        if heal_probability is not None:
            if (
                observation.kind != "rest"
                or not math.isfinite(heal_probability)
                or not 0 <= heal_probability <= 1
            ):
                raise ValueError("Invalid healing diagnostic")
            hp, maximum = (
                observation.context.player_hp,
                observation.context.player_max_hp,
            )
            if maximum <= 0 or not 0 <= hp <= maximum:
                raise ValueError("Invalid pre-choice HP")
            band = min(3, 4 * hp // maximum)
            opportunities = list(self.heal_opportunities)
            heals = list(self.heals)
            probabilities = list(self.heal_probability_sum)
            opportunities[band] += 1
            heals[band] += int(action.kind == "rest_heal")
            probabilities[band] += heal_probability
            result = replace(
                result,
                heal_opportunities=tuple(opportunities),
                heals=tuple(heals),
                heal_probability_sum=tuple(probabilities),
            )
        if action.kind == "choose_map_node":
            if observation.kind != "map" or action.node_slot is None:
                raise ValueError("Map entry diagnostic requires a public destination")
            if not 0 <= action.node_slot < len(observation.screen.nodes):
                raise ValueError("Invalid public map slot")
            elite = observation.screen.nodes[action.node_slot].room_kind == "elite"
            result = replace(
                result,
                map_nodes_visited=result.map_nodes_visited + 1,
                elite_nodes_visited=result.elite_nodes_visited + int(elite),
            )
        if action.kind in TRACKED_ACTIONS:
            counts = dict(result.action_counts)
            counts[action.kind] = counts.get(action.kind, 0) + 1
            result = replace(result, action_counts=tuple(counts.items()))
        if action.kind == "enter_shop":
            location = (observation.context.act, observation.context.floor)
            revisited = location in result.shop_floors_seen
            result = replace(
                result,
                shop_reentries=result.shop_reentries + int(revisited),
                shop_floors_seen=result.shop_floors_seen
                if revisited
                else result.shop_floors_seen + (location,),
            )
        return result


def behavior_scores(records: list[BehaviorStats]) -> dict[str, float]:
    """Pool counts, not per-episode percentages. Empty denominators omit rates."""
    nodes = sum(r.map_nodes_visited for r in records)
    elites = sum(r.elite_nodes_visited for r in records)
    result = {
        "map/nodes_visited": float(nodes),
        "map/elite_nodes_visited": float(elites),
    }
    result.update(
        {
            f"actions/{kind}": float(
                sum(dict(r.action_counts).get(kind, 0) for r in records)
            )
            for kind in TRACKED_ACTIONS
        }
    )
    result["navigation/shop_same_floor_reentries"] = float(
        sum(r.shop_reentries for r in records)
    )
    if nodes:
        result["map/elite_percent"] = 100.0 * elites / nodes
    for i, name in enumerate(HP_BANDS):
        count = sum(r.heal_opportunities[i] for r in records)
        heals = sum(r.heals[i] for r in records)
        prefix = f"healing/{name}"
        result[f"{prefix}/opportunities"] = float(count)
        result[f"{prefix}/heals"] = float(heals)
        if count:
            result[f"{prefix}/heal_rate"] = heals / count
            result[f"{prefix}/heal_probability"] = (
                sum(r.heal_probability_sum[i] for r in records) / count
            )
    return result
