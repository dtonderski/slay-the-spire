"""Training infrastructure tests; scripted boundaries are not gameplay parity evidence."""

import gzip
import io
import json
import os
import signal
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from typing import Any, cast
from unittest.mock import Mock, patch

import torch
from run_training.collector import (
    CollectionFailure,
    FrozenCombat,
    MacroStep,
    RunEpisode,
    SimulatorFailure,
    collect,
    sample,
    task_result,
)
from run_training.contracts import PolicyAction
from run_training.metrics import BehaviorStats
from run_training.model import (
    FEATURE_VERSION,
    HEALTH_FEATURE_VERSION,
    HealthMacroModel,
    MacroModel,
    encode,
)
from run_training.trainer import (
    graceful_stop,
    load_warm_start,
    main,
    update,
    validation_seeds,
)
from sts_sim import State


def macro_decision():
    state = State.new("1")
    return state.step(state.decision().actions[0])


def inputs():
    decision = macro_decision()
    return encode(
        decision.observation,
        tuple(PolicyAction.from_action(a) for a in decision.actions),
    )


def learned_episode(model, reward=6.0):
    public = inputs()
    with torch.no_grad():
        logits, value = model(public)
    return RunEpisode(
        "act1_clear" if reward == 6.0 else "death",
        reward,
        (MacroStep(public, 0, tuple(logits.tolist()), float(value)),),
        1,
        18 if reward == 6.0 else int(reward * 16),
        furthest_act1_floor=16 if reward == 6.0 else int(reward * 16),
    )


class RunTrainerTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        torch.manual_seed(12)
        self.model = MacroModel(8)

    def test_cli_routes_without_combat_required_arguments(self):
        import train

        with (
            patch("run_training.trainer.main") as run,
            patch("train.combat_main") as combat,
        ):
            train.main(["--task", "run", "--run-id", "test"])
            run.assert_called_once_with(["--run-id", "test"])
            combat.assert_not_called()
        with patch("train.combat_main") as combat:
            train.main(["--run-id", "old-combat"])
            combat.assert_called_once_with(["--run-id", "old-combat"])

    def test_encoder_does_not_accept_decision_or_native_action(self):
        decision = macro_decision()
        with self.assertRaises(TypeError):
            encode(decision, ())
        with self.assertRaises(TypeError):
            encode(decision.observation, decision.actions)
        changed = replace(decision, revision=99999)
        self.assertEqual(
            encode(
                decision.observation,
                tuple(PolicyAction.from_action(a) for a in decision.actions),
            ),
            encode(
                changed.observation,
                tuple(PolicyAction.from_action(a) for a in changed.actions),
            ),
        )

    def test_logits_cover_all_candidates_and_public_changes_matter(self):
        decision = macro_decision()
        descriptors = tuple(PolicyAction.from_action(a) for a in decision.actions)
        original = encode(decision.observation, descriptors)
        changed = replace(
            decision.observation,
            context=replace(decision.observation.context, gold=777),
        )
        self.assertNotEqual(original, encode(changed, descriptors))
        logits, value = self.model(original)
        self.assertEqual(logits.shape, (len(decision.actions),))
        self.assertEqual(value.shape, ())
        reversed_logits, _ = self.model(
            replace(original, candidates=original.candidates[::-1])
        )
        torch.testing.assert_close(reversed_logits, logits.flip(0))
        with self.assertRaises(ValueError):
            sample(torch.tensor([float("nan")]), torch.Generator())

    def test_cutoff_and_unknown_results_are_not_death(self):
        optimizer = torch.optim.Adam(self.model.parameters())
        episode = learned_episode(self.model)
        before = {k: v.clone() for k, v in self.model.state_dict().items()}
        with self.assertRaises(CollectionFailure):
            update(
                [episode, replace(episode, status="cutoff", reward=None)],
                self.model,
                optimizer,
                entropy_coef=0.0,
                value_coef=0.1,
            )
        for key, value in before.items():
            torch.testing.assert_close(
                value, self.model.state_dict()[key], rtol=0, atol=0
            )
        decision = macro_decision()
        bad = replace(
            decision,
            observation=replace(
                decision.observation,
                context=replace(
                    decision.observation.context, outcome="unknown_complete"
                ),
            ),
        )
        with self.assertRaises(CollectionFailure):
            task_result(bad, "act1")

    def test_update_recomputation_and_parameter_change(self):
        optimizer = torch.optim.Adam(self.model.parameters(), lr=0.01)
        episode = learned_episode(self.model)
        before = self.model.value.weight.clone()
        bad_step = replace(episode.steps[0], value=100.0)
        with self.assertRaisesRegex(ValueError, "recomputation"):
            update(
                [replace(episode, steps=(bad_step,))],
                self.model,
                optimizer,
                entropy_coef=0,
                value_coef=1,
            )
        self.assertFalse(optimizer.state)
        result = update(
            [episode], self.model, optimizer, entropy_coef=0.01, value_coef=1
        )
        self.assertEqual(result["optimizer_step"], 1)
        self.assertFalse(torch.equal(before, self.model.value.weight))

    def test_collector_terminal_cutoff_and_failure_journals(self):
        initial = macro_decision()
        terminal = replace(
            initial,
            observation=replace(
                initial.observation, context=replace(initial.observation.context, act=2)
            ),
        )

        class ScriptedState:
            def decision(self):
                return initial

            def step(self, action):
                return terminal

        # Dynamic doubles intentionally implement only the exercised boundary.
        forbidden_combat: Any = Mock(spec=FrozenCombat)
        forbidden_combat.choose.side_effect = AssertionError(
            "noncombat must not use the combat model"
        )

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "episode.jsonl"
            episode = collect(
                "1",
                self.model,
                forbidden_combat,
                objective="act1",
                final_act=True,
                max_actions=2,
                macro_rng=torch.Generator().manual_seed(1),
                combat_rng=torch.Generator().manual_seed(2),
                journal=path,
                state_factory=lambda *a, **kw: cast(Any, ScriptedState()),
            )
            self.assertEqual(episode.status, "act1_clear")
            self.assertEqual(episode.reward, 6)
            self.assertEqual(len(episode.steps), 1)
            rows = [json.loads(line) for line in path.read_text().splitlines()]
            self.assertEqual(
                [row["type"] for row in rows],
                ["setup", "attempt", "accepted", "result"],
            )
            with patch.object(ScriptedState, "step", return_value=initial):
                cutoff = collect(
                    "1",
                    self.model,
                    forbidden_combat,
                    objective="act1",
                    final_act=True,
                    max_actions=1,
                    macro_rng=torch.Generator(),
                    combat_rng=torch.Generator(),
                    journal=Path(directory) / "cutoff.jsonl",
                    state_factory=lambda *a, **kw: cast(Any, ScriptedState()),
                )
                self.assertEqual(cutoff.status, "cutoff")
                self.assertIsNone(cutoff.reward)
            with patch.object(
                ScriptedState, "step", side_effect=ValueError("unsupported")
            ) as step:
                error_path = Path(directory) / "error.jsonl"
                with self.assertRaisesRegex(CollectionFailure, "unsupported"):
                    collect(
                        "1",
                        self.model,
                        forbidden_combat,
                        objective="act1",
                        final_act=True,
                        max_actions=2,
                        macro_rng=torch.Generator(),
                        combat_rng=torch.Generator(),
                        journal=error_path,
                        state_factory=lambda *a, **kw: cast(Any, ScriptedState()),
                    )
                self.assertEqual(
                    step.call_count, 1, "never retry a different candidate"
                )
                self.assertEqual(
                    json.loads(error_path.read_text().splitlines()[-1])["accepted"], 0
                )

    def test_combat_scorer_keeps_escape_and_does_not_receive_revision(self):
        spec = {
            "seed": 1,
            "floor": 1,
            "kind": "normal",
            "encounter": "Cultist",
            "deck": [{"key": "Strike_R", "upgrades": 0}],
            "relics": [],
            "potions": ["smoke_bomb", None, None],
            "hp": 80,
            "max_hp": 80,
            "gold": 99,
        }
        state = State.from_synthetic_spec(json.dumps(spec))
        decision = state.decision()
        smoke = next(
            i
            for i, a in enumerate(decision.actions)
            if a.kind == "use_potion_slot" and a.potion_slot == 0
        )
        calls = []

        def score(batch, candidates):
            calls.append(candidates)
            logits = torch.full((1, len(candidates)), -1000.0)
            logits[0, smoke] = 0
            return logits, torch.zeros(1), torch.ones_like(logits, dtype=torch.bool)

        frozen: Any = FrozenCombat.__new__(FrozenCombat)
        frozen.model = score
        choice = frozen.choose(state, decision, torch.Generator())
        self.assertEqual(choice, smoke)
        self.assertEqual(calls[0].shape, (len(decision.actions), 6))
        self.assertEqual(decision.actions[choice].family, "run")
        # Screen routing must still invoke combat for that family='run' candidate.
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(frozen, "choose", wraps=frozen.choose) as choose,
        ):
            # This isolated synthetic combat has no successor map after escape;
            # the collector must reject that boundary, not fabricate a win/loss.
            with self.assertRaises(CollectionFailure):
                collect(
                    "1",
                    self.model,
                    frozen,
                    objective="act1",
                    final_act=False,
                    max_actions=1,
                    macro_rng=torch.Generator(),
                    combat_rng=torch.Generator(),
                    journal=Path(directory) / "combat.jsonl",
                    state_factory=lambda *a, **kw: state,
                )
            choose.assert_called_once()

    def test_uniform_initialization_and_action_linked_features(self):
        public = inputs()
        logits, value = self.model(public)
        torch.testing.assert_close(logits, torch.zeros_like(logits), atol=0, rtol=0)
        self.assertEqual(float(value.detach()), 0.0)
        decision = macro_decision()
        descriptors = tuple(PolicyAction.from_action(a) for a in decision.actions)
        original = encode(decision.observation, descriptors)
        choices = decision.observation.screen.choices
        swapped_screen = replace(decision.observation.screen, choices=choices[::-1])
        swapped = encode(
            replace(decision.observation, screen=swapped_screen), descriptors
        )
        self.assertNotEqual(original.candidates[0], swapped.candidates[0])

    def test_deadline_cutoff_and_compressed_journal(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "deadline.jsonl.gz"
            result = collect(
                "1",
                self.model,
                cast(Any, None),  # Immediate cutoff must not access combat.
                objective="act1",
                final_act=True,
                max_actions=100,
                macro_rng=torch.Generator(),
                combat_rng=torch.Generator(),
                journal=path,
                stop_requested=lambda: True,
            )
            self.assertEqual(result.status, "cutoff")
            self.assertEqual(result.accepted, 0)
            self.assertIsNone(result.reward)
            with gzip.open(path, "rt") as source:
                rows = [json.loads(line) for line in source]
            self.assertEqual([r["type"] for r in rows], ["setup", "result"])

    def test_signal_requests_cooperative_stop_and_restores_handler(self):
        previous = signal.getsignal(signal.SIGTERM)
        with graceful_stop() as flag:
            self.assertFalse(flag.is_set())
            os.kill(os.getpid(), signal.SIGTERM)
            self.assertTrue(flag.is_set())
        self.assertEqual(signal.getsignal(signal.SIGTERM), previous)

    def test_experimental_continuation_quarantines_whole_batch_only(self):
        training_calls = []

        def fake_collect(seed, macro, combat, **kwargs):
            if seed not in ("100", "101"):
                training_calls.append(seed)
                if len(training_calls) == 1:
                    raise SimulatorFailure("unsupported advertised transition")
            return learned_episode(macro, 6.0)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            seeds = root / "seeds.json"
            seeds.write_text('["100", "101"]')
            combat = root / "combat.pt"
            combat.write_bytes(b"mocked loader")
            args = [
                "--run-id",
                "quarantine",
                "--collection-width",
                "1",  # Mocked serial collector; batching has separate tests.
                "--combat-checkpoint",
                str(combat),
                "--validation-seeds",
                str(seeds),
                "--output-root",
                directory,
                "--batch-size",
                "2",
                "--updates",
                "2",
                "--model-width",
                "8",
                "--continue-on-collection-failure",
            ]
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch("run_training.trainer.collect", side_effect=fake_collect),
            ):
                main(args)
            self.assertEqual(len(training_calls), 4)
            self.assertEqual(len(set(training_calls)), 4, "no failed-seed retry")
            checkpoint = torch.load(root / "quarantine/latest.pt", weights_only=True)
            self.assertEqual(checkpoint["optimizer_updates"], 1)
            self.assertEqual(checkpoint["skipped_batches"], 1)
            self.assertTrue((root / "quarantine/collection-errors.jsonl").exists())
            # A model/infrastructure exception must still stop even in continuation mode.
            args[1] = "bad-model"
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch(
                    "run_training.trainer.collect", side_effect=ValueError("bad model")
                ),
                self.assertRaisesRegex(ValueError, "bad model"),
            ):
                main(args)
            self.assertTrue((root / "bad-model/failure.json").exists())
            self.assertFalse((root / "bad-model/collection-errors.jsonl").exists())

    def test_wandb_iteration_axis_and_failed_prefix_metrics(self):
        def fake_collect(seed, macro, combat, **kwargs):
            if seed not in ("100", "101"):
                error = SimulatorFailure("unsupported")
                error.behavior = BehaviorStats(
                    map_nodes_visited=2, elite_nodes_visited=1
                )
                error.accepted = 2
                error.floor = error.furthest_act1_floor = 4
                raise error
            return learned_episode(macro)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            seeds = root / "seeds.json"
            seeds.write_text('["100", "101"]')
            combat = root / "combat.pt"
            combat.write_bytes(b"mocked loader")
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch("run_training.trainer.collect", side_effect=fake_collect),
                patch("run_training.trainer.wandb.init") as init,
            ):
                run = init.return_value.__enter__.return_value
                run.url = "https://example.invalid/test-run"
                main(
                    [
                        "--run-id",
                        "metrics",
                        "--collection-width",
                        "1",
                        "--combat-checkpoint",
                        str(combat),
                        "--validation-seeds",
                        str(seeds),
                        "--output-root",
                        directory,
                        "--batch-size",
                        "1",
                        "--updates",
                        "1",
                        "--model-width",
                        "8",
                        "--continue-on-collection-failure",
                    ]
                )
                run.define_metric.assert_any_call("train/*", step_metric="iteration")
                run.define_metric.assert_any_call(
                    "validation/*", step_metric="iteration"
                )
                rows = [call.args[0] for call in run.log.call_args_list]
                self.assertEqual([row["iteration"] for row in rows], [0, 1, 1])
                self.assertTrue(
                    all(
                        call.kwargs == {"commit": True}
                        for call in run.log.call_args_list
                    )
                )
                train = rows[1]
                self.assertEqual(train["train/errors"], 1)
                self.assertEqual(train["train/map/elite_percent"], 50)
                self.assertEqual(train["train/map/nodes_visited"], 2)
                self.assertEqual(train["train/reward/terminal_samples"], 0)
                self.assertNotIn("train/reward/mean", train)
                self.assertEqual(
                    json.loads((root / "metrics/tracking.json").read_text())["url"],
                    run.url,
                )

    def test_explicit_warm_start_transfers_weights_across_native_versions(self):
        config = {
            key: "same"
            for key in (
                "protocol",
                "feature_version",
                "reward_protocol",
                "model_width",
                "objective",
                "observation_schema",
                "final_act",
                "ascension",
                "gamma",
                "combat_sha256",
            )
        }
        config.update(native_sha256="old-native", source_sha256="old-source")
        with torch.no_grad():
            self.model.value.bias.fill_(2.5)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "parent.pt"
            torch.save(
                {
                    "model": self.model.state_dict(),
                    "config": config,
                    "iteration": 9,
                    "optimizer_updates": 8,
                    "optimizer": {"not_transferred": True},
                },
                path,
            )
            target = MacroModel(8)
            metadata = load_warm_start(
                target,
                path,
                {
                    **config,
                    "native_sha256": "new-native",
                    "source_sha256": "new-source",
                },
            )
            self.assertEqual(metadata["parent_optimizer_updates"], 8)
            for key, value in self.model.state_dict().items():
                torch.testing.assert_close(
                    value, target.state_dict()[key], atol=0, rtol=0
                )
            for key in (
                "reward_protocol",
                "feature_version",
                "combat_sha256",
                "objective",
            ):
                with self.assertRaisesRegex(ValueError, key):
                    load_warm_start(target, path, {**config, key: "changed"})

    def test_warm_start_legacy_hashed_and_explicit_prereview_versions(self):
        self.assertEqual(FEATURE_VERSION, 2)
        self.assertEqual(HEALTH_FEATURE_VERSION, 3)
        config = {
            "protocol": "same",
            "feature_version": 2,
            "model_width": 8,
            "reward_protocol": "same",
            "objective": "act1",
            "observation_schema": 8,
            "final_act": True,
            "ascension": 0,
            "gamma": 1.0,
            "combat_sha256": "frozen",
            "native_sha256": "old-native",
        }
        settings = {**config, "encoder": "hashed", "native_sha256": "new-native"}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "parent.pt"

            def save(source_config, model):
                torch.save(
                    {
                        "config": source_config,
                        "model": model.state_dict(),
                        "iteration": 9,
                        "optimizer_updates": 8,
                    },
                    path,
                )

            for parent in (
                config,
                {**config, "encoder": "hashed", "feature_version": 3},
            ):
                save(parent, self.model)
                target = MacroModel(8)
                metadata = load_warm_start(target, path, settings)
                self.assertEqual(metadata["parent_encoder"], "hashed")
                self.assertEqual(
                    metadata["parent_feature_version"], parent["feature_version"]
                )
                for key, value in self.model.state_dict().items():
                    torch.testing.assert_close(
                        value, target.state_dict()[key], atol=0, rtol=0
                    )
            for parent in (
                {**config, "feature_version": 3},  # No known encoder-v3 producer.
                {**config, "encoder": "hashed", "feature_version": 4},
                {**config, "encoder": "health", "feature_version": 3},
                {**config, "encoder": None},
            ):
                save(parent, self.model)
                with self.assertRaisesRegex(ValueError, "feature_version|encoder"):
                    load_warm_start(MacroModel(8), path, settings)
            health = HealthMacroModel(8)
            health_config = {**config, "encoder": "health", "feature_version": 3}
            save(health_config, health)
            load_warm_start(HealthMacroModel(8), path, health_config)
            save({**health_config, "feature_version": 2}, health)
            with self.assertRaisesRegex(ValueError, "feature_version"):
                load_warm_start(HealthMacroModel(8), path, health_config)

    def test_root_hp_metric_names_must_be_unique(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "seeds.json").write_text('["100"]')
            for fractions in ((".15", ".15"), (".149", ".151")):
                with (
                    patch("sys.stderr", new_callable=io.StringIO) as stderr,
                    patch("run_training.trainer.FrozenCombat") as frozen,
                    self.assertRaises(SystemExit) as error,
                ):
                    main(
                        [
                            "--run-id",
                            "collision",
                            "--output-root",
                            directory,
                            "--combat-checkpoint",
                            "unused.pt",
                            "--validation-seeds",
                            str(path / "seeds.json"),
                            "--root-eval-hp",
                            *fractions,
                        ]
                    )
                self.assertEqual(error.exception.code, 2)
                self.assertIn(
                    "distinct rounded-percent metric names", stderr.getvalue()
                )
                frozen.assert_not_called()
                self.assertFalse((path / "collision").exists())

    def test_abort_logs_native_collection_failure_in_both_execution_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "seeds.json").write_text('["100", "101"]')
            (root / "combat.pt").write_bytes(b"mocked loader")
            for width in (1, 2):
                run_id = f"abort-{width}"
                with (
                    patch("run_training.trainer.FrozenCombat"),
                    patch(
                        "run_training.trainer.State.new",
                        side_effect=ValueError("injected native failure"),
                    ) as native,
                    patch("run_training.trainer.wandb.init") as init,
                ):
                    init.return_value.__enter__.return_value.url = None
                    with self.assertRaisesRegex(
                        CollectionFailure, "injected native failure"
                    ):
                        main(
                            [
                                "--run-id",
                                run_id,
                                "--output-root",
                                directory,
                                "--combat-checkpoint",
                                str(root / "combat.pt"),
                                "--validation-seeds",
                                str(root / "seeds.json"),
                                "--collection-width",
                                str(width),
                                "--model-width",
                                "8",
                                "--updates",
                                "1",
                                "--wandb-mode",
                                "disabled",
                            ]
                        )
                    native.assert_called_once()
                output = root / run_id
                rows = [
                    json.loads(s)
                    for s in (output / "collection-errors.jsonl")
                    .read_text()
                    .splitlines()
                ]
                self.assertEqual(len(rows), 1)
                self.assertEqual(rows[0]["seed"], "100")
                self.assertIn("injected native failure", rows[0]["error"])
                journal = [
                    json.loads(s)
                    for s in Path(rows[0]["journal"]).read_text().splitlines()
                ]
                self.assertEqual(journal[-1]["type"], "error")
                self.assertTrue((output / "failure.json").exists())
                checkpoint = torch.load(output / "latest.pt", weights_only=True)
                self.assertEqual(checkpoint["optimizer_updates"], 0)
                self.assertEqual(checkpoint["config"]["feature_version"], 2)

    def test_checkpoint_commits_before_fallible_telemetry(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "seeds.json").write_text('["100"]')
            (root / "combat.pt").write_bytes(b"mocked")
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch(
                    "run_training.trainer.collect",
                    side_effect=lambda seed, model, combat, **kwargs: learned_episode(
                        model
                    ),
                ),
                patch("run_training.trainer.wandb.init") as init,
            ):
                run = init.return_value.__enter__.return_value
                run.url = None

                def publish(row, **kwargs):
                    if "train/attempts" in row:
                        raise RuntimeError("telemetry disconnected")

                run.log.side_effect = publish
                with self.assertRaisesRegex(RuntimeError, "telemetry disconnected"):
                    main(
                        [
                            "--run-id",
                            "telemetry",
                            "--collection-width",
                            "1",
                            "--combat-checkpoint",
                            str(root / "combat.pt"),
                            "--validation-seeds",
                            str(root / "seeds.json"),
                            "--output-root",
                            directory,
                            "--updates",
                            "1",
                            "--batch-size",
                            "1",
                            "--model-width",
                            "8",
                        ]
                    )
            checkpoint = torch.load(root / "telemetry/latest.pt", weights_only=True)
            failure = json.loads((root / "telemetry/failure.json").read_text())
            self.assertEqual(checkpoint["iteration"], 1)
            self.assertEqual(checkpoint["optimizer_updates"], 1)
            self.assertEqual(
                failure["last_committed_iteration"], checkpoint["iteration"]
            )

    def test_seed_validation_rejects_aliases(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "seeds.json"
            for invalid in (["1", "01"], [1], ["HUMAN1"], [], ["-1"]):
                path.write_text(json.dumps(invalid))
                with self.assertRaises(ValueError):
                    validation_seeds(path)
            path.write_text('["01", "2"]')
            self.assertEqual(validation_seeds(path), ["1", "2"])

    def test_checkpoint_resume_matches_uninterrupted_updates(self):
        def fake_collect(seed, macro, combat, **kwargs):
            return learned_episode(macro, 6.0 * (int(seed) % 2))

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            seeds = root / "seeds.json"
            seeds.write_text('["100", "101"]')
            combat = root / "combat.pt"
            combat.write_bytes(b"test hash only; combat loader is mocked")
            common = [
                "--collection-width",
                "1",
                "--combat-checkpoint",
                str(combat),
                "--validation-seeds",
                str(seeds),
                "--output-root",
                directory,
                "--model-width",
                "8",
                "--batch-size",
                "2",
            ]
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch("run_training.trainer.collect", side_effect=fake_collect),
            ):
                main([*common, "--run-id", "whole", "--updates", "2"])
                main([*common, "--run-id", "first", "--updates", "1"])
                main(
                    [
                        *common,
                        "--run-id",
                        "resumed",
                        "--updates",
                        "1",
                        "--resume-from",
                        str(root / "first/latest.pt"),
                    ]
                )
            whole = torch.load(root / "whole/latest.pt", weights_only=True)
            resumed = torch.load(root / "resumed/latest.pt", weights_only=True)
            self.assertEqual(whole["iteration"], resumed["iteration"])
            self.assertEqual(whole["sampling_rng"], resumed["sampling_rng"])
            for key, value in whole["model"].items():
                torch.testing.assert_close(value, resumed["model"][key], rtol=0, atol=0)


if __name__ == "__main__":
    unittest.main()
