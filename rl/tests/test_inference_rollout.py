import unittest
from typing import Literal
from unittest.mock import patch

import torch
from test_model import combat

import train
from model import CombatValueModel
from validation_set import Root


class InferenceRolloutTests(unittest.TestCase):
    def test_rollout_matches_no_grad_and_replay_updates_normal_parameters(self) -> None:
        torch.set_num_threads(1)
        configurations: list[tuple[str, Literal["fp32", "bf16"]]] = [("cpu", "fp32")]
        if torch.cuda.is_available():
            configurations.extend([("cuda", "fp32"), ("cuda", "bf16")])
        original_play = train.play_combats

        def no_grad_play(*args, **kwargs):
            with torch.inference_mode(False), torch.no_grad():
                return original_play(*args, **kwargs)

        for device, precision in configurations:
            torch.manual_seed(5)
            reference = CombatValueModel(precision=precision).to(device)
            candidate = CombatValueModel(precision=precision).to(device)
            candidate.load_state_dict(reference.state_dict())
            optimizers = [torch.optim.Adam(model.parameters(), lr=1e-4) for model in (reference, candidate)]
            roots = [Root(combat(index), str(index), 1, 80) for index in range(3)]
            contexts = []
            handle = candidate.register_forward_pre_hook(
                lambda _module, _inputs, contexts=contexts: contexts.append(
                    (torch.is_inference_mode_enabled(), torch.is_grad_enabled())
                )
            )
            try:
                for step in range(2):
                    torch.manual_seed(22 + step)
                    with patch("train.play_combats", side_effect=no_grad_play):
                        expected = train.train_batch(roots, reference, optimizers[0], 64, 0.01, chunk_decisions=16)
                    torch.manual_seed(22 + step)
                    actual = train.train_batch(roots, candidate, optimizers[1], 64, 0.01, chunk_decisions=16)
                    self.assertEqual(expected.keys(), actual.keys())
                    for key in expected:
                        self.assertAlmostEqual(expected[key], actual[key], delta=1e-5, msg=key)
                    for left, right in zip(reference.parameters(), candidate.parameters(), strict=True):
                        self.assertFalse(torch.is_inference(right))
                        torch.testing.assert_close(left, right, rtol=1e-4, atol=1e-6)
                        if left.grad is not None:
                            self.assertIsNotNone(right.grad)
                            assert right.grad is not None
                            self.assertFalse(torch.is_inference(right.grad))
                            torch.testing.assert_close(left.grad, right.grad, rtol=1e-4, atol=1e-5)
                    for state in optimizers[1].state.values():
                        for value in state.values():
                            if isinstance(value, torch.Tensor):
                                self.assertFalse(torch.is_inference(value))
            finally:
                handle.remove()
            self.assertIn((True, False), contexts)
            self.assertIn((False, True), contexts)
            self.assertFalse(torch.is_inference_mode_enabled())

    def test_collection_error_restores_context_without_update(self) -> None:
        model = CombatValueModel()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        with (
            patch("train.play_combats", side_effect=RuntimeError("collection failed")),
            self.assertRaisesRegex(RuntimeError, "collection failed"),
        ):
            train.train_batch([], model, optimizer, 64, chunk_decisions=8192)
        self.assertFalse(torch.is_inference_mode_enabled())
        self.assertTrue(torch.is_grad_enabled())
        self.assertFalse(optimizer.state)


if __name__ == "__main__":
    unittest.main()
