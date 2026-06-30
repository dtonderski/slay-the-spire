import unittest

from sts.guided_collector import GuidedCollector, suggest_guided_action
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
    def test_suggest_guided_action_matches_visible_event_choice(self):
        result = suggest_guided_action(
            sample_script(),
            {
                "summary": {
                    "floor": 2,
                    "screen_type": "EVENT",
                    "choices": ["Pray", "Leave"],
                }
            },
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})
        self.assertEqual(result["category"], "event")

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

