import math
import tempfile
import unittest
from pathlib import Path

import torch
from combat_explorer.errors import ExplorerError
from combat_explorer.jsonutil import finite_floats, seed_to_str
from combat_explorer.policy import (
    PolicyAdapter,
    greedy_index,
    sample_index,
    softmax,
    validate_temperature,
)
from model import CombatModel


class SoftmaxTests(unittest.TestCase):
    def test_nonfinite_logits_and_large_seeds(self) -> None:
        with self.assertRaises(ValueError):
            finite_floats([0.1, float("nan")], "logit")
        self.assertEqual(seed_to_str(2**63 + 5), str(2**63 + 5))
        self.assertEqual(seed_to_str(str(2**63 + 5)), str(2**63 + 5))

    def test_temperature_one_matches_softmax(self) -> None:
        logits = [1.0, 2.0, 0.0]
        probs = softmax(logits, 1.0)
        peak = max(logits)
        expected = [math.exp(x - peak) for x in logits]
        total = sum(expected)
        expected = [x / total for x in expected]
        for left, right in zip(probs, expected):
            self.assertAlmostEqual(left, right, places=12)
        self.assertAlmostEqual(sum(probs), 1.0, places=12)

    def test_lower_temperature_peaks(self) -> None:
        logits = [1.0, 3.0, 0.0]
        low = softmax(logits, 0.2)
        high = softmax(logits, 5.0)
        self.assertGreater(low[1], high[1])
        self.assertLess(low[0], high[0])

    def test_rejects_bad_temperature(self) -> None:
        with self.assertRaises(ExplorerError):
            validate_temperature(0)
        with self.assertRaises(ExplorerError):
            validate_temperature(-1)
        with self.assertRaises(ExplorerError):
            validate_temperature(float("nan"))
        with self.assertRaises(ExplorerError):
            validate_temperature(0.01)

    def test_greedy_tie_breaks_earlier_index(self) -> None:
        self.assertEqual(greedy_index([1.0, 3.0, 3.0]), 1)
        self.assertEqual(greedy_index([2.0, 2.0, 1.0]), 0)

    def test_seeded_sampling_repeats_on_one_generator(self) -> None:
        import random

        from combat_explorer.policy import new_sampling_rng

        probs = [0.1, 0.7, 0.2]
        rng_a = new_sampling_rng("99")
        rng_b = new_sampling_rng("99")
        first = [sample_index(probs, rng_a) for _ in range(80)]
        second = [sample_index(probs, rng_b) for _ in range(80)]
        self.assertEqual(first, second)
        self.assertGreater(len(set(first)), 1)
        self.assertTrue(set(first) <= {0, 1, 2})
        restarted = [sample_index(probs, random.Random(99)) for _ in range(80)]
        self.assertEqual(len(set(restarted)), 1)
        self.assertNotEqual(first, restarted)


class CheckpointTests(unittest.TestCase):
    def test_strict_load_and_bad_payloads(self) -> None:
        adapter = PolicyAdapter()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "latest.pt"
            torch.save({"model": CombatModel().state_dict(), "config": {"k": 1}}, path)
            loaded = adapter.load(path, device="cpu")
            self.assertEqual(loaded.architecture, "CombatModel")
            self.assertEqual(len(loaded.fingerprint), 64)
            torch.save({"nope": 1}, Path(directory) / "bad.pt")
            with self.assertRaises(ExplorerError):
                adapter.load(Path(directory) / "bad.pt")
            torch.save({"model": {"not.a.weight": torch.tensor([1.0])}}, Path(directory) / "shape.pt")
            with self.assertRaises(ExplorerError):
                adapter.load(Path(directory) / "shape.pt")


if __name__ == "__main__":
    unittest.main()
