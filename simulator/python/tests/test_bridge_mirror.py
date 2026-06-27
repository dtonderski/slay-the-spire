import json
import tempfile
import unittest
from pathlib import Path

from sts.bridge import BridgeMirror, command_for_descriptor


class BridgeMirrorTests(unittest.TestCase):
    def test_missing_session_reports_disconnected_and_stale(self):
        with tempfile.TemporaryDirectory() as directory:
            status = BridgeMirror(Path(directory)).status(now=1000.0)

        self.assertFalse(status["connected"])
        self.assertTrue(status["stale"])
        self.assertTrue(status["status"]["missing"])

    def test_reads_active_bridge_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(
                json.dumps({"status": "waiting", "client_pid": 12, "trace_path": "trace.jsonl", "step": 4}),
                encoding="utf-8",
            )
            (root / "summary.json").write_text(
                json.dumps({"step": 4, "ready_for_command": True, "available_commands": ["play", "end", "state"]}),
                encoding="utf-8",
            )
            (root / "current_state.json").write_text(json.dumps({"step": 4}), encoding="utf-8")

            status = BridgeMirror(root, stale_after_seconds=9999).status()

        self.assertTrue(status["connected"])
        self.assertFalse(status["stale"])
        self.assertEqual(status["client_pid"], 12)
        self.assertEqual(status["trace_path"], "trace.jsonl")
        self.assertEqual(status["available_commands"], ["play", "end", "state"])

    def test_send_command_writes_pending_command(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")

            result = BridgeMirror(root).send_command("state")

            self.assertTrue(result["ok"])
            self.assertEqual((root / "next_command.txt").read_text(encoding="utf-8"), "state\n")
            self.assertTrue(result["bridge_status"]["pending_command"])

    def test_send_command_rejects_existing_pending_command(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "next_command.txt").write_text("state\n", encoding="utf-8")

            with self.assertRaises(ValueError):
                BridgeMirror(root).send_command("state")

    def test_descriptor_translation_covers_known_command_families(self):
        cases = [
            ({"kind": "PlayHandSlot", "hand_slot": 1, "target_slot": 0}, "PLAY 1 0"),
            ({"kind": "EndTurn"}, "END"),
            ({"kind": "UsePotionSlot", "potion_slot": 0, "target_slot": 1}, "POTION 0 1"),
            ({"kind": "DiscardPotionSlot", "potion_slot": 0}, "POTION 0 DISCARD"),
            ({"kind": "ChooseVisibleOption", "option_slot": 2}, "CHOOSE 2"),
            ({"kind": "ConfirmChoice"}, "CONFIRM"),
            ({"kind": "CancelChoice"}, "CANCEL"),
            ({"kind": "SkipVisibleReward"}, "SKIP"),
            ({"kind": "Proceed"}, "PROCEED"),
            ({"kind": "LeaveScreen"}, "LEAVE"),
            ({"kind": "ReturnToPreviousScreen"}, "RETURN"),
        ]

        for descriptor, command in cases:
            with self.subTest(descriptor=descriptor):
                self.assertEqual(command_for_descriptor(descriptor), command)

    def test_descriptor_translation_rejects_unknown_kind(self):
        with self.assertRaises(ValueError):
            command_for_descriptor({"kind": "Unknown"})


if __name__ == "__main__":
    unittest.main()
