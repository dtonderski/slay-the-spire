import unittest

import numpy as np
from encoders.numeric import ACTION_REVISION, NUMERIC_VERSION, NumericBatch
from encoders.potions import POTION_TO_INDEX
from sts_sim import ACTION_KINDS, PotionKey
from train import _policy_candidates


class PolicyCandidateTests(unittest.TestCase):
    def batch(self, rows, potions):
        tables = {
            "header": (5, np.zeros((6, 5), dtype=np.int64).tobytes()),
            "action_rows": (12, np.asarray(rows, dtype=np.int64).reshape(-1, 12).tobytes()),
            "potions": (3, np.asarray(potions, dtype=np.int64).reshape(-1, 3).tobytes()),
        }
        return NumericBatch((NUMERIC_VERSION, (), tables, [1, 3, 5]))

    def fixture(self):
        smoke = POTION_TO_INDEX[PotionKey.SMOKE_BOMB]
        fire = POTION_TO_INDEX[PotionKey.FIRE]
        codes = [[smoke, fire], [fire, fire], [smoke, smoke]]
        potions = [[owner, code, 1] for owner, slots in enumerate(codes) for code in slots]
        rows = []
        for owner in range(6):
            for index, kind in enumerate(["end_turn", "use_potion_slot", "use_potion_slot"]):
                row = [owner, 10 + index, ACTION_KINDS.index(kind), -1, index - 1, -1, -1, -1, -1, -1, -1, 20 + owner]
                rows.append(row)
        return rows, potions, codes

    def test_sparse_owners_and_interleaved_rows_match_reference(self):
        rows, potions, codes = self.fixture()
        for seed in range(20):
            shuffled = np.asarray(rows)[np.random.default_rng(seed).permutation(len(rows))].tolist()
            expected, legal, revisions, counts = [], [], [], []
            for position, owner in enumerate([1, 3, 5]):
                owned = [r for r in shuffled if r[0] == owner]
                kept = [
                    r
                    for r in owned
                    if r[2] != ACTION_KINDS.index("use_potion_slot")
                    or codes[position][r[4]] != POTION_TO_INDEX[PotionKey.SMOKE_BOMB]
                ]
                expected.extend([[position, *r[2:7]] for r in kept])
                legal.append([r[1] for r in kept])
                revisions.append(20 + owner)
                counts.append(len(kept))
            actual = _policy_candidates(self.batch(shuffled, potions), [1, 3, 5])
            np.testing.assert_array_equal(actual[0], expected)
            self.assertEqual(actual[1:], (legal, revisions, counts))

    def test_missing_or_escape_only_owner_is_rejected(self):
        rows, potions, _ = self.fixture()
        for rejected in ([r for r in rows if r[0] != 5], [r for r in rows if r[0] != 5 or r[1] != 10]):
            with self.assertRaisesRegex(RuntimeError, "No allowed combat actions"):
                _policy_candidates(self.batch(rejected, potions), [1, 3, 5])

    def test_mixed_kept_revisions_are_rejected(self):
        rows, potions, _ = self.fixture()
        next(r for r in rows if r[0] == 3)[ACTION_REVISION] += 1
        with self.assertRaisesRegex(RuntimeError, "mixed revisions"):
            _policy_candidates(self.batch(rows, potions), [1, 3, 5])
