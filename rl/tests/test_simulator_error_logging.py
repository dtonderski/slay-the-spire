"""Failed native steps discard whole batches, never fabricate gameplay outcomes."""

import json
import random
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import torch
from model import CombatModel
from rollout_errors import SimulatorStepError
from sts_sim import State
from train import Episode, first_combat, play_combats
from train_roots import Root, train_batch


class SimulatorErrorLoggingTests(unittest.TestCase):
    def test_typed_and_partially_advanced_numeric_batches_preserve_roots(self) -> None:
        roots = [first_combat("1000000", 0), first_combat("1000001", 0)]
        before = [root.decision() for root in roots]
        advanced = []

        def partial_failure(states, actions):
            previous = states[0].revision
            states[0].step(actions[0])
            advanced.append(states[0].revision > previous)
            raise ValueError("choice is invalid")

        with patch.object(State, "numeric_steps", side_effect=partial_failure) as steps:
            with self.assertRaises(SimulatorStepError) as caught:
                play_combats(roots, None, max_decisions=10, rng=random.Random(0), numeric=True)
            self.assertEqual(steps.call_count, 1)  # No retry of the accepted prefix.
            self.assertEqual(caught.exception.root_indices, [0, 1])
            self.assertEqual([len(p) for p in caught.exception.action_prefixes], [1, 1])
        self.assertEqual(advanced, [True])
        with patch.object(State, "step", side_effect=ValueError("choice is invalid")) as steps:
            with self.assertRaises(SimulatorStepError) as caught:
                play_combats(roots, None, max_decisions=10, rng=random.Random(0))
            self.assertEqual(steps.call_count, 1)
            self.assertEqual(caught.exception.root_indices, [0])
            self.assertEqual(len(caught.exception.action_prefixes[0]), 1)
        # Native Action objects aren't value-comparable; compare typed observations/revisions.
        for root, old in zip(roots, before, strict=True):
            self.assertEqual(root.decision().observation, old.observation)
            self.assertEqual(root.revision, old.revision)

    def test_scary_durable_report_no_update_then_next_batch_trains(self) -> None:
        torch.set_num_threads(1)
        model = CombatModel()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        root = Root(first_combat("1000000", 0), "1000000", 1, 80)
        before = {key: value.clone() for key, value in model.state_dict().items()}
        failure = SimulatorStepError("choice is invalid", 2, [0], [[1, 0, 3]])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "errors" / "update-000001.json"
            with patch("train_roots.play_combats", side_effect=failure), self.assertLogs(level="CRITICAL") as logs:
                scores = train_batch([root], model, optimizer, 32, numeric=True, error_path=path, root_indices=[42])
            self.assertIn("ENTIRE TRAINING BATCH DISCARDED", logs.output[0])
            self.assertEqual(scores["optimizer_step"], 0)
            self.assertEqual(scores["simulator_error_batches"], 1)
            self.assertEqual(scores["discarded_episodes"], 1)
            for key in ("completed", "defeated", "truncated"):
                self.assertEqual(scores[key], 0)
            self.assertNotIn("mean_return_completed", scores)
            self.assertFalse(optimizer.state)
            for key, value in model.state_dict().items():
                torch.testing.assert_close(value, before[key], rtol=0, atol=0)
            report = json.loads(path.read_text())
            self.assertEqual(report["roots"][0]["dataset_root_index"], 42)
            self.assertEqual(report["attempted_prefixes"][0]["native_action_indices"], [1, 0, 3])
            self.assertTrue(report["batch_advancement_may_be_partial"])
            self.assertIn("SimulatorStepError", report["traceback"])
            episode = Episode(0.5, True, 40, 1, (next(model.parameters()).sum(),))
            with patch("train_roots.play_combats", return_value=[episode]):
                scores = train_batch([root], model, optimizer, 32, error_path=path)
            self.assertEqual(scores["optimizer_step"], 1)
            self.assertEqual(scores["simulator_error_batches"], 0)
            self.assertEqual(scores["discarded_episodes"], 0)
            self.assertTrue(optimizer.state)

    def test_other_failures_and_default_api_remain_fail_fast(self) -> None:
        model = CombatModel()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        root = Root(first_combat("1000000", 0), "1000000", 1, 80)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "must-not-exist.json"
            for error in (ValueError("invalid model logits"), RuntimeError("CUDA out of memory")):
                with patch("train_roots.play_combats", side_effect=error), self.assertRaises(type(error)):
                    train_batch([root], model, optimizer, 32, error_path=path)
                self.assertFalse(path.exists())
            with (
                patch("train_roots.play_combats", side_effect=SimulatorStepError("bad", 0, [0], [[0]])),
                self.assertRaises(SimulatorStepError),
            ):
                train_batch([root], model, optimizer, 32)
