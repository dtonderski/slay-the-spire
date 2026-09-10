import random
import unittest
from dataclasses import replace
from unittest.mock import patch

import torch
from train import (
    Episode,
    combat_outcome,
    first_combat,
    metrics,
    play_combat,
    reinforce_loss,
)


class TrainingTests(unittest.TestCase):
    def test_hp_reward_and_defeat(self) -> None:
        root = first_combat("HUMAN1", 0)
        hp = root.decision().observation.context.player_hp
        max_hp = root.decision().observation.context.player_max_hp
        with patch("train.combat_outcome", return_value=True):
            won = play_combat(root, None, max_decisions=1, rng=random.Random(0))
        self.assertEqual(won.reward, hp / max_hp)
        self.assertTrue(won.won)
        with patch("train.combat_outcome", return_value=False):
            lost = play_combat(root, None, max_decisions=1, rng=random.Random(0))
        self.assertEqual(lost.reward, 0)
        self.assertFalse(lost.won)
        obs = root.decision().observation
        assert obs.kind == "combat"
        self.assertIsNone(combat_outcome(obs))
        self.assertFalse(combat_outcome(replace(obs, screen=replace(obs.screen, phase="lost"))))
        self.assertTrue(combat_outcome(replace(obs, screen=replace(obs.screen, phase="won"))))

    def test_truncation_is_not_a_loss(self) -> None:
        root = first_combat("HUMAN1", 0)
        episode = play_combat(root, None, max_decisions=0, rng=random.Random(0))
        self.assertIsNone(episode.reward)
        self.assertIsNone(episode.won)
        self.assertEqual(episode.log_probs, ())

    def test_win_rate_is_independent_of_hp_reward(self) -> None:
        result = metrics([Episode(0.5, True, 40, 10, ()), Episode(0, False, 0, 15, ())])
        self.assertEqual(result["win_rate_completed"], 0.5)
        self.assertEqual(result["mean_return_completed"], 0.25)

    def test_plain_reinforce_gradient(self) -> None:
        logits = torch.tensor([0.0, 0.0], requires_grad=True)
        loss = reinforce_loss((logits.log_softmax(0)[0],), 0.5)
        loss.backward()
        torch.testing.assert_close(logits.grad, torch.tensor([-0.25, 0.25]))


if __name__ == "__main__":
    unittest.main()
