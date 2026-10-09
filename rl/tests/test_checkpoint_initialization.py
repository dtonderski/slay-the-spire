import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import torch
from test_validation_set import sampler

import train
from validation_set import build_validation


class CheckpointInitializationTests(unittest.TestCase):
    def test_continuation_matches_uninterrupted_training_and_checks_protocol(self) -> None:
        configurations = [("cpu", "fp32")]
        if torch.cuda.is_available():
            configurations.append(("cuda", "fp32"))
            if torch.cuda.is_bf16_supported():
                configurations.append(("cuda", "bf16"))
        for device, precision in configurations:
            with self.subTest(device=device, precision=precision), tempfile.TemporaryDirectory() as directory:
                folder = Path(directory)
                fit, validation = folder / "fit.json", folder / "validation.json"
                fit.write_text(json.dumps(sampler().distributions))
                validation.write_text(json.dumps(build_validation(sampler(), 7, 1, 1, repeats=1)))

                def run(
                    name: str,
                    updates: int,
                    *extra: str,
                    folder: Path = folder,
                    fit: Path = fit,
                    validation: Path = validation,
                    device: str = device,
                    precision: str = precision,
                ) -> dict:
                    output = folder / name
                    argv = [
                        "train.py",
                        "--run-id",
                        str(output),
                        "--distributions",
                        str(fit),
                        "--validation-manifest",
                        str(validation),
                        "--updates",
                        str(updates),
                        "--batch-size",
                        "2",
                        "--max-decisions",
                        "128",
                        "--eval-every",
                        "100",
                        "--min-floor",
                        "1",
                        "--max-floor",
                        "1",
                        "--device",
                        device,
                        "--precision",
                        precision,
                        *extra,
                    ]
                    with (
                        patch("sys.argv", argv),
                        patch("train.wandb.init"),
                        patch("train.evaluate_baselines", return_value={}),
                        patch("train.evaluate", return_value={}),
                        contextlib.redirect_stdout(io.StringIO()),
                    ):
                        train.main()
                    return torch.load(output / "latest.pt", map_location="cpu", weights_only=False)

                full = run("full", 4)
                run("first", 2)
                checkpoint = folder / "first" / "latest.pt"
                resumed = run("resumed", 2, "--resume-from", str(checkpoint))
                for name, value in full["model"].items():
                    torch.testing.assert_close(value, resumed["model"][name], rtol=0, atol=0)
                for key, state in full["optimizer"]["state"].items():
                    for field, value in state.items():
                        torch.testing.assert_close(value, resumed["optimizer"]["state"][key][field], rtol=0, atol=0)
                self.assertEqual(full["sampling_rng"], resumed["sampling_rng"])
                self.assertTrue(torch.equal(full["torch_rng"], resumed["torch_rng"]))
                for a, b in zip(full["cuda_rng"], resumed["cuda_rng"], strict=True):
                    self.assertTrue(torch.equal(a, b))
                self.assertEqual(resumed["config"]["initialization"]["mode"], "optimizer_rng_continuation")
                self.assertEqual(resumed["config"]["precision"], precision)
                if precision == "fp32":
                    legacy = torch.load(checkpoint, map_location="cpu", weights_only=False)
                    legacy["config"].pop("precision")
                    torch.save(legacy, folder / "legacy.pt")
                    continued = run("legacy-resume", 2, "--resume-from", str(folder / "legacy.pt"))
                    for name, value in full["model"].items():
                        torch.testing.assert_close(value, continued["model"][name], rtol=0, atol=0)
                if device == "cuda" and torch.cuda.is_bf16_supported():
                    with self.assertRaisesRegex(ValueError, "precision"):
                        run(
                            "mismatch-precision",
                            1,
                            "--resume-from",
                            str(checkpoint),
                            "--precision",
                            "fp32" if precision == "bf16" else "bf16",
                        )
                with self.assertRaisesRegex(ValueError, "value_coef"):
                    run("mismatch", 1, "--resume-from", str(checkpoint), "--value-coef", "0.5")
                for option in ("width", "layers"):
                    with self.assertRaisesRegex(ValueError, f"model_{option}"):
                        run(
                            f"mismatch-{option}",
                            1,
                            "--resume-from",
                            str(checkpoint),
                            f"--model-{option}",
                            "32" if option == "width" else "3",
                        )
                small = run("small", 1, "--model-width", "32", "--model-layers", "3")
                self.assertEqual(small["config"]["model_width"], 32)
                self.assertEqual(small["config"]["model_layers"], 3)
                train.CombatValueModel(d_model=32, action_dim=32, n_layers=3).load_state_dict(
                    small["model"], strict=True
                )

                run("warm", 1, "--warm-start", str(checkpoint))
                initial = torch.load(folder / "warm" / "checkpoint-00000000.pt", map_location="cpu", weights_only=False)
                parent = torch.load(checkpoint, map_location="cpu", weights_only=False)
                for name, value in parent["model"].items():
                    torch.testing.assert_close(value, initial["model"][name], rtol=0, atol=0)
                self.assertFalse(initial["optimizer"]["state"])
                self.assertEqual(initial["config"]["initialization"]["mode"], "weights_only_new_optimizer_and_rng")


if __name__ == "__main__":
    unittest.main()
