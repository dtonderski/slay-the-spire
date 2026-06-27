import unittest

import sts.omni as sts


class OmniRunSmokeTests(unittest.TestCase):
    def test_combat_fixture_steps_without_mutating_clone_parent(self):
        env = sts.OmniRunEnv.combat_fixture()
        before = env.snapshot_hash()
        child = env.clone()
        action = next(action for action in child.exact_legal_actions() if action.kind() == "play_card")

        result = child.step(action)

        self.assertEqual(env.snapshot_hash(), before)
        self.assertNotEqual(result.snapshot_hash, before)
        self.assertEqual(result.transition.previous_hash, before)
        self.assertEqual(child.phase(), "combat")

    def test_map_fixture_round_trips_snapshot_and_exposes_map_actions(self):
        env = sts.OmniRunEnv.map_fixture()
        restored = sts.OmniRunEnv.from_snapshot_json(env.snapshot_json())

        self.assertEqual(restored.snapshot_hash(), env.snapshot_hash())
        self.assertEqual(env.phase(), "idle")
        self.assertTrue(any(action.family() == "map" for action in env.exact_legal_actions()))

    def test_seed_start_reports_explicit_gap(self):
        with self.assertRaises(ValueError):
            sts.OmniRunEnv.new_ironclad(seed="TEST", ascension=0)


if __name__ == "__main__":
    unittest.main()
