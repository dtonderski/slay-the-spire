"""Synthetic-root curriculum contracts, not natural-run or parity evidence."""

import gzip
import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch

import torch
from run_training.collector import task_result
from run_training.contracts import PolicyAction
from run_training.model import HealthMacroModel, MacroModel, encode
from run_training.rewards import succeeded, terminal_parts
from run_training.roots import (
    ROOT_PROTOCOL,
    RootBank,
    file_hash,
    reconstruct,
    validate_natural_setup,
)
from sts_sim import _native
from tools.build_campfire_roots import find_root

FIXTURES = (
    Path(__file__).resolve().parents[2] / "simulator/python/tests/fixtures/campfire"
)


class RootCurriculumTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        self.cases = json.loads((FIXTURES / "metadata.json").read_text())

    def manifest(self):
        return {
            "protocol": ROOT_PROTOCOL,
            "native_sha256": file_hash(Path(_native.__file__)),
            "final_act": True,
            "cases": [
                {**c, "journal": str(FIXTURES / c["journal"])} for c in self.cases
            ],
        }

    def bank(self, path, manifest):
        path.write_text(json.dumps(manifest))
        return RootBank(path, final_act=True)

    def test_split_by_source_seed_and_journal_integrity(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            bank = self.bank(path, self.manifest())
            self.assertFalse(set(bank.training) & set(bank.validation))
            source, _, _ = reconstruct(
                FIXTURES / self.cases[0]["journal"], self.cases[0]["stop_step"]
            )
            low, visible, previous, info = bank.initial(bank.training[0], 0.15)
            high, _, _, _ = bank.initial(bank.training[0], 0.85)
            self.assertIsNotNone(visible)
            self.assertIsNotNone(previous)
            self.assertLess(low.player_hp(), high.player_hp())
            self.assertEqual(
                low.observation().context.deck, high.observation().context.deck
            )
            self.assertEqual(info["initial_hp"], low.player_hp())
            self.assertEqual(
                bank.cache[bank.training[0]][0].observation(), source.observation()
            )
            bad = self.manifest()
            bad["cases"].append({**bad["cases"][0], "split": "validation"})
            with self.assertRaisesRegex(ValueError, "Duplicate"):
                self.bank(path, bad)
            bad = self.manifest()
            bad["cases"][0]["journal_sha256"] = "bad"
            with self.assertRaisesRegex(ValueError, "hash"):
                self.bank(path, bad)
            bad = self.manifest()
            bad["native_sha256"] = "bad"
            with self.assertRaisesRegex(ValueError, "native"):
                self.bank(path, bad)
            bad = self.manifest()
            bad["cases"][0]["seed"] = "999"
            with self.assertRaisesRegex(ValueError, "source seed"):
                self.bank(path, bad)
            for fraction in (0, -1, 1.01, float("nan")):
                with self.assertRaises(ValueError):
                    bank.initial(bank.training[0], fraction)

    def test_sources_must_declare_natural_start_or_be_legacy_natural(self):
        setup = {
            "type": "setup",
            "seed": self.cases[0]["seed"],
            "ascension": 0,
            "final_act": True,
        }
        validate_natural_setup(setup)
        validate_natural_setup(
            {**setup, "initial_state": {"protocol": "natural_start"}}
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            journal = root / "new-non-natural-source.jsonl.gz"
            for initial in (
                {"protocol": ROOT_PROTOCOL},
                {"protocol": "unknown"},
                {},
                None,
                [],
            ):
                with gzip.open(journal, "wt") as stream:
                    stream.write(json.dumps({**setup, "initial_state": initial}) + "\n")
                with patch("run_training.roots.State.new") as new:
                    with self.assertRaisesRegex(ValueError, "natural A0"):
                        reconstruct(journal, 0)
                    with self.assertRaisesRegex(ValueError, "natural A0"):
                        find_root(journal)
                    manifest = self.manifest()
                    manifest["cases"][0].update(
                        journal=str(journal), journal_sha256=file_hash(journal)
                    )
                    with self.assertRaisesRegex(ValueError, "natural A0"):
                        self.bank(root / "manifest.json", manifest)
                    new.assert_not_called()

    def test_binary_reward_has_no_preearned_progress(self):
        for floor in (0, 8, 15, 16):
            self.assertEqual(terminal_parts("act1_binary", "death", floor).total, 0)
            self.assertEqual(
                terminal_parts("act1_binary", "act1_clear", floor).total, 1
            )
        self.assertTrue(succeeded("act1_binary", "act1_clear"))
        for status in ("error", "cutoff", "ongoing"):
            with self.assertRaises(ValueError):
                terminal_parts("act1_binary", status, 15)
        source, _, _ = reconstruct(
            FIXTURES / self.cases[0]["journal"], self.cases[0]["stop_step"]
        )
        d = source.decision()
        self.assertEqual(task_result(d, "act1_binary"), ("ongoing", None))
        completed = replace(
            d,
            observation=replace(
                d.observation, context=replace(d.observation.context, act=2)
            ),
        )
        self.assertEqual(task_result(completed, "act1_binary"), ("act1_clear", 1))

    def test_health_ablation_starts_uniform_and_uses_public_hp(self):
        source, visible, previous = reconstruct(
            FIXTURES / self.cases[0]["journal"], self.cases[0]["stop_step"]
        )
        d = source.synthetic_rest_root(10).decision()
        actions = tuple(PolicyAction.from_action(a) for a in d.actions)
        low = encode(d.observation, actions, visible_map=visible, previous=previous)
        high_ob = replace(
            d.observation, context=replace(d.observation.context, player_hp=60)
        )
        high = encode(high_ob, actions, visible_map=visible, previous=previous)
        self.assertNotEqual(low.health, high.health)
        self.assertEqual(low.candidates, high.candidates)
        model = HealthMacroModel(8)
        base = MacroModel(8)
        for m in (model, base):
            logits, value = m(low)
            torch.testing.assert_close(logits, torch.zeros_like(logits), atol=0, rtol=0)
            self.assertEqual(float(value.detach()), 0)
        # Explicit path can express HP sensitivity without changing hashed features.
        with torch.no_grad():
            first, last = model.health_value[0], model.health_value[-1]
            assert isinstance(first, torch.nn.Linear) and isinstance(
                last, torch.nn.Linear
            )
            first.weight.zero_()
            first.bias.zero_()
            first.weight[0, 0] = 1
            last.weight.zero_()
            last.bias.zero_()
            last.weight[0, 0] = 1
        self.assertGreater(
            float(model(high)[1].detach()), float(model(low)[1].detach())
        )


if __name__ == "__main__":
    unittest.main()
