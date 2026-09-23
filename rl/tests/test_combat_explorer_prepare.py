"""Local artifact helper tests. Paths are temporary; no machine-specific homes."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import torch
from combat_explorer.policy import PolicyAdapter
from combat_explorer.prepare_local import CHECKPOINT_NAME, DISTRIBUTIONS_NAME, ROOTS_NAME, prepare_local
from combat_explorer.roots import RootService
from loadout_sampling import LoadoutSampler, band_for
from model import CombatValueModel
from scenarios import COMBAT_FLOORS
from validation_set import load_validation, native_sha256


def sampler() -> LoadoutSampler:
    band = {
        "runs": 1,
        "deck_sizes": {"10": 1},
        "cards": {"Strike_R": {"0": 5}, "Defend_R": {"0": 4}, "Bash": {"0": 1}},
        "starters": {"Burning Blood": 1},
        "other_relic_counts": {"0": 1},
        "other_relics": {},
        "potions_obtained": {"block": 1},
        "max_hp": {"80": 1},
    }
    return LoadoutSampler(
        {
            "schema": 1,
            "ascension": 0,
            "identity_namespace": "sts_sim_public_base_keys",
            "split_role": "fit",
            "bands": {band_for(floor): band for floor in COMBAT_FLOORS},
        }
    )


class PrepareLocalTests(unittest.TestCase):
    def test_prepare_copies_bytes_and_loader_accepts_every_case(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_dist = root / "source-fit.json"
            source_ckpt = root / "source.pt"
            output = root / "local"
            source_dist.write_text(json.dumps(sampler().distributions))
            payload = {"model": CombatValueModel().state_dict(), "config": {"fixture": True}}
            torch.save(payload, source_ckpt)
            source_dist_bytes = source_dist.read_bytes()
            source_ckpt_bytes = source_ckpt.read_bytes()
            artifacts = prepare_local(
                distributions=source_dist,
                checkpoint=source_ckpt,
                output_dir=output,
                seed=7,
                main_count=1,
                stress_per_stratum=1,
            )
            self.assertTrue(artifacts["not_frozen_benchmark"])
            self.assertEqual((output / DISTRIBUTIONS_NAME).read_bytes(), source_dist_bytes)
            self.assertEqual((output / CHECKPOINT_NAME).read_bytes(), source_ckpt_bytes)
            self.assertEqual(source_dist.read_bytes(), source_dist_bytes)
            self.assertEqual(source_ckpt.read_bytes(), source_ckpt_bytes)
            document, groups = load_validation(output / ROOTS_NAME)
            self.assertEqual(document["native_sha256"], native_sha256())
            self.assertEqual(document["provenance"]["role"], "combat_explorer_saved_roots")
            self.assertIn("Not the frozen validation-a0-v1", document["provenance"]["lineage"])
            self.assertEqual(sum(len(item) for item in groups.values()), len(document["cases"]))
            self.assertGreaterEqual(len(groups["main"]), 1)
            self.assertGreaterEqual(len(groups["stress"]), 1)
            service = RootService(validation_manifest=output / ROOTS_NAME, distributions=output / DISTRIBUTIONS_NAME)
            self.assertEqual(len(service.list_cases()), len(document["cases"]))
            loaded = PolicyAdapter().load(output / CHECKPOINT_NAME, device="cpu")
            self.assertEqual(loaded.fingerprint, artifacts["checkpoint"]["sha256"])
            with self.assertRaises(FileExistsError):
                prepare_local(
                    distributions=source_dist,
                    checkpoint=source_ckpt,
                    output_dir=output,
                    seed=7,
                    main_count=1,
                    stress_per_stratum=1,
                )


if __name__ == "__main__":
    unittest.main()
