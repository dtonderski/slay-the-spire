"""Batched numeric steps: parallel execution must match one-state steps exactly."""

import json
import os
import subprocess
import sys
import unittest

import sts_sim

OWNER, LEGAL_INDEX, REVISION, ROW_WIDTH = 0, 1, 11, 12


def combat(seed: int, strikes: int) -> sts_sim.State:
    """Small synthetic test input, not a captured trace."""
    deck = [{"key": "Strike_R", "upgrades": 0}] * strikes + [{"key": "Defend_R", "upgrades": 0}] * 4
    spec = {
        "seed": seed,
        "floor": 1,
        "kind": "normal",
        "encounter": "Cultist",
        "deck": deck + [{"key": "Bash", "upgrades": 0}],
        "relics": ["Burning Blood"],
        "potions": [None, None, None],
        "hp": 80,
        "max_hp": 80,
        "gold": 99,
    }
    return sts_sim.State.from_synthetic_spec(json.dumps(spec))


def action_rows(payload: tuple) -> list[tuple[int, ...]]:
    width, data = payload[2]["action_rows"]
    assert width == ROW_WIDTH
    values = memoryview(data).cast("q")
    return [tuple(values[start : start + width]) for start in range(0, len(values), width)]


class NumericStepsTest(unittest.TestCase):
    def test_batched_steps_match_single_steps_and_reject_repeated_states(self) -> None:
        # Enough states for several native workers; each must match a one-state call.
        states = [combat(seed, 5 + seed % 7) for seed in range(1, 301)]
        singles = [state.clone() for state in states]
        first: dict[int, tuple[int, ...]] = {}
        for row in action_rows(sts_sim.State.numeric_decisions(states)):
            first.setdefault(row[OWNER], row)
        legal = [first[index][LEGAL_INDEX] for index in range(len(states))]
        revisions = [first[index][REVISION] for index in range(len(states))]
        batched = action_rows(sts_sim.State.numeric_steps(states, legal, revisions))
        for index, (state, choice, revision) in enumerate(
            zip(singles, legal, revisions, strict=True)
        ):
            single = action_rows(sts_sim.State.numeric_steps([state], [choice], [revision]))
            owned = [row[1:] for row in batched if row[OWNER] == index]
            self.assertEqual(owned, [row[1:] for row in single])
        self.assertEqual([s.observation() for s in states], [s.observation() for s in singles])

        before = [state.revision for state in states[:2]]
        with self.assertRaisesRegex(ValueError, "repeated in the batch"):
            sts_sim.State.numeric_steps(
                [states[0], states[1], states[0]], [0, 0, 0], [*before, before[0]]
            )
        self.assertEqual([state.revision for state in states[:2]], before)

    def test_invalid_numeric_thread_override_is_rejected(self) -> None:
        script = (
            "import json, sts_sim; "
            "spec = {'seed': 1, 'floor': 1, 'kind': 'normal', 'encounter': 'Cultist', "
            "'deck': [{'key': 'Strike_R', 'upgrades': 0}] * 10, 'relics': ['Burning Blood'], "
            "'potions': [None, None, None], 'hp': 80, 'max_hp': 80, 'gold': 99}; "
            "s = sts_sim.State.from_synthetic_spec(json.dumps(spec)); "
            "sts_sim.State.numeric_steps([s], [0], [s.revision])"
        )
        result = subprocess.run(
            [sys.executable, "-c", script],
            env={**os.environ, "STS_NUMERIC_THREADS": "zero"},
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("STS_NUMERIC_THREADS must be a positive integer", result.stderr)


if __name__ == "__main__":
    unittest.main()
