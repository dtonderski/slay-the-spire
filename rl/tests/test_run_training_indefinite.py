"""Runner controls and vocabulary migration; not gameplay parity evidence."""

import io
import json
import signal
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest.mock import patch

import torch
from encoders.cards import CARD_TO_INDEX
from encoders.vocabulary import CARD_KEYS, CARD_KEYS_V1
from model import CombatValueModel
from run_training.collector import RunEpisode
from run_training.combat_compat import frozen_weights
from run_training.trainer import main
from sts_sim import CardKey
from test_run_training_trainer import learned_episode


class IndefiniteTests(unittest.TestCase):
    def test_unbounded_updates_and_disabled_skip_limit_stop_on_signal(self):
        for incomplete in (False, True):
            with self.subTest(incomplete=incomplete), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "seeds.json").write_text('["100"]')
                (root / "combat.pt").write_bytes(b"mocked")

                def collect(seed, model, combat, *, incomplete=incomplete, **kwargs):
                    if incomplete:
                        return RunEpisode("cutoff", None, (), 0, 0)
                    return learned_episode(model)

                def publish(row, **kwargs):
                    if "train/attempts" in row and row["iteration"] == 3:
                        signal.raise_signal(signal.SIGTERM)

                with (
                    patch("run_training.trainer.FrozenCombat"),
                    patch("run_training.trainer.collect", side_effect=collect),
                    patch("run_training.trainer.wandb.init") as init,
                ):
                    run = init.return_value.__enter__.return_value
                    run.url = None
                    run.log.side_effect = publish
                    main(
                        [
                            "--run-id",
                            "test",
                            "--output-root",
                            directory,
                            "--combat-checkpoint",
                            str(root / "combat.pt"),
                            "--validation-seeds",
                            str(root / "seeds.json"),
                            "--updates",
                            "0",
                            "--max-consecutive-skips",
                            "0",
                            "--continue-on-collection-failure",
                            "--collection-width",
                            "1",
                            "--batch-size",
                            "1",
                            "--model-width",
                            "8",
                            "--eval-every",
                            "100",
                        ]
                    )
                checkpoint = torch.load(root / "test/latest.pt", weights_only=True)
                self.assertEqual(checkpoint["iteration"], 3)
                self.assertEqual(checkpoint["optimizer_updates"], 0 if incomplete else 3)
                self.assertEqual(checkpoint["skipped_batches"], 3 if incomplete else 0)
                self.assertEqual(json.loads((root / "test/finished.json").read_text())["reason"], "budget_or_signal")

    def test_negative_limits_rejected(self):
        for flag in ("--updates", "--max-consecutive-skips"):
            with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                main(["--run-id", "bad", "--combat-checkpoint", "unused", "--validation-seeds", "unused", flag, "-1"])

    def test_card_prefix_and_explicit_weight_extension(self):
        self.assertEqual(CARD_KEYS[322:], ("COLD_SNAP", "PIERCING_WAIL"))
        for index, name in enumerate(CARD_KEYS_V1):
            self.assertEqual(CARD_TO_INDEX[CardKey(name)], index)
        model = CombatValueModel(d_model=8, action_dim=8, n_layers=1, precision="fp32")
        key = "observation_encoder.cards.embedding.weight"
        weights = {name: value.clone() for name, value in model.state_dict().items()}
        weights[key] = weights[key][:322].clone()
        original = weights[key].clone()
        with self.assertRaisesRegex(ValueError, "requires explicit"):
            frozen_weights(model, weights, adapt_legacy=False)
        adapted = frozen_weights(model, weights, adapt_legacy=True)
        torch.testing.assert_close(adapted[key][:322], original, rtol=0, atol=0)
        self.assertEqual(torch.count_nonzero(adapted[key][322:]).item(), 0)
        self.assertEqual(weights[key].shape[0], 322)
        model.load_state_dict(adapted, strict=True)
        for name, value in weights.items():
            if name != key:
                torch.testing.assert_close(model.state_dict()[name], value, rtol=0, atol=0)
        with self.assertRaisesRegex(ValueError, "requires a v1"):
            frozen_weights(model, model.state_dict(), adapt_legacy=True)
        with self.assertRaisesRegex(ValueError, "parameter keys"):
            frozen_weights(model, {**weights, "unknown": torch.zeros(1)}, adapt_legacy=True)
        bad = dict(weights)
        other = next(name for name, value in bad.items() if name != key and value.ndim > 0)
        bad[other] = bad[other][:0]
        with self.assertRaisesRegex(ValueError, "shape mismatch"):
            frozen_weights(model, bad, adapt_legacy=True)


if __name__ == "__main__":
    unittest.main()
