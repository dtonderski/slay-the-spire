from __future__ import annotations

import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from sts.guided_collect import GuidedCollectConfig, _archive_report_path, collect_one_run, main


class FakeBridge:
    def __init__(self, *, preflight=None):
        self.sent = []
        self._preflights = list(preflight) if isinstance(preflight, list) else None
        self._preflight = preflight or {
            "ok": True,
            "problems": [],
            "warnings": [],
            "tcp_control_available": True,
        }
        self._status = {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "bridge-state",
            "last_state_step": 0,
            "trace_path": "trace.jsonl",
            "control": {"protocol": "tcp-jsonl", "host": "127.0.0.1", "port": 12345},
            "summary": {
                "state_id": "bridge-state",
                "ready_for_command": True,
                "available_commands": ["start", "choose", "state"],
                "in_game": False,
            },
        }

    def status(self):
        return self._status

    def preflight(self):
        if self._preflights is not None:
            if len(self._preflights) > 1:
                return self._preflights.pop(0)
            return self._preflights[0]
        return self._preflight

    def send_command(self, command, **kwargs):
        self.sent.append((command, kwargs))
        return {"ok": True, "command_id": f"cmd-{len(self.sent)}", "command": command}


class VerboseObservedUpdateBridge(FakeBridge):
    def send_command(self, command, **kwargs):
        self.sent.append((command, kwargs))
        return {
            "ok": True,
            "command_id": f"cmd-{len(self.sent)}",
            "command": command,
            "transport": "tcp-jsonl",
            "observed_update": {
                "ok": True,
                "state_id": "observed-state",
                "state_seq": 9,
                "step": 3,
                "bridge_status": {
                    "state_id": "observed-state",
                    "last_state_step": 3,
                    "current_state": {"large": "x" * 1000},
                },
            },
        }


