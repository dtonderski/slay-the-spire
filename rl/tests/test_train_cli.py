import unittest
from unittest.mock import patch

import train


class TrainingCliTests(unittest.TestCase):
    def test_default_batch_size(self) -> None:
        # Inspect the parser before any files, CUDA or W&B are initialized.
        with (
            patch(
                "argparse.ArgumentParser.parse_args", autospec=True, side_effect=RuntimeError("parser inspected")
            ) as parse,
            self.assertRaisesRegex(RuntimeError, "parser inspected"),
        ):
            train.main()
        self.assertEqual(parse.call_args.args[0].get_default("batch_size"), 512)


if __name__ == "__main__":
    unittest.main()
