"""Modeled public identities must agree across typed and fixed numeric catalogs."""

import json
import unittest

import sts_sim
from sts_sim import CardKey


class CrossColorNumericCatalogTest(unittest.TestCase):
    def test_all_repaired_card_keys_export_without_unknown_identity_errors(self) -> None:
        for key in (
            "COLD_SNAP",
            "DISTRACTION",
            "EMPTY_FIST",
            "EXPERTISE",
            "FLYING_KNEE",
            "PIERCING_WAIL",
            "STACK",
        ):
            with self.subTest(key=key):
                identity = CardKey(key)
                state = sts_sim.State.from_synthetic_spec(
                    json.dumps(
                        {
                            "seed": 1,
                            "floor": 1,
                            "kind": "normal",
                            "encounter": "Cultist",
                            "deck": [{"key": identity, "upgrades": 0}] * 10,
                            "relics": ["Burning Blood"],
                            "potions": [None, None, None],
                            "hp": 80,
                            "max_hp": 80,
                            "gold": 99,
                        }
                    )
                )
                observation = state.observation()
                assert observation.kind == "combat"
                self.assertTrue(all(c.card.content_key is identity for c in observation.screen.hand))
                payload = sts_sim.State.numeric_decisions([state])
                self.assertIn("hand", payload[2])


if __name__ == "__main__":
    unittest.main()
