import unittest
from unittest.mock import patch

from beam_search import SearchResult
from train import first_combat
from train_roots import Root, evaluate_beam


class BaselineLoggingTests(unittest.TestCase):
    def test_failures_are_not_reported_as_defeats(self) -> None:
        root = Root(first_combat("HUMAN1", 0), "HUMAN1", 1, 80)
        with (
            patch(
                "train_roots.beam_search",
                side_effect=[SearchResult(transitions=10, limit_reached=True), ValueError("test failure")],
            ),
            self.assertLogs("train_roots", level="ERROR"),
        ):
            scores, records = evaluate_beam([root, root], width=4, max_decisions=8, max_transitions=10)
        self.assertEqual(scores["roots"], 2)
        self.assertEqual(scores["completed"], 0)
        self.assertEqual(scores["unfinished"], 1)
        self.assertEqual(scores["errors"], 1)
        self.assertNotIn("mean_return_completed", scores)
        self.assertEqual([r["status"] for r in records], ["unfinished", "error"])
        self.assertIn("test failure", records[1]["error"])

    def test_completed_reference_replays_and_does_not_mutate_root(self) -> None:
        state = first_combat("HUMAN1", 0)
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


if __name__ == "__main__":
    unittest.main()
