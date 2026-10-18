"""Source identity transport and documented unmodeled-offer fail-closed boundary.

FIDL01535 is core natural parity evidence; this does not certify arbitrary
cross-color policy support (python_api.md, Supported-surface limits).
"""

import json
import struct
import unittest

import sts_sim
from sts_sim import CardKey


class ForeignInfluenceNumericTest(unittest.TestCase):
    def test_source_identity_and_unmodeled_offer_fail_closed(self) -> None:
        state = sts_sim.State.from_synthetic_spec(
            json.dumps(
                {
                    "seed": 1,
                    "floor": 1,
                    "kind": "normal",
                    "encounter": "Cultist",
                    "deck": [{"key": CardKey.FOREIGN_INFLUENCE, "upgrades": 0}] * 10,
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
        self.assertTrue(all(c.card.content_key is CardKey.FOREIGN_INFLUENCE for c in observation.screen.hand))
        payload = sts_sim.State.numeric_decisions([state])
        columns, data = payload[2]["hand"]
        rows = list(struct.iter_unpack("<" + "q" * columns, data))
        self.assertEqual(len(rows), 5)
        self.assertTrue(all(row[1] == 331 for row in rows))
        decision = state.decision()
        assert decision is not None
        # This initialized seed's complete native offer includes unmodeled
        # cards. Preserve the documented boundary, not invented metadata or a
        # filtered/re-rolled offer. Core generation is covered independently.
        with self.assertRaisesRegex(ValueError, "decision unavailable"):
            state.step(next(a for a in decision.actions if a.kind == "play_hand_slot"))


if __name__ == "__main__":
    unittest.main()
