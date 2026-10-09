import subprocess
import unittest
from pathlib import Path


JS_TEST = Path(__file__).resolve().parent / "js" / "test_explorer_frontend.mjs"


class FrontendJsTests(unittest.TestCase):
    def test_selection_guards_and_outgoing_action_identity(self) -> None:
        result = subprocess.run(
            ["node", str(JS_TEST)],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("ok", result.stdout)
