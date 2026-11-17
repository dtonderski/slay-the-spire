"""Transport/learning invariants, not historical gameplay parity evidence."""

import argparse
import copy
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

import torch
from pretraining.train_run_prior import TrainingAdapter, slate_from_example
from run_training.contracts import PolicyAction
from run_training.item_prior import (
    ITEM_PRIOR_FORMAT,
    ITEM_PRIOR_PROTOCOL,
    ITEM_PRIOR_WIDTH,
    ItemPrior,
    ItemSlate,
    RewardPriorInput,
    live_prior_input,
)
from run_training.model import ChoiceMacroModel, MacroInput, encode
from run_training.trainer import make_model
from sts_sim import (
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
    CardKey,
    CardSlot,
    RelicKey,
    State,
)
from sts_sim.observations.screens import RewardObservation, RewardScreen

ITEMS = ("card:Anger+0", "card:Bash+1", "relic:Astrolabe", "relic:Runic Pyramid")


def action(kind, reward_slot=None):
    return PolicyAction(kind, None, None, None, None, None, None, reward_slot, None)


def reward():
    context = State.new("1").observation().context
    cards = tuple(
        CardSlot(
            slot=i, card=replace(context.deck[0], content_key=key, upgrade_level=level)
        )
        for i, (key, level) in enumerate(((CardKey.ANGER, 0), (CardKey.BASH_PLUS, 1)))
    )
    screen = RewardScreen(
        cards=cards,
        queued_card_rewards=(),
        gold_offer=0,
        stolen_gold_offer=0,
        potion_offer=None,
        potion_offers=(),
        relic_offer=None,
        relic_offers=(),
        sapphire_key_relic_slot=None,
        emerald_key_offer=False,
        boss_relic_choices=(RelicKey("Astrolabe"), RelicKey("Runic Pyramid")),
        card_reward_flow="active",
    )
    return RewardObservation(
        schema_version=FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
        phase="reward",
        kind="reward",
        context=replace(context, floor=3),
        screen=screen,
    )


def example(kind="card"):
    candidates = list(ITEMS[:2] if kind == "card" else ITEMS[2:])
    return {
        "kind": kind,
        "task": "conditional_on_taking_item",
        "context": {"ascension": 0, "floor": 3},
        "candidates": candidates,
        "label": candidates[0],
    }


def arguments(path, initialization="pretrained"):
    return argparse.Namespace(
        encoder="choice",
        item_prior=path,
        item_prior_init=initialization,
        resume_from=None,
        warm_start=None,
        model_width=8,
    )


class RunItemPriorTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        torch.manual_seed(17)
        self.head = ItemPrior(ITEMS)
        with torch.no_grad():
            self.head.bias.weight[1] = 1.2
            self.head.bias.weight[3] = -0.7

    def test_live_and_offline_identities_match_with_no_state_invention(self):
        observation = reward()
        actions = tuple(
            action(kind=kind, reward_slot=i)
            for kind in ("take_card_reward", "choose_boss_relic_reward")
            for i in range(2)
        )
        live = live_prior_input(observation, actions)
        assert live is not None
        self.assertEqual(live.items, ITEMS)
        self.assertEqual(live.ascension, 0)
        self.assertEqual(live.floor, 3)
        for kind in ("card", "relic"):
            row = example(kind)
            before = slate_from_example(row)
            changed = copy.deepcopy(row)
            changed.update(
                label=changed["candidates"][-1],
                seed=999,
                outcome="win",
                master_deck=["future"],
            )
            changed["context"].update(
                history={"invented": 999}, hp=999, deck=["future"], boss_ordinal=99
            )
            self.assertEqual(before, slate_from_example(changed))
        boss = example("relic")
        del boss["context"]["floor"]
        self.assertIsNone(slate_from_example(boss).floor)
        with self.assertRaises(ValueError):
            slate_from_example({**example(), "task": "hypothetical_route"})

    def test_group_mass_and_unsupervised_actions_are_preserved(self):
        base = torch.tensor([0.3, -0.7, 0.1, 0.4, -0.5, 0.8], requires_grad=True)
        public = RewardPriorInput(
            0, 3, (ITEMS[0], ITEMS[1], None, ITEMS[2], ITEMS[3], None)
        )
        result = self.head.combine(base, (public,), (6,))
        before, after = base.softmax(0), result.softmax(0)
        torch.testing.assert_close(before[[2, 5]], after[[2, 5]])
        torch.testing.assert_close(before[:2].sum(), after[:2].sum())
        torch.testing.assert_close(before[3:5].sum(), after[3:5].sum())
        self.assertGreater(float(after[0].detach()), float(before[0].detach()))
        torch.testing.assert_close(result[[2, 5]], base[[2, 5]], rtol=0, atol=0)
        # Observing Skip/Bowl gives no gradient to a merely conditional item head.
        (-after[2].log()).backward()
        for parameter in self.head.parameters():
            if parameter.grad is not None:
                torch.testing.assert_close(
                    parameter.grad, torch.zeros_like(parameter.grad), atol=1e-6, rtol=0
                )

    def test_unknown_duplicate_and_singleton_slates_are_neutral(self):
        for items in (
            (ITEMS[0], "card:unseen+0", None),
            (ITEMS[0], ITEMS[0], None),
            (ITEMS[0], None, None),
        ):
            base = torch.tensor([0.2, -0.8, 0.4])
            result = self.head.combine(base, (RewardPriorInput(0, 1, items),), (3,))
            torch.testing.assert_close(result, base, rtol=0, atol=1e-7)
        logits = self.head([ItemSlate("card", 0, 1, (ITEMS[0], "card:unseen+0"))])
        self.assertEqual(logits.tolist(), [[0.0, 0.0]])

    def test_model_scalar_batch_permutation_and_online_gradients(self):
        for device in ["cpu", "cuda"] if torch.cuda.is_available() else ["cpu"]:
            model = ChoiceMacroModel(8, items=ITEMS).to(device)
            model.item_prior.load_state_dict(self.head.state_dict())
            policy_output = model.policy[-1]
            assert isinstance(policy_output, torch.nn.Linear)
            with torch.no_grad():
                policy_output.weight.normal_(0, 0.1)
            public = MacroInput(
                ((1, 1.0),),
                (((2, 1.0),), ((3, 1.0),), ((4, 1.0),)),
                reward_prior=RewardPriorInput(0, 3, (*ITEMS[:2], None)),
            )
            ordinary = replace(public, reward_prior=None)
            batched, values = model.forward_batch(model.pack([public, ordinary]))
            for index, inputs in enumerate((public, ordinary)):
                logits, value = model(inputs)
                torch.testing.assert_close(logits, batched[index])
                torch.testing.assert_close(value, values[index])
            assert public.reward_prior is not None
            reverse = replace(
                public,
                candidates=public.candidates[::-1],
                reward_prior=replace(
                    public.reward_prior, items=public.reward_prior.items[::-1]
                ),
            )
            torch.testing.assert_close(model(reverse)[0], batched[0].flip(0))
            (-batched[0].log_softmax(0)[0] + values.square().sum()).backward()
            prior_grad = model.item_prior.bias.weight.grad
            policy_grad = policy_output.weight.grad
            assert prior_grad is not None and policy_grad is not None
            self.assertGreater(float(prior_grad.abs().sum()), 0)
            self.assertGreater(float(policy_grad.abs().sum()), 0)

    def test_supervision_and_checkpoint_initialize_actual_actor_only_in_shared_head(
        self,
    ):
        head = ItemPrior(ITEMS)
        model = TrainingAdapter(head)
        optimizer = torch.optim.Adam(head.parameters(), lr=0.01)
        batch = [example(), example("relic")]
        initial = float(torch.nn.functional.cross_entropy(*model(batch)).detach())
        for _ in range(15):
            optimizer.zero_grad()
            loss = torch.nn.functional.cross_entropy(*model(batch))
            loss.backward()
            optimizer.step()
        self.assertLess(float(loss.detach()), initial / 2)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "prior.pt"
            artifact = {
                "format": ITEM_PRIOR_FORMAT,
                "config": {
                    "protocol": ITEM_PRIOR_PROTOCOL,
                    "items": list(ITEMS),
                    "width": ITEM_PRIOR_WIDTH,
                },
                "model": head.state_dict(),
            }
            torch.save(artifact, path)
            torch.manual_seed(27)
            pretrained, metadata = make_model(arguments(path))
            torch.manual_seed(27)
            scratch, scratch_metadata = make_model(arguments(path, "scratch"))
            self.assertEqual(metadata["choice_items"], scratch_metadata["choice_items"])
            for name, tensor in pretrained.state_dict().items():
                if not name.startswith("item_prior."):
                    torch.testing.assert_close(
                        tensor, scratch.state_dict()[name], rtol=0, atol=0
                    )
            observation = reward()
            actions = (
                action(kind="take_card_reward", reward_slot=0),
                action(kind="take_card_reward", reward_slot=1),
                action(kind="skip_reward"),
            )
            inputs = encode(observation, actions)
            logits, value = pretrained(inputs)
            uniform, scratch_value = scratch(inputs)
            self.assertEqual(value.item(), scratch_value.item())
            self.assertEqual(value.item(), 0)
            self.assertEqual(uniform.tolist(), [0.0, 0.0, 0.0])
            self.assertGreater(
                logits.softmax(0)[0].item(), uniform.softmax(0)[0].item()
            )
            torch.testing.assert_close(logits.softmax(0)[2], uniform.softmax(0)[2])
            # Continuation reads architecture from its own metadata, not old BC paths.
            continuation = Path(directory) / "actor.pt"
            torch.save(
                {"config": metadata, "model": pretrained.state_dict()}, continuation
            )
            args = arguments(None)
            args.resume_from = continuation
            resumed, _ = make_model(args)
            resumed.load_state_dict(
                torch.load(continuation, weights_only=True)["model"], strict=True
            )
            torch.testing.assert_close(resumed(inputs)[0], logits, rtol=0, atol=0)
            artifact["format"] = "slaythedata_choice_bc_v1"
            torch.save(artifact, path)
            with self.assertRaisesRegex(ValueError, "Unsupported supervised"):
                make_model(arguments(path))


if __name__ == "__main__":
    unittest.main()
