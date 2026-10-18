"""Tempest's public identity exports through typed and numeric APIs."""

import json
import unittest

import sts_sim
from sts_sim import CardKey


class TempestNumericTest(unittest.TestCase):
    def test_tempest_catalog_and_numeric_transport(self) -> None:
        state = sts_sim.State.from_synthetic_spec(
            json.dumps(
                {
                    "seed": 1,
                    "floor": 1,
                    "kind": "normal",
                    "encounter": "Cultist",
                    "deck": [{"key": CardKey.TEMPEST, "upgrades": 1}] * 10,
                    "relics": ["Burning Blood"],
                    "potions": [None, None, None],
                    "hp": 80,
                    "max_hp": 80,
                    "gold": 99,
                }
            )
        )
        sts_sim.State.numeric_decisions([state])
        decision = state.decision()
        assert decision is not None
        action = next(a for a in decision.actions if a.kind == "play_hand_slot")
        state.step(action)
        observation = state.observation()
        assert observation.kind == "combat"
        self.assertEqual(observation.screen.player.energy, 0)
        sts_sim.State.numeric_decisions([state])


if __name__ == "__main__":
    unittest.main()
