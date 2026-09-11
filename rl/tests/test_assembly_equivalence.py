"""Row-wise reference implementations guard the packed assembly optimization."""

import unittest
from dataclasses import replace
from functools import partial
from unittest.mock import patch

import torch
from encoders.actions import tensorize_actions
from model import CombatModel
from observation_encoder import OBSERVATION_GROUPS
from sts_sim.observations.combat import Selection, SelectionOption
from test_model import action
from train import first_combat


def rowwise_observations(encoder, observations):
    groups = {}
    _, groups["player"] = encoder.player(observations)
    hand, groups["hand"] = encoder.cards([tuple(entry.card for entry in obs.screen.hand) for obs in observations])
    for name in ("draw", "discard", "exhaust"):
        _, groups[name] = encoder.cards([getattr(obs.screen, name + "_pile").cards for obs in observations])
    enemies, groups["enemies"] = encoder.enemies([obs.screen.monsters for obs in observations], encoder.cards)
    _, groups["relics"] = encoder.relics([obs.context.relics for obs in observations])
    potions, groups["potions"] = encoder.potions([obs.context.potion_slots for obs in observations])
    selection, groups["selection_context"], groups["selection_options"] = encoder.selection(
        [obs.screen.selection for obs in observations], encoder.cards
    )
    sequences = [
        torch.cat(
            [encoder.summary_embedding.weight]
            + [groups[name][i] + encoder.group_embedding.weight[index] for index, name in enumerate(OBSERVATION_GROUPS)]
        )
        for i in range(len(observations))
    ]
    tokens = torch.nn.utils.rnn.pad_sequence(sequences, batch_first=True)
    lengths = torch.tensor([len(sequence) for sequence in sequences], device=tokens.device)
    mask = torch.arange(tokens.shape[1], device=tokens.device)[None, :] >= lengths[:, None]
    features = [
        {"hand": hand[i], "enemies": enemies[i], "potions": potions[i], "selection": selection[i]}
        for i in range(len(observations))
    ]
    return tokens, mask, features


def rowwise_actions(encoder, actions, features):
    result = []
    reference = next(encoder.parameters())
    for candidates, rows in zip(actions, features):
        inputs = tensorize_actions(
            candidates, rows["hand"], rows["potions"], rows["enemies"], selection_features=rows["selection"]
        )
        vectors = [
            encoder.encoders[kind](row)
            if kind in encoder.encoders
            else encoder.constants[kind](reference.new_zeros((), dtype=torch.long))
            for kind, row in inputs
        ]
        result.append(torch.stack(vectors) if vectors else reference.new_empty((0, encoder.action_dim)))
    return result


class AssemblyEquivalenceTests(unittest.TestCase):
    def test_outputs_and_gradients_match_rowwise_reference(self) -> None:
        torch.set_num_threads(1)
        obs = first_combat("HUMAN1", 0).decision().observation
        assert obs.kind == "combat"
        selection = Selection(
            kind="discovery_reward",
            options=tuple(SelectionOption(slot=i, card=entry.card) for i, entry in enumerate(obs.screen.hand[:2])),
            selected_slots=(1,),
        )
        a = replace(obs, screen=replace(obs.screen, selection=selection))
        b = replace(
            obs,
            screen=replace(
                obs.screen,
                monsters=(replace(obs.screen.monsters[0], stasis_card=obs.screen.hand[0].card),),
                hand=obs.screen.hand[:2],
            ),
        )
        actions = [
            (
                action("play_hand_slot", target_slot=0),
                action("end_turn"),
                action("toggle_visible_card", option_slot=1),
                action("choose_visible_option"),
                action("use_potion_slot"),
                action("discard_potion_slot"),
                action("confirm_selection"),
                action("confirm_selection_without_retrieval"),
                action("skip_selection"),
                action("play_hand_slot", target_slot=0),  # Repeated row must accumulate gradients twice.
            ),
            (action("end_turn"), action("play_hand_slot", hand_slot=1), action("use_potion_slot", target_slot=0)),
        ]
        for device in ("cpu", "cuda") if torch.cuda.is_available() else ("cpu",):
            with self.subTest(device=device):
                model = CombatModel(d_model=16, action_dim=8, n_layers=1).double().to(device)
                tokens, mask, _ = model.observation_encoder.prepare_batch([a, b])
                expected_tokens, expected_mask, _ = rowwise_observations(model.observation_encoder, [a, b])
                torch.testing.assert_close(tokens, expected_tokens)
                self.assertTrue(torch.equal(mask, expected_mask))
                logits, valid = model([a, b], actions)
                logits[valid].square().sum().backward()
                gradients = {
                    name: p.grad.clone() if p.grad is not None else None for name, p in model.named_parameters()
                }
                model.zero_grad(set_to_none=True)
                with (
                    patch.object(
                        model.observation_encoder,
                        "prepare_batch",
                        side_effect=partial(rowwise_observations, model.observation_encoder),
                    ),
                    patch.object(
                        model.action_encoder,
                        "forward",
                        side_effect=partial(rowwise_actions, model.action_encoder),
                    ),
                ):
                    expected, expected_valid = model([a, b], actions)
                torch.testing.assert_close(logits, expected)
                self.assertTrue(torch.equal(valid, expected_valid))
                expected[expected_valid].square().sum().backward()
                for name, parameter in model.named_parameters():
                    torch.testing.assert_close(parameter.grad, gradients[name])


if __name__ == "__main__":
    unittest.main()
