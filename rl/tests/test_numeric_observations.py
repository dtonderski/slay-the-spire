"""Numeric export/refinement checks; fixtures are not real-game parity evidence."""

import random
import unittest
from dataclasses import replace
from typing import cast
from unittest.mock import patch

import numpy as np
import torch
from encoders.numeric import NumericBatch
from model import CombatValueModel
from numeric_reference import CATEGORICAL, reference_batch, semantic_tables
from sts_sim import CounterKey, PowerKey, State
from sts_sim.observations import (
    Counter,
    Power,
    Selection,
    SelectionOption,
    VisibleIntent,
)
from test_model import action, combat
from train import play_combats


class NumericObservationTests(unittest.TestCase):
    def test_partial_native_step_failure_is_not_retried(self) -> None:
        from rollout_errors import SimulatorStepError

        roots = [combat(1), combat(2)]
        before = [root.observation() for root in roots]
        native_steps = State.numeric_steps

        def partial(states, actions):
            native_steps(states[:1], actions[:1])
            raise ValueError("synthetic partial batch failure")

        with patch.object(State, "numeric_steps", side_effect=partial) as steps:
            with self.assertRaises(SimulatorStepError) as caught:
                play_combats(roots, None, max_decisions=10, rng=random.Random(0))
            self.assertEqual(steps.call_count, 1)
        self.assertEqual(caught.exception.root_indices, [0, 1])
        self.assertEqual([len(p) for p in caught.exception.action_prefixes], [1, 1])
        self.assertEqual(before, [root.observation() for root in roots])

    def test_empty_legal_actions_are_reported_as_simulator_error(self) -> None:
        from rollout_errors import SimulatorStepError

        root = combat()
        version, symbols, tables, _, rows = State.numeric_decisions([root])
        with (
            patch.object(State, "numeric_decisions", return_value=(version, symbols, tables, [()], rows)),
            self.assertRaises(SimulatorStepError) as caught,
        ):
            play_combats([root], None, max_decisions=1, rng=random.Random(0))
        self.assertEqual(caught.exception.action_prefixes, [[]])

    def test_native_tables_actions_and_rng_noninterference(self) -> None:
        for seed in ("HUMAN1", "2000001", "2000014", "2000140", "1000009"):
            left = State.new(seed)
            right = left.clone()
            decision = left.decision()
            numeric = NumericBatch(State.numeric_decisions([right]))
            rng = random.Random(seed)
            for _ in range(250):
                expected = reference_batch([decision])
                self.assertEqual(semantic_tables(expected), semantic_tables(numeric))
                self.assertEqual(expected.model_rows, numeric.model_rows)
                self.assertEqual([repr(a) for a in decision.actions], [repr(a) for a in numeric.actions[0]])
                self.assertEqual(left.revision, right.revision)
                self.assertEqual(
                    semantic_tables(numeric), semantic_tables(NumericBatch(State.numeric_decisions([right])))
                )
                if not decision.actions:
                    break
                index = rng.randrange(len(decision.actions))
                chosen = numeric.actions[0][index]
                decision = left.step(decision.actions[index])
                numeric = NumericBatch(State.numeric_steps([right], [chosen]))

    def test_immutable_buffer_lifetime_and_rejected_transition(self) -> None:
        state = combat()
        batch = NumericBatch(State.numeric_decisions([state]))
        before = batch.table("hand", 18).copy()
        stale = batch.actions[0][0]
        State.numeric_steps([state], [stale])
        revision = state.revision
        observation = state.observation()
        with self.assertRaises(ValueError):
            State.numeric_steps([state], [stale])
        self.assertEqual(revision, state.revision)
        self.assertEqual(observation, state.observation())
        del state
        np.testing.assert_array_equal(before, batch.table("hand", 18))
        with self.assertRaises(ValueError):
            batch.table("hand", 18)[0, 2] = 999

    def test_features_masks_logits_and_gradients_on_edge_cases(self) -> None:
        torch.set_num_threads(1)
        decision = combat().decision()
        obs = decision.observation
        assert obs.kind == "combat"
        base = obs.screen.hand[0].card
        card = replace(
            base,
            cost=-1,
            upgrade_level=2,
            bottled=True,
            temporary=True,
            cost_is_modified=True,
            cost_resets_next_turn=True,
            dynamic=replace(
                base.dynamic,
                rampage_damage_bonus=17,
                ritual_dagger_damage_bonus=0,
                windmill_retain_damage=31,
                steam_barrier_block_reduction=5,
                combat_cost_under_turn_override=0,
            ),
        )
        selection = Selection(
            kind="discovery_reward", options=(SelectionOption(slot=0, card=card),), selected_slots=(0,)
        )
        enemy = replace(
            obs.screen.monsters[0],
            stasis_card=card,
            hp=0,
            alive=False,
            targetable=False,
            escaped=True,
            minion=True,
            in_defensive_mode=True,
            stolen_gold=33,
            slime_size="Large",
            powers=(Power(key=PowerKey.STRENGTH, amount=-3),),
            intent=VisibleIntent(visibility="visible", category="attack", damage=0, hits=None),
        )
        relic = replace(
            obs.context.relics[0],
            state=(Counter(key=CounterKey.ARMED, value=7), Counter(key=CounterKey.ACTIVE, value=-1)),
        )
        a = replace(
            obs,
            context=replace(obs.context, relics=(relic,)),
            screen=replace(
                obs.screen,
                selection=selection,
                monsters=(enemy,),
                hand=(replace(obs.screen.hand[0], card=card),),
                player=replace(obs.screen.player, powers=(Power(key=PowerKey.STRENGTH, amount=5),)),
            ),
        )
        b = replace(obs, screen=replace(obs.screen, hand=(), monsters=()))
        actions = [
            (
                action("play_hand_slot", target_slot=0),
                action("end_turn"),
                action("toggle_visible_card"),
                action("choose_visible_option"),
                action("use_potion_slot"),
                action("discard_potion_slot"),
                action("confirm_selection"),
                action("confirm_selection_without_retrieval"),
                action("skip_selection"),
            ),
            (action("end_turn"),),
        ]
        raw = reference_batch([replace(decision, observation=a), replace(decision, observation=b)])
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            for dtype in (torch.float32, torch.float64):
                with self.subTest(device=device, dtype=dtype):
                    torch.manual_seed(123)
                    model = CombatValueModel(d_model=16, action_dim=8, n_layers=1).to(device=device, dtype=dtype)
                    logits, values, mask = model(raw, actions)
                    self.assertTrue(torch.isfinite(logits[mask]).all())
                    self.assertTrue(torch.isneginf(logits[~mask]).all())
                    (logits[mask].square().sum() + values.square().sum()).backward()
                    self.assertTrue(all(torch.isfinite(p.grad).all() for p in model.parameters() if p.grad is not None))

    def test_dictionary_numbers_are_not_model_features(self) -> None:
        state = combat()
        batch = NumericBatch(State.numeric_decisions([state]))
        tables = {}
        for name, values in batch.tables.items():
            values = values.copy()
            for column in CATEGORICAL.get(name, ()):
                values[:, column] = np.where(values[:, column] < 0, -1, len(batch.symbols) - 1 - values[:, column])
            tables[name] = (values.shape[1], values.tobytes())
        permuted = NumericBatch((1, batch.symbols[::-1], tables, batch.actions, batch.model_rows))
        model = CombatValueModel()
        a, va, mask_a = model(batch, [tuple(batch.actions[0])])
        b, vb, mask_b = model(permuted, [tuple(batch.actions[0])])
        torch.testing.assert_close(a, b, rtol=0, atol=0)
        torch.testing.assert_close(va, vb, rtol=0, atol=0)
        self.assertTrue(torch.equal(mask_a, mask_b))

    def test_generated_smoke_is_excluded_after_rows_finish(self) -> None:
        # Accepted public-action fixture from the simulator collector, not a captured game trace.
        smoke = State.new("2000140")
        prefix = [0, 1, 0, 3, 4, 8, 6, 5, 4, 3, 3, 2, 0, 4, 3, 0, 0, 3, 5, 0, 1, 0, 3, 2, 8, 8, 1, 6, 10, 0, 0]
        for index in prefix:
            smoke.step(smoke.decision().actions[index])
        action_to_brew = next(a for a in smoke.decision().actions if a.kind == "use_potion_slot" and a.potion_slot == 0)
        after = smoke.step(action_to_brew)
        self.assertEqual(after.observation.context.potion_slots[0].content_key, "smoke_bomb")

        class ChoosePotionOrEnd(torch.nn.Module):
            def forward(self, observations, actions):
                logits = torch.full((len(actions), max(map(len, actions))), -torch.inf)
                for row, candidates in enumerate(actions):
                    indices = [
                        i for i, a in enumerate(candidates) if a.kind == "use_potion_slot" and a.potion_slot == 0
                    ]
                    index = indices[0] if indices else next(i for i, a in enumerate(candidates) if a.kind == "end_turn")
                    logits[row, index] = 0
                return logits, torch.zeros((len(actions), 1)), logits.isfinite()

        roots = [combat(), smoke]
        policy = cast(CombatValueModel, ChoosePotionOrEnd())
        actual = play_combats(roots, policy, max_decisions=1, rng=random.Random(0))
        self.assertIsNone(actual[0].reward)
        self.assertIsNone(actual[1].won)
        self.assertIsNone(actual[1].reward)
        self.assertEqual(actual[1].decisions, 1)

        # Headers include completed rows, but potion features contain only compact model rows.
        from beam_search import beam_search

        finished = roots[0].clone()
        result = beam_search(finished, width=16, max_transitions=5000)
        self.assertTrue(result.won)
        for candidate in result.actions:
            finished.step(candidate)
        mixed = [finished, smoke]
        actual = play_combats(mixed, policy, max_decisions=1, rng=random.Random(0))
        self.assertTrue(actual[0].won)
        self.assertIsNone(actual[1].reward)


    def test_symbol_codes_are_scoped_to_each_batch_table(self) -> None:
        vocabulary = {"defend": 1, "strike": 3}

        def batch(symbols):
            return NumericBatch((1, symbols, {}, [], [0]))

        first = batch(["defend", "strike"])
        second = batch(["strike", "defend"])
        np.testing.assert_array_equal(first.codes(np.array([0, 1]), vocabulary), [1, 3])
        np.testing.assert_array_equal(second.codes(np.array([0, 1]), vocabulary), [3, 1])
        # A repeated call uses this batch's cached lookup, not the other batch's positions.
        np.testing.assert_array_equal(first.codes(np.array([1]), vocabulary), [3])
        with self.assertRaisesRegex(ValueError, "absent from encoder vocabulary"):
            first.codes(np.array([1]), {"defend": 1})



if __name__ == "__main__":
    unittest.main()
