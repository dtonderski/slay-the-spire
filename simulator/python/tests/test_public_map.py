import copy
import unittest
from dataclasses import asdict

from sts_sim import MapScreen, State
from sts_sim.observations import decode_observation


class PublicMapTests(unittest.TestCase):
    def test_neow_observation_includes_map_without_navigation(self) -> None:
        state = State.new("713", final_act=True)
        before = state.decision()
        self.assertEqual(before.observation.kind, "event")
        public_map = before.observation.context.map
        self.assertIsInstance(public_map, MapScreen)
        self.assertGreater(len(public_map.nodes), 15)
        self.assertTrue(any(n.room_kind == "event" for n in public_map.nodes))
        self.assertTrue(any(n.burning_elite for n in public_map.nodes))
        for _ in range(3):
            self.assertEqual(state.observation(), before.observation)
            self.assertEqual(state.revision, before.revision)
        other = state.clone()
        for _ in range(2):
            left, right = state.decision(), other.decision()
            self.assertEqual(left.observation, right.observation)
            self.assertEqual(len(left.actions), len(right.actions))
            state.observation()  # Only the left side performs extra inspections.
            self.assertEqual(
                state.step(left.actions[0]).observation,
                other.step(right.actions[0]).observation,
            )

    def test_map_decoder_is_strict_and_immutable(self) -> None:
        observation = State.new("713").observation()
        raw = asdict(observation)
        self.assertEqual(decode_observation(raw), observation)
        for field, value in (("seed", 713), ("act", True)):
            malformed = copy.deepcopy(raw)
            malformed["context"]["map"][field] = value
            with self.assertRaises((TypeError, ValueError)):
                decode_observation(malformed)
        malformed = copy.deepcopy(raw)
        malformed["context"]["map"]["nodes"][0]["encounter"] = "hidden"
        with self.assertRaises(ValueError):
            decode_observation(malformed)
        del raw["context"]["map"]
        with self.assertRaises(ValueError):
            decode_observation(raw)


if __name__ == "__main__":
    unittest.main()
