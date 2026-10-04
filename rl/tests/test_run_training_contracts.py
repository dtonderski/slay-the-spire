"""Infrastructure tests only; synthetic observations do not establish game parity."""

import json
import math
import unittest
from dataclasses import fields, replace

from run_training.contracts import PolicyAction, controller, outcome, terminal_reward
from run_training.targets import Transition, generalized_advantages
from sts_sim import (
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
    CompleteObservation,
    Decision,
    State,
)


class ContractTests(unittest.TestCase):
    def setUp(self):
        self.initial = State.new("HUMAN1", 0).decision()

    def complete(self, act, hp=80, actions=(), status=None):
        if status is None:
            status = (
                "ongoing"
                if actions
                else (
                    "death"
                    if hp <= 0
                    else {3: "act3_clear", 4: "heart_clear"}.get(
                        act, "unknown_complete"
                    )
                )
            )
        context = replace(
            self.initial.observation.context, act=act, player_hp=hp, outcome=status
        )
        observation = CompleteObservation(
            schema_version=FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
            phase="complete",
            kind="complete",
            context=context,
            screen=None,
        )
        return Decision(
            schema_version=1, revision=1, observation=observation, actions=actions
        )

    def test_initial_is_forced_not_terminal(self):
        self.assertEqual(outcome(self.initial), "ongoing")
        self.assertEqual(controller(self.initial), "forced")

    def test_victory_with_proceed_is_not_terminal(self):
        self.assertEqual(
            outcome(self.complete(3, actions=self.initial.actions)), "ongoing"
        )
        self.assertEqual(
            controller(self.complete(4, actions=self.initial.actions)), "forced"
        )

    def test_act3_is_not_heart_success(self):
        status = outcome(self.complete(3))
        self.assertEqual(status, "act3_clear")
        self.assertEqual(terminal_reward(status, "heart"), 0)
        self.assertEqual(terminal_reward(status, "act3"), 1)

    def test_heart_terminal(self):
        self.assertEqual(outcome(self.complete(4)), "heart_clear")
        self.assertEqual(terminal_reward("heart_clear", "heart"), 1)
        self.assertEqual(controller(self.complete(4)), "terminal")

    def test_death_is_zero(self):
        self.assertEqual(outcome(self.complete(2, hp=0)), "death")
        self.assertEqual(terminal_reward("death", "heart"), 0)

    def test_ambiguous_complete_is_invalid(self):
        self.assertEqual(outcome(self.complete(1)), "invalid")

    def test_positive_hp_complete_does_not_infer_success(self):
        self.assertEqual(
            outcome(self.complete(4, status="unknown_complete")), "invalid"
        )
        self.assertEqual(outcome(self.complete(3, status="ongoing")), "invalid")

    def test_empty_nonterminal_is_invalid(self):
        self.assertEqual(outcome(replace(self.initial, actions=())), "invalid")

    def test_invalid_or_unfinished_has_no_reward(self):
        for status in ("ongoing", "invalid"):
            with self.assertRaises(ValueError):
                terminal_reward(status, "heart")
        with self.assertRaises(ValueError):
            # Deliberately invalid input tests the runtime contract.
            terminal_reward("death", "unknown")  # ty: ignore[invalid-argument-type]

    def test_descriptor_omits_transport_and_family(self):
        descriptor = PolicyAction.from_action(self.initial.actions[0])
        names = {field.name for field in fields(descriptor)}
        self.assertNotIn("revision", names)
        self.assertNotIn("family", names)
        self.assertEqual(descriptor.kind, "choose_event_option")
        self.assertEqual(descriptor.option_slot, 0)

    def test_lost_combat_is_terminal_before_death_proceed(self):
        spec = {
            "seed": 1,
            "floor": 1,
            "kind": "normal",
            "encounter": "Cultist",
            "deck": [{"key": "Strike_R", "upgrades": 0}],
            "relics": [],
            "potions": [None, None, None],
            "hp": 1,
            "max_hp": 80,
            "gold": 99,
        }
        state = State.from_synthetic_spec(json.dumps(spec))
        decision = state.decision()
        for _ in range(10):
            if outcome(decision) == "death":
                self.assertTrue(
                    decision.actions
                )  # UI still permits death-screen Proceed.
                self.assertEqual(controller(decision), "terminal")
                return
            action = next(a for a in decision.actions if a.kind == "end_turn")
            decision = state.step(action)
        self.fail("Expected the explicit 1-HP synthetic combat to end in death")

    def test_macro_is_screen_routed(self):
        native = State.new("HUMAN1", 0)
        decision = native.decision()
        for _ in range(10):
            if len(decision.actions) > 1:
                self.assertEqual(controller(decision), "macro")
                return
            decision = native.step(decision.actions[0])
        self.fail("Expected a macro choice after Neow Talk")


