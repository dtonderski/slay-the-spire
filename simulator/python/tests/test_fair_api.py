import unittest

import sts_sim


class FairApiTest(unittest.TestCase):
    def test_policy_package_exposes_only_fair_state_surface(self) -> None:
        state = sts_sim.State.new("HUMAN1")
        self.assertFalse(hasattr(sts_sim, "FullState"))
        self.assertFalse(hasattr(state, "full_state"))
        self.assertFalse(hasattr(state, "to_json"))
        self.assertFalse(hasattr(sts_sim.State, "from_json"))

        decision = state.decision()
        self.assertEqual(decision.schema_version, 1)
        self.assertEqual(decision.revision, state.revision)
        self.assertEqual(decision.observation.schema_version, 8)
        self.assertIsInstance(decision.actions, tuple)
        self.assertEqual(len(decision.actions), len(state.legal_actions()))

    def test_natural_heart_profile_is_initial_configuration_not_synthetic_hp(self) -> None:
        for seed in ("1", "7", "HUMAN1"):
            ordinary = sts_sim.State.new(seed)
            disabled = sts_sim.State.new(seed, final_act=False)
            self.assertEqual(ordinary.observation(), disabled.observation())
            state = sts_sim.State.new(seed, ascension=0, final_act=True)
            observation = state.observation()
            self.assertEqual(state.revision, 0)
            self.assertEqual(observation.context.player_hp, 80)
            self.assertEqual(observation.context.player_max_hp, 80)
            self.assertEqual(observation.context.deck, ordinary.observation().context.deck)
            self.assertTrue(observation.context.final_act_available)
            self.assertEqual(
                observation.context.keys, sts_sim.RunKeys(ruby=False, emerald=False, sapphire=False)
            )
            self.assertEqual(state.clone().observation(), observation)
            self.assertEqual(sts_sim.State.new(seed, final_act=True).observation(), observation)
            # Skip the initial Neow dialogue/reward without overriding game state.
            for _ in range(12):
                decision = state.decision()
                if decision.observation.kind == "map":
                    break
                action = next(
                    (a for a in decision.actions if a.kind in {"skip_reward", "confirm_grid"}),
                    decision.actions[0],
                )
                state.step(action)
            observation = state.observation()
            self.assertEqual(observation.kind, "map")
            if observation.kind == "map":
                self.assertEqual(sum(node.burning_elite for node in observation.screen.nodes), 1)

    def assert_matching_decisions(self, left: sts_sim.State, right: sts_sim.State) -> None:
        a, b = left.decision(), right.decision()
        self.assertEqual(a.revision, b.revision)
        self.assertEqual(a.observation, b.observation)
        fields = (
            "kind", "hand_slot", "target_slot", "option_slot", "card_slot",
            "node_slot", "reward_slot", "shop_slot", "potion_slot",
        )
        self.assertEqual(
            [tuple(getattr(choice, field) for field in fields) for choice in a.actions],
            [tuple(getattr(choice, field) for field in fields) for choice in b.actions],
        )

    def test_training_rng_is_explicit_private_initial_configuration(self) -> None:
        strict = sts_sim.State.new("HUMAN1", final_act=True)
        disabled = sts_sim.State.new("HUMAN1", final_act=True, training_rng_seed=None)
        training = sts_sim.State.new("HUMAN1", final_act=True, training_rng_seed=123456789)
        self.assert_matching_decisions(strict, disabled)
        self.assert_matching_decisions(strict, training)
        self.assert_matching_decisions(training.clone(), training)
        self.assertFalse(hasattr(training, "training_rng_seed"))
        self.assertFalse(hasattr(training, "set_training_rng_seed"))
        self.assertEqual(training.player_hp(), 80)
        for value in (-1, 1 << 64):
            with self.assertRaises(OverflowError):
                sts_sim.State.new("HUMAN1", training_rng_seed=value)

    def test_training_rng_clones_repeat_public_steps(self) -> None:
        left = sts_sim.State.new("7", final_act=True, training_rng_seed=987654321)
        right = left.clone()
        for _ in range(40):
            before = left.decision()
            self.assert_matching_decisions(left, right)
            if not before.actions:
                break
            left.step(before.actions[0])
            right.step(right.decision().actions[0])
            self.assert_matching_decisions(left, right)

    def test_natural_death_is_explicit_before_and_after_ui_proceed(self) -> None:
        state = sts_sim.State.new("1")
        for _ in range(80):
            decision = state.decision()
            if decision.observation.context.outcome == "death":
                break
            action = next(
                (
                    a
                    for a in decision.actions
                    if a.kind in {"end_turn", "skip_reward", "confirm_grid"}
                ),
                decision.actions[0],
            )
            state.step(action)
        decision = state.decision()
        self.assertEqual(decision.observation.context.outcome, "death")
        self.assertEqual(decision.observation.kind, "combat")
        self.assertEqual([a.kind for a in decision.actions], ["proceed"])
        self.assertEqual(state.clone().observation().context.outcome, "death")
        complete = state.step(decision.actions[0])
        self.assertEqual(complete.observation.kind, "complete")
        self.assertEqual(complete.observation.context.outcome, "death")
        self.assertFalse(complete.actions)
        self.assertEqual(state.clone().observation().context.outcome, "death")

    def test_clone_and_step_preserve_revision_contract(self) -> None:
        state = sts_sim.State.new("1")
        clone = state.clone()
        decision = state.decision()
        self.assertEqual(clone.revision, decision.revision)
        self.assertEqual(clone.decision().revision, decision.revision)

        result = state.step(decision.actions[0])
        self.assertEqual(result.revision, decision.revision + 1)
        self.assertEqual(state.revision, result.revision)
        self.assertEqual(clone.revision, decision.revision)

        with self.assertRaisesRegex(ValueError, "stale"):
            state.step(decision.actions[0])

    def test_player_hp_matches_public_context_without_mutation(self) -> None:
        state = sts_sim.State.new("HUMAN1")
        revision = state.revision
        before = state.observation()
        self.assertEqual(state.player_hp(), before.context.player_hp)
        self.assertEqual(state.revision, revision)
        self.assertEqual(state.observation(), before)


if __name__ == "__main__":
    unittest.main()
