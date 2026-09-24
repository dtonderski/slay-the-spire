import random
import unittest

import torch
from model import CombatValueModel
from test_model import combat
from train import ReplayRound, accumulate_replay_loss, play_combats, train_batch
from trajectories import Trajectories
from validation_set import Root


def _collect(model: CombatValueModel, states: list) -> tuple:
    replays: list[ReplayRound] = []
    with torch.no_grad():
        episodes = play_combats(
            states,
            model,
            max_decisions=64,
            training=False,
            rng=random.Random(0),
            replays=replays,
        )
    return episodes, replays


class _NaNValuesWithGrad(CombatValueModel):
    """Finite rollouts, but a non-finite replay objective."""

    def forward(self, observations, candidates):  # type: ignore[override]
        logits, values, mask = super().forward(observations, candidates)
        if torch.is_grad_enabled():
            values = values * float("nan")
        return logits, values, mask


class ChunkedUpdateTests(unittest.TestCase):
    def test_chunks_match_one_backward_and_fight_normalization(self) -> None:
        torch.set_num_threads(1)
        devices = ["cpu"] + (["cuda"] if torch.cuda.is_available() else [])
        for device in devices:
            torch.manual_seed(3)
            model = CombatValueModel().to(device)
            states = [combat(index) for index in range(3)]
            episodes, replays = _collect(model, states)
            self.assertTrue(replays)
            self.assertGreater(sum(episode.decisions for episode in episodes), 1)
            one = CombatValueModel().to(device)
            many = CombatValueModel().to(device)
            one.load_state_dict(model.state_dict())
            many.load_state_dict(model.state_dict())
            full = accumulate_replay_loss(one, replays, episodes, 0.01, 0.1, chunk_decisions=10**9)
            split = accumulate_replay_loss(many, replays, episodes, 0.01, 0.1, chunk_decisions=1)
            assert full is not None and split is not None
            for left, right in zip(full, split, strict=True):
                torch.testing.assert_close(left, right, rtol=1e-5, atol=1e-6)
            compared = 0
            for left, right in zip(one.parameters(), many.parameters(), strict=True):
                if left.grad is None and right.grad is None:
                    continue
                self.assertIsNotNone(left.grad)
                self.assertIsNotNone(right.grad)
                torch.testing.assert_close(left.grad, right.grad, rtol=1e-4, atol=1e-5)
                compared += 1
            self.assertGreater(compared, 0)

    def test_similar_rounds_share_a_stacked_forward(self) -> None:
        torch.set_num_threads(1)
        model = CombatValueModel()
        states = [combat(1).clone() for _ in range(3)]
        episodes, replays = _collect(model, states)
        self.assertGreater(len(replays), 1)
        calls = []
        handle = model.register_forward_pre_hook(lambda _module, inputs: calls.append(len(inputs[0])))
        try:
            accumulated = accumulate_replay_loss(model, replays, episodes, 0.01, 0.1, chunk_decisions=10**9)
        finally:
            handle.remove()
        self.assertIsNotNone(accumulated)
        self.assertLess(len(calls), len(replays))
        self.assertGreater(max(calls), 1)

    def test_train_batch_takes_one_step_without_retaining_rollout_graph(self) -> None:
        torch.set_num_threads(1)
        model = CombatValueModel()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        roots = [Root(combat(1), "1", 1, 80, 1)]
        scores = train_batch(roots, model, optimizer, 64, 0.01, chunk_decisions=2)
        self.assertEqual(scores["optimizer_step"], 1.0)
        self.assertTrue(optimizer.state)
        self.assertGreaterEqual(scores["replay_storage_mib"], 0.0)

    def test_chunked_recompute_matches_retained_graph_on_the_same_samples(self) -> None:
        torch.set_num_threads(1)
        devices = ["cpu"] + (["cuda"] if torch.cuda.is_available() else [])
        for device in devices:
            torch.manual_seed(5)
            model = CombatValueModel().to(device).train()
            trajectories = Trajectories()
            replays: list[ReplayRound] = []
            episodes = play_combats(
                [combat(index) for index in range(3)],
                model,
                max_decisions=64,
                training=True,
                rng=random.Random(0),
                trajectories=trajectories,
                replays=replays,
            )
            self.assertTrue(replays)
            retained_loss, _ = trajectories.losses(episodes, 0.01, value_coef=0.1)
            assert retained_loss is not None
            retained_grads = torch.autograd.grad(retained_loss, list(model.parameters()), allow_unused=True)
            recomputed = CombatValueModel().to(device).train()
            recomputed.load_state_dict(model.state_dict())
            accumulated = accumulate_replay_loss(recomputed, replays, episodes, 0.01, 0.1, chunk_decisions=1)
            assert accumulated is not None
            torch.testing.assert_close(accumulated[0], retained_loss.detach(), rtol=1e-5, atol=1e-6)
            compared = 0
            for left, right in zip(
                retained_grads, (parameter.grad for parameter in recomputed.parameters()), strict=True
            ):
                if left is None and right is None:
                    continue
                self.assertIsNotNone(left)
                self.assertIsNotNone(right)
                torch.testing.assert_close(left, right, rtol=1e-4, atol=1e-5)
                compared += 1
            self.assertGreater(compared, 0)

    def test_non_finite_replay_loss_raises_without_an_optimizer_step(self) -> None:
        torch.set_num_threads(1)
        devices = ["cpu"] + (["cuda"] if torch.cuda.is_available() else [])
        for device in devices:
            torch.manual_seed(5)
            model = _NaNValuesWithGrad().to(device)
            optimizer = torch.optim.Adam(model.parameters(), lr=1e-2)
            before = [parameter.detach().clone() for parameter in model.parameters()]
            roots = [Root(combat(index), str(index), 1, 80) for index in range(3)]
            with self.assertRaisesRegex(RuntimeError, "Non-finite loss"):
                train_batch(roots, model, optimizer, 64, 0.01, value_coef=0.1, chunk_decisions=4)
            for old, new in zip(before, model.parameters(), strict=True):
                self.assertTrue(torch.equal(old, new.detach()))

if __name__ == "__main__":
    unittest.main()
