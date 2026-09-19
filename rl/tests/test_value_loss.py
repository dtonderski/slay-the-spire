import json
import tempfile
import unittest
from pathlib import Path

import torch
from train import Episode, cached_references
from trajectories import DecisionRound, Trajectories


class ValueLossTests(unittest.TestCase):
    def test_negative_advantage_and_truncation_exclusion(self) -> None:
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            logs = torch.tensor([-0.4, -0.3, -0.2], device=device, requires_grad=True)
            values = torch.full_like(logs, 0.3, requires_grad=True)
            trajectories = Trajectories()
            trajectories.rounds.append(
                DecisionRound((0, 1, 2), (2, 2, 2), logs, torch.zeros_like(logs), torch.zeros_like(logs), values)
            )
            episodes = [Episode(0.8, True, 80, 1), Episode(0.0, False, 0, 1), Episode(None, None, 10, 1)]
            loss, _ = trajectories.losses(episodes, 0.0, value_coef=0.5)
            assert loss is not None
            loss.backward()
            torch.testing.assert_close(logs.grad, torch.tensor([-0.25, 0.15, 0.0], device=device))
            torch.testing.assert_close(values.grad, torch.tensor([-0.25, 0.15, 0.0], device=device))
            with self.assertRaises(ValueError):
                trajectories.losses(episodes, 0.0, value_coef=float("nan"))

    def test_multiround_loss_and_gradients_match_explicit_objective(self) -> None:
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            for dtype in (torch.float32, torch.float64):
                a = torch.ones((2, 2), device=device, dtype=dtype, requires_grad=True)
                b = torch.zeros((2, 3), device=device, dtype=dtype, requires_grad=True)
                da, db = torch.distributions.Categorical(logits=a), torch.distributions.Categorical(logits=b)
                selected = torch.zeros(2, dtype=torch.long, device=device)
                la, lb = da.log_prob(selected), db.log_prob(selected)
                ha, hb = da.entropy(), db.entropy()
                rows = Trajectories()
                rows.rounds = [
                    DecisionRound((0, 1), (2, 2), la, ha, torch.zeros_like(la), torch.full_like(la, 0.2)),
                    DecisionRound((0, 2), (3, 3), lb, hb, torch.zeros_like(lb), torch.full_like(lb, 0.2)),
                ]
                episodes = [Episode(0.8, True, 80, 2), Episode(0.0, False, 0, 1), Episode(None, None, 10, 1)]
                actual, _ = rows.losses(episodes, 0.01, value_coef=0.5)
                assert actual is not None
                expected = (-0.6 * (la[0] + lb[0]) + 0.2 * la[1] - 0.01 * (ha.sum() + hb[0])) / 2
                expected = expected + 0.5 * (0.6**2 + 0.2**2 + 0.6**2) / 2
                torch.testing.assert_close(actual, expected)
                gradients = torch.autograd.grad(actual, (a, b), retain_graph=True)
                reference = torch.autograd.grad(expected, (a, b))
                for x, y in zip(gradients, reference, strict=True):
                    torch.testing.assert_close(x, y)

    def test_value_coefficient_controls_only_value_head_gradient(self) -> None:
        for coefficient in (0.0, 0.5, 1.0):
            logs = torch.tensor([-0.4], requires_grad=True)
            values = torch.tensor([0.3], requires_grad=True)
            rows = Trajectories()
            rows.rounds = [DecisionRound((0,), (2,), logs, logs * 0, logs * 0, values)]
            loss, policy = rows.losses([Episode(0.8, True, 80, 1)], 0.0, value_coef=coefficient)
            assert loss is not None and policy is not None
            self.assertIsNone(torch.autograd.grad(policy, values, allow_unused=True, retain_graph=True)[0])
            loss.backward()
            torch.testing.assert_close(logs.grad, torch.tensor([-0.5]))
            torch.testing.assert_close(values.grad, torch.tensor([-coefficient]))

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
            self.assertFalse(any("stress" in key for key in references))
            self.assertEqual((output / "baselines.json").read_bytes(), (source / "baselines.json").read_bytes())
            with self.assertRaisesRegex(ValueError, "validation_sha256"):
                cached_references(source, output, {**settings, "validation_sha256": "different"})


if __name__ == "__main__":
    unittest.main()
