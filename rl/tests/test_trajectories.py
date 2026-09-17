import copy
import unittest
from typing import cast

import torch
from model import CombatModel
from torch.distributions import Categorical
from train import Episode, episode_loss, metrics
from train_roots import collect_roots, train_batch
from trajectories import DecisionRound, Trajectories, validate_gradients


def fixture(device, dtype):
    owners = [(0, 1, 2, 3), (0, 2, 3), (0, 3)]
    counts = [(3, 2, 2, 4), (2, 3, 2), (2, 1)]
    parameters, rows = [], Trajectories()
    histories = [[] for _ in range(4)]
    entropies = [[] for _ in range(4)]
    maxima = [[] for _ in range(4)]
    sizes = [[] for _ in range(4)]
    for ids, lengths in zip(owners, counts):
        width = max(lengths)
        p = torch.nn.Parameter(torch.arange(len(ids) * width, device=device, dtype=dtype).reshape(len(ids), width) / 7)
        parameters.append(p)
        valid = torch.arange(width, device=device)[None, :] < torch.tensor(lengths, device=device)[:, None]
        distribution = Categorical(logits=p.masked_fill(~valid, -torch.inf))
        logs = distribution.log_prob(torch.zeros(len(ids), dtype=torch.long, device=device))
        entropy = distribution.entropy()
        maximum = cast(torch.Tensor, distribution.probs).max(dim=1).values.detach()
        rows.rounds.append(DecisionRound(ids, lengths, logs, entropy, maximum))
        for position, owner in enumerate(ids):
            histories[owner].append(logs[position])
            entropies[owner].append(entropy[position])
            maxima[owner].append(maximum[position].item())
            sizes[owner].append(lengths[position])
    rewards = [0.625, 0.0, None, 0.4]
    episodes = [
        Episode(
            r,
            None if r is None else r > 0,
            0,
            len(histories[i]),
            tuple(histories[i]) if r is not None else (),
            tuple(entropies[i]),
            tuple(maxima[i]),
            tuple(sizes[i]),
        )
        for i, r in enumerate(rewards)
    ]
    return parameters, rows, episodes


class TrajectoryTests(unittest.TestCase):
    def test_objective_gradients_adam_and_metrics(self):
        torch.set_num_threads(1)
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            for dtype in (torch.float32, torch.float64):
                for coefficient in (0.0, 0.01, 1.0):
                    with self.subTest(device=device, dtype=dtype, coefficient=coefficient):
                        left, _, original = fixture(device, dtype)
                        right, rows, episodes = fixture(device, dtype)
                        reference = torch.stack(
                            [episode_loss(e, coefficient) for e in original if e.reward is not None]
                        ).mean()
                        actual, policy = rows.losses(episodes, coefficient)
                        assert actual is not None and policy is not None
                        rtol, atol = (1e-12, 1e-12) if dtype == torch.float64 else (2e-6, 1e-7)
                        torch.testing.assert_close(actual, reference, rtol=rtol, atol=atol)
                        reference.backward()
                        actual.backward()
                        for a, b in zip(left, right):
                            torch.testing.assert_close(a.grad, b.grad, rtol=rtol, atol=atol)
                        if coefficient:
                            self.assertGreater(right[0].grad[1].abs().sum().item(), 0)  # Defeat still gets entropy.
                        # Truncated root 2 occurs in rows 0/1 and receives no gradient.
                        self.assertTrue(torch.equal(right[0].grad[2], torch.zeros_like(right[0].grad[2])))
                        self.assertTrue(torch.equal(right[1].grad[1], torch.zeros_like(right[1].grad[1])))
                        torch.optim.Adam(left, lr=1e-4).step()
                        torch.optim.Adam(right, lr=1e-4).step()
                        for a, b in zip(left, right):
                            torch.testing.assert_close(a, b, rtol=rtol, atol=atol)
                        expected_metrics = metrics(original)
                        for name, value in rows.metrics().items():
                            self.assertAlmostEqual(value, expected_metrics[name], places=12)

    def test_all_truncated_and_invalid_coefficients(self):
        _, rows, episodes = fixture("cpu", torch.float32)
        for episode in episodes:
            episode.reward = None
        self.assertEqual(rows.losses(episodes, 0.01), (None, None))
        self.assertTrue(rows.metrics())  # Truncated decision statistics still count.
        for value in (-1, float("inf"), float("nan")):
            with self.assertRaises(ValueError):
                rows.losses(episodes, value)
        self.assertEqual(Trajectories().metrics(), {})
        forced = Trajectories()
        forced.rounds.append(DecisionRound((0,), (1,), torch.zeros(1), torch.zeros(1), torch.ones(1)))
        self.assertEqual(forced.metrics(), {})
        with self.assertRaises(ValueError):
            Trajectories().losses([Episode(1.0, True, 80, 0, ())], 0.01)

    def test_finite_check_keeps_original_semantics(self):
        parameter = torch.nn.Parameter(torch.zeros(2))
        parameter.grad = torch.tensor([torch.finfo(torch.float32).max, 0.0])
        validate_gradients([parameter])  # A norm could overflow despite finite entries.
        for value in (float("inf"), float("nan")):
            parameter.grad[0] = value
            with self.assertRaises(RuntimeError):
                validate_gradients([parameter])

    def test_real_training_updates_and_all_truncated_batch(self):
        roots, _ = collect_roots(["2000000", "2000001", "2000002"], 3, 123)
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            torch.manual_seed(123)
            original = CombatModel().to(device)
            fast = copy.deepcopy(original)
            for limit in (128, 1):
                original.load_state_dict(fast.state_dict())
                optimizers = [torch.optim.Adam(m.parameters(), lr=1e-4) for m in (original, fast)]
                # Consecutive updates also test warmed Adam state, not just its first step.
                for update in range(2):
                    logs = []
                    for mode, model, optimizer in zip((False, True), (original, fast), optimizers):
                        torch.manual_seed(30000 + update)
                        logs.append(train_batch(roots, model, optimizer, limit, 0.01, numeric=True, batched_loss=mode))
                    self.assertEqual(logs[0].keys(), logs[1].keys())
                    for name in logs[0]:
                        self.assertAlmostEqual(logs[0][name], logs[1][name], delta=2e-5)
                    for a, b in zip(original.parameters(), fast.parameters()):
                        torch.testing.assert_close(a, b, rtol=1e-5, atol=2e-6)
                    if limit == 1:
                        self.assertEqual(logs[1]["optimizer_step"], 0)
                        self.assertFalse(optimizers[1].state)


if __name__ == "__main__":
    unittest.main()
