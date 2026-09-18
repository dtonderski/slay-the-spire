import copy
import io
import json
import random
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import torch
from loadout_sampling import LoadoutSampler, band_for
from model import CombatModel
from rollout_errors import SimulatorStepError
from scenarios import COMBAT_FLOORS, ScenarioConfig
from train_synthetic import evaluate_baselines, evaluate_sets, fresh_batch, update
from validation_set import build_validation, load_validation, main


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
            "bands": {band_for(f): band for f in COMBAT_FLOORS},
        }
    )


class ValidationSetTests(unittest.TestCase):
    def test_deterministic_frozen_round_trip_and_strata(self) -> None:
        before = random.getstate()
        a = build_validation(sampler(), 7, 4, 1, repeats=1)
        self.assertEqual(a, build_validation(sampler(), 7, 4, 1, repeats=1))
        self.assertEqual(before, random.getstate())
        stress = [case for case in a["cases"] if case["label"] == "stress"]
        self.assertEqual(len({(case["act"], case["kind"]) for case in stress}), 11)
        self.assertTrue(all(8 <= case["spec"]["hp"] <= 24 for case in stress))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "validation.json"
            path.write_text(json.dumps(a))
            _, first = load_validation(path)
            _, second = load_validation(path)
            for label in first:
                self.assertEqual(
                    [r.state.observation() for r in first[label]], [r.state.observation() for r in second[label]]
                )
            self.assertEqual(len(first["main"]), 4)
            self.assertEqual(len(first["stress"]), 11)

    def test_creation_refuses_overwrite(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixed.json"
            path.write_text("preserve this file")
            with (
                patch("sys.argv", ["validation_set", "--distributions", "unused.json", "--output", str(path)]),
                patch("sys.stderr", new=io.StringIO()),
                self.assertRaises(SystemExit) as stopped,
            ):
                main()
            self.assertEqual(stopped.exception.code, 2)
            self.assertEqual(path.read_text(), "preserve this file")

    def test_changed_version_inputs_and_duplicates_rejected(self) -> None:
        document = build_validation(sampler(), 7, 1, 1)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "validation.json"
            for kind in ("version", "hp", "duplicate", "legacy"):
                value = copy.deepcopy(document)
                if kind == "version":
                    value["native_sha256"] = "wrong"
                elif kind == "hp":
                    value["cases"][0]["spec"]["hp"] = 80 if value["cases"][0]["spec"]["hp"] != 80 else 79
                elif kind == "duplicate":
                    value["cases"].append(value["cases"][0])
                else:
                    value = {"val": []}
                path.write_text(json.dumps(value))
                with self.subTest(kind=kind), self.assertRaises(ValueError):
                    load_validation(path)

    def test_fresh_batches_and_real_optimizer_update(self) -> None:
        torch.set_num_threads(1)
        rng = random.Random(12)
        config = ScenarioConfig(min_floor=1, max_floor=1)
        first, specs = fresh_batch(rng, sampler(), 2, config)
        _, next_specs = fresh_batch(rng, sampler(), 2, config)
        _, repeated = fresh_batch(random.Random(12), sampler(), 2, config)
        self.assertEqual([s.spec_json for s in specs], [s.spec_json for s in repeated])
        self.assertNotEqual([s.spec_json for s in specs], [s.spec_json for s in next_specs])
        excluded = frozenset([int(first[0].seed)])
        filtered, _ = fresh_batch(random.Random(12), sampler(), 1, config, excluded)
        self.assertNotIn(int(filtered[0].seed), excluded)
        model = CombatModel()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        observations = [r.state.observation() for r in first]
        with tempfile.TemporaryDirectory() as directory:
            scores = update(first, specs, model, optimizer, 128, 0.01, Path(directory) / "failure.json")
        self.assertEqual(scores["optimizer_step"], 1)
        self.assertTrue(optimizer.state)
        self.assertEqual(observations, [r.state.observation() for r in first])

    def test_errors_preserve_specs_without_reward_or_update(self) -> None:
        roots, specs = fresh_batch(random.Random(12), sampler(), 1, ScenarioConfig(min_floor=1, max_floor=1))
        model = CombatModel()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        error = SimulatorStepError("test failure", 0, [0], [[0]])
        with tempfile.TemporaryDirectory() as directory:
            for proceed in (False, True):
                path = Path(directory) / f"error-{proceed}.json"
                with patch("train_synthetic.train_batch", side_effect=error), self.assertLogs(level="CRITICAL"):
                    if proceed:
                        result = update(roots, specs, model, optimizer, 128, 0.01, path, continue_on_error=True)
                        self.assertEqual(result["optimizer_step"], 0)
                        self.assertNotIn("defeated", result)
                    else:
                        with self.assertRaises(SimulatorStepError):
                            update(roots, specs, model, optimizer, 128, 0.01, path)
                saved = json.loads(path.read_text())
                self.assertEqual(saved["specifications"][0], json.loads(specs[0].spec_json))
                self.assertEqual(saved["attempted_prefixes"], [[0]])
        self.assertFalse(optimizer.state)

    def test_validation_errors_are_unavailable_not_losses(self) -> None:
        roots, _ = fresh_batch(random.Random(12), sampler(), 1, ScenarioConfig(min_floor=1, max_floor=1))
        model = CombatModel()
        error = SimulatorStepError("test failure", 0, [0], [[0]])
        before = torch.get_rng_state().clone()
        with tempfile.TemporaryDirectory() as directory:
            with patch("train_roots.play_combat", side_effect=error):
                scores = evaluate_sets({"main": roots}, model, 2, 2, Path(directory))
            rows = [json.loads(line) for line in (Path(directory) / "main.jsonl").read_text().splitlines()]
            self.assertEqual([row["policy_seed"] for row in rows], [90000, 90001])
            self.assertEqual(scores["val_main/simulator_error_episodes"], 2)
            self.assertEqual(scores["val_main/episode_coverage"], 0)
            self.assertNotIn("val_main/win_rate_completed", scores)
            self.assertNotIn("val_main/defeated", scores)
        self.assertTrue(torch.equal(before, torch.get_rng_state()))

    def test_baselines_cover_both_sets_and_preserve_records(self) -> None:
        roots, _ = fresh_batch(random.Random(12), sampler(), 1, ScenarioConfig(min_floor=1, max_floor=1))
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch("train_synthetic.evaluate", return_value={"episode_coverage": 0.5}) as random_eval,
                patch("train_synthetic.evaluate_beam", return_value=({"errors": 1}, [{"status": "error"}])) as beam,
            ):
                scores = evaluate_baselines(
                    {"main": roots, "stress": roots}, 3, 512, Path(directory), beam_width=64, beam_transitions=10000
                )
            self.assertEqual(random_eval.call_count, 2)
            self.assertEqual(beam.call_count, 2)
            self.assertEqual(scores["random_main/episode_coverage"], 0.5)
            self.assertEqual(scores["privileged_beam_stress/errors"], 1)
            saved = json.loads((Path(directory) / "baselines.json").read_text())
            self.assertTrue(saved["beam_privileged"])
            self.assertEqual(set(saved["sets"]), {"main", "stress"})
            self.assertEqual(saved["sets"]["main"]["beam_roots"], [{"status": "error"}])

    def test_evaluation_labels_and_rng_isolation(self) -> None:
        torch.set_num_threads(1)
        roots, _ = fresh_batch(random.Random(12), sampler(), 1, ScenarioConfig(min_floor=1, max_floor=1))
        model = CombatModel()
        before = torch.get_rng_state().clone()
        scores = evaluate_sets({"main": roots, "stress": roots}, model, 1, 2)
        self.assertTrue(torch.equal(before, torch.get_rng_state()))
        self.assertIn("val_main/episodes", scores)
        self.assertIn("val_stress/episodes", scores)


if __name__ == "__main__":
    unittest.main()
