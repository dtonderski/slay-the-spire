import unittest

from sts.parity import combat_parity


class ParityTests(unittest.TestCase):
    def test_missing_combat_summary_is_unknown(self):
        result = combat_parity({"player": {"hp": 80}}, {"summary": {"missing": True}})

        self.assertEqual(result["status"], "unknown")

    def test_matching_combat_summary_is_in_sync(self):
        result = combat_parity(
            {
                "player": {"hp": 80, "block": 0, "energy": 3},
                "monsters": [{"hp": 40, "block": 0, "alive": True}],
            },
            {
                "summary": {
                    "step": 2,
                    "combat": {
                        "player_hp": 80,
                        "player_block": 0,
                        "energy": 3,
                        "monsters": [{"hp": 40, "block": 0, "gone": False}],
                    },
                },
                "stale": False,
            },
        )

        self.assertEqual(result["status"], "in_sync")
        self.assertEqual(result["diffs"], [])

    def test_mismatching_combat_summary_reports_diffs(self):
        result = combat_parity(
            {
                "player": {"hp": 80, "block": 0, "energy": 3},
                "monsters": [{"hp": 40, "block": 0, "alive": True}],
            },
            {
                "summary": {
                    "combat": {
                        "player_hp": 70,
                        "player_block": 5,
                        "energy": 2,
                        "monsters": [{"hp": 39, "block": 0, "gone": False}],
                    },
                },
                "stale": False,
            },
        )

        self.assertEqual(result["status"], "diverged")
        self.assertEqual(
            [diff["path"] for diff in result["diffs"]],
            ["player.hp", "player.block", "player.energy", "monsters.0.hp"],
        )


if __name__ == "__main__":
    unittest.main()
