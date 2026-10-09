"""Synthetic weights-only compatibility tests, not gameplay parity evidence."""

import unittest

import torch
from encoders.cards import CARD_FEATURE_DIM, CARD_TO_INDEX
from encoders.enemies import ENEMY_FEATURE_DIM
from encoders.potions import POTION_EMBEDDING_DIM
from encoders.vocabulary import LEGACY_V1
from model import CombatValueModel
from run_training.combat_compat import LEGACY_PROTOCOL, STRICT_PROTOCOL, frozen_weights
from sts_sim import CardKey


class CombatCompatibilityTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        self.model = CombatValueModel(d_model=16, action_dim=16, n_layers=1)
        old_enemy = ENEMY_FEATURE_DIM - 9
        # Independent semantic mapping: each OLD feature position points to its
        # current position. No optimizer state or source checkpoint is modified.
        self.columns = {
            "observation_encoder.cards.projection.weight": list(range(27)),
            "observation_encoder.enemies.projection.weight": list(range(old_enemy)),
            "action_encoder.encoders.play_hand_slot.0.weight": [
                *range(27),
                *range(CARD_FEATURE_DIM, CARD_FEATURE_DIM + old_enemy),
                CARD_FEATURE_DIM + ENEMY_FEATURE_DIM,
            ],
            "action_encoder.encoders.use_potion_slot.0.weight": [
                *range(POTION_EMBEDDING_DIM + old_enemy),
                POTION_EMBEDDING_DIM + ENEMY_FEATURE_DIM,
            ],
            "action_encoder.encoders.toggle_visible_card.0.weight": [*range(27), CARD_FEATURE_DIM],
            "action_encoder.encoders.choose_visible_option.0.weight": [*range(27), CARD_FEATURE_DIM],
        }
        self.weights = {key: value.clone() for key, value in self.model.state_dict().items()}
        for key, columns in self.columns.items():
            self.weights[key] = self.weights[key][:, columns].clone()
        key = "observation_encoder.cards.embedding.weight"
        self.weights[key] = self.weights[key][: len(LEGACY_V1["CardKey"])].clone()

    def test_current_weights_are_strict_and_legacy_needs_explicit_opt_in(self):
        current = self.model.state_dict()
        loaded, protocol = frozen_weights(self.model, current, adapt_legacy=False)
        self.assertIs(loaded, current)
        self.assertEqual(protocol, STRICT_PROTOCOL)
        with self.assertRaisesRegex(ValueError, "explicit --adapt-legacy"):
            frozen_weights(self.model, self.weights, adapt_legacy=False)

    def test_semantic_weights_and_public_identity_indices_are_preserved(self):
        weights, protocol = frozen_weights(self.model, self.weights, adapt_legacy=True)
        self.assertEqual(protocol, LEGACY_PROTOCOL)
        for index, key in enumerate(LEGACY_V1["CardKey"]):
            self.assertEqual(CARD_TO_INDEX[CardKey(key)], index)
        for key, columns in self.columns.items():
            old = self.weights[key]
            new = weights[key]
            torch.testing.assert_close(new[:, columns], old, rtol=0, atol=0)
            extra = sorted(set(range(new.shape[1])) - set(columns))
            self.assertEqual(torch.count_nonzero(new[:, extra]).item(), 0)
            inputs = torch.arange(new.shape[1], dtype=old.dtype)[None, :]
            torch.testing.assert_close(inputs @ new.T, inputs[:, columns] @ old.T)
        key = "observation_encoder.cards.embedding.weight"
        count = len(LEGACY_V1["CardKey"])
        torch.testing.assert_close(weights[key][:count], self.weights[key], rtol=0, atol=0)
        self.assertEqual(torch.count_nonzero(weights[key][count:]).item(), 0)
        self.model.load_state_dict(weights, strict=True)

    def test_unknown_shapes_parameters_and_mixed_protocols_are_rejected(self):
        for mode in ("unknown_shape", "missing_parameter", "mixed_protocol"):
            weights = dict(self.weights)
            if mode == "unknown_shape":
                weights["policy_head.weight"] = weights["policy_head.weight"][:, :-1]
            elif mode == "missing_parameter":
                weights.pop("policy_head.bias")
            else:
                weights["observation_encoder.cards.projection.weight"] = self.model.state_dict()[
                    "observation_encoder.cards.projection.weight"
                ]
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                frozen_weights(self.model, weights, adapt_legacy=True)


if __name__ == "__main__":
    unittest.main()
