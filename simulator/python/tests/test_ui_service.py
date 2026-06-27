import unittest

from sts.ui_service import SessionManager


class UiServiceTests(unittest.TestCase):
    def test_session_exposes_state_actions_and_snapshot(self):
        manager = SessionManager()
        session = manager.create_session()

        self.assertEqual(session["mode"], "offline_simulator")
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


if __name__ == "__main__":
    unittest.main()
