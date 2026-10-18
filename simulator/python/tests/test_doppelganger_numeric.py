"""Synthetic transport coverage; real-game evidence is the unchanged FIDL01504 capture."""

import json
import unittest

import sts_sim
from sts_sim import CardKey, PowerKey


class DoppelgangerNumericTest(unittest.TestCase):
    def test_visible_next_turn_powers_have_typed_and_numeric_identities(self) -> None:
        state = sts_sim.State.from_synthetic_spec(
            json.dumps(
                {
                    "seed": 1,
                    "floor": 1,
                    "kind": "normal",
                    "encounter": "Cultist",
                    "deck": [{"key": CardKey.DOPPELGANGER, "upgrades": 0}] * 10,
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
        self.assertEqual(observation.screen.player.energy, 0)
        powers = {p.key: p.amount for p in observation.screen.player.powers}
        self.assertEqual(powers[PowerKey.DRAW_CARD], 3)
        self.assertEqual(powers[PowerKey.ENERGIZED], 3)
        payload = sts_sim.State.numeric_decisions([state])
        self.assertIn("player_powers", payload[2])


if __name__ == "__main__":
    unittest.main()
