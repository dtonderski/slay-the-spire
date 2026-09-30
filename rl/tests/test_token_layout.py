import unittest

import numpy as np

from observation_encoder import token_layout


def reference_layout(counts: np.ndarray) -> tuple[int, np.ndarray, np.ndarray]:
    width = int(counts.sum(axis=1).max()) + 1
    positions = np.ones(len(counts), dtype=np.int64)
    destinations = [np.arange(len(counts)) * width]
    for lengths in counts.T:
        owners = np.repeat(np.arange(len(counts)), lengths)
        starts = np.cumsum(lengths) - lengths
        local = np.arange(len(owners)) - np.repeat(starts, lengths)
        destinations.append(owners * width + positions[owners] + local)
        positions += lengths
    return width, positions, np.concatenate(destinations)


class TokenLayoutTests(unittest.TestCase):
    def test_matches_group_loop_with_zeros_skew_and_tail_sizes(self) -> None:
        rng = np.random.default_rng(905)
        for size in (1, 2, 8, 32, 128, 512):
            for counts in (
                np.zeros((size, 10), dtype=np.int64),
                np.ones((size, 10), dtype=np.int64),
                rng.integers(0, 40, size=(size, 10)),
                rng.integers(0, 2, size=(size, 10)) * 100,
            ):
                with self.subTest(size=size, total=counts.sum()):
                    before = counts.copy()
                    actual = token_layout(counts)
                    expected = reference_layout(counts)
                    self.assertEqual(actual[0], expected[0])
                    np.testing.assert_array_equal(actual[1], expected[1])
                    np.testing.assert_array_equal(actual[2], expected[2])
                    np.testing.assert_array_equal(counts, before)
                    self.assertEqual(len(np.unique(actual[2])), len(actual[2]))
                    self.assertTrue(np.all(actual[2] >= 0))
                    self.assertTrue(np.all(actual[2] < size * actual[0]))


if __name__ == "__main__":
    unittest.main()
