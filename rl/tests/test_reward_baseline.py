import json
import tempfile
import unittest
from pathlib import Path

import torch
from train import Episode
from train_synthetic import RewardBaseline, cached_references
from trajectories import DecisionRound, Trajectories


class RewardBaselineTests(unittest.TestCase):
    def test_negative_advantage_and_truncation_exclusion(self) -> None:
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            logs = torch.tensor([-0.4, -0.3, -0.2], device=device, requires_grad=True)
            trajectories = Trajectories()
            trajectories.rounds.append(
                DecisionRound((0, 1, 2), (2, 2, 2), logs, torch.zeros_like(logs), torch.zeros_like(logs))
            )
            episodes = [Episode(0.8, True, 80, 1, ()), Episode(0.0, False, 0, 1, ()), Episode(None, None, 10, 1, ())]
            loss, _ = trajectories.losses(episodes, 0.0, reward_baseline=0.3)
            assert loss is not None
            loss.backward()
            torch.testing.assert_close(logs.grad, torch.tensor([-0.25, 0.15, 0.0], device=device))
            with self.assertRaises(ValueError):
                trajectories.losses(episodes, 0.0, reward_baseline=float("nan"))

    def test_ema_uses_only_previous_successful_completed_batches(self) -> None:
        baseline = RewardBaseline()
        self.assertEqual(baseline.value, 0.0)
        baseline.observe({"optimizer_step": 0, "completed": 256, "mean_return_completed": 0.9})
        self.assertEqual(baseline.updates, 0)
        baseline.observe({"optimizer_step": 1, "completed": 2, "mean_return_completed": 0.4})
        self.assertEqual(baseline.value, 0.4)
        baseline.observe({"optimizer_step": 1, "completed": 0})
        self.assertEqual(baseline.value, 0.4)
        baseline.observe({"optimizer_step": 1, "completed": 2, "mean_return_completed": 0.2})
        self.assertAlmostEqual(baseline.value, 0.39)
        self.assertEqual(baseline.updates, 2)

    def test_reference_reuse_requires_matching_inputs(self) -> None:
        settings = {
            "validation_sha256": "validation",
            "validation_native_sha256": "native",
            "evaluation_repeats": 3,
            "evaluation_max_decisions": 512,
            "beam_width": 64,
            "beam_transitions": 10000,
        }
        payload = {
            "beam_privileged": True,
            "sets": {
                label: {"random": {"completed": 3}, "privileged_beam": {"errors": 0}} for label in ("main", "stress")
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            source, output = Path(directory) / "source", Path(directory) / "output"
            source.mkdir()
            output.mkdir()
            (source / "config.json").write_text(json.dumps(settings))
            (source / "baselines.json").write_text(json.dumps(payload))
            references = cached_references(source, output, settings)
            self.assertEqual(references["random_main/completed"], 3)
            self.assertEqual((output / "baselines.json").read_bytes(), (source / "baselines.json").read_bytes())
            with self.assertRaisesRegex(ValueError, "validation_sha256"):
                cached_references(source, output, {**settings, "validation_sha256": "different"})


if __name__ == "__main__":
    unittest.main()
