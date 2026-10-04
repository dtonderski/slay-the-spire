"""Initial-state API invariants using simulator-only journals, not parity traces."""

import gzip
import hashlib
import json
import unittest
from pathlib import Path

from sts_sim import State

FIXTURES = Path(__file__).parent / "fixtures" / "campfire"


def natural_campfire(case: dict) -> State:
    path = FIXTURES / case["journal"]
    assert hashlib.sha256(path.read_bytes()).hexdigest() == case["journal_sha256"]
    with gzip.open(path, "rt") as source:
        rows = [json.loads(line) for line in source]
    state = State.new(rows[0]["seed"], ascension=0, final_act=True)
    accepted = {r["step"] for r in rows if r["type"] == "accepted"}
    for row in rows:
        if row["type"] != "attempt":
            continue
        if row["step"] == case["stop_step"]:
            return state
        assert row["step"] in accepted
        decision = state.decision()
        assert decision.revision == row["revision"]
        action = decision.actions[row["index"]]
        assert {key: getattr(action, key) for key in row["action"]} == row["action"]
        state.step(action)
    raise AssertionError("Missing campfire prefix")


class SyntheticRestRootTests(unittest.TestCase):
    def test_independent_hp_only_configuration_and_revision_fence(self) -> None:
        for case in json.loads((FIXTURES / "metadata.json").read_text()):
            source = natural_campfire(case)
            original = source.decision()
            for hp in (1, 25, original.observation.context.player_max_hp):
                root = source.synthetic_rest_root(hp)
                ob = root.observation()
                self.assertEqual(ob.context.player_hp, hp)
                self.assertEqual(
                    ob.context.player_max_hp, original.observation.context.player_max_hp
                )
                self.assertEqual(ob.context.deck, original.observation.context.deck)
                self.assertEqual(ob.context.potion_slots, original.observation.context.potion_slots)
                self.assertEqual(root.revision, source.revision + 1)
                self.assertEqual(source.observation(), original.observation)
                self.assertEqual(source.revision, original.revision)
                with self.assertRaisesRegex(ValueError, "stale"):
                    root.step(original.actions[0])
                self.assertEqual(root.observation(), ob)
                again = source.synthetic_rest_root(hp)
                one = root.step(root.decision().actions[0])
                two = again.step(again.decision().actions[0])
                self.assertEqual(one.observation, two.observation)
                self.assertEqual(source.observation(), original.observation)
                self.assertEqual(source.revision, original.revision)
                with self.assertRaises(ValueError):
                    root.synthetic_rest_root(hp)  # Campfire already used.
            for hp in (0, -1, original.observation.context.player_max_hp + 1):
                with self.assertRaises(ValueError):
                    source.synthetic_rest_root(hp)
            self.assertEqual(source.observation(), original.observation)
            self.assertEqual(source.revision, original.revision)

    def test_non_rest_constructor_rejected(self) -> None:
        with self.assertRaises(ValueError):
            State.new("1").synthetic_rest_root(25)


if __name__ == "__main__":
    unittest.main()