class TargetTests(unittest.TestCase):
    def terminal(self, reward=1, value=0.3):
        return Transition(reward, value, 0, 0, 1, boundary=True, terminated=True)

    def test_undiscounted_lambda_one_is_monte_carlo(self):
        rows = [Transition(0, 0.2, 0.3, 1, 1), self.terminal()]
        targets = generalized_advantages(rows)
        for actual in targets.returns:
            self.assertAlmostEqual(actual, 1)
        self.assertAlmostEqual(targets.advantages[0], 0.8)

    def test_failure_still_trains_value(self):
        targets = generalized_advantages([self.terminal(0)])
        self.assertAlmostEqual(targets.advantages[0], -0.3)
        self.assertAlmostEqual(targets.returns[0], 0)

    def test_cutoff_bootstraps_instead_of_becoming_death(self):
        row = Transition(0, 0.2, 0.7, 1, 0.95, boundary=True)
        targets = generalized_advantages([row])
        self.assertAlmostEqual(targets.returns[0], 0.7)

    def test_trace_does_not_cross_reset(self):
        rows = [Transition(0, 0.2, 0.7, 1, 0.95, boundary=True), self.terminal(10)]
        targets = generalized_advantages(rows)
        self.assertAlmostEqual(targets.returns[0], 0.7)
        self.assertAlmostEqual(targets.returns[1], 10)

    def test_lambda_zero_one_step_td(self):
        rows = [Transition(0, 0.2, 0.3, 1, 0), self.terminal()]
        self.assertAlmostEqual(generalized_advantages(rows).returns[0], 0.3)

    def test_explicit_variable_duration_discount(self):
        rows = [Transition(0, 0.2, 0.3, 0.9**7, 1), self.terminal()]
        targets = generalized_advantages(rows)
        self.assertAlmostEqual(targets.returns[0], 0.9**7)

    def test_gamma_one_is_duration_independent(self):
        for duration in (1, 10, 1000):
            rows = [Transition(0, 0.2, 0.3, 1**duration, 1), self.terminal()]
            self.assertAlmostEqual(generalized_advantages(rows).returns[0], 1)

    def test_empty(self):
        self.assertEqual(generalized_advantages([]).returns, ())

    def test_bad_discount_decay_nonfinite(self):
        for row in (
            Transition(0, 0, 0, -1, 1, True),
            Transition(0, 0, 0, 1, 1.1, True),
            Transition(math.nan, 0, 0, 1, 1, True),
            Transition(0, math.inf, 0, 1, 1, True),
        ):
            with self.assertRaises(ValueError):
                generalized_advantages([row])

    def test_finite_input_overflow_refused(self):
        with self.assertRaisesRegex(ValueError, "overflowed"):
            generalized_advantages([Transition(1e308, -1e308, 0, 0, 1, True, True)])

    def test_terminal_requires_no_bootstrap(self):
        for row in (
            Transition(1, 0, 0.5, 0, 1, True, True),
            Transition(1, 0, 0, 1, 1, True, True),
            Transition(1, 0, 0, 0, 1, False, True),
        ):
            with self.assertRaises(ValueError):
                generalized_advantages([row])

    def test_missing_boundary_and_mismatched_successor_fail(self):
        with self.assertRaises(ValueError):
            generalized_advantages([Transition(0, 0.2, 0.3, 1, 1)])
        with self.assertRaises(ValueError):
            generalized_advantages([Transition(0, 0.2, 0.4, 1, 1), self.terminal()])


if __name__ == "__main__":
    unittest.main()
