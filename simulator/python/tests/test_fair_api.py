import unittest

import sts_sim


class FairApiTest(unittest.TestCase):
    def test_policy_package_exposes_only_fair_state_surface(self) -> None:
        state = sts_sim.State.new("HUMAN1")
        self.assertFalse(hasattr(sts_sim, "FullState"))
        self.assertFalse(hasattr(state, "full_state"))
        self.assertFalse(hasattr(state, "to_json"))
        self.assertFalse(hasattr(sts_sim.State, "from_json"))

        decision = state.decision()
        self.assertEqual(decision.schema_version, 1)
        self.assertEqual(decision.revision, state.revision)
        self.assertEqual(decision.observation.schema_version, 3)
        self.assertEqual(len(decision.actions), len(state.legal_actions()))

    def test_clone_and_step_preserve_revision_contract(self) -> None:
        state = sts_sim.State.new("1")
        clone = state.clone()
        decision = state.decision()
        self.assertEqual(clone.revision, decision.revision)
        self.assertEqual(clone.decision().revision, decision.revision)

        result = state.step(decision.actions[0])
        self.assertEqual(result.revision, decision.revision + 1)
        self.assertEqual(state.revision, result.revision)
        self.assertEqual(clone.revision, decision.revision)

        with self.assertRaisesRegex(ValueError, "stale"):
            state.step(decision.actions[0])


if __name__ == "__main__":
    unittest.main()
