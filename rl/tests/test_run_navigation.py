"""Policy commitment tests, not real-game parity evidence."""

import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import torch
from run_training.collector import MacroPrediction, MacroRequest, _trajectory
from run_training.contracts import PolicyAction
from run_training.model import encode
from run_training.restrictions import NAVIGATION_PROTOCOL, macro_eligibility
from sts_sim import MapObservation, ShopObservation, ShopScreen, State


class NavigationTests(unittest.TestCase):
    def test_neow_map_is_an_ordinary_network_input(self):
        decision = State.new("713").decision()
        actions = tuple(PolicyAction.from_action(a) for a in decision.actions)
        original = encode(decision.observation, actions)
        without_map = replace(
            decision.observation,
            context=replace(decision.observation.context, map=None),
        )
        self.assertNotEqual(original.context, encode(without_map, actions).context)
        wrong_act = replace(
            decision.observation,
            context=replace(
                decision.observation.context,
                map=replace(decision.observation.context.map, act=2),
            ),
        )
        with self.assertRaisesRegex(ValueError, "current act"):
            encode(wrong_act, actions)

    def test_commitments_retain_native_candidates_and_reset_on_new_room(self):
        base = State.new("713").observation()
        context = replace(base.context, floor=5)
        closed = ShopObservation(
            schema_version=base.schema_version,
            phase="shop",
            kind="shop",
            context=context,
            screen=ShopScreen(
                merchant_open=False, remove_cost=75, cards=(), relics=(), potions=()
            ),
        )
        opened = replace(closed, screen=replace(closed.screen, merchant_open=True))
        map_ob = MapObservation(
            schema_version=base.schema_version,
            phase="idle",
            kind="map",
            context=context,
            screen=context.map,
        )
        terminal = replace(closed, context=replace(context, act=2, floor=18))

        def a(kind, **slots):
            fields = {
                name: None
                for name in (
                    "hand_slot",
                    "potion_slot",
                    "option_slot",
                    "target_slot",
                    "card_slot",
                    "node_slot",
                    "reward_slot",
                    "shop_slot",
                )
            }
            return PolicyAction(kind=kind, **(fields | slots))

        states = [
            (closed, (a("enter_shop"), a("proceed"))),
            (opened, (a("leave_shop"),)),
            (closed, (a("enter_shop"), a("proceed"))),
            (map_ob, (a("return_to_room"), a("choose_map_node", node_slot=0))),
            (
                replace(closed, context=replace(context, floor=6)),
                (a("enter_shop"), a("proceed")),
            ),
            (terminal, (a("proceed"),)),
        ]
        accepted = []

        class Scripted:
            i = 0

            def decision(self):
                ob, actions = states[self.i]
                return SimpleNamespace(observation=ob, actions=actions, revision=self.i)

            def step(self, action):
                accepted.append(action.kind)
                self.i += 1
                return self.decision()

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "journal.jsonl"
            trajectory = _trajectory(
                "713",
                objective="act1",
                final_act=True,
                max_actions=20,
                macro_rng=torch.Generator().manual_seed(1),
                combat_rng=torch.Generator().manual_seed(2),
                journal=path,
                state_factory=lambda *a, **kw: Scripted(),
            )
            request = next(trajectory)
            for expected, choice in [
                ((True, True), 0),
                ((False, True), 1),
                ((False, True), 1),
                ((True, True), 1),
            ]:
                self.assertIsInstance(request, MacroRequest)
                self.assertEqual(request.inputs.eligibility, expected)
                prediction = MacroPrediction(
                    choice,
                    tuple(0.0 if ok else -float("inf") for ok in expected),
                    0.0,
                    None,
                )
                try:
                    request = trajectory.send(prediction)
                except StopIteration as done:
                    self.assertEqual(done.value.status, "act1_clear")
            rows = [json.loads(line) for line in path.read_text().splitlines()]
        self.assertEqual(rows[0]["navigation_protocol"], NAVIGATION_PROTOCOL)
        self.assertEqual(
            accepted,
            ["enter_shop", "leave_shop", "proceed", "choose_map_node", "proceed"],
        )
        self.assertEqual(
            [row["index"] for row in rows if row["type"] == "attempt"], [0, 0, 1, 1, 1]
        )
        # Native room return remains a candidate, and raw-navigation ablations retain it.
        self.assertEqual(
            macro_eligibility(map_ob, states[3][1], avoid_prismatic=False), ()
        )
        with self.assertRaisesRegex(ValueError, "no eligible"):
            macro_eligibility(
                map_ob,
                (a("return_to_room"),),
                avoid_prismatic=False,
                commit_navigation=True,
            )


if __name__ == "__main__":
    unittest.main()
