"""Rollout cast caching must end before gradients or a parameter update."""

import copy
import unittest

import torch
from test_model import combat

from model import CombatValueModel
from train import train_batch
from validation_set import Root


class RolloutAutocastCacheTests(unittest.TestCase):
    @unittest.skipUnless(torch.cuda.is_available() and torch.cuda.is_bf16_supported(), "CUDA BF16 required")
    def test_two_updates_match_uncached_and_keep_scope_local(self) -> None:
        torch.set_num_threads(1)
        torch.manual_seed(42)
        cached = CombatValueModel(precision="bf16").cuda()
        uncached = copy.deepcopy(cached)
        optimizers = [torch.optim.Adam(model.parameters(), lr=1e-4) for model in (cached, uncached)]
        roots = [Root(combat(i), str(i), 1, 80, i) for i in range(3)]
        observed = []
        handle = cached.policy_head.register_forward_pre_hook(
            lambda _module, _inputs: observed.append((torch.is_grad_enabled(), torch.is_autocast_enabled("cuda")))
        )
        try:
            for step in range(2):
                torch.manual_seed(100 + step)
                actual = train_batch(roots, cached, optimizers[0], 128, 0.01, chunk_decisions=32)
                torch.manual_seed(100 + step)
                with torch.autocast("cuda", enabled=False, cache_enabled=False):
                    expected = train_batch(roots, uncached, optimizers[1], 128, 0.01, chunk_decisions=32)
                self.assertEqual(actual["optimizer_step"], 1.0)
                self.assertEqual(expected["optimizer_step"], 1.0)
                self.assertAlmostEqual(actual["loss"], expected["loss"], places=5)
                for left, right in zip(cached.parameters(), uncached.parameters(), strict=True):
                    torch.testing.assert_close(left, right, rtol=1e-5, atol=1e-6)
                    self.assertEqual(left.grad is None, right.grad is None)
                    if left.grad is not None:
                        torch.testing.assert_close(left.grad, right.grad, rtol=1e-4, atol=1e-5)
                self.assertFalse(torch.is_autocast_enabled("cuda"))
            self.assertTrue(any(grad for grad, _ in observed))
            self.assertTrue(any(not grad for grad, _ in observed))
            self.assertFalse(any(amp for _, amp in observed))
        finally:
            handle.remove()


if __name__ == "__main__":
    unittest.main()
