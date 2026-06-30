import unittest

from sts.guided_collector import GuidedCollector, send_guided_suggestion, suggest_guided_action
from sts.slaythedata_policy import build_guided_run_script


def sample_script():
    return build_guided_run_script(
        {
            "run_id": 42,
            "event": {
                "character_chosen": "IRONCLAD",
                "ascension_level": 0,
                "seed_played": "ABC",
                "card_choices": [{"floor": 1, "picked": "Inflame"}],
                "event_choices": [
                    {"floor": 2, "event_name": "Golden Shrine", "player_choice": "Pray"}
                ],
                "potions_floor_usage": [3],
            },
        }
    )


class GuidedCollectorTests(unittest.TestCase):
    def ready_event_bridge(self):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "bridge-state",
            "summary": {
                "floor": 2,
                "screen_type": "EVENT",
                "choices": ["Pray", "Leave"],
                "available_commands": ["choose"],
            },
        }

    def test_suggest_guided_action_matches_visible_event_choice(self):
        result = suggest_guided_action(
            sample_script(),
            self.ready_event_bridge(),
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})
        self.assertEqual(result["category"], "event")

    def test_send_guided_suggestion_sends_matching_descriptor_with_source_state(self):
        calls = []

        def send_command(command, *, source_state_id=None):
            calls.append((command, source_state_id))
            return {"ok": True, "command_id": "cmd-1", "command": command}

        suggestion = suggest_guided_action(sample_script(), self.ready_event_bridge())

        result = send_guided_suggestion(
            suggestion,
            self.ready_event_bridge(),
            send_command=send_command,
        )

        self.assertEqual(result["status"], "sent")
        self.assertEqual(result["command"], "CHOOSE 0")
        self.assertEqual(result["source_state_id"], "bridge-state")
        self.assertEqual(calls, [("CHOOSE 0", "bridge-state")])

    def test_send_guided_suggestion_blocks_when_bridge_is_not_ready(self):
        bridge = self.ready_event_bridge()
        bridge["ready_for_command"] = False
        suggestion = suggest_guided_action(sample_script(), bridge)

        result = send_guided_suggestion(
            suggestion,
            bridge,
            send_command=lambda *_args, **_kwargs: {"ok": True},
        )

        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reason"], "bridge_not_ready")

    def test_collector_tick_send_is_opt_in(self):
        collector = GuidedCollector()
        collector.start({"script": sample_script()})
        calls = []

        dry_run = collector.tick(
            self.ready_event_bridge(),
            send_command=lambda command, **kwargs: calls.append((command, kwargs)) or {"ok": True},
        )

        sent = collector.tick(
            self.ready_event_bridge(),
            {"send": True},
            send_command=lambda command, **kwargs: calls.append((command, kwargs)) or {
                "ok": True,
                "command_id": "cmd-2",
                "command": command,
            },
        )

        self.assertEqual(dry_run["suggestion"]["status"], "matched")
        self.assertEqual(sent["suggestion"]["status"], "sent")
        self.assertEqual(calls, [("CHOOSE 0", {"source_state_id": "bridge-state"})])

    def test_suggest_guided_action_reports_combat_potion_budget(self):
        result = suggest_guided_action(
            sample_script(),
            {
                "summary": {
                    "floor": 3,
                    "phase": "combat",
                    "combat": {"monsters": []},
                }
            },
        )

        self.assertEqual(result["status"], "combat")
        self.assertEqual(result["mode"], "combat_agent")
        self.assertEqual(result["potion_uses_allowed"], 1)

    def test_collector_tracks_blockers_and_status(self):
        collector = GuidedCollector()
        started = collector.start({"script": sample_script()})
        self.assertTrue(started["active"])
        self.assertEqual(started["status"], "ready")

        tick = collector.tick(
            {
                "summary": {
                    "floor": 2,
                    "screen_type": "EVENT",
                    "choices": ["Leave"],
                }
            }
        )

        self.assertEqual(tick["status"], "blocked")
        self.assertEqual(tick["blocker"]["reason"], "target_not_visible")
        self.assertEqual(tick["history_count"], 1)

        stopped = collector.stop()
        self.assertEqual(stopped["status"], "stopped")
        self.assertFalse(collector.status()["active"])


if __name__ == "__main__":
    unittest.main()
