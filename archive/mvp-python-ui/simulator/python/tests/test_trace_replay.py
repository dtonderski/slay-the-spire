import json
import tempfile
import unittest
from pathlib import Path

from sts.trace_replay import TraceReplayStore


class TraceReplayTests(unittest.TestCase):
    def test_list_traces_counts_records_and_last_state_summary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trace = root / "trace-a.jsonl"
            self._write_jsonl(
                trace,
                [
                    {"type": "metadata", "schema": 1, "source": "communication_mod"},
                    {
                        "type": "state",
                        "step": 0,
                        "received_at": "2026-06-26T00:00:00Z",
                        "message": {
                            "available_commands": ["choose", "state"],
                            "ready_for_command": True,
                            "in_game": True,
                            "game_state": {
                                "screen_type": "EVENT",
                                "floor": 0,
                                "current_hp": 80,
                                "max_hp": 80,
                                "gold": 99,
                                "choice_list": ["talk"],
                            },
                        },
                    },
                    {"type": "action", "step": 1, "command": "CHOOSE 0"},
                ],
            )

            result = TraceReplayStore(root).list_traces()

            self.assertEqual(len(result["traces"]), 1)
            metadata = result["traces"][0]
            self.assertEqual(metadata["id"], "trace-a.jsonl")
            self.assertEqual(metadata["records"], 3)
            self.assertEqual(metadata["states"], 1)
            self.assertEqual(metadata["actions"], 1)
            self.assertEqual(metadata["first_step"], 0)
            self.assertEqual(metadata["last_step"], 1)
            self.assertEqual(metadata["summary"]["screen_type"], "EVENT")
            self.assertEqual(metadata["summary"]["hp"], "80/80")

    def test_load_trace_returns_summarized_records(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trace = root / "trace-b.jsonl"
            self._write_jsonl(
                trace,
                [
                    {"type": "metadata", "started_at": "2026-06-26T00:00:00Z"},
                    {"type": "action", "step": 2, "sent_at": "2026-06-26T00:00:01Z", "command": "END"},
                ],
            )

            result = TraceReplayStore(root).load_trace("trace-b.jsonl")

            self.assertEqual(result["trace"]["records"], 2)
            self.assertEqual(result["records"][1]["line"], 2)
            self.assertEqual(result["records"][1]["type"], "action")
            self.assertEqual(result["records"][1]["command"], "END")
            self.assertEqual(result["records"][1]["summary"]["command"], "END")
            self.assertIn("raw", result["records"][1])

    def test_rejects_path_traversal_and_non_jsonl_ids(self):
        with tempfile.TemporaryDirectory() as directory:
            store = TraceReplayStore(Path(directory))

            with self.assertRaises(KeyError):
                store.load_trace("../trace-a.jsonl")

            with self.assertRaises(KeyError):
                store.load_trace("trace-a.txt")

    def _write_jsonl(self, path, records):
        with path.open("w", encoding="utf-8") as handle:
            for record in records:
                handle.write(json.dumps(record))
                handle.write("\n")


if __name__ == "__main__":
    unittest.main()
