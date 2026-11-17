"""Token macro encoder contracts; not real-game parity evidence."""

import random
import unittest
from dataclasses import replace

import numpy as np
import torch
from run_training.contracts import PolicyAction, controller
from run_training.restrictions import macro_eligibility
from run_training.token_model import TokenInput, TokenMacroModel, encode_tokens
from sts_sim import CounterKey, State
from sts_sim.observations.common import Counter


def macro_decisions(seed: str, limit: int):
    """Public macro decisions from a natural run driven by a fixed random policy."""
    rng = random.Random(seed)
    state = State.new(seed, ascension=0)
    decision = state.decision()
    found = []
    while len(found) < limit and controller(decision) != "terminal":
        if controller(decision) == "macro":
            found.append(
                (
                    decision.observation,
                    tuple(PolicyAction.from_action(a) for a in decision.actions),
                )
            )
        decision = state.step(rng.choice(decision.actions))
    return found


def first_decision(predicate):
    """The first random-policy macro decision, over a few seeds, that matches."""
    for i in range(30):
        for observation, actions in macro_decisions(f"FIND{i}", 10_000):
            if predicate(observation, actions):
                return observation, actions
    raise AssertionError("No matching decision")


def changes(before: TokenInput, after: TokenInput) -> bool:
    return before.ids.shape != after.ids.shape or not (
        np.array_equal(before.ids, after.ids)
        and np.array_equal(before.numbers, after.numbers)
    )


def trained_like_model() -> TokenMacroModel:
    torch.manual_seed(7)
    model = TokenMacroModel()
    # Zero heads make every output equal; give them weights so tests can see inputs.
    torch.nn.init.normal_(model.policy.weight)
    torch.nn.init.normal_(model.value.weight)
    return model.eval()


def action(kind: str, card_slot: int | None = None) -> PolicyAction:
    return PolicyAction(kind, None, None, None, None, card_slot, None, None, None)


class TokenModelTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.decisions = macro_decisions("TOKENS1", 40) + macro_decisions("TOKENS2", 40)
        cls.model = trained_like_model()

    def test_fresh_model_is_uniform_with_zero_value(self) -> None:
        observation, actions = self.decisions[0]
        logits, value = TokenMacroModel()(encode_tokens(observation, actions))
        self.assertEqual(len(logits), len(actions))
        self.assertTrue(torch.equal(logits, torch.zeros_like(logits)))
        self.assertEqual(float(value.detach()), 0.0)

    def test_deck_order_does_not_change_outputs(self) -> None:
        checked = 0
        for observation, actions in self.decisions:
            deck = observation.context.deck
            # Candidates that point at a deck slot would move with the shuffle.
            if len(set(deck)) < 2 or any(a.card_slot is not None for a in actions):
                continue
            shuffled = replace(
                observation, context=replace(observation.context, deck=deck[::-1])
            )
            with torch.no_grad():
                logits, value = self.model(encode_tokens(observation, actions))
                other_logits, other_value = self.model(encode_tokens(shuffled, actions))
            self.assertTrue(torch.allclose(logits, other_logits, atol=1e-5))
            self.assertTrue(torch.allclose(value, other_value, atol=1e-5))
            checked += 1
        self.assertGreater(checked, 10)

    def test_deck_contents_change_outputs(self) -> None:
        observation, actions = next(
            (o, a) for o, a in self.decisions if len(a) > 1 and len(o.context.deck) > 3
        )
        smaller = replace(
            observation,
            context=replace(observation.context, deck=observation.context.deck[:3]),
        )
        with torch.no_grad():
            logits, _ = self.model(encode_tokens(observation, actions))
            other, _ = self.model(encode_tokens(smaller, actions))
        self.assertFalse(torch.allclose(logits, other, atol=1e-4))

    def test_batch_matches_single_rows(self) -> None:
        inputs = [encode_tokens(o, a) for o, a in self.decisions]
        with torch.no_grad():
            logits, values = self.model.forward_batch(self.model.pack(inputs))
            for i, row in enumerate(inputs):
                single_logits, single_value = self.model(row)
                self.assertTrue(
                    torch.allclose(
                        logits[i, : row.count],
                        single_logits,
                        atol=self.model.recompute_atol,
                        rtol=self.model.recompute_rtol,
                    )
                )
                self.assertTrue(torch.isinf(logits[i, row.count :]).all())
                self.assertTrue(
                    torch.allclose(
                        values[i], single_value, atol=self.model.recompute_atol
                    )
                )

    def test_unopened_reward_cards_inform_the_open_candidate(self) -> None:
        observation, actions = next(
            (o, a)
            for o, a in self.decisions
            if o.kind == "reward"
            and o.screen.cards
            and any(x.kind == "open_card_reward" for x in a)
        )
        opened = next(i for i, a in enumerate(actions) if a.kind == "open_card_reward")
        cards = observation.screen.cards
        other = next(
            c
            for c in observation.context.deck
            if c.content_key != cards[0].card.content_key
        )
        changed = replace(
            observation,
            screen=replace(
                observation.screen,
                cards=(replace(cards[0], card=other), *cards[1:]),
            ),
        )
        with torch.no_grad():
            logits, _ = self.model(encode_tokens(observation, actions))
            other_logits, _ = self.model(encode_tokens(changed, actions))
            # Deck order still does not matter with reward cards in the sequence.
            shuffled = replace(
                observation,
                context=replace(
                    observation.context, deck=observation.context.deck[::-1]
                ),
            )
            shuffled_logits, _ = self.model(encode_tokens(shuffled, actions))
        self.assertNotAlmostEqual(
            float(logits[opened]), float(other_logits[opened]), places=4
        )
        self.assertTrue(torch.allclose(logits, shuffled_logits, atol=1e-5))

    def test_card_cost_and_flags_are_encoded(self) -> None:
        observation, actions = self.decisions[0]
        deck = observation.context.deck
        row = encode_tokens(observation, actions)
        for change in (
            {"cost": deck[0].cost + 1},
            {"cost_is_modified": not deck[0].cost_is_modified},
            {"temporary": not deck[0].temporary},
            {"dynamic": replace(deck[0].dynamic, rampage_damage_bonus=5)},
        ):
            other = replace(
                observation,
                context=replace(
                    observation.context,
                    deck=(replace(deck[0], **change), *deck[1:]),
                ),
            )
            self.assertTrue(changes(row, encode_tokens(other, actions)), change)

    def test_relic_counters_are_encoded(self) -> None:
        observation, actions = self.decisions[0]
        relics = observation.context.relics
        counted = replace(relics[0], state=(Counter(key=CounterKey.LIFTS, value=2),))
        rows = [
            encode_tokens(
                replace(
                    observation,
                    context=replace(observation.context, relics=(relic, *relics[1:])),
                ),
                actions,
            )
            for relic in (
                counted,
                replace(counted, state=(replace(counted.state[0], value=3),)),
            )
        ]
        self.assertTrue(changes(encode_tokens(observation, actions), rows[0]))
        self.assertTrue(changes(rows[0], rows[1]))

    def test_every_unsold_shop_offer_is_visible(self) -> None:
        observation, actions = first_decision(
            lambda o, a: (
                o.kind == "shop"
                and o.screen.merchant_open
                and any(x.kind == "open_shop_remove" for x in a)
                and any(c.price > o.context.gold for c in o.screen.cards)
            )
        )
        screen = observation.screen
        offers = [
            o for o in (*screen.cards, *screen.relics, *screen.potions) if not o.sold
        ]
        row = encode_tokens(observation, actions)
        # One context token per unsold offer, affordable or not.
        sold = replace(
            screen,
            cards=tuple(replace(o, sold=True) for o in screen.cards),
            relics=tuple(replace(o, sold=True) for o in screen.relics),
            potions=tuple(replace(o, sold=True) for o in screen.potions),
        )
        all_sold = encode_tokens(replace(observation, screen=sold), actions)
        self.assertEqual(len(row.ids) - len(all_sold.ids), len(offers))
        richer = replace(observation, context=replace(observation.context, gold=10_000))
        self.assertTrue(changes(row, encode_tokens(richer, actions)))
        cheaper = replace(screen, remove_cost=screen.remove_cost - 1)
        remove = next(i for i, a in enumerate(actions) if a.kind == "open_shop_remove")
        other = encode_tokens(replace(observation, screen=cheaper), actions)
        self.assertFalse(
            np.array_equal(
                row.numbers[len(row.ids) - row.count + remove],
                other.numbers[len(other.ids) - other.count + remove],
            )
        )

    def test_summary_carries_card_reward_flow(self) -> None:
        observation, actions = next(
            (o, a) for o, a in self.decisions if o.kind == "reward"
        )
        flows = [
            encode_tokens(
                replace(
                    observation,
                    screen=replace(observation.screen, card_reward_flow=flow),
                ),
                actions,
            ).ids[0]
            for flow in ("none", "pending", "active")
        ]
        self.assertEqual(len({tuple(ids) for ids in flows}), 3)

    def test_grid_toggle_candidates_carry_selection(self) -> None:
        observation, actions = first_decision(
            lambda o, a: (
                o.kind == "grid" and any(x.kind == "toggle_grid_card" for x in a)
            )
        )
        toggle = next(i for i, a in enumerate(actions) if a.kind == "toggle_grid_card")
        slot = actions[toggle].card_slot
        screen = observation.screen
        selected = replace(
            screen,
            selected_indices=tuple(i for i in screen.selected_indices if i != slot)
            + (slot,),
        )
        unselected = replace(
            screen,
            selected=None,
            selected_indices=tuple(i for i in screen.selected_indices if i != slot),
        )
        self.assertTrue(
            changes(
                encode_tokens(replace(observation, screen=selected), actions),
                encode_tokens(replace(observation, screen=unselected), actions),
            )
        )

    def test_temperature_divides_logits(self) -> None:
        observation, actions = next((o, a) for o, a in self.decisions if len(a) > 1)
        row = encode_tokens(observation, actions)
        with torch.no_grad():
            logits, _ = self.model(row)
            tempered, _ = self.model(replace(row, temperature=4.0))
        self.assertTrue(torch.allclose(tempered, logits / 4, atol=1e-6))

    def test_direct_smith_excludes_only_the_upgrade_screen_detour(self) -> None:
        observation, _ = self.decisions[0]
        direct = (
            action("rest_heal"),
            action("rest_open_smith"),
            action("rest_smith", 0),
        )
        self.assertEqual(
            macro_eligibility(
                observation, direct, avoid_prismatic=False, direct_smith=True
            ),
            (True, False, True),
        )
        # Without a direct upgrade on offer the screen stays reachable.
        self.assertEqual(
            macro_eligibility(
                observation, direct[:2], avoid_prismatic=False, direct_smith=True
            ),
            (),
        )
        self.assertEqual(
            macro_eligibility(observation, direct, avoid_prismatic=False), ()
        )


if __name__ == "__main__":
    unittest.main()
