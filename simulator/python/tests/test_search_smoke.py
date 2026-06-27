import unittest

import sts.omni as omni
from sts.search import CombatSearchConfig, CombatSearchResult, recommend_action, search_combat


class CombatSearchSmokeTests(unittest.TestCase):
    def test_recommend_action_returns_action_without_mutating_root(self):
        env = omni.OmniCombatEnv.initial_fixture()
        before = env.snapshot_hash()

        result = search_combat(env, CombatSearchConfig(max_depth=1))

        self.assertIsInstance(result, CombatSearchResult)
        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.action, result.best_action)
        self.assertGreaterEqual(result.nodes, 2)
        self.assertEqual(result.visits, result.nodes)
        self.assertEqual(result.depth, 1)
        self.assertEqual(result.diagnostics["objective"], "survive_then_damage")
        self.assertGreaterEqual(len(result.principal_variation), 1)
        self.assertEqual(result.best_action.json(), result.principal_variation[0].json())
        self.assertEqual(env.snapshot_hash(), before)

    def test_recommend_action_is_deterministic(self):
        first = omni.OmniCombatEnv.initial_fixture()
        second = omni.OmniCombatEnv.initial_fixture()

        first_result = recommend_action(first, depth=2)
        second_result = recommend_action(second, depth=2)

        self.assertEqual(first_result.score, second_result.score)
        self.assertEqual(
            [action.json() for action in first_result.principal_variation],
            [action.json() for action in second_result.principal_variation],
        )

    def test_recommend_action_rejects_non_positive_depth(self):
        env = omni.OmniCombatEnv.initial_fixture()

        with self.assertRaises(ValueError):
            recommend_action(env, depth=0)

    def test_search_rejects_unknown_objective(self):
        env = omni.OmniCombatEnv.initial_fixture()

        with self.assertRaises(ValueError):
            search_combat(env, CombatSearchConfig(objective="mystery"))

    def test_search_accepts_run_combat_fixture(self):
        env = omni.OmniRunEnv.combat_fixture()

        result = search_combat(env, CombatSearchConfig(max_depth=1))

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.best_action.family(), "combat")
        self.assertGreaterEqual(result.nodes, 2)

    def test_search_rejects_run_map_fixture(self):
        env = omni.OmniRunEnv.map_fixture()

        with self.assertRaises(ValueError):
            search_combat(env, CombatSearchConfig(max_depth=1))


if __name__ == "__main__":
    unittest.main()
