"""Synthetic transport controls; immutable FIDL01364 is the natural evidence."""

import json
import unittest

import sts_sim
from sts_sim import CardKey, PowerKey


class MasterRealityNumericTest(unittest.TestCase):
    def test_marker_and_upgraded_zero_cost_have_typed_and_numeric_identities(self) -> None:
        state = sts_sim.State.from_synthetic_spec(
            json.dumps(
                {
                    "seed": 1,
                    "floor": 1,
                    "kind": "normal",
                    "encounter": "Cultist",
                    "deck": [{"key": CardKey.MASTER_REALITY, "upgrades": 1}] * 10,
                    "relics": ["Burning Blood"],
                    "potions": [None, None, None],
                    "hp": 80,
                    "max_hp": 80,
                    "gold": 99,
                }
            )
        )
        decision = state.decision()
        assert decision is not None
        state.step(next(a for a in decision.actions if a.kind == "play_hand_slot"))
        observation = state.observation()
        assert observation.kind == "combat"
        self.assertEqual(observation.screen.player.energy, 3)
        powers = {p.key: p.amount for p in observation.screen.player.powers}
        self.assertEqual(powers[PowerKey.MASTER_REALITY], 1)
        self.assertIn("player_powers", sts_sim.State.numeric_decisions([state])[2])


if __name__ == "__main__":
    unittest.main()
