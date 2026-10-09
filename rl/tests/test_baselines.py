import unittest
from dataclasses import replace
from types import SimpleNamespace
from typing import cast

from beam_search import Node, beam_search, prune, pruning_score
from combat_task import action_indices, terminal_reward
from sts_sim import Decision, PotionKey, State
from sts_sim.observations import PotionSlot
from sts_sim.observations.combat import HiddenIntent, VisibleIntent
from test_model import action, combat


class Graph:
    """Synthetic decision graph for search/rollout infrastructure, not gameplay parity."""

    def __init__(self, nodes: dict, key: str = "start") -> None:
        self.nodes, self.key = nodes, key

    def clone(self):
        return Graph(self.nodes, self.key)

    def decision(self):
        obs, edges = self.nodes[self.key]
        return SimpleNamespace(observation=obs, actions=tuple(candidate for candidate, _ in edges))

    def step(self, candidate):
        for expected, target in self.nodes[self.key][1]:
            if candidate is expected:
                self.key = target
                return self.decision()
        raise AssertionError("Wrong native action index after filtering")


class BaselineTests(unittest.TestCase):
    def setUp(self) -> None:
        obs = combat().decision().observation
        assert obs.kind == "combat"
        self.obs = obs

    def won(self, hp, max_hp=80):
        return replace(
            self.obs,
            context=replace(self.obs.context, player_hp=hp, player_max_hp=max_hp),
            screen=replace(self.obs.screen, phase="won"),
        )

    def test_weighted_pruning_score(self) -> None:
        enemy = replace(
            self.obs.screen.monsters[0],
            hp=20,
            block=4,
            alive=True,
            intent=VisibleIntent(visibility="visible", category="attack", damage=3, hits=2),
        )
        player = replace(self.obs.screen.player, block=4, energy=2)
        obs = replace(
            self.obs,
            context=replace(self.obs.context, player_hp=60),
            screen=replace(self.obs.screen, player=player, monsters=(enemy, replace(enemy, alive=False))),
        )

        def score(observation):
            root = Graph({"start": (observation, [])})
            return pruning_score(Node(cast(State, root), root.decision(), (action("end_turn"),)))

        self.assertEqual(score(obs), 60 - 5 * 2 + 0.75 * 4 + 0.25 * 2 - 1.75 * 20 - 0.25 * 4 - 12)
        blocked = replace(obs, screen=replace(obs.screen, player=replace(player, block=6)))
        excess = replace(blocked, screen=replace(blocked.screen, player=replace(player, block=50)))
        self.assertEqual(score(blocked), score(excess))
        hidden = replace(
            obs, screen=replace(obs.screen, monsters=(replace(enemy, intent=HiddenIntent(visibility="hidden")),))
        )
        self.assertEqual(score(hidden), 60 + 0.25 * 2 - 1.75 * 20 - 0.25 * 4 - 12)

    def test_pruning_preserves_openings_and_stable_ties(self) -> None:
        def node(hp, opening):
            obs = replace(self.obs, context=replace(self.obs.context, player_hp=hp))
            root = Graph({"start": (obs, [])})
            return Node(cast(State, root), root.decision(), (opening,))

        a = action("end_turn")
        b = action("skip_selection")
        nodes = [node(80, a), node(79, a), node(78, b), node(77, b)]
        self.assertEqual(prune(nodes, 2), [nodes[0], nodes[2]])
        self.assertEqual(prune(nodes, 3), nodes[:3])
        self.assertEqual(prune(nodes, 1), nodes[:1])
        tied = [node(80, a), node(80, a), node(80, b)]
        self.assertEqual(prune(tied, 3), tied)

    def test_terminal_objective_and_search_budget(self) -> None:
        root = Graph(
            {
                "start": (self.obs, [(action("end_turn"), "low"), (action("skip_selection"), "high")]),
                "low": (self.won(40), []),
                "high": (self.won(70, 160), []),
            }
        )
        result = beam_search(cast(State, root), max_decisions=1)
        self.assertEqual(result.reward, 70 / 80)  # Starting, not ending max HP.
        self.assertEqual(result.hp, 70)
        self.assertEqual(result.transitions, 2)
        self.assertFalse(result.limit_reached)
        self.assertEqual(root.key, "start")
        replay = root.clone()
        for candidate in result.actions:
            replay.step(candidate)
        self.assertEqual(terminal_reward(replay.decision().observation, 80), result.reward)
        limited = beam_search(cast(State, root), max_transitions=1)
        self.assertEqual(limited.reward, 0.5)
        self.assertTrue(limited.limit_reached)
        self.assertEqual(limited.transitions, 1)
        lost = replace(self.obs, screen=replace(self.obs.screen, phase="lost"))
        self.assertEqual(terminal_reward(lost, 80), 0)
        with self.assertRaises(ValueError):
            beam_search(cast(State, root), width=0)

    def test_unfinished_is_not_zero_reward(self) -> None:
        root = Graph({"start": (self.obs, [(action("end_turn"), "start")])})
        result = beam_search(cast(State, root), max_decisions=2)
        self.assertIsNone(result.reward)
        self.assertIsNone(result.won)
        self.assertEqual(result.actions, ())
        self.assertEqual(result.transitions, 2)
        self.assertTrue(result.limit_reached)

    def test_smoke_filter_preserves_other_actions_and_inventory(self) -> None:
        obs = replace(
            self.obs,
            context=replace(
                self.obs.context,
                potion_slots=(
                    PotionSlot(slot=0, content_key=PotionKey.SMOKE_BOMB),
                    PotionSlot(slot=1, content_key=PotionKey.FIRE),
                ),
            ),
        )
        candidates = (
            action("use_potion_slot", potion_slot=0),
            action("discard_potion_slot", potion_slot=0),
            action("use_potion_slot", potion_slot=1),
            action("end_turn"),
        )
        decision = cast(Decision, SimpleNamespace(observation=obs, actions=candidates))
        self.assertEqual(action_indices(decision), [1, 2, 3])
        self.assertEqual(obs.context.potion_slots[0].content_key, PotionKey.SMOKE_BOMB)

    def test_generated_smoke_filtered_in_random_learned_and_search(self) -> None:
        before = replace(
            self.obs,
            context=replace(self.obs.context, potion_slots=(PotionSlot(slot=0, content_key=PotionKey.ENTROPIC_BREW),)),
        )
        after = replace(
            before,
            context=replace(before.context, potion_slots=(PotionSlot(slot=0, content_key=PotionKey.SMOKE_BOMB),)),
        )
        root = Graph(
            {
                "start": (before, [(action("use_potion_slot", potion_slot=0), "generated")]),
                "generated": (
                    after,
                    [(action("use_potion_slot", potion_slot=0), "forbidden"), (action("end_turn"), "won")],
                ),
                "won": (self.won(60), []),
            }
        )
        result = beam_search(cast(State, root), max_decisions=2)
        self.assertEqual(result.reward, 0.75)
        self.assertEqual(result.transitions, 2)

    def test_real_search_plan_replays_without_mutating_root(self) -> None:
        root = combat()
        before = root.decision().observation
        result = beam_search(root, width=16, max_decisions=48, max_transitions=5000)
        self.assertIsNotNone(result.reward)
        replay = root.clone()
        for candidate in result.actions:
            replay.step(candidate)
        self.assertEqual(terminal_reward(replay.decision().observation, before.context.player_max_hp), result.reward)
        self.assertEqual(root.decision().observation, before)


if __name__ == "__main__":
    unittest.main()
