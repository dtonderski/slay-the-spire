"""Reward/telemetry infrastructure tests, not real-game parity evidence."""

import contextlib
import copy
import io
import json
import math
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from typing import Any, cast
from unittest.mock import patch

import torch
from run_training.collector import (
    CollectionFailure,
    CollectionOptions,
    MacroRequest,
    MacroStep,
    RunEpisode,
    _predict_one,
    collect,
    failed_episode,
)
from run_training.contracts import PolicyAction
from run_training.diagnostics import decision_diagnostic, policy_scores, probabilities
from run_training.metrics import BehaviorStats, behavior_scores
from run_training.model import (
    FEATURE_VERSION,
    HEALTH_FEATURE_VERSION,
    HealthMacroModel,
    MacroModel,
    encode,
)
from run_training.progress import RunProgress
from run_training.rewards import (
    HEART_REWARD_PROTOCOL,
    REWARD_PROTOCOL,
    reward_protocol,
    terminal_parts,
)
from run_training.trainer import (
    PROTOCOL,
    load_warm_start,
    main,
    scores,
    update,
    validation_selection_key,
)
from sts_sim import CompleteObservation, MonsterKey, State


def public_decision():
    state = State.new("1", final_act=True)
    return state.step(state.decision().actions[0])


def changed(decision, act, floor, status="ongoing", keys=False):
    context = replace(
        decision.observation.context,
        act=act,
        floor=floor,
        outcome=status,
        keys=replace(
            decision.observation.context.keys, ruby=keys, emerald=keys, sapphire=keys
        ),
    )
    if status == "ongoing":
        observation = replace(decision.observation, context=context)
    else:
        observation = CompleteObservation(
            schema_version=8,
            phase="complete",
            kind="complete",
            context=context,
            screen=None,
        )
    return replace(decision, observation=observation)


class HeartInfraTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        torch.manual_seed(7)
        self.model = MacroModel(8)
        self.decision = public_decision()
        self.public = encode(
            self.decision.observation,
            tuple(PolicyAction.from_action(a) for a in self.decision.actions),
        )

    def test_shaped_reward_cliffs_and_heart_dominance(self):
        for floor, cleared in (
            (0, 0),
            (15, 0),
            (17, 1),
            (34, 2),
            (51, 3),
            (55, 3),
            (56, 3),
        ):
            progress = RunProgress(floor, min(4, cleared + 1), cleared)
            parts = terminal_parts("heart_progress", "death", 16, progress=progress)
            self.assertEqual(parts.total, 3 * min(floor / 55, 1) + 2 * cleared)
            self.assertLessEqual(parts.total, 9)
            self.assertEqual(parts.heart_bonus, 0)
        win = RunProgress(56, 4, 3, True, True, True, True)
        self.assertEqual(
            terminal_parts("heart_progress", "heart_clear", 16, progress=win).total, 39
        )
        self.assertLess(
            terminal_parts(
                "heart_progress", "death", 16, progress=RunProgress(16)
            ).total,
            terminal_parts(
                "heart_progress", "death", 16, progress=RunProgress(17, 2, 1)
            ).total,
        )
        self.assertEqual(reward_protocol("heart_progress"), HEART_REWARD_PROTOCOL)
        self.assertEqual(reward_protocol("heart"), REWARD_PROTOCOL)
        self.assertEqual(terminal_parts("heart", "heart_clear", 16).total, 1)
        for status in ("ongoing", "cutoff", "error", "act1_clear"):
            with self.assertRaises(ValueError):
                terminal_parts("heart_progress", status, 16, progress=win)
        with self.assertRaises(ValueError):
            terminal_parts("heart_progress", "death", 16)
        with self.assertRaises(ValueError):
            terminal_parts(
                "heart_progress", "heart_clear", 16, progress=RunProgress(55, 4, 3)
            )

    def test_public_milestones_monotone_and_not_floor_guesses(self):
        p = RunProgress().observed(changed(self.decision, 1, 55).observation)
        self.assertEqual(p.acts_cleared, 0)
        p = p.observed(changed(self.decision, 2, 17, keys=True).observation)
        p = p.observed(changed(self.decision, 2, 17).observation)
        self.assertEqual((p.furthest_floor, p.acts_cleared), (55, 1))
        self.assertTrue(p.ruby_seen and p.emerald_seen and p.sapphire_seen)
        p = p.observed(changed(self.decision, 3, 51, "act3_clear").observation)
        self.assertEqual(p.acts_cleared, 3)
        self.assertFalse(p.heart_reached)
        p = p.observed(changed(self.decision, 4, 56, "heart_clear").observation)
        self.assertTrue(p.heart_reached)
        for _ in range(10):
            self.assertEqual(
                p.observed(changed(self.decision, 4, 56, "heart_clear").observation), p
            )

    def test_heart_encounter_comes_from_public_monster_not_floor(self):
        state = State.from_synthetic_spec(
            json.dumps(
                {
                    "seed": 1,
                    "floor": 1,
                    "kind": "normal",
                    "encounter": "Cultist",
                    "deck": [{"key": "Strike_R", "upgrades": 0}],
                    "relics": [],
                    "potions": [None, None, None],
                    "hp": 80,
                    "max_hp": 80,
                    "gold": 99,
                }
            )
        )
        public = state.decision().observation
        assert public.kind == "combat"
        no_heart = replace(public, context=replace(public.context, act=4, floor=55))
        self.assertFalse(RunProgress().observed(no_heart).heart_reached)
        monsters = (
            replace(public.screen.monsters[0], content_key=MonsterKey.CORRUPT_HEART),
        )
        heart = replace(no_heart, screen=replace(public.screen, monsters=monsters))
        self.assertTrue(RunProgress().observed(heart).heart_reached)

    def test_constructor_failure_has_no_observed_progress_or_reward(self):
        def unavailable(*args, **kwargs):
            raise ValueError("unavailable fixture initialization")

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.jsonl"
            with self.assertRaises(CollectionFailure) as caught:
                collect(
                    "1",
                    self.model,
                    cast(Any, None),
                    objective="heart_progress",
                    final_act=True,
                    max_actions=10,
                    macro_rng=torch.Generator(),
                    combat_rng=torch.Generator(),
                    journal=path,
                    state_factory=unavailable,
                )
            episode = failed_episode(caught.exception, "heart_progress")
            self.assertIsNone(episode.progress)
            self.assertIsNone(episode.reward)
            self.assertIsNone(json.loads(path.read_text().splitlines()[-1])["progress"])

    def scripted(self, diagnostics=False, max_actions=10, fail=False):
        decisions = [
            changed(self.decision, 1, 15),
            changed(self.decision, 2, 17),
            changed(self.decision, 3, 34),
            changed(self.decision, 4, 52, keys=True),
            changed(self.decision, 4, 56, "heart_clear", keys=True),
        ]

        class Scripted:
            index = 0

            def decision(self):
                return decisions[self.index]

            def step(self, action):
                if fail and self.index == 1:
                    raise ValueError("unsupported fixture transition")
                self.index += 1
                return decisions[self.index]

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.jsonl"
            options: CollectionOptions = {
                "objective": "heart_progress",
                "final_act": True,
                "max_actions": max_actions,
                "macro_rng": torch.Generator().manual_seed(123),
                "combat_rng": torch.Generator(),
                "journal": path,
                "state_factory": lambda *a, **kw: cast(Any, Scripted()),
                "decision_diagnostics": diagnostics,
                "initial_metadata": {"protocol": "scripted_public_test_fixture"},
            }
            if fail:
                with self.assertRaises(CollectionFailure) as caught:
                    collect("1", self.model, cast(Any, None), **options)
                result = failed_episode(caught.exception, "heart_progress")
            else:
                result = collect("1", self.model, cast(Any, None), **options)
            rows = [json.loads(line) for line in path.read_text().splitlines()]
        return result, rows

    def test_collection_reward_unknowns_and_accepted_only_progress(self):
        episode, rows = self.scripted(diagnostics=True)
        self.assertEqual((episode.status, episode.reward), ("heart_clear", 39))
        self.assertEqual(
            rows[-1]["reward_parts"],
            {"progress": 3, "clear_bonus": 6, "heart_bonus": 30},
        )
        self.assertEqual(rows[0]["reward_protocol"], HEART_REWARD_PROTOCOL)
        cutoff, rows = self.scripted(max_actions=2)
        self.assertEqual(cutoff.status, "cutoff")
        self.assertIsNone(cutoff.reward)
        self.assertEqual(cutoff.progress.acts_cleared, 2)
        self.assertIsNone(rows[-1]["reward_parts"])
        error, rows = self.scripted(fail=True)
        self.assertEqual(error.status, "error")
        self.assertIsNone(error.reward)
        self.assertEqual(error.progress.acts_cleared, 1)
        self.assertEqual(error.progress.furthest_floor, 17)
        self.assertFalse(error.progress.heart_reached)

    def test_diagnostics_do_not_change_actions_logits_or_rng(self):
        before = torch.get_rng_state().clone()
        plain, a = self.scripted(False)
        audited, b = self.scripted(True)
        self.assertEqual(plain, audited)
        torch.testing.assert_close(before, torch.get_rng_state(), atol=0, rtol=0)
        attempts_a = [r for r in a if r["type"] == "attempt"]
        attempts_b = [r for r in b if r["type"] == "attempt"]
        for left, right in zip(attempts_a, attempts_b, strict=True):
            self.assertIsNone(left["policy_diagnostic"])
            audit = right["policy_diagnostic"]
            self.assertEqual(audit["choice"], right["index"])
            self.assertAlmostEqual(
                sum(c["probability"] for c in audit["candidates"]), 1
            )
            self.assertEqual(left["action"], right["action"])
            self.assertIn("deck", audit)
            self.assertNotIn("seed", audit)
        descriptors = tuple(PolicyAction.from_action(x) for x in self.decision.actions)
        n = len(descriptors)
        logits = (-math.inf,) + (0.0,) * (n - 1)
        audit = decision_diagnostic(
            self.decision.observation, descriptors, logits, 1, 0.0
        )
        self.assertFalse(audit["candidates"][0]["eligible"])
        self.assertIsNone(audit["candidates"][0]["effective_logit"])
        self.assertEqual(audit["candidates"][0]["probability"], 0)
        json.dumps(audit, allow_nan=False)
        for bad in ((math.nan,), (math.inf,), (-math.inf,)):
            with self.assertRaises(ValueError):
                probabilities(bad)
        self.assertAlmostEqual(
            policy_scores([((0.0, 0.0), 1)])["policy/entropy_mean"], math.log(2)
        )

    def test_coverage_metrics_do_not_turn_progress_into_wins(self):
        win, _ = self.scripted()
        error, _ = self.scripted(fail=True)
        cutoff, _ = self.scripted(max_actions=2)
        p = RunProgress(51, 3, 3)
        failure = RunEpisode(
            "act3_clear",
            terminal_parts("heart_progress", "act3_clear", 16, progress=p).total,
            (),
            0,
            51,
            objective="heart_progress",
            progress=p,
        )
        result = scores([win, failure, error, cutoff])
        self.assertEqual(result["outcome/heart_wins"], 1)
        self.assertEqual(result["outcome/heart_win_lower_bound"], 0.25)
        self.assertEqual(result["completed_success_rate"], 0.5)
        self.assertEqual(result["coverage/unknown_rate"], 0.5)
        self.assertEqual(result["progress/act_1_clears_observed"], 4)
        self.assertEqual(result["progress/act_3_clears_observed"], 2)
        self.assertNotIn("reward/mean", scores([error, cutoff]))

    def test_shop_reentry_counts_do_not_filter_actions(self):
        descriptor = replace(
            PolicyAction.from_action(self.decision.actions[0]), kind="enter_shop"
        )
        stats = BehaviorStats()
        for _ in range(4):
            stats = stats.accepted(self.decision.observation, descriptor, None)
        result = behavior_scores([stats])
        self.assertEqual(result["actions/enter_shop"], 4)
        self.assertEqual(result["navigation/shop_same_floor_reentries"], 3)

    def test_policy_only_transfer_preserves_actor_resets_critic(self):
        for cls in (MacroModel, HealthMacroModel):
            parent = cls(8)
            with torch.no_grad():
                for parameter in parent.parameters():
                    parameter.uniform_(0.1, 0.2)
            parent_config = {
                "protocol": PROTOCOL,
                "reward_protocol": REWARD_PROTOCOL,
                "model_width": 8,
                "objective": "act1",
                "observation_schema": 8,
                "final_act": True,
                "ascension": 0,
                "gamma": 1.0,
                "combat_sha256": "combat",
                "feature_version": HEALTH_FEATURE_VERSION
                if cls is HealthMacroModel
                else FEATURE_VERSION,
                "native_sha256": "native",
                "encoder": "health" if cls is HealthMacroModel else "hashed",
            }
            settings = parent_config | {
                "objective": "heart_progress",
                "reward_protocol": HEART_REWARD_PROTOCOL,
            }
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "parent.pt"
                torch.save(
                    {
                        "model": parent.state_dict(),
                        "config": parent_config,
                        "iteration": 9,
                        "optimizer_updates": 7,
                    },
                    path,
                )
                target = cls(8)
                original_critic = {
                    k: v.clone()
                    for k, v in target.state_dict().items()
                    if k.startswith(("value.", "health_value."))
                }
                with self.assertRaisesRegex(ValueError, "objective"):
                    load_warm_start(target, path, settings)
                with self.assertRaisesRegex(ValueError, "fresh zero-output"):
                    load_warm_start(
                        copy.deepcopy(parent), path, settings, policy_only=True
                    )
                info = load_warm_start(target, path, settings, policy_only=True)
                self.assertEqual(info["critic_initialization"], "fresh_zero_output")
                for key, value in target.state_dict().items():
                    torch.testing.assert_close(
                        value,
                        original_critic[key]
                        if key in original_critic
                        else parent.state_dict()[key],
                        atol=0,
                        rtol=0,
                    )
                self.assertEqual(target(self.public)[1].item(), 0.0)

    def test_heart_return_validation_and_serial_batched_learner(self):
        episode, _ = self.scripted()
        cases = [(0, "cpu"), (4, "cpu")]
        if torch.cuda.is_available():
            cases.append((4, "cuda"))
        for width, device in cases:
            model = copy.deepcopy(self.model).to(device)
            optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
            result = update(
                [episode],
                model,
                optimizer,
                entropy_coef=0.01,
                value_coef=0.1,
                batch_steps=width,
            )
            self.assertEqual(result["optimizer_step"], 1)
            self.assertTrue(all(torch.isfinite(p).all() for p in model.parameters()))
            with self.assertRaisesRegex(ValueError, "Terminal returns"):
                update(
                    [replace(episode, reward=38.0)],
                    model,
                    optimizer,
                    entropy_coef=0.01,
                    value_coef=0.1,
                )

    def test_best_selection_requires_coverage_and_prioritizes_success(self):
        base = {
            "validation/scheduled": 128.0,
            "validation/attempts": 128.0,
            "validation/unattempted": 0.0,
            "validation/errors": 0.0,
            "validation/cutoffs": 0.0,
            "validation/success_scheduled_lower_bound": 0.0,
            "validation/reward/mean": 9.0,
        }
        win = base | {
            "validation/success_scheduled_lower_bound": 1 / 128,
            "validation/reward/mean": 1.0,
        }
        win_key, base_key = (
            validation_selection_key(win),
            validation_selection_key(base),
        )
        assert win_key is not None and base_key is not None
        self.assertGreater(win_key, base_key)
        self.assertIsNone(
            validation_selection_key(base | {"validation/unattempted": 1.0})
        )
        self.assertIsNone(
            validation_selection_key(base | {"validation/attempts": 127.0})
        )
        self.assertIsNone(
            validation_selection_key(base | {"validation/reward/mean": math.nan})
        )

    def test_cli_reward_config_audit_routing_and_checkpoint_provenance(self):
        calls = []

        def fake_collect(seed, model, combat, **options):
            calls.append((seed, options["decision_diagnostics"]))
            public = self.public
            prediction = _predict_one(
                model, MacroRequest(public, torch.Generator(), ())
            )
            status = "heart_clear" if seed == "100" else "death"
            p = (
                RunProgress(56, 4, 3, True, True, True, True)
                if status == "heart_clear"
                else RunProgress(17, 2, 1)
            )
            reward = terminal_parts("heart_progress", status, 16, progress=p).total
            return RunEpisode(
                status,
                reward,
                (
                    MacroStep(
                        public, prediction.index, prediction.logits, prediction.value
                    ),
                ),
                1,
                p.furthest_floor,
                objective="heart_progress",
                progress=p,
            )

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "seeds.json").write_text('["100"]')
            (root / "combat.pt").write_bytes(b"mocked loader")
            args = [
                "--run-id",
                "heart",
                "--objective",
                "heart_progress",
                "--decision-diagnostics",
                "evaluation",
                "--combat-checkpoint",
                str(root / "combat.pt"),
                "--validation-seeds",
                str(root / "seeds.json"),
                "--output-root",
                directory,
                "--updates",
                "1",
                "--batch-size",
                "2",
                "--collection-width",
                "1",
                "--model-width",
                "8",
            ]
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch("run_training.trainer.collect", side_effect=fake_collect),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                main(args)
                config = json.loads((root / "heart/config.json").read_text())
                self.assertEqual(config["reward_protocol"], HEART_REWARD_PROTOCOL)
                self.assertEqual(config["heart_reward_spec"]["heart_bonus"], 30)
                self.assertTrue((root / "heart/best-validation.pt").exists())
                metrics = [
                    json.loads(x)
                    for x in (root / "heart/metrics.jsonl").read_text().splitlines()
                ]
                train = next(x for x in metrics if "train/attempts" in x)
                self.assertEqual(train["train/episodes_used_for_learning"], 2)
                self.assertEqual(train["train/completed_episodes_discarded"], 0)
                args[1] = "resumed"
                main(args + ["--resume-from", str(root / "heart/latest.pt")])
                args[1] = "changed"
                args[args.index("--objective") + 1] = "heart"
                with self.assertRaisesRegex(
                    ValueError, "continuation mismatch: objective"
                ):
                    main(args + ["--resume-from", str(root / "heart/latest.pt")])
        self.assertTrue(all(audit == (seed == "100") for seed, audit in calls))


if __name__ == "__main__":
    unittest.main()
