import contextlib
import io
import unittest
from unittest.mock import patch

import train


class EvaluationCadenceTests(unittest.TestCase):
    def test_default_uses_update_count(self) -> None:
        self.assertFalse(train.evaluation_due(99, 100, 10000, None))
        self.assertTrue(train.evaluation_due(100, 100, 0, None))

    def test_interval_overrides_count(self) -> None:
        self.assertFalse(train.evaluation_due(100, 100, 299, 300))
        self.assertTrue(train.evaluation_due(99, 100, 300, 300))
        self.assertFalse(train.evaluation_due(200, 100, 0, 300))

    def test_invalid_interval_rejected_before_loading_files(self) -> None:
        for value in ("0", "-1", "nan", "inf"):
            with self.subTest(value=value):
                argv = [
                    "train.py",
                    "--run-id",
                    "unused",
                    "--distributions",
                    "unused",
                    "--validation-manifest",
                    "unused",
                    "--eval-interval-seconds",
                    value,
                ]
                with (
                    patch("sys.argv", argv),
                    contextlib.redirect_stderr(io.StringIO()) as stderr,
                    self.assertRaises(SystemExit) as error,
                ):
                    train.main()
                self.assertEqual(error.exception.code, 2)
                self.assertIn("Evaluation interval must be finite and positive", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
