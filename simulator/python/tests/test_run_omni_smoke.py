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

    def test_seed_start_uses_placeholder_generated_map(self):
        first = sts.OmniRunEnv.new_ironclad(seed="TEST", ascension=0)
        second = sts.OmniRunEnv.new_ironclad(seed="TEST", ascension=0)
        other = sts.OmniRunEnv.new_ironclad(seed="OTHER", ascension=0)

        self.assertEqual(first.phase(), "idle")
        self.assertEqual(first.current_decision(), "map")
        self.assertEqual(first.snapshot_hash(), second.snapshot_hash())
        self.assertNotEqual(first.snapshot_hash(), other.snapshot_hash())
        self.assertTrue(any(action.family() == "map" for action in first.exact_legal_actions()))


if __name__ == "__main__":
    unittest.main()
