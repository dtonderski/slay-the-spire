import unittest

import sts.omni as sts


class OmniRunSmokeTests(unittest.TestCase):
    def test_combat_fixture_steps_without_mutating_clone_parent(self):
        env = sts.OmniRunEnv.combat_fixture()
        before = env.snapshot_hash()
        child = env.clone()
        action = next(action for action in child.exact_legal_actions() if action.kind() == "play_card")

        result = child.step(action)

        self.assertEqual(env.snapshot_hash(), before)
        self.assertNotEqual(result.snapshot_hash, before)
        self.assertEqual(result.transition.previous_hash, before)
        self.assertEqual(child.phase(), "combat")

    def test_no_rng_combat_draws_from_discard_on_next_turn(self):
        import json

        state = json.loads(sts.OmniRunEnv.combat_fixture().state_json())
        state["combat"]["piles"]["hand"] = []
        state["combat"]["piles"]["draw_pile"] = []
        state["combat"]["piles"]["discard_pile"] = [
            {"id": 900, "content_id": 1, "temp_cost": None, "combat_only": False}
        ]
        state["combat"]["shuffle_rng"] = None
        state["combat"]["card_random_rng"] = None
        env = sts.OmniRunEnv.from_state_json(json.dumps(state))

        end_turn = next(action for action in env.exact_legal_actions() if action.kind() == "end_turn")
        env.step(end_turn)
        combat = json.loads(env.state_json())["combat"]

        self.assertEqual([card["id"] for card in combat["piles"]["hand"]], [900])
        self.assertEqual(combat["piles"]["draw_pile"], [])
        self.assertEqual(combat["piles"]["discard_pile"], [])

    def test_map_fixture_round_trips_snapshot_and_exposes_map_actions(self):
        env = sts.OmniRunEnv.map_fixture()
        restored = sts.OmniRunEnv.from_snapshot_json(env.snapshot_json())

        self.assertEqual(restored.snapshot_hash(), env.snapshot_hash())
        self.assertEqual(env.phase(), "idle")
        self.assertTrue(any(action.family() == "map" for action in env.exact_legal_actions()))

    def test_rust_greedy_combat_search_returns_action(self):
        env = sts.OmniRunEnv.combat_fixture()

        recommendation = env.rust_greedy_combat_search(
            12,
            "tactical_survival",
            [],
        )

        self.assertIsNotNone(recommendation.best_action)
        self.assertIsInstance(recommendation, sts.RustSearchRecommendation)
        self.assertGreater(recommendation.nodes, 0)
        self.assertGreaterEqual(recommendation.actions, 0)
        self.assertIsInstance(recommendation.value, float)

    def test_seed_start_uses_placeholder_generated_map(self):
        first = sts.OmniRunEnv.new_ironclad(seed="TEST", ascension=0)
        second = sts.OmniRunEnv.new_ironclad(seed="TEST", ascension=0)
        other = sts.OmniRunEnv.new_ironclad(seed="OTHER", ascension=0)

        self.assertEqual(first.phase(), "idle")
        self.assertEqual(first.current_decision(), "map")
        self.assertEqual(first.snapshot_hash(), second.snapshot_hash())
        self.assertNotEqual(first.snapshot_hash(), other.snapshot_hash())
        self.assertTrue(any(action.family() == "map" for action in first.exact_legal_actions()))


if __name__ == "__main__":
    unittest.main()
