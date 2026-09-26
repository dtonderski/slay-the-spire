import json
import unittest
from itertools import pairwise
from types import SimpleNamespace
from typing import cast

import numpy as np
import torch
from sts_sim import Action, State

from encoders.numeric import (
    ACTION_HAND,
    ACTION_KIND,
    ACTION_OPTION,
    ACTION_OWNER,
    ACTION_POTION,
    ACTION_TARGET,
    NumericBatch,
)
from model import CombatValueModel
from observation_encoder import width_groups


def action(kind: str, **slots: int | None) -> Action:
    return cast(
        Action,
        SimpleNamespace(
            kind=kind,
            hand_slot=slots.get("hand_slot", 0),
            potion_slot=slots.get("potion_slot", 0),
            target_slot=slots.get("target_slot"),
            option_slot=slots.get("option_slot", 0),
        ),
    )


def candidates_from_rows(groups: list[np.ndarray]) -> np.ndarray:
    pieces = []
    for owner, rows in enumerate(groups):
        pieces.append(
            np.column_stack(
                (
                    np.full(len(rows), owner, dtype=np.int64),
                    rows[:, ACTION_KIND],
                    rows[:, ACTION_HAND],
                    rows[:, ACTION_POTION],
                    rows[:, ACTION_OPTION],
                    rows[:, ACTION_TARGET],
                )
            )
        )
    return np.concatenate(pieces)


def combat(seed: int = 1, hp: int = 80, extra_strikes: int = 0) -> State:
    """Small synthetic test input, not a captured trace."""
    return State.from_synthetic_spec(
        json.dumps(
            {
                "seed": seed,
                "floor": 1,
                "kind": "normal",
                "encounter": "Cultist",
                "deck": [
                    {"key": key, "upgrades": 0}
                    for key in ["Strike_R"] * (5 + extra_strikes) + ["Defend_R"] * 4 + ["Bash"]
                ],
                "relics": ["Burning Blood"],
                "potions": [None, None, None],
                "hp": hp,
                "max_hp": 80,
                "gold": 99,
            }
        )
    )


class ModelTests(unittest.TestCase):
    def test_batch_padding_single_equivalence_and_gradients(self) -> None:
        torch.set_num_threads(1)
        states = [combat(1), combat(2)]
        batch = NumericBatch(State.numeric_decisions(states))
        groups = [batch.action_rows[batch.action_rows[:, ACTION_OWNER] == index] for index in range(2)]
        groups[0] = groups[0][:-2]
        actions = candidates_from_rows(groups)
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            model = CombatValueModel(d_model=16, action_dim=8, n_layers=1).to(device)
            logits, _, mask = model(batch, actions)
            self.assertEqual(tuple(logits.shape), (2, len(groups[1])))
            self.assertTrue(torch.isneginf(logits[~mask]).all())
            for i, state in enumerate(states):
                single = NumericBatch(State.numeric_decisions([state]))
                scores, _, _ = model(single, candidates_from_rows([groups[i]]))
                torch.testing.assert_close(logits[i, : len(groups[i])], scores[0], atol=1e-6, rtol=1e-5)
            logits[mask].square().sum().backward()
            self.assertTrue(all(torch.isfinite(p.grad).all() for p in model.parameters() if p.grad is not None))

    def test_value_model_numeric_batch_and_both_head_gradients(self) -> None:
        torch.set_num_threads(1)
        states = [combat(1), combat(2)]
        batch = NumericBatch(State.numeric_decisions(states))
        groups = [batch.action_rows[batch.action_rows[:, ACTION_OWNER] == index] for index in range(2)]
        groups[0] = groups[0][:-2]
        actions = candidates_from_rows(groups)
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            model = CombatValueModel(d_model=16, action_dim=8, n_layers=1).to(device)
            logits, values, mask = model(batch, actions)
            self.assertEqual(tuple(values.shape), (2, 1))
            self.assertEqual(tuple(logits.shape), tuple(mask.shape))
            self.assertTrue(torch.isneginf(logits[~mask]).all())
            for i, state in enumerate(states):
                single = NumericBatch(State.numeric_decisions([state]))
                scores, value, _ = model(single, candidates_from_rows([groups[i]]))
                torch.testing.assert_close(logits[i, : len(groups[i])], scores[0], atol=1e-6, rtol=1e-5)
                torch.testing.assert_close(values[i], value[0], atol=1e-6, rtol=1e-5)
            (logits[mask].square().sum() + values.square().sum()).backward()
            for head in (model.policy_head, model.value_head, model.observation_encoder.query):
                self.assertIsNotNone(head.weight.grad)
                assert head.weight.grad is not None
                self.assertTrue(torch.isfinite(head.weight.grad).all())
                self.assertGreater(head.weight.grad.abs().sum().item(), 0)
            with self.assertRaises(ValueError):
                model(batch, candidates_from_rows([groups[1]]))

    def test_width_groups_cover_sorted_rows(self) -> None:
        counts = np.array([30, 5, 12, 12, 40, 7, 9, 31, 6, 18])
        groups = width_groups(counts, 40, min_rows=2, growth=1.25)
        self.assertGreater(len(groups), 1)
        ordered = np.sort(counts)
        self.assertEqual(groups[0][0], 0)
        self.assertEqual(groups[-1][1], len(counts))
        for (_, end, _), (start, _, _) in pairwise(groups):
            self.assertEqual(end, start)
        for start, end, width in groups:
            self.assertGreaterEqual(end - start, 2)
            self.assertGreaterEqual(width, ordered[end - 1])
            self.assertLessEqual(width, 40)
        self.assertEqual(width_groups(counts, 40, min_rows=256, growth=1.25), [(0, len(counts), 40)])

    def test_width_grouped_transformer_matches_one_padded_call(self) -> None:
        torch.set_num_threads(1)
        # Different deck sizes give different draw-pile token counts.
        states = [combat(seed, extra_strikes=3 * seed) for seed in range(1, 13)]
        batch = NumericBatch(State.numeric_decisions(states))
        groups = [batch.action_rows[batch.action_rows[:, ACTION_OWNER] == index] for index in range(len(states))]
        actions = candidates_from_rows(groups)
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            model = CombatValueModel(d_model=16, action_dim=8, n_layers=2).to(device)
            outputs = []
            for min_rows in (10**9, 2):
                model.observation_encoder.width_group_min_rows = min_rows
                model.zero_grad()
                logits, values, mask = model(batch, actions)
                (logits[mask].square().sum() + values.square().sum()).backward()
                grads = [p.grad.clone() for p in model.parameters() if p.grad is not None]
                outputs.append((logits.detach(), values.detach(), grads))
            counts = model.observation_encoder.prepare_numeric(batch)[3]
            self.assertGreater(len(width_groups(counts, int(counts.max()), 2, 1.25)), 1)
            (single_logits, single_values, single_grads), (logits, values, grads) = outputs
            torch.testing.assert_close(logits, single_logits, atol=1e-5, rtol=1e-5)
            torch.testing.assert_close(values, single_values, atol=1e-5, rtol=1e-5)
            for grouped, single in zip(grads, single_grads, strict=True):
                torch.testing.assert_close(grouped, single, atol=1e-5, rtol=1e-4)

    def test_empty_candidate_rejected(self) -> None:
        batch = NumericBatch(State.numeric_decisions([combat()]))
        with self.assertRaises(ValueError):
            CombatValueModel()(batch, np.zeros((0, 6), dtype=np.int64))


if __name__ == "__main__":
    unittest.main()
