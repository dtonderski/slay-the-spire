"""Journal integrity tests on temporary synthetic research artifacts."""

import argparse
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from run_training.probe import run_case
from run_training.replay import replay
from sts_sim import _native


class ReplayTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        native_hash = hashlib.sha256(Path(_native.__file__).read_bytes()).hexdigest()
        (self.root / "manifest.json").write_text(
            json.dumps({"native_sha256": native_hash})
        )
        self.path = self.root / "case.jsonl"
        args = argparse.Namespace(
            mode="natural",
            hp=10000,
            ascension=0,
            style="exercise",
            max_decisions=40,
            case_seconds=30,
        )
        self.result = run_case(args, "HUMAN1", 123, self.path)

    def test_prefix_replays_without_repair(self):
        result = replay(self.path)
        self.assertTrue(result["verified"])
        self.assertEqual(result["accepted"], self.result["accepted"])
        self.assertEqual(result["outcome"], self.result["outcome"])

    def test_hash_mismatch_refused(self):
        (self.root / "manifest.json").write_text(json.dumps({"native_sha256": "wrong"}))
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            replay(self.path)

    def test_partial_journal_not_verified(self):
        rows = self.path.read_text().splitlines()
        self.path.write_text("\n".join(rows[:-1]) + "\n")
        with self.assertRaisesRegex(ValueError, "Partial journal"):
            replay(self.path)

    def test_corrupt_candidate_refused(self):
        rows = [json.loads(line) for line in self.path.read_text().splitlines()]
        attempt = next(row for row in rows if row["type"] == "attempt")
        attempt["action"]["kind"] = "end_turn"
        self.path.write_text("".join(json.dumps(row) + "\n" for row in rows))
        with self.assertRaisesRegex(ValueError, "descriptor mismatch"):
            replay(self.path)

    def test_records_after_result_refused(self):
        with self.path.open("a") as stream:
            stream.write(json.dumps({"type": "attempt"}) + "\n")
        with self.assertRaisesRegex(ValueError, "after its result"):
            replay(self.path)

    def test_result_context_mismatch_refused(self):
        rows = [json.loads(line) for line in self.path.read_text().splitlines()]
        rows[-1]["hp"] += 1
        self.path.write_text("".join(json.dumps(row) + "\n" for row in rows))
        with self.assertRaisesRegex(ValueError, "context mismatch"):
            replay(self.path)


if __name__ == "__main__":
    unittest.main()
