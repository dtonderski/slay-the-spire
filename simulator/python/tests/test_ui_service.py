import unittest

from sts.ui_service import CombatSession, SessionManager, _observed_state_from_bridge_status


class EmptyActionEnv:
    def snapshot_hash(self):
        return "empty-state"

    def state_json(self):
        return "{}"

    def snapshot_json(self):
        return "{}"

    def phase(self):
        return "idle"

    def exact_legal_actions(self):
        return []


class FakeEndTurnAction:
    def kind(self):
        return "end_turn"

    def json(self):
        return '{"kind":"end_turn"}'


class InvalidStepEnv:
    def snapshot_hash(self):
        return "invalid-step-state"

    def state_json(self):
        return "{}"

    def snapshot_json(self):
        return "{}"

    def phase(self):
        return "combat"

    def exact_legal_actions(self):
        return [FakeEndTurnAction()]

    def step(self, _action):
        raise RuntimeError("simulator said no")


class UiServiceTests(unittest.TestCase):
    def test_observed_bridge_state_extracts_game_state_message(self):
        observed = {"current_hp": 80, "max_hp": 80, "floor": 1}

        result = _observed_state_from_bridge_status(
            {"current_state": {"message": {"game_state": observed}}}
        )

        self.assertEqual(result, observed)

    def test_observed_bridge_state_rejects_command_only_message(self):
        with self.assertRaises(ValueError):
            _observed_state_from_bridge_status(
                {"current_state": {"message": {"available_commands": ["start", "state"]}}}
            )

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

        state = manager.state(session["session_id"])
        self.assertEqual(state["state_id"], session["state_id"])

        actions = manager.actions(session["session_id"])
        self.assertEqual(actions["state_id"], session["state_id"])
        self.assertEqual(actions["actions"], session["actions"])

        pending = manager.pending_command(session["session_id"])
        self.assertEqual(pending["command_lifecycle"]["status"], "ready")

    def test_step_rejects_stale_action_without_mutating_state(self):
        manager = SessionManager()
        session = manager.create_session()
        action = dict(session["actions"][0])
        action["source_state_id"] = "old"

        result = manager.step(session["session_id"], action)

        self.assertEqual(result["state_id"], session["state_id"])
        self.assertEqual(result["command_lifecycle"]["status"], "stale")
        self.assertTrue(result["actions"])

        pending = manager.pending_command(session["session_id"])
        self.assertEqual(pending["command_lifecycle"]["status"], "stale")
        self.assertEqual(pending["command_lifecycle"]["expected_state_id"], session["state_id"])
        self.assertIn("command_id", pending["command_lifecycle"])

    def test_double_click_old_action_is_stale_and_recoverable(self):
        manager = SessionManager()
        session = manager.create_session()
        action = session["actions"][0]

        applied = manager.step(session["session_id"], action)
        duplicate = manager.step(session["session_id"], action)

        self.assertEqual(duplicate["state_id"], applied["state_id"])
        self.assertEqual(duplicate["command_lifecycle"]["status"], "stale")
        self.assertEqual(duplicate["command_lifecycle"]["received_state_id"], session["state_id"])
        self.assertEqual(duplicate["command_lifecycle"]["expected_state_id"], applied["state_id"])
        self.assertNotEqual(
            duplicate["command_lifecycle"]["command_id"],
            applied["command_lifecycle"]["command_id"],
        )
        self.assertTrue(duplicate["actions"] or duplicate["empty_action_reason"])

    def test_step_rejects_unknown_action_without_clearing_actions(self):
        manager = SessionManager()
        session = manager.create_session()

        result = manager.step(
            session["session_id"],
            {"action_id": "missing", "source_state_id": session["state_id"]},
        )

        self.assertEqual(result["state_id"], session["state_id"])
        self.assertEqual(result["command_lifecycle"]["status"], "rejected")
        self.assertTrue(result["command_lifecycle"]["state_unchanged"])
        self.assertIn("command_id", result["command_lifecycle"])
        self.assertEqual(result["actions"], session["actions"])

    def test_step_rejects_invalid_simulator_action_without_clearing_actions(self):
        manager = SessionManager()
        session = CombatSession(
            id="invalid",
            mode="test_invalid",
            state_kind="combat",
            env=InvalidStepEnv(),
        )
        manager._sessions[session.id] = session
        before = manager.get_session(session.id)

        result = manager.step(session.id, before["actions"][0])

        self.assertEqual(result["state_id"], before["state_id"])
        self.assertEqual(result["command_lifecycle"]["status"], "rejected")
        self.assertTrue(result["command_lifecycle"]["state_unchanged"])
        self.assertIn("simulator said no", result["command_lifecycle"]["error"])
        self.assertEqual(result["actions"], before["actions"])

    def test_step_applies_action_and_regenerates_actions(self):
        manager = SessionManager()
        session = manager.create_session()
        action = session["actions"][0]

        result = manager.step(session["session_id"], action)

        self.assertNotEqual(result["state_id"], session["state_id"])
        self.assertEqual(result["command_lifecycle"]["status"], "applied")
        self.assertIn("command_id", result["command_lifecycle"])
        self.assertTrue(result["actions"] or result["empty_action_reason"])

        pending = manager.pending_command(session["session_id"])
        self.assertEqual(pending["command_lifecycle"]["status"], "applied")
        self.assertEqual(pending["state_id"], result["state_id"])

    def test_restore_replaces_combat_session_from_snapshot(self):
        manager = SessionManager()
        session = manager.create_session()
        snapshot = manager.snapshot(session["session_id"])
        stepped = manager.step(session["session_id"], session["actions"][0])
        self.assertNotEqual(stepped["state_id"], snapshot["state_id"])

        restored = manager.restore(session["session_id"], {"snapshot_json": snapshot["snapshot_json"]})

        self.assertEqual(restored["state_id"], snapshot["state_id"])
        self.assertEqual(restored["command_lifecycle"]["status"], "restored")
        self.assertIn("command_id", restored["command_lifecycle"])
        self.assertEqual(restored["state_kind"], "combat")
        self.assertTrue(restored["actions"])

        pending = manager.pending_command(session["session_id"])
        self.assertEqual(pending["command_lifecycle"]["status"], "restored")
        self.assertEqual(pending["state_id"], snapshot["state_id"])

    def test_restore_replaces_run_session_from_snapshot(self):
        manager = SessionManager()
        session = manager.create_session("run_map_fixture")
        snapshot = manager.snapshot(session["session_id"])
        stepped = manager.step(session["session_id"], session["actions"][0])
        self.assertNotEqual(stepped["state_id"], snapshot["state_id"])

        restored = manager.restore(session["session_id"], {"snapshot_json": snapshot["snapshot_json"]})

        self.assertEqual(restored["state_id"], snapshot["state_id"])
        self.assertEqual(restored["state_kind"], "run")
        self.assertEqual(restored["current_decision"], "map")

    def test_restore_rejects_missing_snapshot_json(self):
        manager = SessionManager()
        session = manager.create_session()

        with self.assertRaises(ValueError):
            manager.restore(session["session_id"], {})

    def test_empty_action_list_reports_explicit_reason(self):
        manager = SessionManager()
        session = CombatSession(
            id="empty",
            mode="test_empty",
            state_kind="combat",
            env=EmptyActionEnv(),
        )
        manager._sessions[session.id] = session

        result = manager.get_session(session.id)

        self.assertFalse(result["actions"])
        self.assertEqual(result["empty_action_reason"]["kind"], "unsupported")
        self.assertIn("no exact combat actions", result["empty_action_reason"]["reason"])

    def test_search_returns_best_current_action_id(self):
        manager = SessionManager()
        session = manager.create_session()

        result = manager.search(session["session_id"], {"max_depth": 1})

        recommendation = result["recommendation"]
        action_ids = {action["action_id"] for action in session["actions"]}
        self.assertIn(recommendation["best_action_id"], action_ids)
        self.assertTrue(recommendation["principal_variation"])
        self.assertEqual(
            recommendation["config"]["algorithm"],
            "rust_terminal_hp_commit_safe_selector",
        )

    def test_search_accepts_named_policy_override(self):
        manager = SessionManager()
        session = manager.create_session()

        result = manager.search(
            session["session_id"],
            {
                "candidate": "rust_beam_terminal_w32_d40",
                "max_depth": 1,
                "allowed_potions": ["Weak Potion"],
            },
        )

        recommendation = result["recommendation"]
        self.assertEqual(recommendation["config"]["algorithm"], "rust_beam")
        self.assertEqual(recommendation["config"]["beam_width"], 32)
        self.assertEqual(recommendation["config"]["allowed_potions"], ("Weak Potion",))

    def test_search_rejects_unknown_named_policy(self):
        manager = SessionManager()
        session = manager.create_session()

        with self.assertRaises(ValueError):
            manager.search(session["session_id"], {"candidate": "missing"})

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

    def test_run_combat_fixture_can_search_current_combat(self):
        manager = SessionManager()
        session = manager.create_session("run_combat_fixture")

        result = manager.search(session["session_id"], {"max_depth": 1})

        action_ids = {action["action_id"] for action in session["actions"]}
        self.assertIn(result["recommendation"]["best_action_id"], action_ids)
        self.assertEqual(result["recommendation"]["best_action"]["kind"], "ExactRunAction")

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
