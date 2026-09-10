import unittest
from dataclasses import replace
from unittest.mock import patch

import torch
from encoders.cards import CARD_EMBEDDING_DIM, CARD_FEATURE_DIM, CardEncoder
from encoders.selection import SelectionEncoder
from observation_encoder import ObservationEncoder
from sts_sim import CombatObservation
from sts_sim.observations import PotionSlot
from sts_sim.observations.combat import Selection, SelectionOption
from train import first_combat


def token_count(obs: CombatObservation) -> int:
    screen = obs.screen
    return (
        3  # Summary, player, selection context.
        + len(screen.hand)
        + len(screen.draw_pile.cards)
        + len(screen.discard_pile.cards)
        + len(screen.exhaust_pile.cards)
        + len(screen.monsters)
        + len(obs.context.relics)
        + len(obs.context.potion_slots)
        + (len(screen.selection.options) if screen.selection else 0)
    )


class ObservationBatchTests(unittest.TestCase):
    def setUp(self) -> None:
        torch.set_num_threads(1)
        obs = first_combat("HUMAN1", 0).decision().observation
        assert obs.kind == "combat"
        hand = obs.screen.hand
        selection = Selection(
            kind="discovery_reward",
            options=(SelectionOption(slot=0, card=hand[0].card), SelectionOption(slot=1, card=hand[1].card)),
            selected_slots=(1,),
        )
        self.a = replace(obs, context=replace(obs.context, relics=(), potion_slots=()))
        self.b = replace(
            obs,
            screen=replace(
                obs.screen,
                hand=hand + (replace(hand[0], slot=len(hand)),),
                monsters=obs.screen.monsters
                + (
                    replace(
                        obs.screen.monsters[0], slot=len(obs.screen.monsters), alive=False, stasis_card=hand[0].card
                    ),
                ),
                selection=selection,
            ),
            context=replace(obs.context, potion_slots=(PotionSlot(slot=0, content_key=None),)),
        )

    def test_card_flatten_and_split_with_empty_entries(self) -> None:
        encoder = CardEncoder(d_model=2)
        card = self.a.screen.hand[0].card
        one, two, three = [replace(card, cost=cost) for cost in (1, 2, 3)]
        # Make token values depend only on cost: [cost + 100, cost * 10 - 100].
        with torch.no_grad():
            encoder.projection.weight.zero_()
            encoder.projection.weight[:, CARD_EMBEDDING_DIM] = torch.tensor([1.0, 10.0])
            encoder.projection.bias.copy_(torch.tensor([100.0, -100.0]))
        raw, tokens = encoder([(), (two, one), (), (three,), ()])
        self.assertEqual(len(raw), 5)
        self.assertEqual(len(tokens), 5)
        self.assertEqual([rows[:, CARD_EMBEDDING_DIM].tolist() for rows in raw], [[], [2, 1], [], [3], []])
        self.assertEqual([rows.tolist() for rows in tokens], [[], [[102, -80], [101, -90]], [], [[103, -70]], []])
        for index in (0, 2, 4):
            self.assertEqual(raw[index].shape, (0, CARD_FEATURE_DIM))
            self.assertEqual(tokens[index].shape, (0, 2))

    def test_selection_flags_stay_with_their_observation(self) -> None:
        cards = CardEncoder(d_model=2)
        encoder = SelectionEncoder(d_model=2)
        card = self.a.screen.hand[0].card
        options = tuple(SelectionOption(slot=i, card=replace(card, cost=i + 1)) for i in range(2))
        first = Selection(kind="discovery_reward", options=options, selected_slots=(1,))
        last = Selection(kind="discovery_reward", options=options[:1], selected_slots=(0,))
        # Zero card tokens; only the selected bit contributes to option tokens.
        with torch.no_grad():
            cards.projection.weight.zero_()
            cards.projection.bias.zero_()
            encoder.selected_projection.weight.fill_(1)
        raw, context, tokens = encoder([first, None, last], cards)
        self.assertEqual([rows[:, CARD_EMBEDDING_DIM].tolist() for rows in raw], [[1, 2], [], [1]])
        self.assertEqual([rows[:, -1].tolist() for rows in raw], [[0, 1], [], [1]])
        self.assertEqual([rows.tolist() for rows in tokens], [[[0, 0], [1, 1]], [], [[1, 1]]])
        self.assertEqual([tuple(rows.shape) for rows in context], [(1, 2)] * 3)

    def test_exact_token_order_and_padding(self) -> None:
        encoder = ObservationEncoder(d_model=4, n_heads=1, n_layers=1)
        # Recognizable tokens: summary=-1, player=10, cards=cost, enemy=9, selection context=20.
        with torch.no_grad():
            for parameter in encoder.parameters():
                parameter.zero_()
            encoder.summary_embedding.weight.fill_(-1)
            encoder.player.projection.bias.fill_(10)
            encoder.cards.projection.weight[:, CARD_EMBEDDING_DIM] = 1
            encoder.enemies.projection.bias.fill_(9)
            encoder.selection.context_projection.bias.fill_(20)
        entry = self.a.screen.hand[0]
        pile = replace(self.a.screen.draw_pile, cards=(), known_positions=())
        screen = replace(
            self.a.screen,
            hand=(
                replace(entry, slot=0, card=replace(entry.card, cost=1)),
                replace(entry, slot=1, card=replace(entry.card, cost=2)),
            ),
            monsters=self.a.screen.monsters[:1],
            draw_pile=pile,
            discard_pile=pile,
            exhaust_pile=pile,
            selection=None,
        )
        a = replace(self.a, screen=screen)
        b = replace(
            a, screen=replace(screen, hand=(replace(entry, slot=0, card=replace(entry.card, cost=3)),), monsters=())
        )
        tokens, mask, features = encoder.prepare_batch([a, b])
        expected = torch.tensor([[-1, 10, 1, 2, 9, 20], [-1, 10, 3, 20, 0, 0]], dtype=tokens.dtype)
        torch.testing.assert_close(tokens, expected.unsqueeze(-1).expand(2, 6, 4))
        self.assertEqual(mask.tolist(), [[False] * 6, [False] * 4 + [True] * 2])
        self.assertEqual([item["hand"][:, CARD_EMBEDDING_DIM].tolist() for item in features], [[1, 2], [3]])

    def test_slices_return_unpadded_lists_and_project_once(self) -> None:
        encoder = ObservationEncoder().double()
        batches = [tuple(entry.card for entry in obs.screen.hand) for obs in (self.a, self.b)]
        with patch.object(encoder.cards.projection, "forward", wraps=encoder.cards.projection.forward) as projection:
            features, tokens = encoder.cards(batches)
            self.assertEqual(projection.call_count, 1)
            self.assertEqual(projection.call_args.args[0].shape, (sum(map(len, batches)), CARD_FEATURE_DIM))
        for index, cards in enumerate(batches):
            self.assertEqual(tokens[index].shape, (len(cards), 64))
            torch.testing.assert_close(features[index], encoder.cards.tensorize(cards))
            torch.testing.assert_close(tokens[index], encoder.cards.projection(features[index]))
        for module, batch, extra in (
            (encoder.enemies, [self.a.screen.monsters, self.b.screen.monsters], (encoder.cards,)),
            (encoder.potions, [self.a.context.potion_slots, self.b.context.potion_slots], ()),
            (encoder.relics, [self.a.context.relics, self.b.context.relics], ()),
            (encoder.player, [self.a, self.b], ()),
        ):
            with patch.object(module.projection, "forward", wraps=module.projection.forward) as projection:
                raw, projected = module(batch, *extra)
                self.assertEqual(projection.call_count, 1)
            for index, item in enumerate(batch):
                single_raw, single_tokens = module([item], *extra)
                torch.testing.assert_close(raw[index], single_raw[0])
                torch.testing.assert_close(projected[index], single_tokens[0])
        options, context, tokens = encoder.selection([None, self.b.screen.selection], encoder.cards)
        self.assertEqual(options[0].shape, (0, CARD_FEATURE_DIM + 1))
        self.assertEqual(options[1][:, -1].tolist(), [0, 1])
        for index, selection in enumerate((None, self.b.screen.selection)):
            single = encoder.selection([selection], encoder.cards)
            for batched, separate in zip((options, context, tokens), single):
                torch.testing.assert_close(batched[index], separate[0])

    def test_concatenate_before_padding(self) -> None:
        encoder = ObservationEncoder().double()
        tokens, mask, features = encoder.prepare_batch([self.a, self.b])
        lengths = [token_count(self.a), token_count(self.b)]
        self.assertEqual(tokens.shape, (2, max(lengths), 64))
        self.assertEqual(mask.dtype, torch.bool)
        for index, obs in enumerate((self.a, self.b)):
            self.assertEqual(mask[index].tolist(), [False] * lengths[index] + [True] * (max(lengths) - lengths[index]))
            single_tokens, _, single_features = encoder.prepare_batch([obs])
            torch.testing.assert_close(tokens[index, : lengths[index]], single_tokens[0])
            self.assertEqual(torch.count_nonzero(tokens[index, lengths[index] :]), 0)
            for name in features[index]:
                torch.testing.assert_close(features[index][name], single_features[0][name])
        self.assertEqual(len(features[0]["potions"]), 0)
        self.assertEqual(len(features[1]["potions"]), 1)  # Real empty slot, not padding.
        self.assertEqual(len(features[1]["enemies"]), len(self.b.screen.monsters))  # Includes dead enemy.

        # Equal total lengths but different group lengths need no padding.
        more_hand = replace(self.a, screen=replace(self.a.screen, hand=self.a.screen.hand + (self.a.screen.hand[0],)))
        more_enemy = replace(
            self.a, screen=replace(self.a.screen, monsters=self.a.screen.monsters + (self.a.screen.monsters[0],))
        )
        equal_tokens, equal_mask, _ = encoder.prepare_batch([more_hand, more_enemy])
        self.assertEqual(equal_tokens.shape[1], token_count(more_hand))
        self.assertFalse(equal_mask.any())

    def test_queries_gradients_and_batch_independence(self) -> None:
        encoder = ObservationEncoder(d_model=16, action_dim=8).double()
        observations = [self.a, self.b]
        queries, features = encoder(observations)
        single = [encoder([obs]) for obs in observations]
        torch.testing.assert_close(queries, torch.cat([q for q, _ in single]))
        torch.testing.assert_close(encoder(observations[::-1])[0], queries.flip(0))
        queries.square().sum().backward()
        gradients = {name: p.grad.clone() for name, p in encoder.named_parameters() if p.grad is not None}
        encoder.zero_grad(set_to_none=True)
        for query, _ in single:
            query.square().sum().backward()
        for name, parameter in encoder.named_parameters():
            if name in gradients:
                torch.testing.assert_close(parameter.grad, gradients[name])
        encoder.zero_grad(set_to_none=True)
        _, features = encoder(observations)
        torch.stack([rows.sum() for item in features for rows in item.values()]).sum().backward()
        for module in (encoder.cards, encoder.enemies, encoder.potions):
            self.assertIsNotNone(module.embedding.weight.grad)
            self.assertIsNone(module.projection.weight.grad)

    def test_padding_is_ignored(self) -> None:
        encoder = ObservationEncoder().double()
        tokens, mask, _ = encoder.prepare_batch([self.a, self.b])
        expected = encoder.transformer(tokens, src_key_padding_mask=mask)[:, 0]
        changed = tokens.clone()
        changed[mask] = 10000
        actual = encoder.transformer(changed, src_key_padding_mask=mask)[:, 0]
        torch.testing.assert_close(actual, expected)

    def test_empty_groups_and_empty_batch(self) -> None:
        encoder = ObservationEncoder()
        raw, tokens = encoder.cards([(), ()])
        self.assertEqual(len(raw), 2)
        self.assertEqual(raw[0].shape, (0, CARD_FEATURE_DIM))
        self.assertEqual(tokens[1].shape, (0, 64))
        pile = replace(self.a.screen.draw_pile, cards=(), known_positions=())
        empty = replace(
            self.a,
            screen=replace(self.a.screen, hand=(), monsters=(), draw_pile=pile, discard_pile=pile, exhaust_pile=pile),
        )
        tokens, mask, _ = encoder.prepare_batch([empty, self.b])
        self.assertEqual(token_count(empty), 3)
        self.assertEqual(mask[0, :3].tolist(), [False] * 3)
        self.assertTrue(mask[0, 3:].all())
        self.assertTrue(torch.isfinite(encoder([empty, self.b])[0]).all())
        with self.assertRaises(ValueError):
            encoder([])

    @unittest.skipUnless(torch.cuda.is_available(), "CUDA unavailable")
    def test_cuda(self) -> None:
        cpu = ObservationEncoder()
        gpu = ObservationEncoder().cuda()
        gpu.load_state_dict(cpu.state_dict())
        queries, features = gpu([self.a, self.b])
        torch.testing.assert_close(queries.cpu(), cpu([self.a, self.b])[0], atol=1e-5, rtol=1e-5)
        self.assertTrue(all(rows.device.type == "cuda" for item in features for rows in item.values()))
        queries.square().sum().backward()
        self.assertIsNotNone(gpu.cards.embedding.weight.grad)


if __name__ == "__main__":
    unittest.main()