class GuidedCollectTests(unittest.TestCase):
    def test_collect_one_run_reports_blocker_and_requires_tcp(self):
        bridge = FakeBridge()
        ticks = [
            {
                "status": "ready",
                "pending_prediction": {"predicted_state_id": "predicted-1"},
                "suggestion": {
                    "status": "sent_non_combat",
                    "floor": 0,
                    "category": "neow",
                    "non_combat_send": {"send_result": {"command": "CHOOSE 0"}},
                },
            },
            {
                "status": "blocked",
                "blocker": {"status": "blocked", "reason": "unsupported_screen", "detail": "done"},
                "suggestion": {"status": "blocked", "reason": "unsupported_screen", "detail": "done"},
            },
        ]

        def fake_tick(_collector, _manager, _bridge, payload, *, require_tcp_control):
            self.assertTrue(require_tcp_control)
            self.assertTrue(payload["send"])
            return ticks.pop(0)

        with patch(
            "sts.guided_collect.export_guided_run_script",
            return_value={"config": {"character": "IRONCLAD", "ascension": 0, "seed_played": "LIVE01"}},
        ), patch(
            "sts.guided_collect._tick_live_collector",
            side_effect=fake_tick,
        ):
            report = collect_one_run(
                GuidedCollectConfig(run_id=123, max_actions=5, max_seconds=5),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        self.assertFalse(report["ok"])
        self.assertEqual(report["run_id"], 123)
        self.assertEqual(report["seed"], "LIVE01")
        self.assertEqual(report["stop_reason"], "blocked")
        self.assertEqual(report["blocker"]["reason"], "unsupported_screen")
        self.assertEqual(report["actions_sent"], 1)
        self.assertEqual(report["history_tail"][0]["event"], "start")
        self.assertEqual(report["history_tail"][1]["command"], "CHOOSE 0")
        self.assertEqual(report["selection"]["mode"], "explicit")
        self.assertEqual(report["selection"]["selected_run_id"], 123)
        self.assertTrue(bridge.sent[0][1]["require_tcp_control"])

    def test_collect_one_run_compacts_start_observed_update_in_history(self):
        bridge = VerboseObservedUpdateBridge()
        ticks = [
            {
                "status": "blocked",
                "blocker": {"status": "blocked", "reason": "done"},
                "suggestion": {"status": "blocked", "reason": "done"},
            }
        ]

        with patch(
            "sts.guided_collect.export_guided_run_script",
            return_value={"config": {"character": "IRONCLAD", "ascension": 0, "seed_played": "LIVE01"}},
        ), patch(
            "sts.guided_collect._tick_live_collector",
            side_effect=lambda *_args, **_kwargs: ticks.pop(0),
        ):
            report = collect_one_run(
                GuidedCollectConfig(run_id=123, max_actions=5, max_seconds=5),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        start_send = report["history_tail"][0]["send_result"]
        self.assertEqual(start_send["transport"], "tcp-jsonl")
        self.assertEqual(start_send["observed_update"]["state_id"], "observed-state")
        self.assertEqual(start_send["observed_update"]["bridge_step"], 3)
        self.assertNotIn("bridge_status", start_send["observed_update"])
        self.assertNotIn("current_state", json.dumps(start_send))

    def test_collect_one_run_blocks_before_export_when_preflight_fails(self):
        bridge = FakeBridge(
            preflight={
                "ok": False,
                "problems": ["session files are stale"],
                "warnings": ["TCP bridge control is not available; guided auto-collection will not send"],
                "tcp_control_available": False,
                "ages": {"status_age_seconds": 130.0, "summary_age_seconds": 131.0},
                "pending_command": {"present": False, "transport": None},
            }
        )

        with patch("sts.guided_collect.export_guided_run_script") as export:
            report = collect_one_run(
                GuidedCollectConfig(run_id=123),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        export.assert_not_called()
        self.assertFalse(report["ok"])
        self.assertEqual(report["stop_reason"], "preflight_blocked")
        self.assertEqual(report["blocker"]["reason"], "bridge_preflight")
        self.assertIn("session files are stale", report["blocker"]["problems"])
        self.assertIn(
            "TCP bridge control is required for guided auto-collection",
            report["blocker"]["problems"],
        )
        self.assertFalse(report["blocker"]["tcp_control_available"])
        self.assertEqual(report["preflight"]["ages"]["status_age_seconds"], 130.0)
        self.assertEqual(report["preflight"]["pending_command"]["present"], False)
        self.assertEqual(report["actions_sent"], 0)

    def test_collect_one_run_reports_tcp_required_as_hard_preflight_problem(self):
        bridge = FakeBridge(
            preflight={
                "ok": True,
                "problems": [],
                "warnings": ["TCP bridge control is not available; guided auto-collection will not send"],
                "tcp_control_available": False,
            }
        )

        with patch("sts.guided_collect.export_guided_run_script") as export:
            report = collect_one_run(
                GuidedCollectConfig(run_id=123),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        export.assert_not_called()
        self.assertFalse(report["ok"])
        self.assertEqual(report["stop_reason"], "preflight_blocked")
        self.assertIn("TCP bridge control is required", "; ".join(report["blocker"]["problems"]))
        self.assertFalse(report["blocker"]["tcp_control_available"])

    def test_collect_one_run_reports_selection_failure(self):
        bridge = FakeBridge()

        with patch("sts.guided_collect.select_guided_collection_candidates", side_effect=RuntimeError("db locked")):
            report = collect_one_run(
                GuidedCollectConfig(run_id=None),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        self.assertFalse(report["ok"])
        self.assertEqual(report["stop_reason"], "selection_failed")
        self.assertEqual(report["blocker"]["reason"], "selection_failed")
        self.assertIn("db locked", report["blocker"]["detail"])
        self.assertEqual(report["actions_sent"], 0)
        self.assertEqual(bridge.sent, [])

    def test_collect_one_run_reports_script_blocker_before_start(self):
        bridge = FakeBridge()
        script = {
            "config": {
                "character": "IRONCLAD",
                "ascension": 0,
                "seed_played": "GRID01",
                "neow_bonus": "REMOVE_CARD",
                "neow_cost": "NONE",
            }
        }

        with patch("sts.guided_collect.export_guided_run_script", return_value=script):
            report = collect_one_run(
                GuidedCollectConfig(run_id=321),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        self.assertFalse(report["ok"])
        self.assertEqual(report["run_id"], 321)
        self.assertEqual(report["seed"], "GRID01")
        self.assertEqual(report["stop_reason"], "script_blocked")
        self.assertEqual(report["blocker"]["reason"], "unsupported_neow_followup")
        self.assertEqual(report["selection"]["mode"], "explicit")
        self.assertEqual(report["selection"]["selected_run_id"], 321)
        self.assertEqual(report["actions_sent"], 0)
        self.assertEqual(bridge.sent, [])

    def test_collect_one_run_reports_start_failure(self):
        bridge = FakeBridge()
        script = {
            "config": {
                "character": "IRONCLAD",
                "ascension": 0,
                "seed_played": "LIVE01",
                "neow_bonus": "THREE_ENEMY_KILL",
                "neow_cost": "NONE",
            }
        }

        with patch("sts.guided_collect.export_guided_run_script", return_value=script), patch(
            "sts.guided_collect._start_guided_live_run",
            side_effect=RuntimeError("START rejected"),
        ):
            report = collect_one_run(
                GuidedCollectConfig(run_id=123),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        self.assertFalse(report["ok"])
        self.assertEqual(report["stop_reason"], "start_failed")
        self.assertEqual(report["blocker"]["reason"], "start_failed")
        self.assertIn("start", report["blocker"]["detail"].lower())
        self.assertEqual(report["run_id"], 123)
        self.assertEqual(report["seed"], "LIVE01")
        self.assertEqual(report["selection"]["mode"], "explicit")
        self.assertEqual(report["actions_sent"], 0)
        self.assertEqual(bridge.sent, [])

    def test_collect_one_run_reports_tick_failure_after_start(self):
        bridge = FakeBridge()
        script = {
            "config": {
                "character": "IRONCLAD",
                "ascension": 0,
                "seed_played": "LIVE01",
                "neow_bonus": "THREE_ENEMY_KILL",
                "neow_cost": "NONE",
            }
        }

        with patch("sts.guided_collect.export_guided_run_script", return_value=script), patch(
            "sts.guided_collect._tick_live_collector",
            side_effect=RuntimeError("prediction mismatch"),
        ):
            report = collect_one_run(
                GuidedCollectConfig(run_id=123),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        self.assertFalse(report["ok"])
        self.assertEqual(report["stop_reason"], "tick_failed")
        self.assertEqual(report["blocker"]["reason"], "tick_failed")
        self.assertIn("prediction mismatch", report["blocker"]["detail"])
        self.assertEqual(report["run_id"], 123)
        self.assertEqual(report["seed"], "LIVE01")
        self.assertEqual(report["history_tail"][0]["event"], "start")
        self.assertEqual(report["history_tail"][1]["reason"], "tick_failed")
        self.assertEqual(report["actions_sent"], 0)

    def test_collect_one_run_auto_selection_skips_unsupported_script(self):
        bridge = FakeBridge()
        unsupported = {
            "config": {
                "character": "IRONCLAD",
                "ascension": 0,
                "seed_played": "GRID01",
                "neow_bonus": "REMOVE_CARD",
                "neow_cost": "NONE",
            }
        }
        supported = {
            "config": {
                "character": "IRONCLAD",
                "ascension": 0,
                "seed_played": "LIVE02",
                "neow_bonus": "THREE_ENEMY_KILL",
                "neow_cost": "NONE",
            }
        }
        ticks = [
            {
                "status": "blocked",
                "blocker": {"status": "blocked", "reason": "done"},
                "suggestion": {"status": "blocked", "reason": "done"},
            }
        ]

        with patch(
            "sts.guided_collect.select_guided_collection_candidates",
            return_value=[{"id": 11}, {"id": 22}],
        ), patch(
            "sts.guided_collect.export_guided_run_script",
            side_effect=[unsupported, supported],
        ) as export, patch(
            "sts.guided_collect._tick_live_collector",
            side_effect=lambda *_args, **_kwargs: ticks.pop(0),
        ):
            report = collect_one_run(
                GuidedCollectConfig(run_id=None),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        self.assertEqual([call.args[0] for call in export.call_args_list], [11, 22])
        self.assertEqual(report["run_id"], 22)
        self.assertEqual(report["seed"], "LIVE02")
        self.assertEqual(report["selection"]["mode"], "auto")
        self.assertEqual(report["selection"]["selected_run_id"], 22)
        self.assertEqual(report["selection"]["considered_count"], 2)
        self.assertEqual(report["selection"]["candidate_count"], 2)
        self.assertEqual(report["selection"]["skipped_unsupported"][0]["run_id"], 11)
        self.assertEqual(report["selection"]["skipped_unsupported"][0]["reason"], "unsupported_neow_followup")
        self.assertEqual(bridge.sent[0][0], "START IRONCLAD 0 LIVE02")

    def test_collect_one_run_waits_for_preflight_to_become_ready(self):
        bridge = FakeBridge(
            preflight=[
                {
                    "ok": False,
                    "problems": ["missing session status.json"],
                    "warnings": [],
                    "tcp_control_available": False,
                },
                {
                    "ok": True,
                    "problems": [],
                    "warnings": [],
                    "tcp_control_available": True,
                },
            ]
        )
        sleeps = []

        def fake_tick(_collector, _manager, _bridge, payload, *, require_tcp_control):
            return {
                "status": "blocked",
                "blocker": {"status": "blocked", "reason": "done"},
                "suggestion": {"status": "blocked", "reason": "done"},
            }

        with patch(
            "sts.guided_collect.export_guided_run_script",
            return_value={"config": {"character": "IRONCLAD", "ascension": 0, "seed_played": "LIVE01"}},
        ) as export, patch(
            "sts.guided_collect._tick_live_collector",
            side_effect=fake_tick,
        ):
            report = collect_one_run(
                GuidedCollectConfig(
                    run_id=123,
                    preflight_timeout_seconds=5,
                    preflight_poll_seconds=0.25,
                ),
                bridge=bridge,
                sleep=lambda seconds: sleeps.append(seconds),
            )

        export.assert_called_once()
        self.assertEqual(sleeps, [0.25])
        self.assertEqual(report["stop_reason"], "blocked")

    def test_collect_one_run_action_cap_is_not_clean_success_without_verified_trace(self):
        bridge = FakeBridge()

        def fake_tick(_collector, _manager, _bridge, payload, *, require_tcp_control):
            return {
                "status": "ready",
                "pending_prediction": None,
                "suggestion": {
                    "status": "sent_non_combat",
                    "floor": 1,
                    "category": "reward",
                    "non_combat_send": {"send_result": {"command": "CHOOSE 0"}},
                },
            }

        with patch(
            "sts.guided_collect.export_guided_run_script",
            return_value={"config": {"character": "IRONCLAD", "ascension": 0, "seed_played": "LIVE01"}},
        ), patch(
            "sts.guided_collect._tick_live_collector",
            side_effect=fake_tick,
        ):
            report = collect_one_run(
                GuidedCollectConfig(run_id=123, max_actions=1, max_seconds=5),
                bridge=bridge,
                sleep=lambda _seconds: None,
            )

        self.assertFalse(report["ok"])
        self.assertEqual(report["stop_reason"], "max_actions")
        self.assertEqual(report["actions_sent"], 1)
        self.assertEqual(report["trace_validation"]["reason"], "trace_path_not_found")

    def test_archive_report_path_is_safe_and_descriptive(self):
        with tempfile.TemporaryDirectory() as directory:
            path = _archive_report_path(
                Path(directory),
                {"run_id": "abc/123", "stop_reason": "preflight blocked"},
            )

        self.assertEqual(path.name[-5:], ".json")
        self.assertIn("abc-123", path.name)
        self.assertIn("preflight-blocked", path.name)

    def test_main_writes_report_when_collect_raises(self):
        with tempfile.TemporaryDirectory() as directory:
            report_path = Path(directory) / "latest.json"
            archive_dir = Path(directory) / "reports"

            with patch(
                "sts.guided_collect.collect_one_run",
                side_effect=RuntimeError("boom"),
            ), patch("sys.stdout", new_callable=io.StringIO):
                main(
                    [
                        "--report-output",
                        str(report_path),
                        "--archive-report-dir",
                        str(archive_dir),
                    ]
                )

            report = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertFalse(report["ok"])
            self.assertEqual(report["stop_reason"], "internal_error")
            self.assertEqual(report["blocker"]["type"], "RuntimeError")
            self.assertIn("boom", report["blocker"]["detail"])
            self.assertEqual(len(list(archive_dir.glob("*.json"))), 1)


if __name__ == "__main__":
    unittest.main()
