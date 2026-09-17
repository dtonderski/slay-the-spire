import copy
import unittest
from typing import ClassVar

from profile_batches import load_roots, select_roots
from train_roots import collect_roots


class ProfileBatchTests(unittest.TestCase):
    manifest: ClassVar[dict]

    @classmethod
    def setUpClass(cls) -> None:
        cls.original, entries = collect_roots(["2000000", "2000001", "2000002"], floors=1, rng_seed=123)
        cls.manifest = {"held_out_seeds": ["1000035"], "train": entries}

    def test_reconstructs_distinct_roots_from_accepted_prefixes(self) -> None:
        roots, identities = load_roots(self.manifest)
        self.assertEqual(len(roots), len(self.original))
        self.assertEqual(len(set(identities)), len(roots))
        for before, after in zip(self.original, roots):
            self.assertEqual(before.state.decision().observation, after.state.decision().observation)
        selected, ids = select_roots(roots, identities, 2)
        self.assertEqual(len(selected), 2)
        self.assertEqual(len(set(ids)), 2)
        self.assertEqual(ids, select_roots(roots, identities, 2)[1])

    def test_never_clamps_or_replaces_missing_roots(self) -> None:
        roots, identities = load_roots(self.manifest)
        with self.assertRaises(ValueError):
            select_roots(roots, identities, len(roots) + 1)
        with self.assertRaises(ValueError):
            select_roots(roots, identities, 0)
        with self.assertRaises(ValueError):
            select_roots(roots, [identities[0]] * len(roots), 2)

    def test_rejects_held_out_seeds_and_mismatched_reconstruction(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        manifest["held_out_seeds"].append(manifest["train"][0]["seed"])
        with self.assertRaises(ValueError):
            load_roots(manifest)
        manifest = copy.deepcopy(self.manifest)
        manifest["train"][0]["roots"][0]["hp"] += 1
        with self.assertRaises(RuntimeError):
            load_roots(manifest)


if __name__ == "__main__":
    unittest.main()
