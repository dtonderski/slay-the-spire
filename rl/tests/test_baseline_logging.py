import unittest
from unittest.mock import patch

from beam_search import SearchResult
from test_model import combat
from train import Episode, Root, evaluate, evaluate_beam


class BaselineLoggingTests(unittest.TestCase):
    def test_failures_are_not_reported_as_defeats(self) -> None:
        root = Root(combat(), "HUMAN1", 1, 80)
        with (
            patch(
                "train.beam_search",
                side_effect=[SearchResult(transitions=10, limit_reached=True), ValueError("test failure")],
            ),
            self.assertLogs("train", level="ERROR"),
        ):
            scores, records = evaluate_beam([root, root], width=4, max_decisions=8, max_transitions=10)
        self.assertEqual(scores["roots"], 2)
        self.assertEqual(scores["completed"], 0)
        self.assertEqual(scores["unfinished"], 1)
        self.assertEqual(scores["errors"], 1)
        self.assertNotIn("mean_return_completed", scores)
        self.assertNotIn("mean_hp_lost_completed", scores)
        self.assertEqual([r["status"] for r in records], ["unfinished", "error"])
        self.assertIn("test failure", records[1]["error"])

    def test_completed_reference_replays_and_does_not_mutate_root(self) -> None:
        state = combat()
        before = state.decision().observation
        scores, records = evaluate_beam(
            [Root(state, "HUMAN1", 1, 80)], width=16, max_decisions=48, max_transitions=5000
        )
        self.assertEqual(scores["completed"], 1)
        self.assertEqual(scores["errors"], 0)
        self.assertEqual(scores["unfinished"], 0)
        self.assertTrue(records[0]["action_indices"])
        self.assertEqual(scores["mean_return_completed"], records[0]["reward"])
        self.assertEqual(state.decision().observation, before)
        self.assertEqual(scores["mean_hp_lost_completed"], 80 - records[0]["hp"])
        self.assertEqual(records[0]["hp_lost"], scores["mean_hp_lost_completed"])

    def test_hp_loss_includes_deaths_and_healing_but_not_truncations(self) -> None:
        root = Root(combat(), "HUMAN1", 1, 80)
        episodes = [Episode(1.0, True, 90, 1), Episode(0.0, False, 0, 1), Episode(None, None, 30, 1)]
        with patch("train.play_combats", side_effect=[[episode] for episode in episodes]):
            scores = evaluate([root], None, repeats=3, max_decisions=1)
        self.assertEqual(scores["mean_hp_lost_completed"], 35.0)
        self.assertEqual(scores["mean_hp_change_completed"], -35.0)
        self.assertEqual(scores["completed"], 2)
        self.assertEqual(scores["truncated"], 1)


if __name__ == "__main__":
    unittest.main()
