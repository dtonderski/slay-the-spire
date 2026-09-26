"""Mixed BF16 is transformer-local, with FP32 parameter and optimizer storage."""

import copy
import unittest
from unittest.mock import patch

import torch
from sts_sim import State
from test_model import combat

from encoders.numeric import NumericBatch
from model import CombatValueModel
from train import _policy_candidates


class MixedPrecisionTests(unittest.TestCase):
    def test_cpu_rejects_mixed_bf16(self) -> None:
        batch = NumericBatch(State.numeric_decisions([combat()]))
        candidates = _policy_candidates(batch, batch.model_rows)[0]
        with self.assertRaisesRegex(ValueError, "requires CUDA"):
            CombatValueModel(precision="bf16")(batch, candidates)

    @unittest.skipUnless(torch.cuda.is_available() and torch.cuda.is_bf16_supported(), "CUDA BF16 required")
    def test_scope_storage_and_benchmark_equivalence(self) -> None:
        torch.set_num_threads(1)
        torch.manual_seed(9)
        batch = NumericBatch(State.numeric_decisions([combat(i, extra_strikes=i * 12) for i in range(1, 4)]))
        candidates = _policy_candidates(batch, batch.model_rows)[0]
        model = CombatValueModel(precision="bf16").cuda()
        reference = copy.deepcopy(model)
        reference.observation_encoder.precision = "fp32"
        original_forward = reference.observation_encoder.transformer.forward

        def benchmark_forward(*args, **kwargs):
            with torch.autocast("cuda", dtype=torch.bfloat16):
                return original_forward(*args, **kwargs).float()

        benchmark_patch = patch.object(
            reference.observation_encoder.transformer, "forward", side_effect=benchmark_forward
        )
        benchmark_patch.start()
        observed: dict[str, list[bool]] = {}
        handles = []
        for name, module in (
            ("encoder", model.observation_encoder.cards.projection),
            ("transformer", model.observation_encoder.transformer),
            ("query", model.observation_encoder.query),
            ("policy", model.policy_head),
            ("value", model.value_head),
            ("actions", model.action_encoder),
        ):

            def observe(_module, _inputs, name=name):
                observed.setdefault(name, []).append(torch.is_autocast_enabled("cuda"))

            handles.append(module.register_forward_pre_hook(observe))
        try:
            for min_rows in (10**9, 1):
                results = []
                for current in (model, reference):
                    current.observation_encoder.width_group_min_rows = min_rows
                    current.zero_grad(set_to_none=True)
                    logits, values, mask = current(batch, candidates)
                    self.assertEqual(logits.dtype, torch.float32)
                    self.assertEqual(values.dtype, torch.float32)
                    self.assertTrue(torch.isfinite(logits[mask]).all())
                    self.assertTrue(torch.isfinite(values).all())
                    (logits[mask].square().mean() + values.square().mean()).backward()
                    results.append(
                        (
                            logits.detach(),
                            values.detach(),
                            [p.grad.clone() for p in current.parameters() if p.grad is not None],
                        )
                    )
                for actual, expected in zip(results[0][:2], results[1][:2], strict=True):
                    torch.testing.assert_close(actual, expected, rtol=0, atol=0)
                for actual, expected in zip(results[0][2], results[1][2], strict=True):
                    torch.testing.assert_close(actual, expected, rtol=1e-5, atol=1e-6)
            for name, active in observed.items():
                self.assertTrue(active)
                self.assertTrue(all(flag == (name == "transformer") for flag in active), name)
            self.assertEqual(len(observed), 6)
            optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
            optimizer.step()
            for parameter in model.parameters():
                self.assertEqual(parameter.dtype, torch.float32)
                if parameter.grad is not None:
                    self.assertEqual(parameter.grad.dtype, torch.float32)
                    self.assertTrue(torch.isfinite(parameter.grad).all())
            for state in optimizer.state.values():
                for value in state.values():
                    if isinstance(value, torch.Tensor):
                        self.assertEqual(value.dtype, torch.float32)
                        self.assertTrue(torch.isfinite(value).all())
        finally:
            benchmark_patch.stop()
            for handle in handles:
                handle.remove()


if __name__ == "__main__":
    unittest.main()
