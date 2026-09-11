import random
import unittest
from dataclasses import replace
from types import SimpleNamespace
from typing import cast
from unittest.mock import patch

import torch
from model import CombatModel
from sts_sim import CombatObservation, State
from test_model import action
from train import first_combat, play_combats, reinforce_loss
from train_roots import Root, evaluate, train_batch


class ScriptedCombat:
    """Tiny rollout fixture, not an implementation of gameplay."""

    def __init__(self, observations: list[CombatObservation]) -> None:
        self.observations = observations
        self.index = 0
        self.actions = (action("end_turn"), action("skip_selection"))

    def clone(self):
        return ScriptedCombat(self.observations)

    def decision(self):
        return SimpleNamespace(observation=self.observations[self.index], actions=self.actions)

    def step(self, candidate):
        assert candidate is self.actions[0], "Wrong observation's action, or unexpected sampled choice"
        self.index += 1
        return self.decision()


class ScriptedPolicy(torch.nn.Module):
    def __init__(self) -> None:
        super().__init__()
        self.weight = torch.nn.Parameter(torch.tensor(0.2, dtype=torch.float64))
        self.calls = []

    def forward(self, observations, actions):
        tags = [obs.context.floor for obs in observations]
        self.calls.append(tags)
        scores = self.weight * self.weight.new_tensor(tags)
        logits = torch.stack((torch.zeros_like(scores), scores), dim=1)
        return logits, torch.ones_like(logits, dtype=torch.bool)


def choose_first(distribution):
    return torch.zeros(distribution.batch_shape, dtype=torch.long, device=distribution.logits.device)


class BatchedRolloutTests(unittest.TestCase):
    def setUp(self) -> None:
        torch.set_num_threads(1)
        obs = first_combat("HUMAN1", 0).decision().observation
        assert obs.kind == "combat"
        self.roots = []
        for tag, max_hp, phases in (
            (1, 80, ("waiting_for_player", "won")),
            (2, 100, ("waiting_for_player", "waiting_for_player", "lost")),
            (3, 120, ("waiting_for_player",) * 3),
        ):
            observations = [
                replace(
                    obs,
                    context=replace(obs.context, floor=tag, player_hp=40, player_max_hp=max_hp),
                    screen=replace(obs.screen, phase=phase),
                )
                for phase in phases
            ]
            self.roots.append(cast(State, ScriptedCombat(observations)))
        self.policy = ScriptedPolicy()
        self.model = cast(CombatModel, self.policy)

    def test_active_batch_shrinks_and_episode_order_is_preserved(self) -> None:
        with patch("train.Categorical.sample", autospec=True, side_effect=choose_first):
            episodes = play_combats(self.roots, self.model, max_decisions=2, training=True, rng=random.Random(0))
        self.assertEqual(self.policy.calls, [[1, 2, 3], [2, 3]])
        self.assertEqual([ep.reward for ep in episodes], [0.5, 0, None])
        self.assertEqual([ep.won for ep in episodes], [True, False, None])
        self.assertEqual([ep.decisions for ep in episodes], [1, 2, 2])
        self.assertEqual([len(ep.log_probs) for ep in episodes], [1, 2, 0])
        torch.testing.assert_close(episodes[0].log_probs[0], -torch.nn.functional.softplus(self.policy.weight))
        for log_prob in episodes[1].log_probs:
            torch.testing.assert_close(log_prob, -torch.nn.functional.softplus(2 * self.policy.weight))
        # One backward across episodes sharing earlier batched forward graphs.
        loss = torch.stack([reinforce_loss(ep.log_probs, ep.reward) for ep in episodes if ep.reward is not None]).mean()
        loss.backward()
        torch.testing.assert_close(self.policy.weight.grad, 0.25 * self.policy.weight.detach().sigmoid())
        self.assertTrue(all(cast(ScriptedCombat, root).index == 0 for root in self.roots))

    def test_training_update_excludes_truncation_and_uses_one_backward(self) -> None:
        optimizer = torch.optim.SGD(self.model.parameters(), lr=0.1)
        before = self.policy.weight.detach().clone()
        roots = [Root(root, str(i), i + 1, 40) for i, root in enumerate(self.roots)]
        with patch("train.Categorical.sample", autospec=True, side_effect=choose_first):
            logs = train_batch(roots, self.model, optimizer, max_decisions=2)
        self.assertEqual(logs["completed"], 2)
        self.assertEqual(logs["truncated"], 1)
        self.assertEqual(logs["optimizer_step"], 1)
        torch.testing.assert_close(self.policy.weight, before - 0.1 * 0.25 * before.sigmoid())

    def test_all_truncated_skips_optimizer_and_eval_keeps_no_graphs(self) -> None:
        optimizer = torch.optim.Adam(self.model.parameters())
        before = self.policy.weight.detach().clone()
        root = Root(self.roots[2], "3", 3, 40)
        with patch("train.Categorical.sample", autospec=True, side_effect=choose_first):
            logs = train_batch([root], self.model, optimizer, max_decisions=1)
            episodes = play_combats(self.roots[:2], self.model, max_decisions=2, rng=random.Random(0))
        self.assertEqual(logs["optimizer_step"], 0)
        self.assertNotIn("loss", logs)
        self.assertEqual(optimizer.state, {})
        torch.testing.assert_close(self.policy.weight, before)
        self.assertTrue(all(ep.log_probs == () for ep in episodes))
        with self.assertRaises(ValueError):
            play_combats([], self.model, max_decisions=2, rng=random.Random(0))

    @unittest.skipUnless(torch.cuda.is_available(), "CUDA unavailable")
    def test_cuda_evaluation_preserves_sampling_rng(self) -> None:
        root = first_combat("HUMAN1", 0)
        model = CombatModel(d_model=16, action_dim=8, n_layers=1).cuda()
        roots = [Root(root, "HUMAN1", 1, root.decision().observation.context.player_hp)]
        cpu_rng = torch.random.get_rng_state().clone()
        gpu_rng = torch.cuda.get_rng_state().clone()
        first = evaluate(roots, model, repeats=1, max_decisions=4)
        self.assertTrue(torch.equal(cpu_rng, torch.random.get_rng_state()))
        self.assertTrue(torch.equal(gpu_rng, torch.cuda.get_rng_state()))
        self.assertEqual(first, evaluate(roots, model, repeats=1, max_decisions=4))

    def test_live_training_and_evaluation_rng_isolation(self) -> None:
        root = first_combat("HUMAN1", 0)
        before = root.decision()
        torch.manual_seed(123)
        model = CombatModel(d_model=16, action_dim=8, n_layers=1)
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        roots = [Root(root, "HUMAN1", 1, before.observation.context.player_hp)] * 3
        old = model.observation_encoder.query.weight.detach().clone()
        logs = train_batch(roots, model, optimizer, max_decisions=128)
        self.assertEqual(logs["episodes"], 3)
        self.assertEqual(logs["optimizer_step"], 1)
        self.assertFalse(torch.equal(old, model.observation_encoder.query.weight))
        self.assertEqual(root.decision().observation, before.observation)
        self.assertEqual(repr(root.decision().actions), repr(before.actions))
        rng = torch.random.get_rng_state().clone()
        first = evaluate(roots[:1], model, repeats=2, max_decisions=128)
        self.assertTrue(torch.equal(rng, torch.random.get_rng_state()))
        self.assertEqual(first, evaluate(roots[:1], model, repeats=2, max_decisions=128))


if __name__ == "__main__":
    unittest.main()
