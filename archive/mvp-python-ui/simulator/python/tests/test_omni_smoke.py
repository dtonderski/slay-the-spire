import json
import unittest

import sts.omni as sts


class OmniSmokeTests(unittest.TestCase):
    def test_initial_fixture_round_trips_snapshot_json(self):
        env = sts.OmniCombatEnv.initial_fixture()
        restored = sts.OmniCombatEnv.from_snapshot_json(env.snapshot_json())

        self.assertEqual(restored.snapshot_hash(), env.snapshot_hash())
        self.assertEqual(json.loads(env.snapshot_json())["schema_version"], 1)

    def test_exact_legal_actions_and_step(self):
        env = sts.OmniCombatEnv.initial_fixture()
        actions = env.exact_legal_actions()

        self.assertTrue(any(action.kind() == "end_turn" for action in actions))

        strike = next(action for action in actions if action.card_id() == 1)
        before = env.snapshot_hash()
        result = env.step(strike)

        self.assertNotEqual(result.snapshot_hash, before)
        self.assertEqual(result.transition.previous_hash, before)
        self.assertEqual(result.transition.resulting_hash, result.snapshot_hash)
        self.assertTrue(json.loads(result.transition.events_json))

    def test_clone_and_inspection_do_not_mutate_parent(self):
        env = sts.OmniCombatEnv.initial_fixture()
        before = env.snapshot_hash()
        child = env.clone()

        env.state_json()
        env.exact_legal_actions()
        self.assertEqual(env.snapshot_hash(), before)

        strike = next(action for action in child.exact_legal_actions() if action.card_id() == 1)
        child.step(strike)
        self.assertEqual(env.snapshot_hash(), before)
        self.assertNotEqual(child.snapshot_hash(), before)


if __name__ == "__main__":
    unittest.main()
