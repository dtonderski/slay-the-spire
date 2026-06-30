import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from sts.bridge import BridgeMirror, command_for_descriptor
from sts.ui_service import (
    CombatSession,
    SessionManager,
    _bridge_action_for_exact_action,
    _collector_start_payload,
    _collector_status_with_preflight,
    _guided_collect_report,
    _guided_script_from_payload,
    _observed_state_from_bridge_status,
    _slaythedata_candidates_from_query,
    _slaythedata_status_from_query,
    _start_guided_live_run,
    _tick_live_collector,
)
from sts.guided_collector import GuidedCollector
from sts.slaythedata_policy import build_guided_run_script


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


class FakeLiveEnv:
    def snapshot_hash(self):
        return "fake-live-state"

    def state_json(self):
        return '{"phase":"Combat","combat":{"player":{"hp":80},"monsters":[],"piles":{"hand":[]}}}'

    def snapshot_json(self):
        return "{}"

    def phase(self):
        return "combat"

    def current_decision(self):
        return "combat"

    def exact_legal_actions(self):
        return []


class FakeRunAction:
    def __init__(self, kind, payload):
        self._kind = kind
        self._payload = payload

    def family(self):
        return "run"

    def kind(self):
        return self._kind

    def json(self):
        import json

        return json.dumps(self._payload, sort_keys=True)


class FakeEventRunEnv:
    def __init__(self):
        self.action = FakeRunAction("event_choose", {"Choose": {"choice_index": 0}})

    def snapshot_hash(self):
        return "fake-event-state"

    def state_json(self):
        return '{"phase":"event"}'

    def snapshot_json(self):
        return "{}"

    def phase(self):
        return "event"

    def current_decision(self):
        return "event"

    def exact_legal_actions(self):
        return [self.action]


class FakeBridge:
    def __init__(self, status):
        self._status = status
        self.sent = []

    def status(self):
        return self._status

    def preflight(self):
        return {"ok": False, "problems": ["not ready"], "warnings": []}

    def send_command(self, command, **kwargs):
        self.sent.append((command, kwargs))
        return {"ok": True, "command_id": "cmd-guided", "command": command}


class MutableFakeBridge(FakeBridge):
    def set_status(self, status):
        self._status = status


