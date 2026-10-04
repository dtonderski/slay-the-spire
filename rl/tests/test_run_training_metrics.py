"""Shaped-return and behavior accounting tests; scripted states are not parity evidence."""

import json
import tempfile
import unittest
from dataclasses import asdict, replace
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast

import torch
from run_training.collector import CollectionFailure, RunEpisode, collect, task_result
from run_training.contracts import PolicyAction
from run_training.metrics import BehaviorStats, behavior_scores
from run_training.model import MacroModel
from run_training.rewards import terminal_parts
from run_training.trainer import scores, update
from sts_sim import State
from sts_sim.observations.screens import (
    MapNode,
    MapObservation,
    MapScreen,
    RestObservation,
    RestScreen,
)


def action(kind, **kwargs):
    return PolicyAction(
        kind=kind,
        **{
            name: kwargs.get(name)
            for name in (
                "hand_slot",
                "potion_slot",
                "option_slot",
                "target_slot",
                "card_slot",
                "node_slot",
                "reward_slot",
                "shop_slot",
            )
        },
    )


def rest_observation(hp=20, maximum=80):
    base = State.new("1").decision().observation
    return RestObservation(
        schema_version=base.schema_version,
        phase="rest",
        kind="rest",
        context=replace(base.context, player_hp=hp, player_max_hp=maximum, floor=8),
        screen=RestScreen(complete=False, options=()),
    )


class RunMetricsTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)

    def test_terminal_reward_scale_and_invalid_boundaries(self):
        for floor, expected in [(0, 0), (4, 0.25), (8, 0.5), (16, 1), (17, 1)]:
            self.assertEqual(terminal_parts("act1", "death", floor).total, expected)
        parts = terminal_parts("act1", "act1_clear", 17)
        self.assertEqual((parts.progress, parts.clear_bonus, parts.total), (1, 5, 6))
        for status in ("cutoff", "error", "ongoing", "unknown_complete", "heart_clear"):
            with self.assertRaises(ValueError):
                terminal_parts("act1", status, 16)
        self.assertEqual(terminal_parts("act3", "act3_clear", 16).total, 1)
        self.assertEqual(terminal_parts("heart", "act3_clear", 16).total, 0)
        self.assertEqual(terminal_parts("heart", "heart_clear", 16).total, 1)
        with self.assertRaises(ValueError):
            terminal_parts("act1", "death", -1)

    def test_floor_does_not_imply_success(self):
        d = State.new("1").decision()
        d = replace(
            d,
            observation=replace(
                d.observation, context=replace(d.observation.context, floor=17)
            ),
        )
        self.assertEqual(task_result(d, "act1"), ("ongoing", None))
        dead = replace(
            d,
            observation=replace(
                d.observation, context=replace(d.observation.context, outcome="death")
            ),
        )
        self.assertEqual(task_result(dead, "act1", 16), ("death", 1))
        clear = replace(
            d,
            observation=replace(
                d.observation, context=replace(d.observation.context, act=2, floor=18)
            ),
        )
        self.assertEqual(task_result(clear, "act1", 17), ("act1_clear", 6))

    def test_shaped_returns_do_not_inflate_success_rates(self):
        episodes = [
            RunEpisode("death", 1, (), 100, 16, furthest_act1_floor=16),
            RunEpisode("death", 0.5, (), 50, 8, furthest_act1_floor=8),
            RunEpisode("act1_clear", 6, (), 200, 18, furthest_act1_floor=17),
            RunEpisode("cutoff", None, (), 30, 6, furthest_act1_floor=6),
        ]
        result = scores(episodes)
        self.assertEqual(result["successes"], 1)
        self.assertEqual(result["completed_success_rate"], 1 / 3)
        self.assertEqual(result["success_lower_bound"], 0.25)
        self.assertEqual(result["reward/mean"], 2.5)
        self.assertAlmostEqual(result["reward/progress_mean"], 2.5 / 3)
        self.assertAlmostEqual(result["reward/clear_bonus_mean"], 5 / 3)
        self.assertNotIn("reward/mean", scores([episodes[-1]]))

    def test_hp_bands_probability_and_opportunity_denominators(self):
        stats = BehaviorStats()
        for hp, band in [
            (0, 0),
            (19, 0),
            (20, 1),
            (39, 1),
            (40, 2),
            (59, 2),
            (60, 3),
            (80, 3),
        ]:
            new = BehaviorStats().accepted(
                rest_observation(hp), action("rest_heal"), 0.2
            )
            self.assertEqual(new.heal_opportunities[band], 1)
            self.assertEqual(new.heals[band], 1)
        stats = stats.accepted(rest_observation(), action("rest_heal"), 0.8)
        stats = stats.accepted(
            rest_observation(), action("rest_smith", card_slot=0), 0.2
        )
        # Healing unavailable must not be mistaken for choosing not to heal.
        same = stats.accepted(rest_observation(), action("rest_proceed"), None)
        self.assertEqual(stats, same)
        result = behavior_scores([stats])
        self.assertEqual(result["healing/hp_25_50/opportunities"], 2)
        self.assertEqual(result["healing/hp_25_50/heal_rate"], 0.5)
        self.assertEqual(result["healing/hp_25_50/heal_probability"], 0.5)
        self.assertEqual(result["healing/hp_00_25/opportunities"], 0)
        self.assertNotIn("healing/hp_00_25/heal_rate", result)
        with self.assertRaises(ValueError):
            stats.accepted(rest_observation(), action("rest_heal"), float("nan"))

    def test_elite_metric_is_visited_destination_not_downstream_exposure(self):
        base = rest_observation()
        nodes = (
            MapNode(
                slot=0, act=1, room_kind="combat", burning_elite=False, children=(1,)
            ),
            MapNode(slot=1, act=1, room_kind="elite", burning_elite=True, children=()),
        )
        ob = MapObservation(
            schema_version=base.schema_version,
            phase="idle",
            kind="map",
            context=base.context,
            screen=MapScreen(
                act=1, floor=8, current_node=0, reachable_nodes=(0, 1), nodes=nodes
            ),
        )
        stats = BehaviorStats().accepted(
            ob, action("choose_map_node", node_slot=0), None
        )
        self.assertEqual(stats.map_nodes_visited, 1)
        self.assertEqual(stats.elite_nodes_visited, 0)
        stats = stats.accepted(ob, action("choose_map_node", node_slot=1), None)
        # UI actions never add another visit.
        stats = stats.accepted(base, action("rest_proceed"), None)
        self.assertEqual((stats.map_nodes_visited, stats.elite_nodes_visited), (2, 1))
        result = behavior_scores([stats, BehaviorStats(map_nodes_visited=8)])
        self.assertEqual(result["map/elite_percent"], 10)
        self.assertNotIn("map/elite_percent", behavior_scores([]))

    def test_collector_records_pre_hp_and_keeps_only_accepted_behavior_on_failure(self):
        original = State.new("1").decision()
        ob = rest_observation()
        decisions = replace(
            original,
            observation=ob,
            actions=tuple(
                SimpleNamespace(**asdict(action(kind)))
                for kind in ("rest_heal", "rest_lift")
            ),
        )
        dead = replace(
            original,
            observation=replace(
                original.observation,
                context=replace(
                    original.observation.context, outcome="death", floor=8, player_hp=0
                ),
            ),
        )

        class Scripted:
            def __init__(self, fail):
                self.calls = 0
                self.fail = fail

            def decision(self):
                return decisions

            def step(self, selected):
                self.calls += 1
                if self.fail and self.calls == 2:
                    raise ValueError("unsupported")
                return decisions if self.fail else dead

        macro = MacroModel(8)
        with tempfile.TemporaryDirectory() as directory:
            for fail in (False, True):
                path = Path(directory) / f"{fail}.jsonl"
                state = Scripted(fail)
                # Dynamic boundary fixture, not a native State implementation.
                kwargs: dict[str, Any] = {
                    "objective": "act1",
                    "final_act": True,
                    "max_actions": 10,
                    "macro_rng": torch.Generator().manual_seed(1),
                    "combat_rng": torch.Generator(),
                    "journal": path,
                    "state_factory": lambda *a, state=state, **kw: state,
                }
                if fail:
                    with self.assertRaises(CollectionFailure) as raised:
                        collect("1", macro, cast(Any, None), **kwargs)
                    behavior = raised.exception.behavior
                    self.assertEqual(raised.exception.accepted, 1)
                    self.assertEqual(raised.exception.furthest_act1_floor, 8)
                else:
                    episode = collect("1", macro, cast(Any, None), **kwargs)
                    behavior = episode.behavior
                    self.assertEqual(episode.reward, 0.5)
                    self.assertEqual(episode.furthest_act1_floor, 8)
                self.assertEqual(behavior.heal_opportunities, (0, 1, 0, 0))
                self.assertEqual(behavior.heal_probability_sum, (0, 0.5, 0, 0))
                rows = [json.loads(line) for line in path.read_text().splitlines()]
                attempt = rows[1]
                self.assertEqual(attempt["rest_pre_hp"], 20)
                self.assertEqual(attempt["heal_probability"], 0.5)
                self.assertEqual(
                    behavior.heals[1], int(attempt["action"]["kind"] == "rest_heal")
                )
                self.assertEqual(
                    rows[-1]["behavior"]["heal_opportunities"], [0, 1, 0, 0]
                )

    def test_optimizer_rejects_old_binary_clear_targets_and_nonfinite_returns(self):
        model = MacroModel(8)
        optimizer = torch.optim.Adam(model.parameters())
        for reward in (1.0, float("nan"), float("inf"), 7.0):
            with self.assertRaises(ValueError):
                update(
                    [RunEpisode("act1_clear", reward, (), 1, 18)],
                    model,
                    optimizer,
                    entropy_coef=0,
                    value_coef=0.1,
                )
        self.assertFalse(optimizer.state)


if __name__ == "__main__":
    unittest.main()
