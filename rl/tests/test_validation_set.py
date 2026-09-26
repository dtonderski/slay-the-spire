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
from model import CombatValueModel
from rollout_errors import SimulatorStepError
from scenarios import COMBAT_FLOORS, ScenarioConfig
from sts_sim import State
from train import (
    RootPrefetcher,
    evaluate,
    evaluate_baselines,
    fresh_batch,
    play_combats,
    sample_unpadded_action,
    update_with_diagnostics,
)
from validation_set import build_validation, load_validation, main, native_sha256, revalidate_native


def sampler_document(deck_sizes: dict[str, int] | None = None) -> dict:
    band = {
        "runs": 1,
        "deck_sizes": deck_sizes or {"10": 1},
        "cards": {"Strike_R": {"0": 5}, "Defend_R": {"0": 4}, "Bash": {"0": 1}},
        "starters": {"Burning Blood": 1},
        "other_relic_counts": {"0": 1},
        "other_relics": {},
        "potions_obtained": {"block": 1},
        "max_hp": {"80": 1},
    }
    return {
        "schema": 1,
        "ascension": 0,
        "identity_namespace": "sts_sim_public_base_keys",
        "split_role": "fit",
        "bands": {band_for(f): band for f in COMBAT_FLOORS},
    }


def sampler() -> LoadoutSampler:
    return LoadoutSampler(sampler_document())


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

    def test_revalidation_repins_native_without_changing_cases(self) -> None:
        document = build_validation(sampler(), 7, 4, 1, repeats=1)
        old = copy.deepcopy(document)
        old["native_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "already pinned"):
            revalidate_native(document, source_name="v", source_sha256="s", reason="r")
        revalidated = revalidate_native(old, source_name="v.json", source_sha256="abc", reason="parallel steps")
        self.assertEqual(revalidated["native_sha256"], native_sha256())
        self.assertEqual(revalidated["cases"], document["cases"])
        record = revalidated["provenance"]["revalidation"]
        self.assertEqual((record["source_file"], record["source_sha256"]), ("v.json", "abc"))
        self.assertEqual(record["source_native_sha256"], "0" * 64)
        self.assertEqual(old["native_sha256"], "0" * 64)  # The source document is not modified.
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "revalidated.json"
            path.write_text(json.dumps(revalidated))
            _, groups = load_validation(path)
            self.assertEqual(sum(len(roots) for roots in groups.values()), len(document["cases"]))
        tampered = copy.deepcopy(old)
        tampered["cases"][1]["initial_observation_sha256"] = "f" * 64
        with self.assertRaisesRegex(ValueError, "Initial public observation changed"):
            revalidate_native(tampered, source_name="v", source_sha256="s", reason="r")

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

    def test_root_prefetcher_matches_serial_fresh_batches(self) -> None:
        document = sampler_document({"8": 1, "10": 2, "14": 1})
        config = ScenarioConfig(min_floor=1, max_floor=55)
        serial_rng = random.Random(31)
        serial_sampler = LoadoutSampler(document)
        first, _ = fresh_batch(random.Random(31), serial_sampler, 1, config)
        excluded = frozenset([int(first[0].combat_seed)])
        rng = random.Random(31)
        prefetcher = RootPrefetcher(rng, json.dumps(document).encode(), 6, config, excluded)
        try:
            for _ in range(3):
                roots, specs = prefetcher.next()
                expected_roots, expected_specs = fresh_batch(serial_rng, serial_sampler, 6, config, excluded)
                # The training RNG is exactly where serial sampling leaves it.
                self.assertEqual(rng.getstate(), serial_rng.getstate())
                self.assertEqual(
                    [(s.spec_json, s.rejected_loadouts, s.encounter, s.loadout) for s in specs],
                    [(s.spec_json, s.rejected_loadouts, s.encounter, s.loadout) for s in expected_specs],
                )
                self.assertEqual(
                    [(r.combat_seed, r.floor, r.start_hp, r.act) for r in roots],
                    [(r.combat_seed, r.floor, r.start_hp, r.act) for r in expected_roots],
                )
                self.assertNotIn(int(roots[0].combat_seed), excluded)
                self.assertEqual(
                    State.numeric_decisions([r.state for r in roots]),
                    State.numeric_decisions([r.state for r in expected_roots]),
                )
                for root, spec in zip(roots, specs, strict=True):
                    self.assertIs(root.state, spec.state)
            before_close = rng.getstate()
        finally:
            prefetcher.close()
        # Closing discards the prefetched batch without advancing the training RNG.
        self.assertEqual(rng.getstate(), before_close)

    def test_fresh_batches_and_real_optimizer_update(self) -> None:
        torch.set_num_threads(1)
        rng = random.Random(12)
        config = ScenarioConfig(min_floor=1, max_floor=1)
        first, specs = fresh_batch(rng, sampler(), 2, config)
        _, next_specs = fresh_batch(rng, sampler(), 2, config)
        _, repeated = fresh_batch(random.Random(12), sampler(), 2, config)
        self.assertEqual([s.spec_json for s in specs], [s.spec_json for s in repeated])
        self.assertNotEqual([s.spec_json for s in specs], [s.spec_json for s in next_specs])
        excluded = frozenset([int(first[0].combat_seed)])
        filtered, _ = fresh_batch(random.Random(12), sampler(), 1, config, excluded)
        self.assertNotIn(int(filtered[0].combat_seed), excluded)
        model = CombatValueModel()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        observations = [r.state.observation() for r in first]
        with tempfile.TemporaryDirectory() as directory:
            scores = update_with_diagnostics(
                first, specs, model, optimizer, 128, 0.01, Path(directory) / "failure.json"
            )
        self.assertEqual(scores["optimizer_step"], 1)
        self.assertTrue(optimizer.state)
        self.assertEqual(observations, [r.state.observation() for r in first])
        self.assertEqual([root.start_hp for root in first], [root.state.player_hp() for root in first])
        self.assertEqual(
            [root.start_hp for root in first],
            [root.state.observation().context.player_hp for root in first],
        )

    def test_errors_preserve_specs_without_reward_or_update(self) -> None:
        roots, specs = fresh_batch(random.Random(12), sampler(), 1, ScenarioConfig(min_floor=1, max_floor=1))
        model = CombatValueModel()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        error = SimulatorStepError("test failure", 0, [0], [[0]])
        with tempfile.TemporaryDirectory() as directory:
            for proceed in (False, True):
                path = Path(directory) / f"error-{proceed}.json"
                with patch("train.train_batch", side_effect=error), self.assertLogs(level="CRITICAL"):
                    if proceed:
                        result = update_with_diagnostics(
                            roots, specs, model, optimizer, 128, 0.01, path, continue_on_error=True
                        )
                        self.assertEqual(result["optimizer_step"], 0)
                        self.assertNotIn("defeated", result)
                    else:
                        with self.assertRaises(SimulatorStepError):
                            update_with_diagnostics(roots, specs, model, optimizer, 128, 0.01, path)
                saved = json.loads(path.read_text())
                self.assertEqual(saved["specifications"][0], json.loads(specs[0].spec_json))
                self.assertEqual(saved["attempted_prefixes"], [[0]])
        self.assertFalse(optimizer.state)

    def test_validation_errors_are_unavailable_not_losses(self) -> None:
        roots, _ = fresh_batch(random.Random(12), sampler(), 1, ScenarioConfig(min_floor=1, max_floor=1))
        model = CombatValueModel()
        error = SimulatorStepError("test failure", 0, [0], [[0]])
        before = torch.get_rng_state().clone()
        with tempfile.TemporaryDirectory() as directory:
            with patch("train.play_combats", side_effect=error):
                scores = evaluate(roots, model, 2, 2, error_path=Path(directory) / "main.jsonl")
            rows = [json.loads(line) for line in (Path(directory) / "main.jsonl").read_text().splitlines()]
            self.assertEqual([row["policy_seed"] for row in rows], [90000, 90001])
            self.assertEqual(scores["simulator_error_episodes"], 2)
            self.assertEqual(scores["episode_coverage"], 0)
            self.assertNotIn("win_rate_completed", scores)
            self.assertNotIn("defeated", scores)
        self.assertTrue(torch.equal(before, torch.get_rng_state()))

    def test_baselines_cover_main_and_preserve_records(self) -> None:
        roots, _ = fresh_batch(random.Random(12), sampler(), 1, ScenarioConfig(min_floor=1, max_floor=1))
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch("train.evaluate", return_value={"episode_coverage": 0.5}) as random_eval,
                patch("train.evaluate_beam", return_value=({"errors": 1}, [{"status": "error"}])) as beam,
            ):
                scores = evaluate_baselines(roots, 3, 512, Path(directory), beam_width=64, beam_transitions=10000)
            self.assertEqual(random_eval.call_count, 1)
            self.assertEqual(beam.call_count, 1)
            self.assertEqual(scores["random_main/episode_coverage"], 0.5)
            self.assertEqual(scores["privileged_beam_main/errors"], 1)
            saved = json.loads((Path(directory) / "baselines.json").read_text())
            self.assertTrue(saved["beam_privileged"])
            self.assertEqual(set(saved["sets"]), {"main"})
            self.assertEqual(saved["sets"]["main"]["beam_roots"], [{"status": "error"}])

    def test_evaluation_labels_and_rng_isolation(self) -> None:
        torch.set_num_threads(1)
        roots, _ = fresh_batch(random.Random(12), sampler(), 1, ScenarioConfig(min_floor=1, max_floor=1))
        model = CombatValueModel()
        before = torch.get_rng_state().clone()
        scores = evaluate(roots, model, 1, 2)
        self.assertTrue(torch.equal(before, torch.get_rng_state()))
        self.assertIn("episodes", scores)

    def test_batched_evaluation_preserves_streams_and_versions_policy_scores(self) -> None:
        torch.set_num_threads(1)
        roots, _ = fresh_batch(random.Random(12), sampler(), 2, ScenarioConfig(min_floor=1, max_floor=1))
        serial = [
            play_combats([root.state], None, max_decisions=12, rng=random.Random(90000 + index))[0]
            for index, root in enumerate(roots)
        ]
        batched = play_combats(
            [root.state for root in roots],
            None,
            max_decisions=12,
            rng=random.Random(0),
            episode_rngs=[random.Random(90000 + index) for index in range(len(roots))],
        )
        fields = lambda episodes: [(ep.reward, ep.won, ep.hp, ep.decisions) for ep in episodes]
        self.assertEqual(fields(serial), fields(batched))
        self.assertEqual(
            evaluate(roots, None, 2, 12, batch_size=1),
            evaluate(roots, None, 2, 12, batch_size=4),
        )
        devices = ["cpu"] + (["cuda"] if torch.cuda.is_available() else [])
        for device in devices:
            model = CombatValueModel().to(device)
            before = torch.get_rng_state().clone()
            first = evaluate(roots, model, 2, 8, batch_size=4)
            second = evaluate(roots, model, 2, 8, batch_size=4)
            self.assertEqual(first, second)
            self.assertEqual(
                evaluate(roots, model, 2, 8, batch_size=1),
                evaluate(roots, model, 2, 8, batch_size=1),
            )
            self.assertTrue(torch.equal(before, torch.get_rng_state()))
        with self.assertRaises(ValueError):
            evaluate(roots, None, 1, 1, batch_size=0)

    def test_foreign_padding_does_not_change_per_fight_rng(self) -> None:
        legal = torch.tensor([0.2, -0.4])
        choices = []
        states = []
        for width in (2, 8, 64):
            padded = torch.full((width,), float("-inf"))
            padded[:2] = legal
            torch.manual_seed(12345)
            choices.append(sample_unpadded_action(padded, 2))
            choices.append(sample_unpadded_action(padded, 2))
            states.append(torch.get_rng_state().clone())
        self.assertEqual(choices[0::2], [choices[0]] * 3)
        self.assertEqual(choices[1::2], [choices[1]] * 3)
        for state in states[1:]:
            self.assertTrue(torch.equal(states[0], state))

    def test_per_fight_generator_matches_historical_global_draws(self) -> None:
        from train import _policy_generator

        cases = [
            (torch.tensor([0.2, -0.4, 1.5, -2.0]), 3),
            # softmax(raw logits) rounds differently from Categorical normalization.
            (torch.tensor([0.9145662784576416, -2.2833824157714844], dtype=torch.float32), 2),
        ]
        for logits, n_legal in cases:
            for seed in (1, 2, 12345, 90000, 2**31 - 1):
                torch.manual_seed(seed)
                historical = [sample_unpadded_action(logits, n_legal) for _ in range(6)]
                generator = _policy_generator(seed, logits.device)
                actual = [sample_unpadded_action(logits, n_legal, generator) for _ in range(6)]
                self.assertEqual(actual, historical)
        if torch.cuda.is_available():
            cuda_cases = [
                (cases[1][0].to(device="cuda"), 2),
                (torch.tensor([0.2, -0.4, 1.5, -2.0, 0.05], dtype=torch.float32, device="cuda"), 5),
            ]
            for logits, n_legal in cuda_cases:
                for seed in (1, 2, 12345):
                    torch.manual_seed(seed)
                    historical = [sample_unpadded_action(logits, n_legal) for _ in range(4)]
                    generator = _policy_generator(seed, logits.device)
                    actual = [sample_unpadded_action(logits, n_legal, generator) for _ in range(4)]
                    self.assertEqual(actual, historical, f"CUDA seed {seed}")
        logits = cases[0][0]
        before = torch.get_rng_state().clone()
        generator = _policy_generator(99, torch.device("cpu"))
        sample_unpadded_action(logits, 3, generator)
        self.assertTrue(torch.equal(before, torch.get_rng_state()))

    def test_batched_simulator_failure_is_not_replaced_by_serial_success(self) -> None:
        roots, _ = fresh_batch(random.Random(12), sampler(), 2, ScenarioConfig(min_floor=1, max_floor=1))
        calls = []

        def batch_only_failure(states, *args, **kwargs):
            calls.append(len(states))
            if len(states) > 1:
                raise SimulatorStepError("batch only", 3, [0, 1], [[4], [5, 6]])
            return [Episode(1.0, True, 70, 1)]

        from train import Episode

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "errors.jsonl"
            with patch("train.play_combats", side_effect=batch_only_failure):
                scores = evaluate(roots, CombatValueModel(), 1, 4, error_path=path, batch_size=2)
            rows = [json.loads(line) for line in path.read_text().splitlines()]
        self.assertEqual(calls, [2])
        self.assertEqual(scores["simulator_error_episodes"], 2)
        self.assertEqual(scores["batched_protocol_unavailable_episodes"], 2)
        self.assertEqual(scores["episode_coverage"], 0)
        self.assertNotIn("win_rate_completed", scores)
        self.assertEqual([row["source"] for row in rows], ["batched_chunk", "batched_chunk"])
        self.assertEqual([row["fallback"] for row in rows], ["none", "none"])
        self.assertEqual(rows[0]["error"], "batch only")
        self.assertEqual(rows[0]["attempted_prefixes"], [4])
        self.assertEqual(rows[1]["attempted_prefixes"], [5, 6])
        self.assertEqual(rows[0]["step"], 3)


if __name__ == "__main__":
    unittest.main()
