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

    def test_beam_search_accepts_large_depth_without_mutating_root(self):
        env = omni.OmniCombatEnv.initial_fixture()
        before = env.snapshot_hash()

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=20,
                objective="tactical_survival",
                algorithm="beam",
                beam_width=4,
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.diagnostics["algorithm"], "beam")
        self.assertEqual(result.diagnostics["beam_width"], 4)
        self.assertGreater(result.visits, 1)
        self.assertEqual(env.snapshot_hash(), before)

    def test_greedy_search_is_reported_as_greedy(self):
        env = omni.OmniCombatEnv.initial_fixture()

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=20,
                objective="aggressive_lethal",
                algorithm="greedy",
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.diagnostics["algorithm"], "greedy")
        self.assertEqual(result.diagnostics["beam_width"], 1)

    def test_terminal_probe_search_returns_action(self):
        env = omni.OmniCombatEnv.initial_fixture()

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=4,
                objective="tactical_survival",
                algorithm="terminal_probe",
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.diagnostics["algorithm"], "terminal_probe")

    def test_trace_probe_search_returns_action(self):
        env = omni.OmniCombatEnv.initial_fixture()

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=4,
                objective="tactical_survival",
                algorithm="trace_probe",
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.diagnostics["algorithm"], "trace_probe")

    def test_potion_rescue_trace_probe_search_returns_action(self):
        env = omni.OmniCombatEnv.initial_fixture()

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=4,
                objective="tactical_survival",
                algorithm="potion_rescue_trace_probe",
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.diagnostics["algorithm"], "potion_rescue_trace_probe")

    def test_aggressive_rescue_trace_probe_search_returns_action(self):
        env = omni.OmniCombatEnv.initial_fixture()

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=4,
                objective="tactical_survival",
                algorithm="aggressive_rescue_trace_probe",
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.diagnostics["algorithm"], "aggressive_rescue_trace_probe")

    def test_scaling_survival_objective_is_supported(self):
        env = omni.OmniCombatEnv.initial_fixture()

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=4,
                objective="scaling_survival",
                algorithm="greedy",
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.diagnostics["objective"], "scaling_survival")

    def test_hp_preserving_greedy_still_returns_painful_legal_action(self):
        import json

        state = json.loads(omni.OmniCombatEnv.initial_fixture().state_json())
        state["piles"]["hand"] = []
        state["player"]["block"] = 0
        state["monsters"][0]["intent"] = {"Attack": {"damage": 6}}
        env = omni.OmniCombatEnv.from_state_json(json.dumps(state))

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=8,
                objective="hp_preserving_lethal",
                algorithm="greedy",
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.best_action.kind(), "end_turn")
        self.assertEqual(result.diagnostics["objective"], "hp_preserving_lethal")

    def test_portfolio_search_returns_action_without_mutating_root(self):
        env = omni.OmniCombatEnv.initial_fixture()
        before = env.snapshot_hash()

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=12,
                objective="aggressive_lethal",
                algorithm="portfolio",
                beam_width=12,
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.diagnostics["algorithm"], "portfolio")
        self.assertGreater(result.visits, 1)
        self.assertEqual(env.snapshot_hash(), before)

    def test_exhaustive_search_rejects_runaway_depth(self):
        env = omni.OmniCombatEnv.initial_fixture()

        with self.assertRaises(ValueError):
            search_combat(env, CombatSearchConfig(max_depth=40, algorithm="exhaustive"))

    def test_search_accepts_run_combat_fixture(self):
        env = omni.OmniRunEnv.combat_fixture()

        result = search_combat(env, CombatSearchConfig(max_depth=1))

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.best_action.family(), "combat")
        self.assertGreaterEqual(result.nodes, 2)

    def test_allowed_potions_filters_run_potion_actions(self):
        env = self._run_combat_with_fire_potion()

        blocked = search_combat(
            env,
            CombatSearchConfig(max_depth=1, algorithm="greedy", allowed_potions=()),
        )

        self.assertTrue(blocked.principal_variation)
        self.assertTrue(
            all(action.kind() != "use_potion" for action in blocked.principal_variation)
        )
        self.assertEqual(blocked.diagnostics["allowed_potions"], ())

    def test_allowed_potion_names_match_short_inventory_names(self):
        env = self._run_combat_with_fire_potion(monster_hp=5, empty_hand=True)

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=1,
                objective="aggressive_lethal",
                algorithm="greedy",
                allowed_potions=("Fire Potion",),
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.best_action.kind(), "use_potion")

    def test_nonlethal_potion_is_not_free_heuristic_value(self):
        env = self._run_combat_with_fire_potion(monster_hp=80)

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=1,
                objective="aggressive_lethal",
                algorithm="greedy",
                allowed_potions=("Fire Potion",),
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertNotEqual(result.best_action.kind(), "use_potion")

    def test_select_screen_uses_local_shortcut(self):
        env = self._run_combat_with_potion("Elixir")
        elixir = next(action for action in env.exact_legal_actions() if action.kind() == "use_potion")
        env.step(elixir)

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=40,
                objective="aggressive_lethal",
                algorithm="beam",
                beam_width=8,
                allowed_potions=("Elixir",),
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.best_action.kind(), "confirm_exhaust_select")
        self.assertTrue(result.diagnostics["select_screen_shortcut"])

    def test_elixir_select_shortcut_exhausts_bad_cards_before_confirming(self):
        import json

        state = json.loads(self._run_combat_with_potion("Elixir").state_json())
        state["combat"]["piles"]["hand"] = [
            {"id": 100, "content_id": 67, "temp_cost": None, "combat_only": False},
            {"id": 101, "content_id": 62, "temp_cost": None, "combat_only": False},
            {"id": 102, "content_id": 1, "temp_cost": None, "combat_only": False},
        ]
        env = omni.OmniRunEnv.from_state_json(json.dumps(state))
        elixir = next(action for action in env.exact_legal_actions() if action.kind() == "use_potion")
        env.step(elixir)

        result = search_combat(
            env,
            CombatSearchConfig(
                max_depth=40,
                objective="aggressive_lethal",
                algorithm="beam",
                beam_width=8,
                allowed_potions=("Elixir",),
            ),
        )

        self.assertIsNotNone(result.best_action)
        self.assertEqual(result.best_action.kind(), "choose_exhaust_select")
        self.assertIn(
            result.best_action.json(),
            {
                '{"ChooseExhaustSelect":{"index":0}}',
                '{"ChooseExhaustSelect":{"index":1}}',
            },
        )
        self.assertTrue(result.diagnostics["select_screen_shortcut"])

    def test_search_rejects_run_map_fixture(self):
        env = omni.OmniRunEnv.map_fixture()

        with self.assertRaises(ValueError):
            search_combat(env, CombatSearchConfig(max_depth=1))

    def _run_combat_with_fire_potion(self, monster_hp=None, empty_hand=False):
        return self._run_combat_with_potion("Fire", monster_hp=monster_hp, empty_hand=empty_hand)

    def _run_combat_with_potion(self, potion, monster_hp=None, empty_hand=False):
        import json

        state = json.loads(omni.OmniRunEnv.combat_fixture().state_json())
        state["potions"] = [potion]
        if monster_hp is not None:
            state["combat"]["monsters"][0]["hp"] = monster_hp
        if empty_hand:
            state["combat"]["piles"]["hand"] = []
        return omni.OmniRunEnv.from_state_json(json.dumps(state))


if __name__ == "__main__":
    unittest.main()
