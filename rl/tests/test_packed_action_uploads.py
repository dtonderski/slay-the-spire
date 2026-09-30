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
