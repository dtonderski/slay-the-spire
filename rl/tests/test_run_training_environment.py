"""Environmental inputs are experiment metadata, never actor features or repairs."""

import gzip
import io
import json
import tempfile
import unittest
from dataclasses import asdict, replace
from pathlib import Path
from typing import cast
from unittest.mock import Mock, patch

import torch
from run_training.collector import MacroStep, RunEpisode, collect
from run_training.contracts import PolicyAction
from run_training.environment import (
    ENVIRONMENT_PROTOCOL,
    episode_environment_seed,
    journal_environment_seed,
)
from run_training.model import MacroModel, encode
from run_training.roots import natural_state_from_setup
from run_training.trainer import main
from sts_sim import State

FIXTURE = Path(__file__).parent / "fixtures/run-boundary/courier-strict-failure.jsonl.gz"


class TrainingEnvironmentTests(unittest.TestCase):
    def test_allocation_is_stable_separate_and_unsigned(self):
        values = {
            episode_environment_seed(master, str(seed)) for master in (0, 2**63, 2**64 - 1) for seed in range(100)
        }
        self.assertEqual(len(values), 300)
        self.assertTrue(any(value >= 2**63 for value in values))
        self.assertEqual(episode_environment_seed(0, "1"), 2563137854517948835)
        for master in (-1, 2**64, True, 1.0):
            with self.assertRaises(ValueError):
                episode_environment_seed(cast(int, master), "1")
        for seed in ("01", "-1", str(2**63), "", "HUMAN1"):
            with self.assertRaises(ValueError):
                episode_environment_seed(0, seed)

    def test_legacy_and_invalid_journal_profiles(self):
        self.assertIsNone(journal_environment_seed({}))
        self.assertIsNone(journal_environment_seed({"training_rng_seed": None}))
        self.assertEqual(journal_environment_seed({"training_rng_seed": 0}), 0)
        for value in (-1, 2**64, True, "1", 1.0):
            with self.assertRaises(ValueError):
                journal_environment_seed({"training_rng_seed": value})

    def test_reconstruction_uses_declared_inputs_without_reallocating_or_drawing(self):
        setup = {"type": "setup", "seed": "123", "ascension": 0, "final_act": True}
        for seed in (None, 0, 2**64 - 1):
            with patch("run_training.roots.State.new") as factory:
                natural_state_from_setup({**setup, "training_rng_seed": seed})
                factory.assert_called_once_with("123", ascension=0, final_act=True, training_rng_seed=seed)
        with patch("run_training.roots.State.new") as factory:
            with self.assertRaises(ValueError):
                natural_state_from_setup({**setup, "training_rng_seed": True})
            factory.assert_not_called()

    def test_private_profile_does_not_change_initial_policy_inputs(self):
        decisions = [State.new("123", training_rng_seed=value).decision() for value in (None, 0, 2**64 - 1)]
        public = [encode(d.observation, tuple(PolicyAction.from_action(a) for a in d.actions)) for d in decisions]
        self.assertEqual(public[0], public[1])
        self.assertEqual(public[0], public[2])

    def test_collector_passes_and_journals_explicit_profile_only(self):
        initial = State.new("1").decision()
        terminal = replace(
            initial, observation=replace(initial.observation, context=replace(initial.observation.context, act=2))
        )
        model = MacroModel(8)
        for seed in (None, 0, 2**64 - 1):
            native = Mock()
            native.decision.return_value = initial
            native.step.return_value = terminal
            factory = Mock(return_value=native)
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "episode.jsonl"
                collect(
                    "1",
                    model,
                    Mock(),
                    objective="act1",
                    final_act=True,
                    max_actions=1,
                    macro_rng=torch.Generator().manual_seed(1),
                    combat_rng=torch.Generator().manual_seed(2),
                    journal=path,
                    state_factory=factory,
                    training_rng_seed=seed,
                )
                expected = {} if seed is None else {"training_rng_seed": seed}
                factory.assert_called_once_with("1", ascension=0, final_act=True, **expected)
                self.assertEqual(json.loads(path.read_text().splitlines()[0])["training_rng_seed"], seed)

    def test_immutable_courier_prefix_strict_failure_and_explicit_input_success(self):
        with gzip.open(FIXTURE, "rt") as source:
            rows = [json.loads(line) for line in source]
        setup = rows[0]
        accepted = {r["step"] for r in rows if r["type"] == "accepted"}
        for seed in (None, 0, 2**64 - 1):
            state = State.new(setup["seed"], ascension=0, final_act=setup["final_act"], training_rng_seed=seed)
            decision = state.decision()
            for row in rows:
                if row["type"] != "attempt":
                    continue
                self.assertEqual(decision.revision, row["revision"])
                action = decision.actions[row["index"]]
                self.assertEqual(asdict(PolicyAction.from_action(action)), row["action"])
                if row["step"] in accepted:
                    decision = state.step(action)
                    continue
                self.assertEqual(action.kind, "buy_shop_card")
                self.assertIn("The Courier", [r.content_key for r in decision.observation.context.relics])
                if seed is None:
                    with self.assertRaisesRegex(ValueError, "choice is invalid"):
                        state.step(action)
                    self.assertEqual(state.revision, decision.revision)
                    self.assertEqual(state.observation(), decision.observation)
                else:
                    clone = state.clone()
                    successor = state.step(action)
                    copied = clone.step(clone.decision().actions[row["index"]])
                    self.assertEqual(successor.revision, decision.revision + 1)
                    self.assertEqual(successor.observation, copied.observation)
                    self.assertLess(successor.observation.context.gold, decision.observation.context.gold)
                break

    def test_cli_rejects_invalid_master_before_loading_or_creating_output(self):
        with tempfile.TemporaryDirectory() as directory:
            for value in (-1, 2**64):
                with (
                    patch("sys.stderr", new_callable=io.StringIO),
                    patch("run_training.trainer.FrozenCombat") as frozen,
                    self.assertRaises(SystemExit),
                ):
                    main(
                        [
                            "--run-id",
                            "invalid",
                            "--combat-checkpoint",
                            "unused.pt",
                            "--validation-seeds",
                            "unused.json",
                            "--output-root",
                            directory,
                            "--training-environment-seed",
                            str(value),
                        ]
                    )
                frozen.assert_not_called()
                self.assertFalse((Path(directory) / "invalid").exists())

    def test_cli_rejects_environment_override_for_root_curricula(self):
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch("sys.stderr", new_callable=io.StringIO) as stderr,
                patch("run_training.trainer.FrozenCombat") as frozen,
                self.assertRaises(SystemExit),
            ):
                main([
                    "--run-id", "invalid-root-override",
                    "--combat-checkpoint", "unused.pt",
                    "--validation-seeds", "unused.json",
                    "--root-manifest", "unused-root.json",
                    "--training-environment-seed", "0",
                    "--output-root", directory,
                ])
            self.assertIn("Root curricula inherit journaled environmental inputs", stderr.getvalue())
            frozen.assert_not_called()
            self.assertFalse((Path(directory) / "invalid-root-override").exists())

    def test_cli_profile_resume_and_fixed_validation_allocation(self):
        calls = []
        decision = State.new("1").decision()
        public = encode(decision.observation, tuple(PolicyAction.from_action(a) for a in decision.actions))

        def episode(seed, model, combat, **options):
            calls.append((seed, options["training_rng_seed"]))
            with torch.no_grad():
                logits, value = model(public)
            clear = int(seed) % 2 == 0
            return RunEpisode(
                "act1_clear" if clear else "death",
                6.0 if clear else 1.0,
                (MacroStep(public, 0, tuple(logits.tolist()), float(value)),),
                1,
                18 if clear else 16,
                furthest_act1_floor=16,
            )

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "seeds.json").write_text('["100", "101"]')
            (root / "combat.pt").write_bytes(b"mocked trusted loader")
            common = [
                "--collect-only",
                "--collection-width",
                "1",
                "--combat-checkpoint",
                str(root / "combat.pt"),
                "--validation-seeds",
                str(root / "seeds.json"),
                "--output-root",
                directory,
                "--model-width",
                "8",
                "--batch-size",
                "2",
                "--training-environment-seed",
                str(2**64 - 1),
            ]
            with (
                patch("run_training.trainer.FrozenCombat", return_value=Mock()),
                patch("run_training.trainer.update", side_effect=AssertionError("No learning in this test")),
                patch("run_training.trainer.collect", side_effect=episode),
            ):
                main([*common, "--run-id", "whole", "--updates", "2"])
                main([*common, "--run-id", "first", "--updates", "1"])
                main([*common, "--run-id", "resumed", "--updates", "1", "--resume-from", str(root / "first/latest.pt")])
                with self.assertRaisesRegex(ValueError, "training_environment_seed"):
                    main(
                        [
                            *common,
                            "--run-id",
                            "changed",
                            "--training-environment-seed",
                            "0",
                            "--resume-from",
                            str(root / "first/latest.pt"),
                        ]
                    )
            whole = torch.load(root / "whole/latest.pt", weights_only=True)
            resumed = torch.load(root / "resumed/latest.pt", weights_only=True)
            self.assertEqual(whole["sampling_rng"], resumed["sampling_rng"])
            self.assertEqual(whole["optimizer_updates"], 0)
            self.assertEqual(whole["config"]["environment_protocol"], ENVIRONMENT_PROTOCOL)
            for key, value in whole["model"].items():
                torch.testing.assert_close(value, resumed["model"][key], rtol=0, atol=0)
            self.assertTrue(calls)
            for seed, allocated in calls:
                self.assertEqual(allocated, episode_environment_seed(2**64 - 1, seed))
            for seed in ("100", "101"):
                self.assertGreater(sum(s == seed for s, _ in calls), 1)
                self.assertEqual(len({allocated for s, allocated in calls if s == seed}), 1)
            self.assertFalse((root / "changed").exists())

    def test_environment_protocol_identifies_derivation_not_exact_game_global_rng(self):
        self.assertEqual(ENVIRONMENT_PROTOCOL, "libgdx_training_environment_sha256_episode_v1")


if __name__ == "__main__":
    unittest.main()
