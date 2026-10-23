"""Public exploration profiles and identical collector/learner distributions."""

import contextlib
import copy
import io
import json
import math
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch

import torch
from run_training.collector import MacroStep, RunEpisode
from run_training.exploration import exploration_controls
from run_training.model import HealthMacroModel, MacroModel, encode
from run_training.restrictions import restrict_batch_logits, restrict_logits
from run_training.trainer import main, update
from sts_sim import RelicKey
from sts_sim.observations.common import Relic
from test_run_training_keys_budget import action, rest, reward


class ExplorationTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        torch.manual_seed(61)
        self.rest = rest()
        self.actions = tuple(
            action(k) for k in ("rest_heal", "rest_open_smith", "rest_recall")
        )

    def test_public_mask_preserves_rows_and_low_hp_and_dream_catcher(self):
        original = encode(self.rest, self.actions)
        masked = encode(self.rest, self.actions, exploration_profile="fullhp")
        self.assertEqual(masked.context, original.context)
        self.assertEqual(masked.candidates, original.candidates)
        self.assertEqual(masked.eligibility, (False, True, True))
        self.assertEqual(masked.temperature, 1)
        for ob in (
            replace(self.rest, context=replace(self.rest.context, player_hp=1)),
            replace(
                self.rest,
                context=replace(
                    self.rest.context,
                    relics=(
                        Relic(slot=0, content_key=RelicKey.DREAM_CATCHER, state=()),
                    ),
                ),
            ),
        ):
            row = encode(ob, self.actions, exploration_profile="fullhp")
            self.assertEqual(row.eligibility, ())
        self.assertEqual(
            exploration_controls(self.rest, (self.actions[0],), (), "fullhp"), ((), 1)
        )
        self.assertEqual(
            exploration_controls(
                self.rest, self.actions, (True, False, False), "fullhp"
            ),
            ((True, False, False), 1),
        )
        with self.assertRaises(ValueError):
            exploration_controls(self.rest, self.actions, (), "unknown")

    def test_targeted_temperature_no_forced_key_or_global_flattening(self):
        for profile, t in (("rest_keys_t4", 4), ("rest_keys_t8", 8)):
            row = encode(self.rest, self.actions, exploration_profile=profile)
            self.assertEqual(row.temperature, t)
            self.assertEqual(row.eligibility, (False, True, True))
            ob = reward()
            actions = (
                action("take_sapphire_key"),
                action("take_relic_reward_at", reward_slot=0),
            )
            # Test control selection only: no fabricated reward tables enter encode.
            self.assertEqual(exploration_controls(ob, actions, (), profile), ((), t))
            held = replace(
                ob,
                context=replace(
                    ob.context, keys=replace(ob.context.keys, sapphire=True)
                ),
            )
            self.assertEqual(exploration_controls(held, actions, (), profile), ((), 1))
            self.assertEqual(
                exploration_controls(ob, actions, (False, True), profile),
                ((False, True), 1),
            )
            self.assertEqual(
                exploration_controls(
                    ob, (action("take_gold_reward"), action("proceed")), (), profile
                ),
                ((), 1),
            )

    def test_tempered_forward_cpu_cuda_batch_gradients_and_integrity(self):
        devices = ["cpu"] + (["cuda"] if torch.cuda.is_available() else [])
        for device in devices:
            for cls in (MacroModel, HealthMacroModel):
                model = cls(8).to(device)
                with torch.no_grad():
                    for param in model.parameters():
                        param.uniform_(-0.1, 0.1)
                inputs = [
                    encode(self.rest, self.actions, exploration_profile=p)
                    for p in ("none", "fullhp", "rest_keys_t4", "rest_keys_t8")
                ]
                raw, _ = model(inputs[0])
                batch = model.pack(inputs)
                logits, values = model.forward_batch(batch)
                effective = restrict_batch_logits(
                    logits, batch.lengths, [x.eligibility for x in inputs]
                )
                for i, row in enumerate(inputs):
                    single, v = model(row)
                    torch.testing.assert_close(
                        single, raw / row.temperature, rtol=0, atol=0
                    )
                    torch.testing.assert_close(single, logits[i], rtol=0, atol=0)
                    torch.testing.assert_close(v, values[i], rtol=0, atol=0)
                    torch.testing.assert_close(
                        restrict_logits(single, row.eligibility),
                        effective[i],
                        rtol=0,
                        atol=0,
                    )
                effective[1, 1:].sum().backward()
                self.assertTrue(
                    all(
                        torch.isfinite(p.grad).all()
                        for p in model.parameters()
                        if p.grad is not None
                    )
                )
                steps = []
                for row in inputs:
                    with torch.no_grad():
                        z, v = model(row)
                        z = restrict_logits(z, row.eligibility)
                    steps.append(MacroStep(row, 1, tuple(z.tolist()), v.item()))
                episode = RunEpisode(
                    "death", 0.5, tuple(steps), 4, 8, furthest_act1_floor=8
                )
                optimizer = torch.optim.Adam(model.parameters(), lr=0.001)
                bad = replace(
                    episode,
                    steps=(replace(steps[0], value=steps[0].value + 1),)
                    + tuple(steps[1:]),
                )
                before = copy.deepcopy(model.state_dict())
                with self.assertRaisesRegex(ValueError, "recomputation mismatch"):
                    update(
                        [bad],
                        model,
                        optimizer,
                        entropy_coef=0.01,
                        value_coef=0.1,
                        batch_steps=128,
                    )
                self.assertFalse(optimizer.state)
                for k, v in model.state_dict().items():
                    self.assertTrue(torch.equal(v, before[k]))
                result = update(
                    [episode],
                    model,
                    optimizer,
                    entropy_coef=0.01,
                    value_coef=0.1,
                    batch_steps=128,
                )
                self.assertEqual(result["optimizer_step"], 1)
                for t in (0, -1, math.inf, math.nan):
                    with self.assertRaises(ValueError):
                        model.pack([replace(inputs[0], temperature=t)])
                last = model.policy[-1]
                assert isinstance(last, torch.nn.Linear) and last.bias is not None
                with torch.no_grad():
                    last.bias.fill_(math.inf)
                # Nonfinite raw values stay nonfinite; temperature cannot hide them.
                self.assertFalse(torch.isfinite(model(inputs[-1])[0]).all())

    def test_cli_profile_archiving_and_exact_resume_rejection(self):
        profiles = []

        def fake_collect(seed, model, combat, **options):
            profiles.append(options["exploration_profile"])
            return RunEpisode("death", 0.5, (), 1, 8, furthest_act1_floor=8)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "seeds.json").write_text('["100"]')
            (root / "combat.pt").write_bytes(b"mocked")
            args = [
                "--run-id",
                "profile",
                "--combat-checkpoint",
                str(root / "combat.pt"),
                "--validation-seeds",
                str(root / "seeds.json"),
                "--output-root",
                directory,
                "--collection-width",
                "1",
                "--batch-size",
                "1",
                "--updates",
                "1",
                "--model-width",
                "8",
                "--macro-exploration",
                "rest_keys_t4",
            ]
            with (
                patch("run_training.trainer.FrozenCombat"),
                patch("run_training.trainer.collect", side_effect=fake_collect),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                main(args)
                cp = root / "profile/latest.pt"
                cfg = json.loads((root / "profile/config.json").read_text())
                self.assertEqual(cfg["exploration_profile"], "rest_keys_t4")
                self.assertTrue(
                    (root / "profile/source/run_training/exploration.py").exists()
                )
                args[1] = "bad"
                args[-1] = "rest_keys_t8"
                with self.assertRaisesRegex(
                    ValueError, "continuation mismatch: macro_exploration"
                ):
                    main(args + ["--resume-from", str(cp)])
                args[1] = "continued"
                args[-1] = "rest_keys_t4"
                main(args + ["--resume-from", str(cp)])
        self.assertTrue(profiles and all(p == "rest_keys_t4" for p in profiles))


if __name__ == "__main__":
    unittest.main()
