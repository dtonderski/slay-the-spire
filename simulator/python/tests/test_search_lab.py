import unittest

from sts.search_lab import default_candidates, generate_roots, run_benchmark, trace_autopilot_candidates


class SearchLabTests(unittest.TestCase):
    def test_generate_roots_is_deterministic_and_split(self):
        first = generate_roots(max_source_depth=3, max_roots=12)
        second = generate_roots(max_source_depth=3, max_roots=12)

        self.assertTrue(first)
        self.assertEqual([root.state_id for root in first], [root.state_id for root in second])
        self.assertTrue({root.split for root in first}.issubset({"dev", "eval"}))

    def test_default_candidates_include_large_depth_beam(self):
        candidates = default_candidates()

        names = {candidate.name for candidate in candidates}
        self.assertIn("beam_tactical_w8_d40", names)
        self.assertIn("portfolio_rollout_d40", names)

    def test_run_benchmark_returns_ranked_candidates(self):
        candidates = trace_autopilot_candidates()[:2]
        report = run_benchmark(
            split="all",
            max_source_depth=2,
            max_roots=8,
            max_actions=12,
            candidates=candidates,
        )

        self.assertGreater(report["benchmark"]["roots"], 0)
        self.assertEqual(len(report["ranking"]), len(candidates))
        self.assertTrue(report["episodes"])
        self.assertIn("mean_start_hp", report["benchmark"])
        self.assertGreater(report["benchmark"]["mean_start_hp"], 0)
        self.assertIn("candidate", report["ranking"][0])
        self.assertIn("mean_score", report["ranking"][0])
        self.assertIn("potion_use_counts", report["ranking"][0])


if __name__ == "__main__":
    unittest.main()
