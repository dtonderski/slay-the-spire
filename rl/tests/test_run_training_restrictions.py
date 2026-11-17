"""Public policy restriction infrastructure; not real-game parity evidence."""

import copy
import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast
from unittest.mock import patch

import torch
from run_training.collector import (
    MacroRequest,
    MacroStep,
    RunEpisode,
    _predict_many,
    _predict_one,
    collect,
    sample,
    sample_many,
)
from run_training.contracts import PolicyAction
from run_training.model import MacroModel, encode
from run_training.restrictions import (
    AVOID_PRISMATIC_PROTOCOL,
    CARD_REWARD_PROTOCOL,
    ClosedCardRewards,
    checked_eligibility,
    macro_eligibility,
    restrict_logits,
)
from run_training.token_model import encode_tokens
from run_training.trainer import main, update
from sts_sim import RelicKey, State
from sts_sim.observations.screens import (
    RelicOffer,
    RewardObservation,
    RewardScreen,
    ShopObservation,
    ShopOffer,
    ShopScreen,
)


def action(kind, **slots):
    fields = {
        "hand_slot": None,
        "potion_slot": None,
        "option_slot": None,
        "target_slot": None,
        "card_slot": None,
        "node_slot": None,
        "reward_slot": None,
        "shop_slot": None,
    }
    return PolicyAction(kind, **(fields | slots))


def shop():
    context = State.new("1").decision().observation.context
    return ShopObservation(
        schema_version=9,
        phase="shop",
        kind="shop",
        context=context,
        screen=ShopScreen(
            merchant_open=True,
            remove_cost=None,
            cards=(),
            potions=(),
            relics=tuple(
                ShopOffer(slot=i, content_key=key, price=50, sold=False)
                for i, key in enumerate((RelicKey.ANCHOR, RelicKey.PRISMATIC_SHARD))
            ),
        ),
    )


def reward():
    return RewardObservation(
        schema_version=9,
        phase="reward",
        kind="reward",
        context=shop().context,
        screen=RewardScreen(
            cards=(),
            queued_card_rewards=(),
            gold_offer=0,
            stolen_gold_offer=0,
            potion_offer=None,
            potion_offers=(),
            relic_offer=RelicKey.PRISMATIC_SHARD,
            relic_offers=(
                RelicOffer(slot=0, content_key=RelicKey.ANCHOR),
                RelicOffer(slot=1, content_key=RelicKey.PRISMATIC_SHARD),
            ),
            sapphire_key_relic_slot=1,
            emerald_key_offer=False,
            boss_relic_choices=(RelicKey.ANCHOR, RelicKey.PRISMATIC_SHARD),
            card_reward_flow="none",
        ),
    )


def descriptors():
    return (
        action("buy_shop_relic", shop_slot=0),
        action("buy_shop_relic", shop_slot=1),
        action("leave_shop"),
    )


class RestrictionTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        torch.manual_seed(17)
        self.model = MacroModel(8)
        self.public = encode(shop(), descriptors(), avoid_prismatic=True)

    def test_visible_acquisitions_only_and_default_unrestricted(self):
        cases = [
            (shop(), descriptors(), (True, False, True)),
            (reward(), (action("take_relic_reward"), action("proceed")), (False, True)),
            (
                reward(),
                (
                    action("take_relic_reward_at", reward_slot=0),
                    action("take_relic_reward_at", reward_slot=1),
                    action("take_sapphire_key_at", reward_slot=1),
                ),
                (True, False, True),
            ),
            (
                reward(),
                (
                    action("choose_boss_relic_reward", reward_slot=1),
                    action("skip_boss_relic_reward"),
                ),
                (False, True),
            ),
        ]
        for observation, actions, expected in cases:
            self.assertEqual(
                macro_eligibility(observation, actions, avoid_prismatic=True), expected
            )
            self.assertEqual(
                macro_eligibility(observation, actions, avoid_prismatic=False), ()
            )
        # Unknown/random outcomes are not inspected; forced grants remain possible.
        self.assertEqual(
            macro_eligibility(
                reward(), (action("take_relic_reward"),), avoid_prismatic=True
            ),
            (),
        )
        self.assertEqual(
            macro_eligibility(
                reward(),
                (action("choose_event_option", option_slot=0), action("rest_dig")),
                avoid_prismatic=True,
            ),
            (True, True),
        )
        with self.assertRaisesRegex(ValueError, "no eligible"):
            macro_eligibility(
                reward(),
                (
                    action("take_relic_reward"),
                    action("take_relic_reward_at", reward_slot=1),
                ),
                avoid_prismatic=True,
            )
        for slot in (-1, 2):
            with self.assertRaisesRegex(ValueError, "public relic"):
                macro_eligibility(
                    shop(),
                    (action("buy_shop_relic", shop_slot=slot), action("leave_shop")),
                    avoid_prismatic=True,
                )

    def test_features_candidates_indices_and_rng_unchanged(self):
        before = torch.get_rng_state().clone()
        all_legal = encode(shop(), descriptors())
        self.assertEqual(replace(self.public, eligibility=()), all_legal)
        self.assertEqual(len(self.public.candidates), 3)
        self.assertEqual(self.public.eligibility, (True, False, True))
        torch.testing.assert_close(before, torch.get_rng_state(), atol=0, rtol=0)
        raw = torch.tensor([-1000.0, 1000.0, 0.0])
        probabilities = restrict_logits(raw, self.public.eligibility).softmax(0)
        self.assertEqual(probabilities.tolist(), [0.0, 0.0, 1.0])
        for seed in range(64):
            self.assertEqual(
                sample(
                    raw,
                    torch.Generator().manual_seed(seed),
                    eligibility=self.public.eligibility,
                ),
                2,
            )
        for bad in ((False, False, False), (True, False), (True, 0, True)):
            with self.assertRaises(ValueError):
                checked_eligibility(3, cast(Any, bad))  # Deliberately malformed record.
        with self.assertRaises(ValueError):
            sample(
                torch.tensor([0.0, float("nan"), 1.0]),
                torch.Generator(),
                eligibility=self.public.eligibility,
            )
        with self.assertRaises(ValueError):
            sample_many(
                [torch.tensor([0.0, float("inf"), 1.0])],
                [torch.Generator()],
                eligibility=[self.public.eligibility],
            )

    def test_serial_batched_predictions_probabilities_and_generators(self):
        rows = [
            self.public,
            replace(
                self.public,
                candidates=self.public.candidates[:2],
                eligibility=(True, False),
            ),
        ]
        requests_a = [
            MacroRequest(row, torch.Generator().manual_seed(200 + i), (0,))
            for i, row in enumerate(rows)
        ]
        requests_b = [
            MacroRequest(row, torch.Generator().manual_seed(200 + i), (0,))
            for i, row in enumerate(rows)
        ]
        expected = [_predict_one(self.model, request) for request in requests_a]
        actual = _predict_many(self.model, requests_b)
        self.assertEqual(expected, actual)
        for prediction, a, b in zip(actual, requests_a, requests_b, strict=True):
            self.assertNotEqual(prediction.index, 1)
            self.assertEqual(prediction.logits[1], -torch.inf)
            self.assertEqual(prediction.heal_probability, 1 / sum(a.inputs.eligibility))
            torch.testing.assert_close(
                a.generator.get_state(), b.generator.get_state(), atol=0, rtol=0
            )

    def episode(self, model, public=None):
        public = public or self.public
        prediction = _predict_one(
            model, MacroRequest(public, torch.Generator().manual_seed(3), ())
        )
        return RunEpisode(
            "act1_clear",
            6.0,
            (MacroStep(public, prediction.index, prediction.logits, prediction.value),),
            1,
            18,
            furthest_act1_floor=16,
        )

    def test_masked_learning_serial_batched_and_mismatch_rejection(self):
        models = [copy.deepcopy(self.model) for _ in range(2)]
        results = []
        for model, width in zip(models, (0, 2), strict=True):
            optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
            episodes = [
                self.episode(model),
                self.episode(
                    model,
                    replace(
                        self.public,
                        candidates=self.public.candidates[:2],
                        eligibility=(True, False),
                    ),
                ),
            ]
            results.append(
                update(
                    episodes,
                    model,
                    optimizer,
                    entropy_coef=0.01,
                    value_coef=0.1,
                    batch_steps=width,
                )
            )
            self.assertTrue(all(torch.isfinite(p).all() for p in model.parameters()))
        for key in results[0]:
            self.assertAlmostEqual(results[0][key], results[1][key], places=5)
        for a, b in zip(models[0].parameters(), models[1].parameters(), strict=True):
            torch.testing.assert_close(a, b, atol=2e-7, rtol=1e-4)
        episode = self.episode(self.model)
        for width in (0, 2):
            for step in (
                replace(episode.steps[0], choice=1),
                replace(episode.steps[0], inputs=replace(self.public, eligibility=())),
            ):
                model = copy.deepcopy(self.model)
                optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
                with self.assertRaisesRegex(ValueError, "eligibility|recomputation"):
                    update(
                        [replace(episode, steps=(step,))],
                        model,
                        optimizer,
                        entropy_coef=0.01,
                        value_coef=0.1,
                        batch_steps=width,
                    )
                self.assertFalse(optimizer.state)
                for a, b in zip(
                    model.parameters(), self.model.parameters(), strict=True
                ):
                    torch.testing.assert_close(a, b, atol=0, rtol=0)

    @unittest.skipUnless(torch.cuda.is_available(), "CUDA unavailable")
    def test_cuda_masked_predictions_and_learning(self):
        model = copy.deepcopy(self.model).to("cuda")
        requests = [
            MacroRequest(self.public, torch.Generator(device="cuda").manual_seed(i), ())
            for i in range(4)
        ]
        predictions = _predict_many(model, requests)
        for prediction in predictions:
            self.assertNotEqual(prediction.index, 1)
            self.assertEqual(prediction.logits[1], -torch.inf)
        steps = tuple(
            MacroStep(self.public, p.index, p.logits, p.value) for p in predictions
        )
        episode = RunEpisode("act1_clear", 6.0, steps, 4, 18, furthest_act1_floor=16)
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        metrics = update(
            [episode],
            model,
            optimizer,
            entropy_coef=0.01,
            value_coef=0.1,
            batch_steps=4,
        )
        self.assertTrue(metrics["optimizer_step"])
        self.assertTrue(all(torch.isfinite(p).all() for p in model.parameters()))

    def test_excluded_logits_have_zero_gradient_and_entropy_is_finite(self):
        from torch.distributions import Categorical

        raw = torch.tensor([0.2, 1000.0, -0.3], requires_grad=True)
        distribution = Categorical(logits=restrict_logits(raw, (True, False, True)))
        loss = -distribution.log_prob(torch.tensor(2)) - 0.01 * distribution.entropy()
        self.assertTrue(torch.isfinite(loss))
        loss.backward()
        assert raw.grad is not None
        self.assertEqual(raw.grad[1].item(), 0.0)
        self.assertTrue(torch.isfinite(raw.grad).all())

    def test_collection_journals_full_indices_profile_and_eligibility(self):
        actions = descriptors()
        public = shop()
        initial = SimpleNamespace(observation=public, actions=actions, revision=0)
        terminal = SimpleNamespace(
            observation=replace(
                public, context=replace(public.context, act=2, floor=18)
            ),
            actions=(action("proceed"),),
            revision=1,
        )
        selected = []

        class ScriptedState:
            def decision(self):
                return initial

            def advance(self, action):
                return self.step(action)

            def step(self, chosen):
                selected.append(actions.index(chosen))
                return terminal

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.jsonl"
            episode = collect(
                "1",
                self.model,
                cast(Any, None),  # This public scripted fixture never enters combat.
                objective="act1",
                final_act=True,
                max_actions=10,
                macro_rng=torch.Generator().manual_seed(3),
                combat_rng=torch.Generator(),
                journal=path,
                state_factory=lambda *a, **kw: cast(Any, ScriptedState()),
                avoid_prismatic=True,
                initial_metadata={"protocol": "scripted_public_test_fixture"},
            )
            rows = [json.loads(line) for line in path.read_text().splitlines()]
        self.assertNotEqual(selected[0], 1)
        self.assertEqual(
            rows[0]["policy_restriction_protocol"], AVOID_PRISMATIC_PROTOCOL
        )
        self.assertTrue(rows[0]["avoid_prismatic"])
        self.assertEqual(rows[1]["policy_eligibility"], [True, False, True])
        self.assertEqual(rows[1]["index"], selected[0])
        self.assertEqual(episode.steps[0].inputs.eligibility, (True, False, True))

    def test_cli_training_evaluation_archive_and_strict_resume(self):
        flags = []

        def fake_collect(seed, model, combat, **options):
            self.assertTrue(options["commit_navigation"])
            flags.append(options["avoid_prismatic"])
            return self.episode(model)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "seeds.json").write_text('["100"]')
            (root / "combat.pt").write_bytes(b"mocked loader")
            args = [
                "--run-id",
                "restricted",
                "--combat-checkpoint",
                str(root / "combat.pt"),
                "--validation-seeds",
                str(root / "seeds.json"),
                "--output-root",
                directory,
                "--collection-width",
                "1",
                "--batch-size",
                "1",
                "--updates",
                "1",
                "--model-width",
                "8",
                "--avoid-prismatic",
            ]
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch("run_training.trainer.collect", side_effect=fake_collect),
            ):
                main(args)
                checkpoint = root / "restricted/latest.pt"
                config = json.loads((root / "restricted/config.json").read_text())
                self.assertEqual(
                    config["policy_restriction_protocol"], AVOID_PRISMATIC_PROTOCOL
                )
                self.assertTrue(config["avoid_prismatic"])
                self.assertTrue(
                    (root / "restricted/source/run_training/restrictions.py").exists()
                )
                args[1] = "incompatible"
                args.remove("--avoid-prismatic")
                with self.assertRaisesRegex(
                    ValueError, "continuation mismatch: avoid_prismatic"
                ):
                    main(args + ["--resume-from", str(checkpoint)])
                args[1] = "incompatible-navigation"
                with self.assertRaisesRegex(
                    ValueError, "continuation mismatch: raw_navigation"
                ):
                    main(
                        args
                        + [
                            "--avoid-prismatic",
                            "--raw-navigation",
                            "--resume-from",
                            str(checkpoint),
                        ]
                    )
                args[1] = "continued"
                main(args + ["--avoid-prismatic", "--resume-from", str(checkpoint)])
        self.assertTrue(flags and all(flags))


