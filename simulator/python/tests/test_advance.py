"""`State.advance` reports the same public run fields as `State.step`."""

import unittest

from sts_sim import CombatStep, Decision, State


class AdvanceTests(unittest.TestCase):
    def test_advance_matches_step_through_combat(self) -> None:
        stepped = State.new("81", final_act=True)
        advanced = stepped.clone()
        decision = stepped.decision()
        combat_steps = 0
        for turn in range(400):
            if not decision.actions or decision.observation.context.outcome != "ongoing":
                break
            # Deterministic, varied choices; both states take the same action index.
            index = turn % len(decision.actions)
            expected = stepped.step(decision.actions[index])
            actual = advanced.advance(advanced.legal_actions()[index])
            observation = expected.observation
            context = observation.context
            if observation.kind == "combat" and context.outcome == "ongoing":
                self.assertIsInstance(actual, CombatStep)
                assert isinstance(actual, CombatStep)
                combat_steps += 1
                self.assertEqual(
                    (actual.revision, actual.act, actual.floor, actual.player_hp),
                    (expected.revision, context.act, context.floor, context.player_hp),
                )
                self.assertEqual(actual.keys, context.keys)
                self.assertEqual(
                    actual.monsters,
                    tuple(m.content_key for m in observation.screen.monsters),
                )
                self.assertEqual(
                    [a.kind for a in actual.actions], [a.kind for a in expected.actions]
                )
            else:
                self.assertIsInstance(actual, Decision)
                assert isinstance(actual, Decision)
                self.assertEqual(actual.observation, observation)
                self.assertEqual(
                    [a.kind for a in actual.actions], [a.kind for a in expected.actions]
                )
            decision = expected
        self.assertGreater(combat_steps, 0)


if __name__ == "__main__":
    unittest.main()
