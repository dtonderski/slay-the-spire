"""Act-4 shaping contract; public/scripted infrastructure, not game parity evidence."""

import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from typing import Any, cast

import torch
from run_training.collector import RunEpisode, collect
from run_training.model import FEATURE_VERSION, MacroModel
from run_training.progress import RunProgress
from run_training.rewards import KEYS_BUDGET_PROTOCOL, terminal_parts
from run_training.trainer import PROTOCOL, load_warm_start, scores, update
from sts_sim import State


class Act4EntryTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        self.initial = State.new("1", final_act=True).decision()

    def view(self, act, floor, keys=False):
        original = self.initial.observation
        return replace(
            original,
            context=replace(
                original.context,
                act=act,
                floor=floor,
                map=None,
                keys=replace(
                    original.context.keys, ruby=keys, emerald=keys, sapphire=keys
                ),
            ),
        )

    def test_keys_or_floor_or_act3_clear_do_not_imply_act4_entry(self):
        for view in (self.view(3, 51, True), self.view(3, 56, False)):
            progress = RunProgress().observed(view)
            parts = terminal_parts("heart_keys_budget", "death", 16, progress=progress)
            self.assertEqual(parts.act4_entry_bonus, 0)
        progress = RunProgress().observed(
            replace(
                self.view(3, 51, True),
                context=replace(self.view(3, 51, True).context, outcome="act3_clear"),
            )
        )
        self.assertEqual(progress.acts_cleared, 3)
        self.assertEqual(
            terminal_parts(
                "heart_keys_budget", "act3_clear", 16, progress=progress
            ).act4_entry_bonus,
            0,
        )

    def test_observed_entry_pays_once_before_heart_and_never_again_at_combat(self):
        progress = RunProgress().observed(self.view(3, 51, True))
        progress = progress.observed(self.view(4, 52, True))
        self.assertFalse(progress.heart_reached)
        for _ in range(20):
            progress = progress.observed(self.view(4, 52, True))
        before = terminal_parts("heart_keys_budget", "death", 16, progress=progress)
        self.assertEqual(before.act4_entry_bonus, 3)
        self.assertEqual(before.heart_bonus, 0)
        self.assertAlmostEqual(before.total, 3 * 52 / 55 + 6 + 1.5 + 3)
        # An observed Heart encounter changes a diagnostic, not this entry reward.
        at_heart = replace(progress, furthest_floor=55, heart_reached=True)
        after = terminal_parts("heart_keys_budget", "death", 16, progress=at_heart)
        self.assertEqual(after.act4_entry_bonus, 3)
        self.assertEqual(after.total, 13.5)
        self.assertEqual(after.heart_bonus, 0)
        win = terminal_parts(
            "heart_keys_budget",
            "heart_clear",
            16,
            progress=replace(at_heart, furthest_floor=56),
        )
        self.assertEqual(win.total, 43.5)
        self.assertEqual(win.heart_bonus, 30)
        self.assertEqual(
            terminal_parts(
                "heart_progress",
                "heart_clear",
                16,
                progress=replace(at_heart, furthest_floor=56),
            ).total,
            39,
        )

    def test_budget_penalty_remains_negative_and_unknown_not_a_death(self):
        progress = RunProgress(55, 4, 3, True, True, True, True)
        parts = terminal_parts(
            "heart_keys_budget", "action_limit", 16, progress=progress
        )
        self.assertEqual(parts.total, -6.5)
        episode = RunEpisode(
            "action_limit",
            parts.total,
            (),
            5000,
            55,
            objective="heart_keys_budget",
            progress=progress,
            action_limit=5000,
            termination_reason="action_limit",
            game_outcome="ongoing",
        )
        result = scores([episode])
        self.assertEqual(result["reward/act4_entry_bonus_mean"], 3)
        self.assertEqual(result["outcome/deaths"], 0)
        self.assertEqual(result["completed"], 0)
        self.assertEqual(result["coverage/unknown_rate"], 1)
        for status in ("cutoff", "error", "ongoing"):
            with self.assertRaises(ValueError):
                terminal_parts("heart_keys_budget", status, 16, progress=progress)

    def test_collector_records_entry_without_any_heart_encounter(self):
        state = State.new("1", final_act=True)
        macro = state.step(state.decision().actions[0])
        views = [
            replace(
                macro,
                revision=i,
                observation=replace(
                    macro.observation, context=self.view(act, floor, True).context
                ),
            )
            for i, (act, floor) in enumerate(((3, 51), (4, 52), (4, 52)))
        ]

        class Scripted:
            index = 0

            def decision(self):
                return views[self.index]

            def advance(self, action):
                return self.step(action)

            def step(self, chosen):
                self.index += 1
                return views[self.index]

        model = MacroModel(8)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.jsonl"
            episode = collect(
                "1",
                model,
                cast(Any, None),
                objective="heart_keys_budget",
                final_act=True,
                max_actions=2,
                macro_rng=torch.Generator(),
                combat_rng=torch.Generator(),
                journal=path,
                state_factory=lambda *a, **kw: cast(Any, Scripted()),
                initial_metadata={"protocol": "scripted_public_test_fixture"},
            )
            rows = [json.loads(line) for line in path.read_text().splitlines()]
        self.assertEqual(episode.status, "action_limit")
        self.assertEqual(rows[0]["reward_protocol"], KEYS_BUDGET_PROTOCOL)
        self.assertEqual(rows[-1]["reward_parts"]["act4_entry_bonus"], 3)
        assert episode.progress is not None
        self.assertEqual(episode.progress.furthest_act, 4)
        self.assertFalse(episode.progress.heart_reached)
        self.assertEqual(episode.reward, episode.reward_parts().total)
        result = update(
            [episode],
            model,
            torch.optim.Adam(model.parameters()),
            entropy_coef=0.01,
            value_coef=0.1,
            batch_steps=128,
        )
        self.assertEqual(result["optimizer_step"], 1)

    def test_old_reward_weights_need_explicit_policy_only_transfer(self):
        parent = MacroModel(8)
        assert parent.value.bias is not None
        with torch.no_grad():
            parent.value.bias.fill_(10)
        config = {
            "protocol": PROTOCOL,
            "reward_protocol": "heart_floor55x3_act2_key0p5_win30_actionlimit_minus20_v1",
            "objective": "heart_keys_budget",
            "model_width": 8,
            "encoder": "hashed",
            "feature_version": FEATURE_VERSION,
            "observation_schema": self.initial.observation.schema_version,
            "final_act": True,
            "ascension": 0,
            "gamma": 1.0,
            "combat_sha256": "fixture",
            "native_sha256": "fixture",
        }
        settings = config | {"reward_protocol": KEYS_BUDGET_PROTOCOL}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "v1-fixture.pt"
            torch.save(
                {
                    "model": parent.state_dict(),
                    "config": config,
                    "iteration": 1,
                    "optimizer_updates": 1,
                },
                path,
            )
            model = MacroModel(8)
            with self.assertRaisesRegex(ValueError, "reward_protocol"):
                load_warm_start(model, path, settings)
            info = load_warm_start(model, path, settings, policy_only=True)
            self.assertEqual(info["critic_initialization"], "fresh_zero_output")
            assert model.value.bias is not None
            self.assertEqual(model.value.bias.item(), 0)
            self.assertEqual(info["parent_reward_protocol"], config["reward_protocol"])
            for name, value in model.state_dict().items():
                if not name.startswith("value."):
                    torch.testing.assert_close(
                        value, parent.state_dict()[name], atol=0, rtol=0
                    )


if __name__ == "__main__":
    unittest.main()