def reward_screen(decision) -> RewardScreen:
    screen = decision.observation.screen
    assert isinstance(screen, RewardScreen)
    return screen


class FinalCardSkipTests(unittest.TestCase):
    """Simulator-backed: a masked open would reopen a closed reward, never another."""

    def rewards(self, relics):
        state = State.from_synthetic_spec(
            json.dumps(
                {
                    "seed": 5,
                    "floor": 3,
                    "kind": "normal",
                    "encounter": "Cultist",
                    "deck": [{"key": "Strike_R", "upgrades": 1}] * 8,
                    "relics": relics,
                    "potions": [None, None, None],
                    "hp": 80,
                    "max_hp": 80,
                    "gold": 99,
                }
            )
        )
        decision = state.decision()
        while decision.observation.kind != "reward":
            plays = [a for a in decision.actions if a.kind == "play_card"]
            decision = state.step((plays or decision.actions)[0])
        self.state, self.decision, self.closed = state, decision, ClosedCardRewards()

    def step(self, kind, slot=None):
        before = self.decision.observation
        chosen = next(
            a
            for a in self.decision.actions
            if a.kind == kind and (slot is None or a.reward_slot == slot)
        )
        self.decision = self.state.step(chosen)
        self.closed = self.closed.advanced(
            before, PolicyAction.from_action(chosen), self.decision.observation
        )
        return reward_screen(self.decision).cards

    def eligible(self):
        actions = tuple(PolicyAction.from_action(a) for a in self.decision.actions)
        eligibility = macro_eligibility(
            self.decision.observation,
            actions,
            avoid_prismatic=False,
            closed_card_rewards=self.closed,
        )
        self.assertTrue(any(eligibility))
        for encoder in (encode, encode_tokens):
            row = encoder(
                self.decision.observation, actions, closed_card_rewards=self.closed
            )
            self.assertEqual(row.eligibility, eligibility)
        return {
            (a.kind, a.reward_slot): ok
            for a, ok in zip(actions, eligibility, strict=True)
        }

    def test_closing_the_only_card_reward_is_final(self):
        self.rewards([])
        self.assertTrue(self.eligible()[("open_card_reward", None)])
        self.step("open_card_reward")
        self.assertTrue(all(self.eligible().values()))  # Taking stays possible.
        self.step("close_card_reward")
        eligible = self.eligible()
        self.assertFalse(eligible[("open_card_reward", None)])
        self.assertTrue(eligible[("skip_reward", None)])
        self.assertEqual(sum(not ok for ok in eligible.values()), 1)
        # Default and opens-only decisions are unrestricted.
        actions = tuple(PolicyAction.from_action(a) for a in self.decision.actions)
        observation = self.decision.observation
        self.assertEqual(
            macro_eligibility(observation, actions, avoid_prismatic=False), ()
        )
        opens = tuple(a for a in actions if a.kind == "open_card_reward")
        self.assertEqual(
            macro_eligibility(
                observation,
                opens,
                avoid_prismatic=False,
                closed_card_rewards=self.closed,
            ),
            (),
        )

    def test_prayer_wheel_rewards_are_tracked_by_public_slot(self):
        self.rewards(["Prayer Wheel"])
        closed_cards = self.step("open_queued_card_reward", 1)
        self.step("close_card_reward")
        eligible = self.eligible()
        self.assertFalse(eligible[("open_card_reward", None)])  # Shows slot 1.
        self.assertFalse(eligible[("open_queued_card_reward", 1)])
        self.assertTrue(eligible[("open_queued_card_reward", 0)])
        self.step("open_queued_card_reward", 0)
        self.assertTrue(all(self.eligible().values()))
        self.step("take_card_reward", 0)
        # The used reward left the queue, so the closed one shifted to slot 0.
        eligible = self.eligible()
        self.assertEqual(len(reward_screen(self.decision).queued_card_rewards), 1)
        self.assertFalse(eligible[("open_card_reward", None)])
        self.assertFalse(eligible[("open_queued_card_reward", 0)])
        probe = self.state.clone()
        reopened = probe.step(
            next(a for a in self.decision.actions if a.kind == "open_card_reward")
        )
        self.assertEqual(reward_screen(reopened).cards, closed_cards)

    def test_collector_records_the_protocol_only_when_enabled(self):
        public = shop()
        terminal = SimpleNamespace(
            observation=replace(public, context=replace(public.context, act=2)),
            actions=(action("proceed"),),
            revision=1,
        )

        class ScriptedState:
            def decision(self):
                return SimpleNamespace(
                    observation=public, actions=descriptors(), revision=0
                )

            def advance(self, _action):
                return terminal

        for enabled in (False, True):
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "fixture.jsonl"
                collect(
                    "1",
                    MacroModel(8),
                    cast(Any, None),
                    objective="act1",
                    final_act=True,
                    max_actions=10,
                    macro_rng=torch.Generator().manual_seed(3),
                    combat_rng=torch.Generator(),
                    journal=path,
                    state_factory=lambda *a, **kw: cast(Any, ScriptedState()),
                    final_card_skip=enabled,
                    initial_metadata={"protocol": "scripted_public_test_fixture"},
                )
                setup = json.loads(path.read_text().splitlines()[0])
            self.assertEqual(
                setup.get("card_reward_protocol"),
                CARD_REWARD_PROTOCOL if enabled else None,
            )


if __name__ == "__main__":
    unittest.main()
