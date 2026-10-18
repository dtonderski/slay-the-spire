"""Visible Blur survives typed/numeric transport; synthetic input, not parity evidence."""

import json
import unittest

import sts_sim
from sts_sim import CardKey, PowerKey


class BlurNumericTest(unittest.TestCase):
    def test_blur_card_and_power_export_without_unknown_vocabulary(self) -> None:
        state = sts_sim.State.from_synthetic_spec(
            json.dumps(
                {
                    "seed": 1,
                    "floor": 1,
                    "kind": "normal",
                    "encounter": "Cultist",
                    "deck": [{"key": CardKey.BLUR, "upgrades": 0}] * 10,
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
        action = next(a for a in decision.actions if a.kind == "play_hand_slot")
        state.step(action)
        observation = state.observation()
        assert observation.kind == "combat"
        self.assertTrue(
            any(p.key is PowerKey.BLUR and p.amount == 1 for p in observation.screen.player.powers)
        )
        payload = sts_sim.State.numeric_decisions([state])
        self.assertIn("player_powers", payload[2])


if __name__ == "__main__":
    unittest.main()
