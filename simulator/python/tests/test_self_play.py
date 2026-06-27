import json
import tempfile
import unittest
from pathlib import Path

from sts.self_play import run_self_play, verify_self_play_trace


class SelfPlayTests(unittest.TestCase):
    def test_map_fixture_self_play_writes_replayable_trace(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "selfplay.jsonl"

            result = run_self_play(
                output=trace_path,
                start="map_fixture",
                random_seed=7,
                max_steps=12,
            )

            self.assertEqual(result.trace_path, trace_path)
            self.assertGreater(result.steps, 0)
            self.assertTrue(result.verified)

            verification = verify_self_play_trace(trace_path)
            self.assertTrue(verification["ok"])
            self.assertEqual(verification["steps"], result.steps)

            records = self._read_jsonl(trace_path)
            self.assertEqual(records[0]["source"], "sim_selfplay")
            self.assertIn("initial_snapshot_json", records[0])
            self.assertTrue(any(record.get("type") == "step" for record in records))
            self.assertTrue(
                all("potions" in record["before_summary"] for record in records[1:])
            )

    def test_seed_start_records_explicit_unsupported_trace(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "seed.jsonl"

            result = run_self_play(
                output=trace_path,
                start="seed",
                seed="TEST",
                max_steps=4,
            )

            self.assertFalse(result.verified)
            self.assertEqual(result.steps, 0)
            self.assertIn("unsupported_start", result.stop_reason)

            records = self._read_jsonl(trace_path)
            self.assertEqual(records[0]["source"], "sim_selfplay")
            self.assertIn("unsupported_start", records[0]["stop_reason"])
            self.assertIsNone(records[0]["initial_snapshot_json"])

    def test_verify_rejects_action_mismatch(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "selfplay.jsonl"
            run_self_play(
                output=trace_path,
                start="map_fixture",
                random_seed=1,
                max_steps=4,
            )

            records = self._read_jsonl(trace_path)
            step_record = next(record for record in records if record.get("type") == "step")
            step_record["action_json"] = '"not a legal action"'
            self._write_jsonl(trace_path, records)

            verification = verify_self_play_trace(trace_path)
            self.assertFalse(verification["ok"])
            self.assertEqual(verification["error"], "action not legal during replay")

    def _read_jsonl(self, path):
        with path.open("r", encoding="utf-8") as handle:
            return [json.loads(line) for line in handle if line.strip()]

    def _write_jsonl(self, path, records):
        with path.open("w", encoding="utf-8") as handle:
            for record in records:
                handle.write(json.dumps(record))
                handle.write("\n")


if __name__ == "__main__":
    unittest.main()
