import unittest

import numpy as np
from encoders.numeric import NUMERIC_VERSION, NumericBatch
from train import _stack_observations


def batch(size: int, enemy_owners: list[int], relic_owners: list[int]) -> NumericBatch:
    def owned(owners: list[int], width: int) -> np.ndarray:
        rows = np.zeros((len(owners), width), dtype=np.int64)
        rows[:, 0] = owners
        return rows

    tables = {
        "player": np.zeros((size, 6), dtype=np.int64),
        "selection": np.zeros((size, 1), dtype=np.int64),
        "enemies": owned(enemy_owners, 18),
        "relics": owned(relic_owners, 2),
        "enemy_powers": owned(list(range(len(enemy_owners))), 3),
        "stasis": owned([len(enemy_owners) - 1] if enemy_owners else [], 27),
        "relic_counters": owned(list(range(len(relic_owners))), 3),
    }
    payload = {key: (rows.shape[1], rows.tobytes()) for key, rows in tables.items()}
    return NumericBatch((NUMERIC_VERSION, [], payload, list(range(size))))


class ArrayReplayStackTests(unittest.TestCase):
    def test_distinct_model_enemy_relic_offsets_and_input_immutability(self) -> None:
        first = batch(2, [0, 0, 1], [0, 0, 1, 1])
        second = batch(1, [0], [0])
        before = [{key: rows.copy() for key, rows in b.tables.items()} for b in (first, second)]
        result = _stack_observations([first, second])
        self.assertEqual(result.model_rows, [0, 1, 2])
        for key, owners in {
            "enemies": [0, 0, 1, 2],
            "relics": [0, 0, 1, 1, 2],
            "enemy_powers": [0, 1, 2, 3],
            "stasis": [2, 3],
            "relic_counters": [0, 1, 2, 3, 4],
        }.items():
            np.testing.assert_array_equal(result.tables[key][:, 0], owners)
        for original, saved in zip((first, second), before, strict=True):
            for key, rows in saved.items():
                np.testing.assert_array_equal(original.tables[key], rows)
        for rows in result.tables.values():
            self.assertFalse(rows.flags.writeable)
            with self.assertRaises(ValueError):
                rows.setflags(write=True)
        self.assertEqual(result.action_rows.shape, (0, 12))
        self.assertEqual(len(_stack_observations([])), 0)

    def test_owned_arrays_are_zero_copy_readonly_and_validated(self) -> None:
        rows = np.zeros((2, 6), dtype=np.int64)
        result = NumericBatch.from_owned_arrays({"player": rows}, 2)
        self.assertTrue(np.shares_memory(rows, result.tables["player"]))
        with self.assertRaises(ValueError):
            result.tables["player"][0, 0] = 1
        for invalid in (rows.astype(np.float32), rows[:, ::2], rows.reshape(-1)):
            with self.assertRaisesRegex(ValueError, "contiguous two-dimensional int64"):
                NumericBatch.from_owned_arrays({"player": invalid}, 2)

    def test_aligned_table_mismatch_still_rejected(self) -> None:
        malformed = batch(2, [], [])
        malformed.tables["player"] = np.zeros((1, 6), dtype=np.int64)
        with self.assertRaisesRegex(RuntimeError, "player rows do not match"):
            _stack_observations([malformed])


if __name__ == "__main__":
    unittest.main()
