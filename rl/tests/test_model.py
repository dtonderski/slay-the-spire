import unittest
from dataclasses import replace
from types import SimpleNamespace
from typing import ClassVar, cast, get_args
from unittest.mock import patch

import torch
from encoders.selection import SELECTION_TO_INDEX
from jaxtyping import Float
from model import CombatModel
from sts_sim import Action, CombatObservation, State
from sts_sim.observations.combat import Selection, SelectionKind, SelectionOption
from torch import Tensor


def action(kind: str, **slots: int | None) -> Action:
    """Public-action-shaped fixture; real native candidates are tested separately."""
    return cast(
        Action,
        SimpleNamespace(
            **{
                "kind": kind,
                "hand_slot": 0,
                "potion_slot": 0,
                "target_slot": None,
                "option_slot": 0,
                **slots,
            }
        ),
    )


class ModelTests(unittest.TestCase):
    observation: ClassVar[CombatObservation]
    actions: ClassVar[tuple[Action, ...]]

    @classmethod
    def setUpClass(cls) -> None:
        torch.set_num_threads(1)
        state = State.new("HUMAN1")
        for _ in range(100):
            decision = state.decision()
            if decision.observation.kind == "combat":
                cls.observation = decision.observation
                cls.actions = decision.actions
                return
            state.step(decision.actions[0])
        raise AssertionError("No combat reached")

    def setUp(self) -> None:
        self.model = CombatModel(d_model=16, action_dim=8, n_heads=4, n_layers=1).double()

    def observe(
        self, observation: CombatObservation
    ) -> tuple[Float[Tensor, " action_dim"], dict[str, Float[Tensor, "?n_rows ?feature_dim"]]]:
        queries, features = self.model.observation_encoder([observation])
        return queries[0], features[0]

    def test_live_decision_and_parameter_ownership(self) -> None:
        logits = self.model(self.observation, self.actions)
        self.assertEqual(logits.shape, (len(self.actions),))
        self.assertEqual(logits.dtype, torch.float64)
        self.assertTrue(torch.isfinite(logits).all())
        torch.testing.assert_close(self.model(self.observation, self.actions[::-1]), logits.flip(0))
        self.assertEqual(self.model(self.observation, ()).shape, (0,))
        parameters = list(self.model.named_parameters(remove_duplicate=False))
        self.assertEqual(len(parameters), len({id(parameter) for _, parameter in parameters}))
        for name in ("cards", "enemies", "potions", "relics"):
            table = getattr(self.model.observation_encoder, name).embedding
            self.assertEqual(
                [key for key, parameter in parameters if parameter is table.weight],
                [f"observation_encoder.{name}.embedding.weight"],
            )

    def test_shared_rows_and_gradients_from_both_paths(self) -> None:
        query, features = self.observe(self.observation)
        expected = self.model.observation_encoder.cards.tensorize(
            tuple(entry.card for entry in self.observation.screen.hand)
        )
        torch.testing.assert_close(features["hand"], expected)
        query.square().sum().backward()
        for name in ("cards", "enemies", "potions", "relics"):
            gradient = getattr(self.model.observation_encoder, name).embedding.weight.grad
            self.assertIsNotNone(gradient)
            self.assertGreater(gradient.abs().sum().item(), 0)
        self.model.zero_grad(set_to_none=True)
        _, features = self.observe(self.observation)
        actions = (action("play_hand_slot", target_slot=0), action("use_potion_slot", target_slot=0))
        # Action-only gradients must reach identity tables, not observation projections.
        with patch.object(
            self.model.observation_encoder.cards.embedding,
            "forward",
            side_effect=AssertionError("Card rows recomputed"),
        ):
            self.model.action_encoder(actions, features).sum().backward()
        for name in ("cards", "enemies", "potions"):
            gradient = getattr(self.model.observation_encoder, name).embedding.weight.grad
            self.assertIsNotNone(gradient)
            self.assertGreater(gradient.abs().sum().item(), 0)
        self.assertIsNone(self.model.observation_encoder.cards.projection.weight.grad)
        self.assertIsNone(self.model.observation_encoder.enemies.projection.weight.grad)

    def test_selection_and_all_action_kinds(self) -> None:
        card = self.observation.screen.hand[0].card
        selection = Selection(
            kind=get_args(SelectionKind)[0],
            options=(SelectionOption(slot=0, card=card), SelectionOption(slot=1, card=card)),
            selected_slots=(1,),
        )
        obs = replace(self.observation, screen=replace(self.observation.screen, selection=selection))
        query, features = self.observe(obs)
        self.assertEqual(features["selection"][:, -1].tolist(), [0, 1])
        kinds = (
            "play_hand_slot",
            "use_potion_slot",
            "discard_potion_slot",
            "toggle_visible_card",
            "choose_visible_option",
            "end_turn",
            "confirm_selection",
            "confirm_selection_without_retrieval",
            "skip_selection",
        )
        actions = tuple(action(kind, target_slot=0, option_slot=1) for kind in kinds)
        vectors = self.model.action_encoder(actions, features)
        self.assertEqual(vectors.shape, (9, 8))
        torch.testing.assert_close(self.model(obs, actions), vectors @ query)
        vectors.sum().backward()
        self.assertTrue(all(parameter.grad is not None for parameter in self.model.action_encoder.parameters()))
        # Every public selection kind remains encodable through the slice.
        for kind in SELECTION_TO_INDEX:
            changed = None if kind is None else replace(selection, kind=kind)
            options, context, tokens = self.model.observation_encoder.selection(
                [changed], self.model.observation_encoder.cards
            )
            self.assertEqual(context[0].shape, (1, 16))
            self.assertEqual(tokens[0].shape[0], 0 if kind is None else 2)
            self.assertEqual(options[0].shape[0], tokens[0].shape[0])
        with self.assertRaises(NotImplementedError):
            self.model.action_encoder((action("proceed"),), features)
        with self.assertRaises(ValueError):
            self.model.action_encoder((action("toggle_visible_card", option_slot=-1),), features)

    def test_stasis_uses_shared_card_encoder(self) -> None:
        cards = self.model.observation_encoder.cards
        card = self.observation.screen.hand[0].card
        monster = replace(self.observation.screen.monsters[0], stasis_card=card)
        features, _ = self.model.observation_encoder.enemies([(monster,)], cards)
        expected = cards.tensorize((card,))
        held_card = features[0][:, -expected.shape[1] :]
        torch.testing.assert_close(held_card, expected)
        held_card.sum().backward()
        gradient = cards.embedding.weight.grad
        assert gradient is not None
        self.assertGreater(gradient.abs().sum().item(), 0)

    def test_empty_groups_and_draw_permutation(self) -> None:
        screen = self.observation.screen
        query, _ = self.observe(self.observation)
        reversed_draw = replace(screen.draw_pile, cards=screen.draw_pile.cards[::-1])
        permuted = replace(self.observation, screen=replace(screen, draw_pile=reversed_draw))
        torch.testing.assert_close(self.observe(permuted)[0], query)
        empty_pile = replace(screen.draw_pile, cards=(), known_positions=())
        empty = replace(
            self.observation,
            screen=replace(
                screen,
                hand=(),
                monsters=(),
                draw_pile=empty_pile,
                discard_pile=empty_pile,
                exhaust_pile=empty_pile,
                selection=None,
            ),
            context=replace(self.observation.context, relics=(), potion_slots=()),
        )
        logits = self.model(empty, (action("end_turn"),))
        self.assertEqual(logits.shape, (1,))
        self.assertTrue(torch.isfinite(logits).all())


if __name__ == "__main__":
    unittest.main()
