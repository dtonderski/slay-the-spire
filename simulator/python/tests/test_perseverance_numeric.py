"""Synthetic API controls, not live parity; natural witness is FIDL00213."""

import json
import unittest

import sts_sim
from sts_sim import CardKey


class PerseveranceNumericTest(unittest.TestCase):
    def test_retained_growth_is_typed_and_numeric_identity_is_append_only(self) -> None:
        state = sts_sim.State.from_synthetic_spec(
            json.dumps(
                {
                    "seed": 1,
                    "floor": 1,
                    "kind": "normal",
                    "encounter": "Cultist",
                    "deck": [{"key": CardKey.PERSEVERANCE, "upgrades": 0}] * 10,
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
        state.step(next(a for a in decision.actions if a.kind == "end_turn"))
        obs = state.observation()
        assert obs.kind == "combat"
        growth = [s.card.dynamic.perseverance_retain_block for s in obs.screen.hand]
        self.assertEqual(growth.count(2), 5)
        self.assertEqual(growth.count(None), 5)
        payload = sts_sim.State.numeric_decisions([state])
        width, _ = payload[2]["hand"]
        self.assertEqual(width, 18)  # v3 remains a documented subset, not lossless growth.
        # Playing one of the original retained cards gives its increased7 block.
        decision = state.decision()
        assert decision is not None
        state.step(next(a for a in decision.actions if a.kind == "play_hand_slot"))
        obs = state.observation()
        assert obs.kind == "combat"
        self.assertEqual(obs.screen.player.block, 7)


if __name__ == "__main__":
    unittest.main()
