"""Numeric export/refinement checks; fixtures are not real-game parity evidence."""

import copy
import random
import unittest
from dataclasses import replace
from typing import cast

import numpy as np
import torch
from encoders.numeric import NumericBatch
from model import CombatModel
from numeric_reference import CATEGORICAL, reference_batch, semantic_tables
from sts_sim import CounterKey, PowerKey, State
from sts_sim.observations import (
    Counter,
    Power,
    Selection,
    SelectionOption,
    VisibleIntent,
)
from test_model import action
from train import episode_loss, first_combat, play_combats
from train_roots import collect_roots


class NumericObservationTests(unittest.TestCase):
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
        state = first_combat("HUMAN1", 0)
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
        decision = first_combat("HUMAN1", 0).decision()
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
                    left = CombatModel(d_model=16, action_dim=8, n_layers=1).to(device=device, dtype=dtype)
                    right = copy.deepcopy(left)
                    x = left.observation_encoder.prepare_batch([a, b])
                    y = right.observation_encoder.prepare_batch(raw)
                    torch.testing.assert_close(x[0], y[0], rtol=0, atol=0)
                    self.assertTrue(torch.equal(x[1], y[1]))
                    for xf, yf in zip(x[2], y[2]):
                        for name in xf:
                            torch.testing.assert_close(xf[name], yf[name], rtol=0, atol=0)
                    logits, mask = left([a, b], actions)
                    numeric_logits, numeric_mask = right(raw, actions)
                    torch.testing.assert_close(logits, numeric_logits, rtol=0, atol=0)
                    self.assertTrue(torch.equal(mask, numeric_mask))
                    logits[mask].square().sum().backward()
                    numeric_logits[numeric_mask].square().sum().backward()
                    for lp, rp in zip(left.parameters(), right.parameters()):
                        self.assertEqual(lp.grad is None, rp.grad is None)
                        if lp.grad is not None:
                            torch.testing.assert_close(lp.grad, rp.grad, rtol=1e-6, atol=1e-7)

    def test_dictionary_numbers_are_not_model_features(self) -> None:
        state = first_combat("HUMAN1", 0)
        batch = NumericBatch(State.numeric_decisions([state]))
        tables = {}
        for name, values in batch.tables.items():
            values = values.copy()
            for column in CATEGORICAL.get(name, ()):
                values[:, column] = np.where(values[:, column] < 0, -1, len(batch.symbols) - 1 - values[:, column])
            tables[name] = (values.shape[1], values.tobytes())
        permuted = NumericBatch((1, batch.symbols[::-1], tables, batch.actions, batch.model_rows))
        model = CombatModel()
        a, mask_a = model(batch, [tuple(batch.actions[0])])
        b, mask_b = model(permuted, [tuple(batch.actions[0])])
        torch.testing.assert_close(a, b, rtol=0, atol=0)
        self.assertTrue(torch.equal(mask_a, mask_b))

    def test_generated_smoke_is_excluded_in_both_rollout_paths(self) -> None:
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
                return logits, logits.isfinite()

        roots = [first_combat("HUMAN1", 0), smoke]
        policy = cast(CombatModel, ChoosePotionOrEnd())
        expected = play_combats(roots, policy, max_decisions=1, rng=random.Random(0))
        actual = play_combats(roots, policy, max_decisions=1, rng=random.Random(0), numeric=True)
        self.assertEqual(expected, actual)
        self.assertIsNone(actual[0].reward)
        self.assertFalse(actual[1].escaped)
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
        expected = play_combats(mixed, policy, max_decisions=1, rng=random.Random(0))
        actual = play_combats(mixed, policy, max_decisions=1, rng=random.Random(0), numeric=True)
        self.assertEqual(expected, actual)
        self.assertTrue(actual[0].won)
        self.assertIsNone(actual[1].reward)

    def test_native_rollout_loss_and_gradients(self) -> None:
        roots = [r.state for r in collect_roots(["2000000", "2000001", "2000002"], 3, 123)[0]]
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            torch.manual_seed(123)
            model = CombatModel().to(device)
            results, gradients = [], []
            for numeric in (False, True):
                torch.manual_seed(30000)
                model.zero_grad(set_to_none=True)
                episodes = play_combats(
                    roots, model, max_decisions=128, training=True, rng=random.Random(0), numeric=numeric
                )
                results.append(
                    [
                        (e.reward, e.won, e.hp, e.decisions, e.escaped, [float(p.detach()) for p in e.log_probs])
                        for e in episodes
                    ]
                )
                torch.stack(
                    [episode_loss(e, 0.01) for e in episodes if e.reward is not None and e.log_probs]
                ).mean().backward()
                gradients.append([p.grad.clone() if p.grad is not None else None for p in model.parameters()])
            self.assertEqual(results[0], results[1])
            for a, b in zip(*gradients):
                if a is None:
                    self.assertIsNone(b)
                else:
                    torch.testing.assert_close(a, b, rtol=1e-6, atol=1e-7)


if __name__ == "__main__":
    unittest.main()
