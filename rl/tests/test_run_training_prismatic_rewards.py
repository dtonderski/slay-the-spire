"""Immutable simulator-only journals: reward publication, NOT full Prismatic combat."""

import gzip
import json
import unittest
from dataclasses import asdict
from pathlib import Path

from run_training.contracts import PolicyAction
from run_training.model import encode
from run_training.roots import natural_state_from_setup
from sts_sim import CardKey

FIXTURES = Path(__file__).parent / "fixtures/run-boundary"


class PrismaticRewardBoundaryTests(unittest.TestCase):
    def test_original_legal_kill_publishes_reward_and_can_skip_without_fake_card_effects(self):
        for filename, expected in (
            ("prismatic-reward-failure.jsonl.gz", ("CHILL", 0, 0)),
            ("prismatic-egg-reward-failure.jsonl.gz", ("DAGGER_THROW", 1, 1)),
        ):
            with self.subTest(filename=filename), gzip.open(FIXTURES / filename, "rt") as source:
                rows = [json.loads(line) for line in source]
            state = natural_state_from_setup(rows[0])
            decision = state.decision()
            accepted = {row["step"] for row in rows if row["type"] == "accepted"}
            found = False
            for row in rows:
                if row["type"] != "attempt":
                    continue
                self.assertEqual(decision.revision, row["revision"])
                action = decision.actions[row["index"]]
                self.assertEqual(asdict(PolicyAction.from_action(action)), row["action"])
                if row["step"] in accepted:
                    decision = state.step(action)
                    continue
                self.assertIn("Prismatic Shard", [r.content_key for r in decision.observation.context.relics])
                clone = state.clone()
                decision = state.step(action)
                copied = clone.step(clone.decision().actions[row["index"]])
                self.assertEqual(decision.observation, copied.observation)
                self.assertEqual(decision.observation.kind, "reward")
                assert decision.observation.kind == "reward"
                self.assertEqual(decision.revision, row["revision"] + 1)
                preview = next(
                    c.card for c in decision.observation.screen.cards if c.card.content_key.value == expected[0]
                )
                self.assertEqual((preview.content_key.value, preview.cost, preview.upgrade_level), expected)
                # Every original legal candidate remains present in actor encoding.
                encoded = encode(decision.observation, tuple(PolicyAction.from_action(a) for a in decision.actions))
                self.assertEqual(len(encoded.candidates), len(decision.actions))
                state.step(next(a for a in decision.actions if a.kind == "skip_reward"))
                self.assertTrue(state.decision().actions)
                found = True
                break
            self.assertTrue(found)

    def test_chill_is_acquired_as_a_real_card_without_rewriting_the_journal(self):
        with gzip.open(FIXTURES / "prismatic-reward-failure.jsonl.gz", "rt") as source:
            rows = [json.loads(line) for line in source]
        state = natural_state_from_setup(rows[0])
        for row in rows:
            if row["type"] == "attempt":
                state.step(state.decision().actions[row["index"]])
        decision = state.decision()
        decision = state.step(next(a for a in decision.actions if a.kind == "open_card_reward"))
        assert decision.observation.kind == "reward"
        slot = next(c.slot for c in decision.observation.screen.cards if c.card.content_key == CardKey.CHILL)
        action = next(a for a in decision.actions if a.kind == "take_card_reward" and a.reward_slot == slot)
        before = decision.observation.context.deck
        clone = state.clone()
        after = state.step(action)
        copied = clone.step(clone.decision().actions[decision.actions.index(action)])
        self.assertEqual(after.observation, copied.observation)
        self.assertEqual(state.revision, decision.revision + 1)
        deck = after.observation.context.deck
        self.assertEqual(len(deck), len(before) + 1)
        self.assertEqual(sum(c.content_key == CardKey.CHILL for c in deck), 1)
        self.assertEqual(after.observation.context.outcome, "ongoing")


if __name__ == "__main__":
    unittest.main()
