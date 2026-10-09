import unittest

import numpy as np
import torch

from encoders.numeric import NUMERIC_VERSION, NumericBatch
from train import Episode, ReplayRound, _decision_rounds_from_forward
from trajectories import DecisionRound, Trajectories


class FlatReplayDecisionTests(unittest.TestCase):
    def test_group_preserves_order_masks_truncation_loss_and_gradients(self) -> None:
        for device in ["cpu"] + (["cuda"] if torch.cuda.is_available() else []):
            torch.manual_seed(4)
            batch = NumericBatch((NUMERIC_VERSION, [], {}, []))
            rounds = [
                ReplayRound(batch, np.empty((0, 6)), (0, 1, 2), (1, 0, 0), (3, 2, 1)),
                ReplayRound(batch, np.empty((0, 6)), (0, 2), (0, 3), (1, 4)),
                ReplayRound(batch, np.empty((0, 6)), (2,), (1,), (2,)),
            ]
            weights = torch.nn.Parameter(torch.randn(6, 4, device=device))
            values = torch.nn.Parameter(torch.randn(6, 1, device=device))
            counts = torch.tensor([3, 2, 1, 1, 4, 2], device=device)
            logits = weights.masked_fill(torch.arange(4, device=device)[None, :] >= counts[:, None], float("-inf"))
            actual = Trajectories()
            actual.rounds = _decision_rounds_from_forward(rounds, logits, values)
            self.assertEqual(len(actual.rounds), 1)
            flat = actual.rounds[0]
            self.assertEqual(flat.owners, (0, 1, 2, 0, 2, 2))
            self.assertEqual(flat.counts, (3, 2, 1, 1, 4, 2))
            self.assertEqual(flat.values.data_ptr(), values.data_ptr())
            expected = Trajectories()
            offset = 0
            for replay in rounds:
                stop = offset + len(replay.owners)
                expected.rounds.append(
                    DecisionRound(
                        replay.owners,
                        replay.counts,
                        flat.log_probs[offset:stop],
                        flat.entropies[offset:stop],
                        flat.max_probabilities[offset:stop],
                        flat.values[offset:stop],
                    )
                )
                offset = stop
            episodes = [Episode(0.7, True, 56, 2), Episode(None, None, 0, 1), Episode(0.0, False, 0, 3)]
            left, left_policy = actual.losses(episodes, 0.01, 0.1)
            right, right_policy = expected.losses(episodes, 0.01, 0.1)
            assert left is not None and right is not None
            torch.testing.assert_close(left, right, rtol=0, atol=0)
            torch.testing.assert_close(left_policy, right_policy, rtol=0, atol=0)
            a = torch.autograd.grad(left, (weights, values), retain_graph=True)
            b = torch.autograd.grad(right, (weights, values))
            for x, y in zip(a, b, strict=True):
                torch.testing.assert_close(x, y, rtol=0, atol=0)
                self.assertTrue(torch.equal(x[1], torch.zeros_like(x[1])))
            self.assertEqual(actual.metrics(), expected.metrics())


if __name__ == "__main__":
    unittest.main()