class FakeCollectorManager:
    def send_live_non_combat_action(self, bridge_status, suggestion, payload, *, send_command):
        command = command_for_descriptor(suggestion["descriptor"])
        result = send_command(
            command,
            source_state_id=bridge_status.get("state_id"),
            metadata=payload.get("provenance"),
        )
        return {
            "source_state_id": f"sim-{bridge_status.get('state_id')}",
            "bridge_state_id": bridge_status.get("state_id"),
            "bridge_step": bridge_status.get("last_state_step"),
            "predicted_state_id": bridge_status["next_predicted_state_id"],
            "send_result": {
                "ok": result.get("ok"),
                "command_id": result.get("command_id"),
                "command": result.get("command"),
            },
        }

    def send_live_combat_action(self, bridge_status, suggestion, payload, *, send_command):
        result = send_command(
            "END",
            source_state_id=bridge_status.get("state_id"),
            metadata=payload.get("provenance"),
        )
        return {
            "source_state_id": f"sim-{bridge_status.get('state_id')}",
            "bridge_state_id": bridge_status.get("state_id"),
            "bridge_step": bridge_status.get("last_state_step"),
            "predicted_state_id": bridge_status["next_predicted_state_id"],
            "recommendation": {"best_action": {"kind": "EndTurn"}},
            "send_result": {
                "ok": result.get("ok"),
                "command_id": result.get("command_id"),
                "command": result.get("command"),
            },
        }

    def verify_live_prediction(self, prediction, *, bridge_status):
        expected = prediction.get("predicted_state_id")
        observed = bridge_status.get("state_id")
        if expected == observed:
            return {"status": "matched", "expected_state_id": expected, "observed_state_id": observed}
        return {
            "status": "mismatch",
            "detail": f"expected {expected}, observed {observed}",
            "expected_state_id": expected,
            "observed_state_id": observed,
        }


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

    def test_guided_script_payload_accepts_exported_slaythedata_run(self):
        result = _guided_script_from_payload(
            {
                "exported_run": {
                    "run_id": 7,
                    "event": {
                        "character_chosen": "IRONCLAD",
                        "ascension_level": 0,
                        "seed_played": "ABC",
                        "card_choices": [{"floor": 1, "picked": "Inflame"}],
                    },
                }
            }
        )

        self.assertEqual(result["script"]["source"]["run_id"], 7)
        self.assertEqual(result["script"]["config"]["seed_played"], "ABC")
        self.assertEqual(result["script"]["floor_decisions"][0]["card_rewards"][0]["picked"], "Inflame")

    def test_guided_script_payload_rejects_missing_source(self):
        with self.assertRaises(ValueError):
            _guided_script_from_payload({})

    def test_guided_script_payload_accepts_slaythedata_run_id(self):
        with patch("sts.ui_service.export_guided_run_script", return_value={"schema": 1}) as export:
            result = _guided_script_from_payload({"run_id": 123})

        self.assertEqual(result, {"script": {"schema": 1}})
        export.assert_called_once_with(123)

    def test_collector_start_payload_accepts_slaythedata_run_id(self):
        with patch("sts.ui_service.export_guided_run_script", return_value={"schema": 1}) as export:
            result = _collector_start_payload({"run_id": "123"})

        self.assertEqual(result, {"script": {"schema": 1}})
        export.assert_called_once_with(123)

    def test_collector_status_includes_bridge_preflight(self):
        bridge = FakeBridge({"connected": True})
        result = _collector_status_with_preflight(GuidedCollector(), bridge)

        self.assertEqual(result["preflight"]["problems"], ["not ready"])

    def test_guided_collect_report_reports_missing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            result = _guided_collect_report(Path(directory) / "missing.json")

        self.assertFalse(result["ok"])
        self.assertTrue(result["missing"])
        self.assertIn("not found", result["error"])

    def test_guided_collect_report_summarizes_blocked_report(self):
        with tempfile.TemporaryDirectory() as directory:
            report_path = Path(directory) / "latest.json"
            report_path.write_text(
                json.dumps(
                    {
                        "ok": False,
                        "run_id": 123,
                        "seed": None,
                        "stop_reason": "preflight_blocked",
                        "actions_sent": 0,
                        "trace_path": "trace.jsonl",
                        "bridge_step": 7,
                        "tcp_control_available": False,
                        "blocker": {
                            "reason": "bridge_preflight",
                            "problems": ["session files are stale"],
                            "warnings": ["TCP bridge control is not available"],
                        },
                        "history_tail": [{"event": "preflight"}],
                    }
                ),
                encoding="utf-8",
            )

            result = _guided_collect_report(report_path)

        self.assertFalse(result["ok"])
        self.assertFalse(result["missing"])
        self.assertEqual(result["run_id"], 123)
        self.assertEqual(result["stop_reason"], "preflight_blocked")
        self.assertEqual(result["blocker"]["reason"], "bridge_preflight")
        self.assertEqual(result["history_tail_count"], 1)

    def test_start_guided_live_run_sends_script_start_with_provenance(self):
        collector = GuidedCollector()
        collector.start(
            {
                "script": build_guided_run_script(
                    {
                        "run_id": 77,
                        "event": {
                            "character_chosen": "IRONCLAD",
                            "ascension_level": 3,
                            "seed_played": "LIVE01",
                        },
                    }
                )
            }
        )
        bridge = FakeBridge({"state_id": "menu-state"})

        result = _start_guided_live_run(collector, bridge)

        self.assertEqual(result["command"], "START IRONCLAD 3 LIVE01")
        self.assertEqual(bridge.sent[0][0], "START IRONCLAD 3 LIVE01")
        self.assertEqual(bridge.sent[0][1]["source_state_id"], "menu-state")
        self.assertTrue(bridge.sent[0][1]["require_tcp_control"])
        self.assertEqual(bridge.sent[0][1]["metadata"]["source"], "guided_collector_start")
        self.assertEqual(bridge.sent[0][1]["metadata"]["script_source"]["run_id"], 77)

    def test_start_guided_live_run_requires_active_script_seed(self):
        collector = GuidedCollector()
        bridge = FakeBridge({"state_id": "menu-state"})
        with self.assertRaisesRegex(ValueError, "active collector"):
            _start_guided_live_run(collector, bridge)

        collector.start({"script": {"config": {"character": "IRONCLAD", "ascension": 0}}})
        with self.assertRaisesRegex(ValueError, "seed is required"):
            _start_guided_live_run(collector, bridge)

    def test_guided_start_then_strict_collector_tick_smoke(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._write_bridge_files(
                root,
                status={"status": "waiting", "step": 0},
                summary={
                    "ready_for_command": True,
                    "available_commands": ["start", "state"],
                    "in_game": False,
                    "step": 0,
                },
                current_state={"message": {"available_commands": ["start", "state"]}},
            )
            bridge = BridgeMirror(root, stale_after_seconds=9999)
            manager = SessionManager()
            manager._sessions["live"] = CombatSession(
                id="live",
                mode="live_bridge",
                state_kind="run",
                env=FakeEventRunEnv(),
            )
            collector = GuidedCollector()
            collector.start(
                {
                    "script": build_guided_run_script(
                        {
                            "run_id": 99,
                            "event": {
                                "character_chosen": "IRONCLAD",
                                "ascension_level": 0,
                                "seed_played": "LIVE01",
                                "event_choices": [
                                    {"floor": 2, "event_name": "Golden Shrine", "player_choice": "Pray"}
                                ],
                            },
                        }
                    )
                }
            )

            start = _start_guided_live_run(collector, bridge, require_tcp_control=False)
            command_meta = json.loads((root / "next_command.json").read_text(encoding="utf-8"))
            self.assertEqual(start["command"], "START IRONCLAD 0 LIVE01")
            self.assertEqual((root / "next_command.txt").read_text(encoding="utf-8"), "START IRONCLAD 0 LIVE01\n")
            self.assertEqual(command_meta["metadata"]["source"], "guided_collector_start")
            self.assertEqual(command_meta["metadata"]["script_source"]["run_id"], 99)

            (root / "next_command.txt").unlink()
            (root / "next_command.json").unlink()
            self._write_bridge_files(
                root,
                status={"status": "waiting", "step": 12},
                summary={
                    "floor": 2,
                    "screen_type": "EVENT",
                    "choices": ["Pray", "Leave"],
                    "ready_for_command": True,
                    "available_commands": ["choose", "state"],
                    "step": 12,
                },
                current_state={
                    "message": {
                        "game_state": {
                            "floor": 2,
                            "screen_type": "EVENT",
                            "choice_list": ["Pray", "Leave"],
                        }
                    }
                },
            )
            live_session = {
                "session_id": "live",
                "state_id": "fake-event-state",
                "attach_fidelity": "seed_replay",
                "state_kind": "run",
                "state": {"phase": "event"},
            }
            with patch.object(manager, "create_live_session", return_value=live_session), patch.object(
                manager,
                "predict",
                return_value={"predicted_state_id": "predicted-event-state"},
            ):
                sent = _tick_live_collector(
                    collector,
                    manager,
                    bridge,
                    {"send": True},
                    require_tcp_control=False,
                )

            self.assertEqual(sent["suggestion"]["status"], "sent_non_combat")
            self.assertEqual(sent["pending_prediction"]["predicted_state_id"], "predicted-event-state")
            self.assertEqual((root / "next_command.txt").read_text(encoding="utf-8"), "CHOOSE 0\n")
            choose_meta = json.loads((root / "next_command.json").read_text(encoding="utf-8"))
            self.assertEqual(choose_meta["metadata"]["source"], "guided_collector")

            (root / "next_command.txt").unlink()
            (root / "next_command.json").unlink()
            observed_session = live_session | {"state_id": "predicted-event-state"}
            with patch.object(manager, "create_live_session", return_value=observed_session):
                verified = _tick_live_collector(collector, manager, bridge, {"send": False})

            self.assertIsNone(verified["pending_prediction"])

    def test_guided_auto_collection_multiscreen_smoke(self):
        def bridge_status(
            state_id,
            *,
            floor,
            screen_type=None,
            choices=None,
            available_commands=None,
            game_state=None,
            summary_extra=None,
            next_predicted_state_id=None,
        ):
            summary = {
                "floor": floor,
                "ready_for_command": True,
                "available_commands": available_commands or ["choose", "state"],
                "step": len(state_id),
            }
            if screen_type:
                summary["screen_type"] = screen_type
            if choices is not None:
                summary["choices"] = choices
            if summary_extra:
                summary.update(summary_extra)
            state = game_state or {
                "floor": floor,
                "screen_type": screen_type,
                "choice_list": choices or [],
            }
            return {
                "connected": True,
                "exited": False,
                "pending_command": False,
                "ready_for_command": True,
                "state_id": state_id,
                "last_state_step": len(state_id),
                "summary": summary,
                "current_state": {"message": {"game_state": state}},
                "next_predicted_state_id": next_predicted_state_id or f"after-{state_id}",
            }

        script = build_guided_run_script(
            {
                "run_id": 1234,
                "event": {
                    "character_chosen": "IRONCLAD",
                    "ascension_level": 0,
                    "seed_played": "GUIDED01",
                    "neow_bonus": "THREE_CARDS",
                    "neow_cost": "NONE",
                    "path_per_floor": ["M", "?", "$"],
                    "card_choices": [
                        {"floor": 0, "picked": "True Grit", "not_picked": ["Flex", "Anger"]},
                        {"floor": 1, "picked": "SKIP", "not_picked": ["Clash", "Flex", "Anger"]},
                    ],
                },
            }
        )
        collector = GuidedCollector()
        collector.start({"script": script})
        manager = FakeCollectorManager()
        bridge = MutableFakeBridge(
            {
                "connected": True,
                "exited": False,
                "pending_command": False,
                "ready_for_command": True,
                "state_id": "menu",
                "summary": {
                    "ready_for_command": True,
                    "available_commands": ["start", "state"],
                    "in_game": False,
                },
            }
        )

        _start_guided_live_run(collector, bridge)
        self.assertEqual(bridge.sent[-1][0], "START IRONCLAD 0 GUIDED01")

        screens = [
            bridge_status(
                "neow-talk",
                floor=0,
                screen_type="EVENT",
                choices=["talk"],
                game_state={
                    "floor": 0,
                    "screen_type": "EVENT",
                    "choice_list": ["talk"],
                    "screen_state": {"event_name": "Neow", "event_id": "Neow Event"},
                },
                next_predicted_state_id="neow-bonus",
            ),
            bridge_status(
                "neow-bonus",
                floor=0,
                screen_type="EVENT",
                choices=["choose a card to obtain"],
                game_state={
                    "floor": 0,
                    "screen_type": "EVENT",
                    "choice_list": ["choose a card to obtain"],
                    "screen_state": {"event_name": "Neow", "event_id": "Neow Event"},
                },
                next_predicted_state_id="neow-card",
            ),
            bridge_status(
                "neow-card",
                floor=0,
                screen_type="CARD_REWARD",
                choices=["Flex", "True Grit", "Anger"],
                available_commands=["choose", "skip", "state"],
                next_predicted_state_id="map-0",
            ),
            bridge_status(
                "map-0",
                floor=0,
                screen_type="MAP",
                choices=["x=1", "x=2"],
                game_state={
                    "floor": 0,
                    "act": 1,
                    "screen_type": "MAP",
                    "choice_list": ["x=1", "x=2"],
                    "screen_state": {
                        "next_nodes": [
                            {"x": 1, "y": 0, "symbol": "M"},
                            {"x": 2, "y": 0, "symbol": "M"},
                        ]
                    },
                    "map": [
                        {"x": 1, "y": 0, "symbol": "M", "children": [{"x": 1, "y": 1}]},
                        {"x": 2, "y": 0, "symbol": "M", "children": [{"x": 2, "y": 1}]},
                        {"x": 1, "y": 1, "symbol": "?", "children": [{"x": 1, "y": 2}]},
                        {"x": 2, "y": 1, "symbol": "$", "children": [{"x": 2, "y": 2}]},
                        {"x": 1, "y": 2, "symbol": "$", "children": []},
                        {"x": 2, "y": 2, "symbol": "?", "children": []},
                    ],
                },
                next_predicted_state_id="combat-1",
            ),
            bridge_status(
                "combat-1",
                floor=1,
                available_commands=["end", "state"],
                summary_extra={"phase": "combat", "combat": {"monsters": []}},
                game_state={"floor": 1, "screen_type": "NONE", "choice_list": []},
                next_predicted_state_id="card-skip",
            ),
            bridge_status(
                "card-skip",
                floor=1,
                screen_type="CARD_REWARD",
                choices=["Clash", "Flex", "Anger"],
                available_commands=["choose", "skip", "state"],
                next_predicted_state_id="after-skip",
            ),
        ]

        for status in screens:
            bridge.set_status(status)
            result = _tick_live_collector(collector, manager, bridge, {"send": True, "max_depth": 3})
            self.assertIsNotNone(result["pending_prediction"])
            self.assertEqual(result["pending_prediction"]["predicted_state_id"], status["next_predicted_state_id"])
            self.assertEqual(result["status"], "ready")

        self.assertEqual(
            [command for command, _kwargs in bridge.sent],
            [
                "START IRONCLAD 0 GUIDED01",
                "CHOOSE 0",
                "CHOOSE 0",
                "CHOOSE 1",
                "CHOOSE 0",
                "END",
                "SKIP",
            ],
        )
        self.assertEqual(bridge.sent[5][1]["metadata"]["suggestion"]["mode"], "combat_agent")
        self.assertEqual(bridge.sent[6][1]["metadata"]["suggestion"]["target"], "SKIP")

    def test_slaythedata_candidates_query_uses_filters(self):
        with patch(
            "sts.ui_service.select_guided_collection_candidates",
            return_value=[{"id": 1}],
        ) as select:
            result = _slaythedata_candidates_from_query(
                {
                    "character": ["ironclad"],
                    "ascension": ["0"],
                    "min_floor": ["10"],
                    "max_floor": ["55"],
                    "min_path_length": ["10"],
                    "min_card_choices": ["8"],
                    "min_event_choices": ["1"],
                    "min_shop_purchases": ["1"],
                    "min_potion_usage": ["0"],
                    "safe_neow": ["1"],
                    "limit": ["3"],
                    "ranked": ["0"],
                }
            )

        self.assertEqual(result["candidates"], [{"id": 1}])
        self.assertEqual(result["filters"]["character"], "IRONCLAD")
        self.assertFalse(result["filters"]["ranked"])
        select.assert_called_once_with(
            character="IRONCLAD",
            ascension=0,
            min_floor_reached=10,
            max_floor_reached=55,
            min_path_length=10,
            min_card_choices=8,
            min_event_choices=1,
            min_shop_purchases=1,
            min_potion_usage=0,
            require_guided_safe_neow=True,
            limit=3,
            ranked=False,
        )

    def test_slaythedata_candidates_default_to_safe_neow(self):
        with patch(
            "sts.ui_service.select_guided_collection_candidates",
            return_value=[],
        ) as select:
            result = _slaythedata_candidates_from_query({})

        self.assertTrue(result["filters"]["safe_neow"])
        self.assertEqual(result["candidates"], [])
        self.assertTrue(select.call_args.kwargs["require_guided_safe_neow"])

    def test_slaythedata_status_query_uses_filters(self):
        with patch(
            "sts.ui_service.slaythedata_index_status",
            return_value={"ok": True},
        ) as status:
            result = _slaythedata_status_from_query(
                {
                    "character": ["ironclad"],
                    "ascension": ["0"],
                    "min_floor": ["40"],
                    "min_path_length": ["40"],
                    "include_counts": ["1"],
                }
            )

        self.assertEqual(result, {"ok": True})
        status.assert_called_once_with(
            character="IRONCLAD",
            ascension=0,
            min_floor_reached=40,
            min_path_length=40,
            include_counts=True,
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
        self.assertIn("predicted_final_hp", recommendation)
        self.assertIn("predicted_hp_loss", recommendation)

    def test_predict_returns_next_state_without_mutating_session(self):
        manager = SessionManager()
        session = manager.create_session()
        action = session["actions"][0]

        prediction = manager.predict(
            session["session_id"],
            {
                "action_id": action["action_id"],
                "source_state_id": session["state_id"],
            },
        )
        after = manager.state(session["session_id"])

        self.assertEqual(prediction["source_state_id"], session["state_id"])
        self.assertNotEqual(prediction["predicted_state_id"], session["state_id"])
        self.assertEqual(after["state_id"], session["state_id"])

    def test_predict_rejects_stale_source_state(self):
        manager = SessionManager()
        session = manager.create_session()

        with self.assertRaises(ValueError):
            manager.predict(
                session["session_id"],
                {
                    "action_id": session["actions"][0]["action_id"],
                    "source_state_id": "old-state",
                },
            )

    def test_live_session_prefers_verified_strict_replay_env(self):
        manager = SessionManager()
        replay = SimpleNamespace(
            verified=True,
            env=FakeLiveEnv(),
            trace_path="trace.jsonl",
            steps=3,
            final_state_id="fake-live-state",
            final_phase="combat",
        )

        with patch("sts.ui_service._strict_replay_from_bridge_trace", return_value=replay):
            session = manager.create_live_session(
                {"state_id": "bridge-state", "last_state_step": 12, "trace_path": "trace.jsonl"}
            )

        self.assertEqual(session["attach_fidelity"], "seed_replay")
        self.assertEqual(session["strict_replay_blocker"], None)
        self.assertEqual(session["attach_source"]["final_state_id"], "fake-live-state")
        self.assertEqual(session["state_id"], "fake-live-state")

    def test_live_session_falls_back_to_observed_state_with_replay_blocker(self):
        manager = SessionManager()
        replay = SimpleNamespace(
            verified=False,
            env=None,
            trace_path="trace.jsonl",
            steps=2,
            stop_reason="observed_state_diff",
            blocker={"category": "observed_state_diff"},
            final_state_id="old-state",
        )
        bridge_status = {
            "state_id": "bridge-state",
            "last_state_step": 12,
            "trace_path": "trace.jsonl",
            "current_state": {"message": {"game_state": {"current_hp": 80, "max_hp": 80, "floor": 1}}},
        }

        with patch("sts.ui_service._strict_replay_from_bridge_trace", return_value=replay), patch(
            "sts.ui_service.omni.OmniRunEnv.from_communication_mod_state_json",
            return_value=FakeLiveEnv(),
        ):
            session = manager.create_live_session(bridge_status)

        self.assertEqual(session["attach_fidelity"], "observed_state")
        self.assertEqual(session["strict_replay_blocker"]["stop_reason"], "observed_state_diff")
        self.assertEqual(session["strict_replay_blocker"]["blocker"]["category"], "observed_state_diff")

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

    def test_search_rejects_stale_source_state(self):
        manager = SessionManager()
        session = manager.create_session()

        with self.assertRaises(ValueError):
            manager.search(session["session_id"], {"max_depth": 1, "source_state_id": "old-state"})

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

    def test_bridge_action_for_exact_action_maps_run_play_card_to_visible_slots(self):
        action = {
            "kind": "ExactRunAction",
            "action_kind": "play_card",
            "action": {"PlayCard": {"card_id": 101, "target": 7}},
        }
        bridge_status = {
            "bridge_actions": [
                {
                    "command": "PLAY 1 0",
                    "descriptor": {"kind": "PlayHandSlot", "hand_slot": 1, "target_slot": 0},
                }
            ],
            "summary": {
                "combat": {
                    "hand": [{"index": 1, "id": 101}],
                    "monsters": [{"index": 0, "id": 7}],
                }
            },
        }

        result = _bridge_action_for_exact_action(action, bridge_status, {"combat": {}})

        self.assertEqual(result["command"], "PLAY 1 0")

    def test_send_live_combat_action_attaches_searches_predicts_and_sends_bridge_command(self):
        manager = SessionManager()
        manager._sessions["live"] = CombatSession(
            id="live",
            mode="live_bridge",
            state_kind="run",
            env=FakeLiveEnv(),
        )
        bridge_status = {
            "state_id": "bridge-state",
            "last_state_step": 12,
            "bridge_actions": [
                {
                    "command": "PLAY 1 0",
                    "descriptor": {"kind": "PlayHandSlot", "hand_slot": 1, "target_slot": 0},
                }
            ],
            "summary": {
                "combat": {
                    "hand": [{"index": 1, "id": 101}],
                    "monsters": [{"index": 0, "id": 7}],
                }
            },
        }
        live_session = {
            "session_id": "live",
            "state_id": "fake-live-state",
            "attach_fidelity": "seed_replay",
            "state": {"combat": {}},
        }
        recommendation = {
            "best_action": {
                "kind": "ExactRunAction",
                "action_kind": "play_card",
                "action": {"PlayCard": {"card_id": 101, "target": 7}},
            }
        }
        sent = []

        with patch.object(manager, "create_live_session", return_value=live_session), patch.object(
            manager, "search", return_value={"recommendation": recommendation}
        ) as search, patch.object(
            manager,
            "predict",
            return_value={"predicted_state_id": "predicted-live-state"},
        ) as predict:
            result = manager.send_live_combat_action(
                bridge_status,
                {"status": "combat", "potion_uses_allowed": 0},
                {"potion_uses_allowed": 0, "max_depth": 5},
                send_command=lambda command, **kwargs: sent.append((command, kwargs))
                or {"ok": True, "command_id": "cmd-1", "command": command},
            )

        self.assertEqual(result["bridge_action"]["command"], "PLAY 1 0")
        self.assertEqual(result["predicted_state_id"], "predicted-live-state")
        self.assertEqual(sent, [("PLAY 1 0", {"source_state_id": "bridge-state"})])
        self.assertEqual(search.call_args.args[1]["source_state_id"], "fake-live-state")
        self.assertEqual(search.call_args.args[1]["allowed_potions"], [])
        self.assertEqual(predict.call_args.args[1]["source_state_id"], "fake-live-state")

    def test_send_live_non_combat_action_requires_strict_replay_and_predicts(self):
        manager = SessionManager()
        manager._sessions["live"] = CombatSession(
            id="live",
            mode="live_bridge",
            state_kind="run",
            env=FakeEventRunEnv(),
        )
        bridge_status = {
            "state_id": "bridge-state",
            "last_state_step": 12,
            "current_state": {
                "message": {
                    "game_state": {
                        "floor": 2,
                        "screen_type": "EVENT",
                        "choice_list": ["Pray", "Leave"],
                    }
                }
            },
        }
        live_session = {
            "session_id": "live",
            "state_id": "fake-event-state",
            "attach_fidelity": "seed_replay",
            "state_kind": "run",
            "state": {"phase": "event"},
        }
        sent = []

        with patch.object(manager, "create_live_session", return_value=live_session), patch.object(
            manager,
            "predict",
            return_value={"predicted_state_id": "predicted-event-state"},
        ) as predict:
            result = manager.send_live_non_combat_action(
                bridge_status,
                {
                    "status": "matched",
                    "descriptor": {"kind": "ChooseVisibleOption", "option_slot": 0},
                },
                {},
                send_command=lambda command, **kwargs: sent.append((command, kwargs))
                or {"ok": True, "command_id": "cmd-event", "command": command},
            )

        self.assertEqual(result["command"], "CHOOSE 0")
        self.assertEqual(result["predicted_state_id"], "predicted-event-state")
        self.assertEqual(sent, [("CHOOSE 0", {"source_state_id": "bridge-state"})])
        self.assertEqual(predict.call_args.args[1]["action_id"], "a0")
        self.assertEqual(predict.call_args.args[1]["source_state_id"], "fake-event-state")

    def test_tick_live_collector_wires_bridge_sender_and_prediction_verifier(self):
        manager = SessionManager()
        manager._sessions["live"] = CombatSession(
            id="live",
            mode="live_bridge",
            state_kind="run",
            env=FakeEventRunEnv(),
        )
        collector = GuidedCollector()
        collector.start(
            {
                "script": build_guided_run_script(
                    {
                        "run_id": 42,
                        "event": {
                            "event_choices": [
                                {"floor": 2, "event_name": "Golden Shrine", "player_choice": "Pray"}
                            ],
                        },
                    }
                )
            }
        )
        bridge_status = {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "bridge-state",
            "last_state_step": 12,
            "current_state": {
                "message": {
                    "game_state": {
                        "floor": 2,
                        "screen_type": "EVENT",
                        "choice_list": ["Pray", "Leave"],
                    }
                }
            },
            "summary": {
                "floor": 2,
                "screen_type": "EVENT",
                "choices": ["Pray", "Leave"],
                "available_commands": ["choose"],
            },
        }
        bridge = FakeBridge(bridge_status)
        live_session = {
            "session_id": "live",
            "state_id": "fake-event-state",
            "attach_fidelity": "seed_replay",
            "state_kind": "run",
            "state": {"phase": "event"},
        }

        with patch.object(manager, "create_live_session", return_value=live_session), patch.object(
            manager,
            "predict",
            return_value={"predicted_state_id": "predicted-event-state"},
        ):
            sent = _tick_live_collector(collector, manager, bridge, {"send": True})

        self.assertEqual(sent["suggestion"]["status"], "sent_non_combat")
        self.assertEqual(sent["pending_prediction"]["predicted_state_id"], "predicted-event-state")
        self.assertEqual(bridge.sent[0][0], "CHOOSE 0")
        self.assertEqual(bridge.sent[0][1]["source_state_id"], "bridge-state")
        self.assertTrue(bridge.sent[0][1]["require_tcp_control"])
        self.assertEqual(bridge.sent[0][1]["metadata"]["source"], "guided_collector")

        observed_session = live_session | {"state_id": "predicted-event-state"}
        with patch.object(manager, "create_live_session", return_value=observed_session):
            verified = _tick_live_collector(collector, manager, bridge, {"send": False})

        self.assertIsNone(verified["pending_prediction"])

    def test_verify_live_prediction_reports_mismatch(self):
        manager = SessionManager()
        live_session = {
            "session_id": "live",
            "state_id": "observed-live-state",
            "attach_fidelity": "seed_replay",
        }

        with patch.object(manager, "create_live_session", return_value=live_session):
            result = manager.verify_live_prediction(
                {"predicted_state_id": "predicted-live-state"},
                bridge_status={"state_id": "bridge-state"},
            )

        self.assertEqual(result["status"], "mismatch")
        self.assertEqual(result["expected_state_id"], "predicted-live-state")
        self.assertEqual(result["observed_state_id"], "observed-live-state")

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

    def _write_bridge_files(self, root, *, status, summary, current_state):
        root.mkdir(parents=True, exist_ok=True)
        (root / "status.json").write_text(json.dumps(status), encoding="utf-8")
        (root / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
        (root / "current_state.json").write_text(json.dumps(current_state), encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
