from __future__ import annotations

import unittest
from unittest.mock import patch

from sts.guided_collect import GuidedCollectConfig, collect_one_run


class FakeBridge:
    def __init__(self, *, preflight=None):
        self.sent = []
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
        return self._preflight

    def send_command(self, command, **kwargs):
        self.sent.append((command, kwargs))
        return {"ok": True, "command_id": f"cmd-{len(self.sent)}", "command": command}


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
        self.assertTrue(bridge.sent[0][1]["require_tcp_control"])

    def test_collect_one_run_blocks_before_export_when_preflight_fails(self):
        bridge = FakeBridge(
            preflight={
                "ok": False,
                "problems": ["session files are stale"],
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
        self.assertEqual(report["blocker"]["reason"], "bridge_preflight")
        self.assertEqual(report["blocker"]["problems"], ["session files are stale"])
        self.assertFalse(report["blocker"]["tcp_control_available"])
        self.assertEqual(report["actions_sent"], 0)


if __name__ == "__main__":
    unittest.main()
