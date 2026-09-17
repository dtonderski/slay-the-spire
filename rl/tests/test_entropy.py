import unittest

import torch
from torch.distributions import Categorical
from train import Episode, episode_loss, metrics, reinforce_loss


class EntropyTests(unittest.TestCase):
    def test_bonus_pushes_overconfident_logits_toward_exploration(self) -> None:
        logits = torch.tensor([3.0, 0.0, float("-inf")], requires_grad=True)
        distribution = Categorical(logits=logits)
        entropy = distribution.entropy()
        episode = Episode(0.0, False, 0, 1, (distribution.log_prob(torch.tensor(0)),), (entropy,), (0.95,), (2,))
        loss = episode_loss(episode, 0.01)
        self.assertTrue(torch.isfinite(loss))
        loss.backward()
        assert logits.grad is not None
        self.assertGreater(logits.grad[0].item(), 0)
        self.assertLess(logits.grad[1].item(), 0)
        self.assertEqual(logits.grad[2].item(), 0)

    def test_zero_coefficient_and_forced_choice_metrics(self) -> None:
        logp = torch.tensor(-0.3, requires_grad=True)
        episode = Episode(0.5, True, 50, 2, (logp,), (torch.tensor(0.0), torch.tensor(0.69314718)), (1.0, 0.5), (1, 2))
        torch.testing.assert_close(episode_loss(episode, 0), reinforce_loss((logp,), 0.5))
        logs = metrics([episode])
        self.assertAlmostEqual(logs["policy_entropy"], 0.69314718)
        self.assertAlmostEqual(logs["normalized_policy_entropy"], 1.0)
        self.assertEqual(logs["mean_max_action_probability"], 0.5)
        with self.assertRaises(ValueError):
            episode_loss(Episode(None, None, 50, 1, ()), 0.01)


if __name__ == "__main__":
    unittest.main()
