"""Infrastructure tests for isolated fuzz subprocess handling."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("combat_fuzz_campaign.py")
SPEC = importlib.util.spec_from_file_location("campaign", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
campaign = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(campaign)


class CampaignTests(unittest.TestCase):
    def run_fake(self, body: str, timeout: float = 2.0, pinned: bool = False,
                 profile: str = "cards-relics"):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        binary = root / "probe"
        binary.write_text('#!/bin/sh\nmkdir -p "$3"\n' + body)
        binary.chmod(0o700)
        output = root / "seed-7"
        if pinned:
            (root / "manifest.json").write_text(json.dumps({"revision": "pinned-revision"}))
            (root / "implementation.patch").write_text("pinned patch")
        result = campaign.run_case(binary, root, 7, output, timeout, profile)
        return result, output

    def test_terminal_case_is_summarized_and_cleaned(self):
        result, output = self.run_fake('echo "done start=$1 count=1 terminal=1 failures=0 capped=0"\n')
        self.assertEqual(result["status"], "terminal")
        self.assertFalse(output.exists())
        self.assertIsNone(result["artifacts"])

    def test_nonzero_exit_preserves_failure_artifacts(self):
        result, output = self.run_fake('echo evidence > "$3/live.jsonl"\nexit 3\n')
        self.assertEqual(result["status"], "process_failure")
        self.assertEqual(result["exit_code"], 3)
        self.assertEqual((output / "live.jsonl").read_text(), "evidence\n")

    def test_timeout_is_a_candidate_and_keeps_journal(self):
        result, output = self.run_fake('echo attempted > "$3/live.jsonl"\nsleep 30\n', 0.1)
        self.assertEqual(result["status"], "wall_timeout_candidate")
        self.assertEqual((output / "live.jsonl").read_text(), "attempted\n")
        self.assertLess(result["elapsed_seconds"], 2)

    def test_potion_profile_is_explicit_in_child_command(self):
        _, output = self.run_fake('echo "$4" > "$3/profile"\n', profile="cards-relics-potions")
        self.assertEqual((output / "profile").read_text().strip(), "--potions")

    def test_potion_profile_rejects_old_binary_that_ignores_flag(self):
        result, output = self.run_fake('echo \'coverage={"encounter":"Cultist","ascension":20}\'\n'
                                      'echo "done start=$1 count=1 terminal=1 failures=0 capped=0"\n',
                                      profile="cards-relics-potions")
        self.assertEqual(result["status"], "probe_profile_mismatch")
        self.assertTrue(output.exists())

    def test_case_uses_campaign_provenance(self):
        result, output = self.run_fake('echo "$COMBAT_FUZZ_REVISION" > "$3/revision"\n'
                                      'cp "$COMBAT_FUZZ_PATCH" "$3/patch"\n', pinned=True)
        self.assertEqual(result["status"], "failure_candidate")
        self.assertEqual((output / "revision").read_text().strip(), "pinned-revision")
        self.assertEqual((output / "patch").read_text(), "pinned patch")

    def test_coverage_is_recorded(self):
        result, _ = self.run_fake('echo \'coverage={"encounter":"Cultist","ascension":20}\'\n'
                                  'echo "done start=$1 count=1 terminal=1 failures=0 capped=0"\n')
        self.assertEqual(result["coverage"]["encounter"], "Cultist")
        self.assertEqual(result["status"], "terminal")

    def test_incomplete_coverage_preserves_evidence(self):
        result, output = self.run_fake('echo \'coverage={"encounter":\'\n'
                                       'echo "done start=$1 count=1 terminal=1 failures=0 capped=0"\n')
        self.assertEqual(result["status"], "probe_output_failure")
        self.assertTrue(output.exists())
        self.assertIsNotNone(result["coverage_error"])

    def test_action_cap_is_not_classified_as_a_confirmed_hang(self):
        result, output = self.run_fake('echo "done start=$1 count=1 terminal=0 failures=0 capped=1"\n')
        self.assertEqual(result["status"], "action_limit_candidate")
        self.assertTrue(output.exists())


if __name__ == "__main__":
    unittest.main()
