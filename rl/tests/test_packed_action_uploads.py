import unittest
from unittest.mock import patch

import numpy as np
import torch

from encoders.actions import KIND_CODE, ActionEncoder, FlatActionFeatures
from encoders.cards import CARD_FEATURE_DIM
from encoders.enemies import ENEMY_FEATURE_DIM
from encoders.numeric import upload
from encoders.potions import POTION_EMBEDDING_DIM


class PackedActionUploadTests(unittest.TestCase):
    def test_multiple_owners_and_nonzero_slots_match_per_candidate_reference(self) -> None:
        torch.set_num_threads(1)
        for device in ["cpu"] + (["cuda"] if torch.cuda.is_available() else []):
            with self.subTest(device=device):
                torch.manual_seed(73)
                encoder = ActionEncoder().to(device)
                lengths = {"hand": [3, 2, 4], "enemies": [2, 4, 1], "potions": [2, 3, 2], "selection": [4, 1, 3]}
                dimensions = {
                    "hand": CARD_FEATURE_DIM,
                    "enemies": ENEMY_FEATURE_DIM,
                    "potions": POTION_EMBEDDING_DIM,
                    "selection": CARD_FEATURE_DIM + 1,
                }
                rows = {
                    key: torch.randn(sum(lengths[key]), size, device=device, requires_grad=True)
                    for key, size in dimensions.items()
                }
                groups = {
                    "play_hand_slot": ("hand", 2),
                    "use_potion_slot": ("potions", 3),
                    "discard_potion_slot": ("potions", 3),
                    "toggle_visible_card": ("selection", 4),
                    "choose_visible_option": ("selection", 4),
                }
                candidates = []
                expected_owners = []
                for owner in range(3):
                    expected = []
                    # Different legal widths, repeated kinds, and distinct owner-local slots.
                    kinds = [*encoder.encoders, *encoder.constants] + ["play_hand_slot"] * (owner + 1)
                    for position, kind in enumerate(kinds):
                        candidate = [owner, KIND_CODE[kind], -1, -1, -1, -1]
                        if kind in encoder.constants:
                            value = encoder.constants[kind](torch.zeros(1, dtype=torch.long, device=device))
                        else:
                            group, column = groups[kind]
                            slot = (position + owner + 1) % lengths[group][owner]
                            candidate[column] = slot
                            index = sum(lengths[group][:owner]) + slot
                            inputs = rows[group][index : index + 1]
                            if kind in ("play_hand_slot", "use_potion_slot"):
                                targeted = (position + owner) % 2 == 0
                                target = (position + 1) % lengths["enemies"][owner] if targeted else -1
                                candidate[5] = target
                                index = sum(lengths["enemies"][:owner]) + target
                                enemy = (
                                    rows["enemies"][index : index + 1]
                                    if targeted
                                    else inputs.new_zeros((1, ENEMY_FEATURE_DIM))
                                )
                                inputs = torch.cat((inputs, enemy, inputs.new_full((1, 1), float(targeted))), dim=1)
                            value = encoder.encoders[kind](inputs)
                        candidates.append(candidate)
                        expected.append(value.squeeze(0))
                    expected_owners.append(torch.stack(expected))
                actual = encoder(np.asarray(candidates, dtype=np.int64), FlatActionFeatures(rows, lengths))
                reference = torch.nn.utils.rnn.pad_sequence(expected_owners, batch_first=True)
                torch.testing.assert_close(actual, reference, rtol=1e-5, atol=1e-6)
                parameters = list(encoder.parameters()) + list(rows.values())
                left = torch.autograd.grad(actual.square().sum(), parameters)
                right = torch.autograd.grad(reference.square().sum(), parameters)
                for a, b in zip(left, right, strict=True):
                    torch.testing.assert_close(a, b, rtol=1e-4, atol=1e-5)

    def test_all_kinds_targets_outputs_gradients_and_copy_count(self) -> None:
        torch.set_num_threads(1)
        for device in ["cpu"] + (["cuda"] if torch.cuda.is_available() else []):
            torch.manual_seed(31)
            encoder = ActionEncoder().to(device)
            rows = {
                key: torch.randn(1, size, device=device, requires_grad=True)
                for key, size in (
                    ("hand", CARD_FEATURE_DIM),
                    ("enemies", ENEMY_FEATURE_DIM),
                    ("potions", POTION_EMBEDDING_DIM),
                    ("selection", CARD_FEATURE_DIM + 1),
                )
            }
            features = FlatActionFeatures(rows, {key: [1] for key in rows})
            specs = [(kind, 0) for kind in encoder.encoders] + [("play_hand_slot", -1)]
            specs += [(kind, -1) for kind in encoder.constants]
            candidates = np.array([[0, KIND_CODE[kind], 0, 0, 0, target] for kind, target in specs], dtype=np.int64)
            with patch("encoders.actions.upload", wraps=upload) as copies:
                actual = encoder(candidates, features).squeeze(0)
            self.assertEqual(copies.call_count, 2)  # one packed input, one padded-output index
            expected = []
            groups = {
                "play_hand_slot": "hand",
                "use_potion_slot": "potions",
                "discard_potion_slot": "potions",
                "toggle_visible_card": "selection",
                "choose_visible_option": "selection",
            }
            for kind, target in specs:
                if kind in encoder.constants:
                    value = encoder.constants[kind](torch.zeros(1, dtype=torch.long, device=device))
                else:
                    inputs = rows[groups[kind]]
                    if kind in ("play_hand_slot", "use_potion_slot"):
                        enemy = rows["enemies"] if target >= 0 else torch.zeros_like(rows["enemies"])
                        inputs = torch.cat((inputs, enemy, inputs.new_full((1, 1), float(target >= 0))), dim=1)
                    value = encoder.encoders[kind](inputs)
                expected.append(value.squeeze(0))
            reference = torch.stack(expected)
            torch.testing.assert_close(actual, reference, rtol=1e-5, atol=1e-6)
            parameters = list(encoder.parameters()) + list(rows.values())
            left = torch.autograd.grad(actual.square().sum(), parameters)
            right = torch.autograd.grad(reference.square().sum(), parameters)
            for a, b in zip(left, right, strict=True):
                torch.testing.assert_close(a, b, rtol=1e-4, atol=1e-5)
            invalid = candidates.copy()
            invalid[0, 5] = 99
            with self.assertRaisesRegex(ValueError, "Invalid action slot"):
                encoder(invalid, features)
            invalid[0, 1] = 999
            with self.assertRaisesRegex(NotImplementedError, "Unsupported action kind"):
                encoder(invalid, features)


if __name__ == "__main__":
    unittest.main()
