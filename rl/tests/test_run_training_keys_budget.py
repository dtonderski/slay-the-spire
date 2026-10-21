"""Bounded-task reward and public choice accounting, not gameplay parity evidence."""

import contextlib
import copy
import io
import json
import math
import tempfile
import unittest
from dataclasses import asdict, replace
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
from run_training.keys import KeyChoiceStats, key_choice_scores, key_decision
from run_training.model import MacroModel, encode
from run_training.progress import RunProgress
from run_training.rewards import KEYS_BUDGET_PROTOCOL, reward_protocol, terminal_parts
from run_training.trainer import main, scores, update
from sts_sim import State
from sts_sim.observations.screens import (
    MapNode,
    MapObservation,
    MapScreen,
    RestObservation,
    RestScreen,
    RewardObservation,
    RewardScreen,
)


def action(kind, **slots):
    return PolicyAction(
        kind,
        **{
            k: slots.get(k)
            for k in (
                "hand_slot",
                "potion_slot",
                "option_slot",
                "target_slot",
                "card_slot",
                "node_slot",
                "reward_slot",
                "shop_slot",
            )
        },
    )


def public_decision():
    state = State.new("1", final_act=True)
    return state.step(state.decision().actions[0])


def rest():
    base = public_decision().observation
    return RestObservation(
        schema_version=base.schema_version,
        phase="rest",
        kind="rest",
        context=base.context,
        screen=RestScreen(complete=False, options=()),
    )


def reward():
    base = rest()
    return RewardObservation(
        schema_version=base.schema_version,
        phase="reward",
        kind="reward",
        context=base.context,
        screen=RewardScreen(
            cards=(),
            queued_card_rewards=(),
            gold_offer=0,
            stolen_gold_offer=0,
            potion_offer=None,
            potion_offers=(),
            relic_offer=None,
            relic_offers=(),
            sapphire_key_relic_slot=None,
            emerald_key_offer=True,
            boss_relic_choices=(),
            card_reward_flow="none",
        ),
    )


class KeyBudgetTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        torch.manual_seed(25)
        self.model = MacroModel(8)
        self.initial = public_decision()

    def test_once_per_key_and_old_objective_unchanged(self):
        p = RunProgress()
        ob = self.initial.observation
        for ruby, emerald, sapphire, expected in (
            (False, False, False, 0),
            (True, False, False, 0.5),
            (True, True, False, 1),
            (True, True, True, 1.5),
        ):
            view = replace(
                ob,
                context=replace(
                    ob.context,
                    keys=replace(
                        ob.context.keys, ruby=ruby, emerald=emerald, sapphire=sapphire
                    ),
                ),
            )
            for _ in range(5):
                p = p.observed(view)
            self.assertEqual(
                terminal_parts("heart_keys_budget", "death", 0, progress=p).key_bonus,
                expected,
            )
        # Later absence does not erase an already-observed acquisition.
        p = p.observed(ob)
        self.assertEqual(
            terminal_parts("heart_keys_budget", "death", 0, progress=p).key_bonus, 1.5
        )
        win = RunProgress(56, 4, 3, True, True, True, True)
        self.assertEqual(
            terminal_parts("heart_keys_budget", "heart_clear", 16, progress=win).total,
            43.5,
        )
        self.assertEqual(
            terminal_parts("heart_progress", "heart_clear", 16, progress=win).total, 39
        )
        self.assertEqual(
            terminal_parts("heart_keys_budget", "action_limit", 16, progress=win).total,
            -6.5,
        )
        self.assertEqual(reward_protocol("heart_keys_budget"), KEYS_BUDGET_PROTOCOL)
        for objective, status in (
            ("heart_progress", "action_limit"),
            ("heart_keys_budget", "cutoff"),
            ("heart_keys_budget", "error"),
        ):
            with self.assertRaises(ValueError):
                terminal_parts(objective, status, 0, progress=p)

    def scripted(
        self,
        *,
        limit=2,
        objective="heart_keys_budget",
        stop_at=None,
        error_at=None,
        death_at=None,
        seed=12,
    ):
        initial = self.initial

        class Scripted:
            index = 0

            def decision(self):
                context = replace(
                    initial.observation.context,
                    floor=self.index,
                    keys=replace(
                        initial.observation.context.keys, ruby=self.index >= 1
                    ),
                    outcome="death" if death_at == self.index else "ongoing",
                )
                return replace(
                    initial,
                    revision=self.index,
                    observation=replace(initial.observation, context=context),
                )

            def step(self, chosen):
                if self.index == error_at:
                    raise ValueError("unsupported scripted transition")
                self.index += 1
                return self.decision()

        state = Scripted()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.jsonl"
            options: CollectionOptions = {
                "objective": cast(Any, objective),
                "final_act": True,
                "max_actions": limit,
                "macro_rng": torch.Generator().manual_seed(seed),
                "combat_rng": torch.Generator(),
                "journal": path,
                "state_factory": lambda *a, **kw: cast(Any, state),
                "stop_requested": lambda: (
                    stop_at is not None and state.index >= stop_at
                ),
                "initial_metadata": {"protocol": "scripted_public_test_fixture"},
            }
            try:
                episode = collect("1", self.model, cast(Any, None), **options)
            except CollectionFailure as error:
                episode = failed_episode(error, cast(Any, objective))
            rows = [json.loads(line) for line in path.read_text().splitlines()]
        return episode, rows

    def test_action_limit_is_a_target_not_a_death(self):
        episode, rows = self.scripted()
        self.assertEqual(episode.status, "action_limit")
        self.assertEqual(episode.game_outcome, "ongoing")
        self.assertEqual(episode.termination_reason, "action_limit")
        self.assertEqual(episode.accepted, episode.action_limit)
        self.assertAlmostEqual(episode.reward, -20 + 0.5 + 3 * 2 / 55)
        self.assertEqual(rows[-1]["reward_parts"]["action_limit_penalty"], -20)
        result = scores([episode])
        self.assertEqual(result["completed"], 0)
        self.assertEqual(result["outcome/deaths"], 0)
        self.assertEqual(result["cutoffs"], 1)
        self.assertEqual(result["learning_target_episodes"], 1)
        self.assertEqual(result["coverage/unknown_rate"], 1)
        self.assertEqual(result["coverage/training_target_rate"], 1)
        self.assertEqual(result["reward/terminal_samples"], 0)
        self.assertEqual(result["reward/target_samples"], 1)
        self.assertEqual(result["success_lower_bound"], 0)
        legacy, _ = self.scripted(objective="heart_progress")
        self.assertEqual(legacy.status, "cutoff")
        self.assertIsNone(legacy.reward)

    def test_interruptions_and_errors_never_get_penalized_or_key_targets(self):
        # Stop coincides with the budget: interruption conservatively wins.
        for kwargs in ({"stop_at": 0}, {"stop_at": 2}, {"error_at": 1}):
            episode, rows = self.scripted(**kwargs)
            self.assertIsNone(episode.reward)
            self.assertNotEqual(episode.status, "death")
            self.assertNotEqual(episode.status, "action_limit")
            self.assertEqual(rows[-1].get("reward_parts"), None)
            with self.assertRaises(CollectionFailure):
                update(
                    [episode],
                    self.model,
                    torch.optim.Adam(self.model.parameters()),
                    entropy_coef=0.01,
                    value_coef=0.1,
                )
        death, _ = self.scripted(death_at=2)
        self.assertEqual(death.status, "death")
        self.assertEqual(death.reward_parts().action_limit_penalty, 0)
        self.assertEqual(death.termination_reason, "game_terminal")
        self.assertEqual(death.reward_parts().key_bonus, 0.5)

    def test_mixed_budget_and_terminal_batches_learn_cpu_cuda(self):
        budget, _ = self.scripted()
        death, _ = self.scripted(death_at=2, seed=17)
        result = scores([budget, death])
        self.assertEqual(result["completed"], 1)
        self.assertEqual(result["outcome/deaths"], 1)
        self.assertEqual(result["learning_target_episodes"], 2)
        self.assertEqual(result["coverage/unknown_rate"], 0.5)
        cases = [(0, "cpu"), (8, "cpu")]
        if torch.cuda.is_available():
            cases.append((8, "cuda"))
        for width, device in cases:
            model = copy.deepcopy(self.model).to(device)
            optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
            info = update(
                [budget, death],
                model,
                optimizer,
                entropy_coef=0.01,
                value_coef=0.1,
                batch_steps=width,
            )
            self.assertEqual(info["optimizer_step"], 1)
            self.assertTrue(all(torch.isfinite(p).all() for p in model.parameters()))
        for fake in (
            replace(budget, accepted=1),
            replace(budget, action_limit=None),
            replace(budget, termination_reason="interrupted"),
            replace(budget, game_outcome="death"),
        ):
            model = copy.deepcopy(self.model)
            optimizer = torch.optim.Adam(model.parameters())
            with self.assertRaisesRegex(ValueError, "genuine exhausted-budget"):
                update(
                    [fake],
                    model,
                    optimizer,
                    entropy_coef=0.01,
                    value_coef=0.1,
                    batch_steps=8,
                )
            self.assertFalse(optimizer.state)

    def test_optional_ruby_rates_and_unique_locations(self):
        ob = rest()
        actions = (action("rest_heal"), action("rest_recall"))
        before = torch.get_rng_state().clone()
        stats = KeyChoiceStats()
        for choice in (0, 0, 1):
            proposal = key_decision(
                ob, actions, choice, logits=(math.log(0.75), math.log(0.25))
            )
            stats = stats.accepted(proposal)
        report = key_choice_scores([stats])
        self.assertEqual(report["keys/ruby/eligible_opportunities"], 3)
        self.assertEqual(report["keys/ruby/choices"], 1)
        self.assertAlmostEqual(report["keys/ruby/optional_choice_rate"], 1 / 3)
        self.assertAlmostEqual(report["keys/ruby/choice_probability"], 0.25)
        self.assertEqual(report["keys/ruby/unique_opportunity_locations"], 1)
        self.assertEqual(report["keys/ruby/location_choice_rate"], 1)
        torch.testing.assert_close(before, torch.get_rng_state(), atol=0, rtol=0)
        owned = replace(
            ob, context=replace(ob.context, keys=replace(ob.context.keys, ruby=True))
        )
        self.assertIsNone(key_decision(owned, actions, 0, logits=(0.0, 0.0)))
        masked = KeyChoiceStats().accepted(
            key_decision(
                ob, actions, 0, eligibility=(True, False), logits=(0.0, -math.inf)
            )
        )
        report = key_choice_scores([masked])
        self.assertEqual(report["keys/ruby/native_opportunities"], 1)
        self.assertEqual(report["keys/ruby/eligible_opportunities"], 0)
        self.assertNotIn("keys/ruby/choice_rate", report)
        forced = KeyChoiceStats().accepted(key_decision(ob, (actions[1],), 0))
        report = key_choice_scores([forced])
        self.assertEqual(report["keys/ruby/forced_choices"], 1)
        self.assertNotIn("keys/ruby/optional_choice_rate", report)

    def test_sapphire_emerald_pickup_and_route_are_distinct(self):
        ob = reward()
        for kind, label in (
            ("take_sapphire_key", "sapphire"),
            ("take_emerald_key", "emerald_pickup"),
        ):
            stats = KeyChoiceStats().accepted(
                key_decision(
                    ob, (action(kind), action("proceed")), 0, logits=(0.0, 0.0)
                )
            )
            self.assertEqual(key_choice_scores([stats])[f"keys/{label}/choices"], 1)
        nodes = (
            MapNode(slot=0, act=1, room_kind="elite", burning_elite=True, children=()),
            MapNode(
                slot=1, act=1, room_kind="combat", burning_elite=False, children=()
            ),
        )
        view = MapObservation(
            schema_version=ob.schema_version,
            phase="idle",
            kind="map",
            context=ob.context,
            screen=MapScreen(
                act=1, floor=0, current_node=0, reachable_nodes=(0, 1), nodes=nodes
            ),
        )
        proposal = key_decision(
            view,
            (
                action("choose_map_node", node_slot=0),
                action("choose_map_node", node_slot=1),
            ),
            0,
            logits=(0.0, 0.0),
        )
        stats = key_choice_scores([KeyChoiceStats().accepted(proposal)])
        self.assertEqual(stats["keys/emerald_route/choices"], 1)
        self.assertEqual(stats["keys/emerald_pickup/choices"], 0)
        self.assertFalse(RunProgress().observed(view).emerald_seen)

    def test_only_successful_key_choices_count(self):
        original = public_decision()
        ob = rest()
        descriptors = (action("rest_recall"), action("rest_heal"))
        initial = replace(
            original,
            observation=ob,
            actions=tuple(
                cast(Any, type("PublicAction", (), asdict(a))()) for a in descriptors
            ),
        )

        class Rejecting:
            def decision(self):
                return initial

            def step(self, chosen):
                raise ValueError("rejected fixture key action")

        with (
            tempfile.TemporaryDirectory() as directory,
            self.assertRaises(CollectionFailure) as caught,
        ):
            collect(
                "1",
                self.model,
                cast(Any, None),
                objective="heart_keys_budget",
                final_act=True,
                max_actions=10,
                macro_rng=torch.Generator(),
                combat_rng=torch.Generator(),
                journal=Path(directory) / "fixture.jsonl",
                state_factory=lambda *a, **kw: cast(Any, Rejecting()),
            )
        stats = key_choice_scores([caught.exception.behavior.key_choices])
        self.assertEqual(stats["keys/ruby/native_opportunities"], 0)
        assert caught.exception.progress is not None
        self.assertFalse(caught.exception.progress.ruby_seen)
        self.assertIsNone(failed_episode(caught.exception, "heart_keys_budget").reward)

    def test_cli_budget_targets_update_without_false_completion(self):
        seen = []

        def fake_collect(seed, model, combat, **options):
            seen.append(options["objective"])
            public = encode(
                self.initial.observation,
                tuple(PolicyAction.from_action(a) for a in self.initial.actions),
            )
            p = _predict_one(model, MacroRequest(public, torch.Generator(), ()))
            progress = RunProgress(1, 1, 0, True)
            target = terminal_parts(
                "heart_keys_budget", "action_limit", 0, progress=progress
            ).total
            return RunEpisode(
                "action_limit",
                target,
                (MacroStep(public, p.index, p.logits, p.value),),
                1,
                1,
                objective="heart_keys_budget",
                progress=progress,
                termination_reason="action_limit",
                game_outcome="ongoing",
                action_limit=1,
            )

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "seeds.json").write_text('["100"]')
            (root / "combat.pt").write_bytes(b"mocked")
            args = [
                "--run-id",
                "budget",
                "--objective",
                "heart_keys_budget",
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
                "--learner-batch-size",
                "8",
                "--model-width",
                "8",
                "--max-actions",
                "1",
            ]
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch("run_training.trainer.collect", side_effect=fake_collect),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                main(args)
                cp = torch.load(root / "budget/latest.pt", weights_only=True)
                self.assertEqual(cp["optimizer_updates"], 1)
                self.assertEqual(cp["skipped_batches"], 0)
                self.assertEqual(cp["config"]["key_bonus"], 0.5)
                self.assertEqual(cp["config"]["act4_entry_bonus"], 3)
                self.assertEqual(cp["config"]["action_limit_penalty"], -20)
                rows = [
                    json.loads(x)
                    for x in (root / "budget/metrics.jsonl").read_text().splitlines()
                ]
                train = next(r for r in rows if "train/attempts" in r)
                self.assertEqual(train["train/completed"], 0)
                self.assertEqual(train["train/learning_target_episodes"], 2)
                self.assertEqual(train["train/episodes_used_for_learning"], 2)
                self.assertEqual(train["train/outcome/deaths"], 0)
                args[1] = "resume"
                main(args + ["--resume-from", str(root / "budget/latest.pt")])
                args[1] = "incompatible"
                args[args.index("--objective") + 1] = "heart_progress"
                with self.assertRaisesRegex(
                    ValueError, "continuation mismatch: objective"
                ):
                    main(args + ["--resume-from", str(root / "budget/latest.pt")])
        self.assertTrue(all(x == "heart_keys_budget" for x in seen))


if __name__ == "__main__":
    unittest.main()
