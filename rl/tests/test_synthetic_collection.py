import random
import unittest

import torch
from combat_task import combat_outcome
from model import CombatModel
from sts_sim import State
from train import play_combats
from train_roots import collect_roots


class SyntheticCollectionTests(unittest.TestCase):
    def test_collects_through_heart_via_legal_keys_and_boss_chests(self) -> None:
        roots, manifest = collect_roots(["1000005"], 1, 123, synthetic_act4=True)
        entry = manifest[0]
        self.assertEqual(entry["status"], "complete")
        self.assertEqual(entry["last_act"], 4)
        self.assertEqual({root.act for root in roots}, {1, 2, 3, 4})
        self.assertTrue(entry["final_act_available"])
        self.assertEqual(entry["synthetic_acts"], 4)
        replay = State.new_synthetic(entry["seed"], final_act=True)
        keys = set()
        chests = 0
        for index in entry["accepted_action_indices"]:
            decision = replay.decision()
            action = decision.actions[index]
            if action.kind in ("rest_recall", "take_sapphire_key", "take_emerald_key"):
                keys.add(action.kind)
            if decision.observation.kind == "treasure" and decision.observation.screen.chest_size == "boss":
                chests += 1
            replay.step(action)
        self.assertEqual(keys, {"rest_recall", "take_sapphire_key", "take_emerald_key"})
        self.assertGreater(chests, 0)
        self.assertTrue(combat_outcome(replay.decision().observation))
        for root in roots:
            obs = root.state.decision().observation
            self.assertEqual((obs.context.player_hp, obs.context.player_max_hp), (100, 100))
        # Later-act public features work in both rollout transports, including terminal victory.
        sample = [next(root.state for root in roots if root.act == act) for act in range(1, 5)] + [
            roots[-1].state,
            replay,
        ]
        torch.set_num_threads(1)
        torch.manual_seed(1)
        model = CombatModel()
        records = []
        for numeric in (False, True):
            torch.manual_seed(2)
            episodes = play_combats(sample, model, max_decisions=128, rng=random.Random(0), numeric=numeric)
            records.append([(e.reward, e.won, e.hp, e.decisions) for e in episodes])
        self.assertEqual(records[0], records[1])
        with self.assertRaises(ValueError):
            collect_roots(["1000005"], 1, 123, synthetic_act1=True, synthetic_act4=True)

    def test_seed_prefix_reconstruction_and_independent_hp_normalization(self) -> None:
        roots, manifest = collect_roots(["1000000"], 1, 123, synthetic_act1=True)
        self.assertEqual(manifest[0]["status"], "act1_complete")
        self.assertEqual(roots[-1].floor, 16)  # Flag overrides the ordinary floor limit.
        self.assertEqual(manifest[0]["synthetic_initial_hp"], 10000)
        self.assertEqual(manifest[0]["synthetic_root_hp"], 100)
        replay = State.new_synthetic("1000000")
        before = replay.decision().observation
        self.assertEqual(before.context.player_hp, 10000)
        self.assertEqual(before.context.player_max_hp, 10000)
        first = manifest[0]["roots"][0]
        for index in manifest[0]["accepted_action_indices"][: first["prefix_length"]]:
            replay.step(replay.decision().actions[index])
        before = replay.decision().observation
        normalized = replay.synthetic_combat_root(100)
        self.assertEqual(replay.decision().observation, before)
        self.assertEqual(normalized.decision().observation, roots[0].state.decision().observation)
        for root in roots:
            obs = root.state.decision().observation
            self.assertEqual((obs.context.player_hp, obs.context.player_max_hp), (100, 100))
        with self.assertRaises(ValueError):
            replay.synthetic_combat_root(0)
        with self.assertRaises(ValueError):
            State.new("HUMAN1").synthetic_combat_root(100)
        with self.assertRaises(ValueError):
            State.new_synthetic("HUMAN1", hp=0)


if __name__ == "__main__":
    unittest.main()
