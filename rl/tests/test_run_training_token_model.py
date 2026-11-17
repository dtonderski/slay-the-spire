"""Token macro encoder contracts; not real-game parity evidence."""

import random
import unittest
from dataclasses import replace

import torch
from run_training.contracts import PolicyAction, controller
from run_training.restrictions import macro_eligibility
from run_training.token_model import TokenMacroModel, encode_tokens
from sts_sim import State


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
