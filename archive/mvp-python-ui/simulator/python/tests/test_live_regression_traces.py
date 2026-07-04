import json
import unittest
from pathlib import Path

from sts.self_play import strict_replay_real_trace_to_env


REPO_ROOT = Path(__file__).resolve().parents[3]
CORPUS_ROOT = REPO_ROOT / "verification" / "corpus"


class LiveRegressionTraceTests(unittest.TestCase):
    def test_manifest_traces_strict_replay(self):
        manifest_path = CORPUS_ROOT / "live_regressions.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

        for entry in manifest["entries"]:
            with self.subTest(trace=entry["path"]):
                trace_path = CORPUS_ROOT / entry["path"]
                result = strict_replay_real_trace_to_env(trace=trace_path)

                self.assertEqual(result.verified, entry["expected_verified"])
                self.assertEqual(result.stop_reason, entry["expected_stop_reason"])
                self.assertEqual(result.steps, entry["expected_steps"])
                self.assertEqual(result.final_phase, entry["expected_final_phase"])
                self.assertIsNone(result.blocker)


if __name__ == "__main__":
    unittest.main()
