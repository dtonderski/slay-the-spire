import unittest

from sts.ui_service import SessionManager


class UiServiceTests(unittest.TestCase):
    def test_session_exposes_state_actions_and_snapshot(self):
        manager = SessionManager()
        session = manager.create_session()

        self.assertEqual(session["mode"], "combat_fixture")
        self.assertEqual(session["state_kind"], "combat")
        self.assertEqual(session["command_lifecycle"]["status"], "ready")
        self.assertTrue(session["actions"])
        self.assertEqual(session["actions"][0]["source_state_id"], session["state_id"])

        snapshot = manager.snapshot(session["session_id"])
        self.assertEqual(snapshot["state_id"], session["state_id"])
        self.assertIn("snapshot_json", snapshot)

    def test_step_rejects_stale_action_without_mutating_state(self):
        manager = SessionManager()
        session = manager.create_session()
        action = dict(session["actions"][0])
        action["source_state_id"] = "old"

        result = manager.step(session["session_id"], action)

        self.assertEqual(result["state_id"], session["state_id"])
        self.assertEqual(result["command_lifecycle"]["status"], "stale")
        self.assertTrue(result["actions"])

    def test_step_applies_action_and_regenerates_actions(self):
        manager = SessionManager()
        session = manager.create_session()
        action = session["actions"][0]

        result = manager.step(session["session_id"], action)

        self.assertNotEqual(result["state_id"], session["state_id"])
        self.assertEqual(result["command_lifecycle"]["status"], "applied")
        self.assertTrue(result["actions"] or result["empty_action_reason"])

    def test_search_returns_best_current_action_id(self):
        manager = SessionManager()
        session = manager.create_session()

        result = manager.search(session["session_id"], {"max_depth": 1})

        recommendation = result["recommendation"]
        action_ids = {action["action_id"] for action in session["actions"]}
        self.assertIn(recommendation["best_action_id"], action_ids)
        self.assertTrue(recommendation["principal_variation"])

    def test_run_map_fixture_exposes_run_decision_and_actions(self):
        manager = SessionManager()
        session = manager.create_session("run_map_fixture")

        self.assertEqual(session["mode"], "run_map_fixture")
        self.assertEqual(session["state_kind"], "run")
        self.assertEqual(session["phase"], "idle")
        self.assertEqual(session["current_decision"], "map")
        self.assertTrue(session["actions"])
        self.assertEqual(session["actions"][0]["descriptor"]["kind"], "ExactRunAction")

    def test_run_session_step_rejects_stale_and_applies_current_action(self):
        manager = SessionManager()
        session = manager.create_session("run_map_fixture")
        stale = dict(session["actions"][0])
        stale["source_state_id"] = "old"

        stale_result = manager.step(session["session_id"], stale)
        self.assertEqual(stale_result["state_id"], session["state_id"])
        self.assertEqual(stale_result["command_lifecycle"]["status"], "stale")

        result = manager.step(session["session_id"], session["actions"][0])
        self.assertNotEqual(result["state_id"], session["state_id"])
        self.assertEqual(result["state_kind"], "run")
        self.assertEqual(result["command_lifecycle"]["status"], "applied")

    def test_run_session_reports_search_and_parity_as_combat_only(self):
        manager = SessionManager()
        session = manager.create_session("run_map_fixture")

        with self.assertRaises(ValueError):
            manager.search(session["session_id"], {"max_depth": 1})

        result = manager.parity(session["session_id"], {"summary": {}})
        self.assertEqual(result["parity"]["status"], "unknown")
        self.assertIn("combat parity", result["parity"]["reason"])

    def test_parity_reports_unknown_without_observed_combat(self):
        manager = SessionManager()
        session = manager.create_session()

        result = manager.parity(session["session_id"], {"summary": {"missing": True}})

        self.assertEqual(result["session_id"], session["session_id"])
        self.assertEqual(result["parity"]["status"], "unknown")

    def test_parity_reports_divergence_against_observed_combat(self):
        manager = SessionManager()
        session = manager.create_session()

        result = manager.parity(
            session["session_id"],
            {
                "summary": {
                    "combat": {
                        "player_hp": 1,
                        "player_block": 99,
                        "energy": 0,
                        "monsters": [{"hp": 1, "block": 0, "gone": False}],
                    }
                },
                "stale": False,
            },
        )

        self.assertEqual(result["parity"]["status"], "diverged")
        self.assertTrue(result["parity"]["diffs"])


if __name__ == "__main__":
    unittest.main()
