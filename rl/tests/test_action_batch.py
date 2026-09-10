import unittest
from contextlib import ExitStack
from unittest.mock import patch

import torch
from encoders.actions import ActionEncoder
from encoders.cards import CARD_FEATURE_DIM
from encoders.enemies import ENEMY_FEATURE_DIM
from encoders.potions import POTION_EMBEDDING_DIM
from model import CombatModel
from test_model import action
from train import first_combat


def feature_rows(value: float, width: int) -> torch.Tensor:
    rows = torch.zeros(2, width, dtype=torch.float64)
    rows[:, 0] = torch.tensor([value, value + 1], dtype=torch.float64)
    return rows.requires_grad_()


def features(scale: int) -> dict[str, torch.Tensor]:
    return {
        "hand": feature_rows(10 * scale, CARD_FEATURE_DIM),
        "potions": feature_rows(20 * scale, POTION_EMBEDDING_DIM),
        "enemies": feature_rows(100 * scale, ENEMY_FEATURE_DIM),
        "selection": feature_rows(30 * scale, CARD_FEATURE_DIM + 1),
    }


class ActionBatchTests(unittest.TestCase):
    def setUp(self) -> None:
        torch.set_num_threads(1)
        self.encoder = ActionEncoder(action_dim=1).double()
        # Feature actions sum their raw inputs; constants have recognizable values.
        with torch.no_grad():
            for module in self.encoder.encoders.values():
                for name, parameter in module.named_parameters():
                    parameter.fill_(0 if "bias" in name else 1)
            for index, table in enumerate(self.encoder.constants.values(), start=1):
                assert isinstance(table, torch.nn.Embedding)
                table.weight.fill_(index)
        self.actions = [
            (
                action("end_turn"),
                action("play_hand_slot", target_slot=0),
                action("use_potion_slot", target_slot=1),
                action("discard_potion_slot", potion_slot=1),
                action("toggle_visible_card", option_slot=0),
                action("choose_visible_option", option_slot=1),
                action("confirm_selection"),
                action("confirm_selection_without_retrieval"),
                action("skip_selection"),
            ),
            (),
            (
                action("skip_selection"),
                action("play_hand_slot", hand_slot=1),
                action("confirm_selection"),
                action("toggle_visible_card", option_slot=1),
                action("use_potion_slot", target_slot=0),
            ),
        ]
        self.features = [features(1), features(2), features(3)]

    def test_group_encode_and_restore_exact_order(self) -> None:
        with ExitStack() as stack:
            spies = [
                stack.enter_context(patch.object(module, "forward", wraps=module.forward))
                for module in (*self.encoder.encoders.values(), *self.encoder.constants.values())
            ]
            vectors = self.encoder(self.actions, self.features)
            self.assertTrue(all(spy.call_count == 1 for spy in spies))
        self.assertEqual(
            [rows.flatten().tolist() for rows in vectors],
            [
                [1, 111, 122, 21, 30, 31, 2, 3, 4],
                [],
                [4, 31, 2, 91, 361],
            ],
        )
        self.assertEqual(vectors[1].shape, (0, 1))
        reversed_vectors = self.encoder(self.actions[::-1], self.features[::-1])
        for a, b in zip(vectors, reversed_vectors[::-1]):
            torch.testing.assert_close(a, b)

    def test_gradients_match_separate_encoding(self) -> None:
        batched = self.encoder(self.actions, self.features)
        torch.cat(batched).square().sum().backward()
        parameter_grads = {}
        for name, parameter in self.encoder.named_parameters():
            assert parameter.grad is not None
            parameter_grads[name] = parameter.grad.clone()
        row_grads = [
            {name: rows.grad.clone() if rows.grad is not None else None for name, rows in item.items()}
            for item in self.features
        ]
        self.encoder.zero_grad(set_to_none=True)
        for item in self.features:
            for rows in item.values():
                rows.grad = None
        for candidates, rows in zip(self.actions, self.features):
            if candidates:
                self.encoder([candidates], [rows])[0].square().sum().backward()
        for name, parameter in self.encoder.named_parameters():
            torch.testing.assert_close(parameter.grad, parameter_grads[name])
        for item, expected in zip(self.features, row_grads):
            for name, rows in item.items():
                torch.testing.assert_close(rows.grad, expected[name])

    def test_empty_batches_and_local_slot_validation(self) -> None:
        self.assertEqual(self.encoder([], []), [])
        self.assertEqual([tuple(rows.shape) for rows in self.encoder([(), ()], self.features[:2])], [(0, 1)] * 2)
        with self.assertRaises(ValueError):
            self.encoder(self.actions, self.features[:1])
        small = {**self.features[0], "hand": self.features[0]["hand"][:1]}
        # Slot 1 exists in the other observation but not in this one.
        with self.assertRaises(ValueError):
            self.encoder([(action("play_hand_slot", hand_slot=1),), self.actions[2]], [small, self.features[2]])
        with self.assertRaises(NotImplementedError):
            self.encoder([(action("proceed"),)], self.features[:1])

    def test_batched_scoring_mask_sampling_and_gradients(self) -> None:
        decision = first_combat("HUMAN1", 0).decision()
        obs = decision.observation
        assert obs.kind == "combat"
        model = CombatModel(d_model=16, action_dim=8, n_layers=1).double()
        actions = [decision.actions, (decision.actions[-1],)]
        logits, valid = model([obs, obs], actions)
        self.assertEqual(valid.tolist(), [[True] * len(actions[0]), [True] + [False] * (len(actions[0]) - 1)])
        self.assertTrue(torch.isneginf(logits[~valid]).all())
        single = [model([obs], [candidates])[0][0] for candidates in actions]
        for index, expected in enumerate(single):
            torch.testing.assert_close(logits[index, : len(expected)], expected)
        samples = torch.distributions.Categorical(logits=logits).sample((100,))
        self.assertTrue(valid.gather(1, samples.T).all())
        # Fixed choices and rewards: compare the same policy-gradient objective.
        rewards = logits.new_tensor([0.25, 0.75])
        loss = -(logits.log_softmax(-1)[:, 0] * rewards).mean()
        loss.backward()
        expected_grads = {name: p.grad.clone() if p.grad is not None else None for name, p in model.named_parameters()}
        model.zero_grad(set_to_none=True)
        separate_loss = -torch.stack([row.log_softmax(-1)[0] for row in single]).mul(rewards).mean()
        separate_loss.backward()
        for name, parameter in model.named_parameters():
            torch.testing.assert_close(parameter.grad, expected_grads[name])
        with self.assertRaises(ValueError):
            model([obs], [()])
        with self.assertRaises(ValueError):
            model([obs, obs], actions[:1])
        with self.assertRaises(ValueError):
            model([], [])

    @unittest.skipUnless(torch.cuda.is_available(), "CUDA unavailable")
    def test_cuda_scoring(self) -> None:
        decision = first_combat("HUMAN1", 0).decision()
        obs = decision.observation
        assert obs.kind == "combat"
        cpu = CombatModel()
        gpu = CombatModel().cuda()
        gpu.load_state_dict(cpu.state_dict())
        actions = [decision.actions, decision.actions[:1]]
        expected, mask = cpu([obs, obs], actions)
        actual, gpu_mask = gpu([obs, obs], actions)
        torch.testing.assert_close(actual.cpu(), expected, atol=1e-5, rtol=1e-5)
        self.assertTrue(torch.equal(gpu_mask.cpu(), mask))
        actual[gpu_mask].sum().backward()
        self.assertIsNotNone(gpu.observation_encoder.cards.embedding.weight.grad)


if __name__ == "__main__":
    unittest.main()
