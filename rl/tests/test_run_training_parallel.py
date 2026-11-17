"""Sharded collection is independent of the worker count; not gameplay parity evidence."""

import json
import multiprocessing
import tempfile
import unittest
from pathlib import Path

import numpy as np
import torch
from model import CombatValueModel
from run_training.collector import (
    CollectionFailure,
    FrozenCombat,
    RunEpisode,
    collect_many,
)
from run_training.parallel import (
    JobSpec,
    ParallelCollector,
    collect_sharded,
    collection_job,
    split,
    state_bytes,
)
from run_training.token_model import TokenMacroModel
from run_training.trainer import main

SEEDS = ["11", "12", "13", "14", "15", "16", "17"]
TEMPORARY: tempfile.TemporaryDirectory
COMBAT: Path
MACRO: TokenMacroModel


def setUpModule():
    global TEMPORARY, COMBAT, MACRO
    TEMPORARY = tempfile.TemporaryDirectory()
    torch.manual_seed(5)
    combat = CombatValueModel(d_model=16, action_dim=16, n_layers=1, precision="fp32")
    COMBAT = Path(TEMPORARY.name) / "combat.pt"
    torch.save(
        {
            "config": {"model_width": 16, "model_layers": 1},
            "model": combat.state_dict(),
        },
        COMBAT,
    )
    MACRO = TokenMacroModel(16)
    with torch.no_grad():
        for head in (MACRO.policy, MACRO.value):
            head.weight.normal_(0, 0.3)


def tearDownModule():
    TEMPORARY.cleanup()


def specs(directory: Path, seeds=SEEDS, max_actions=120) -> list[JobSpec]:
    return [
        JobSpec(
            seed,
            directory / f"{i}.jsonl",
            500 + i,
            objective="act1",
            final_act=True,
            avoid_prismatic=False,
            commit_navigation=True,
            exploration_profile="none",
            encoder="tokens",
            direct_smith=False,
            final_card_skip=False,
            decision_diagnostics=False,
            max_actions=max_actions,
            training_rng_seed=None,
        )
        for i, seed in enumerate(seeds)
    ]


def pool(
    workers,
    shards,
    directory,
    device="cpu",
    continue_on_failure=False,
    overlap_inference=False,
):
    return ParallelCollector(
        workers,
        shards,
        model_type=TokenMacroModel,
        model_width=16,
        device=device,
        combat=COMBAT.read_bytes(),
        adapt_legacy=False,
        width=3,
        continue_on_failure=continue_on_failure,
        errors=directory / "errors.jsonl",
        stop_requested=lambda: False,
        overlap_inference=overlap_inference,
    )


def run(directory: Path, shards: int, workers: int, device="cpu") -> list[RunEpisode]:
    macro = MACRO.to(device)
    if not workers:
        return collect_sharded(
            [collection_job(spec, device) for spec in specs(directory)],
            macro,
            FrozenCombat(COMBAT, device),
            shards=shards,
            width=3,
            continue_on_failure=False,
            errors=directory / "errors.jsonl",
            stop_requested=lambda: False,
        )
    collector = pool(workers, shards, directory, device)
    try:
        return collector.collect(state_bytes(macro), specs(directory))
    finally:
        collector.close()


class ParallelCollectionTests(unittest.TestCase):
    def setUp(self):
        torch.set_num_threads(1)
        self.directory = Path(tempfile.mkdtemp(dir=TEMPORARY.name))

    def tearDown(self):
        self.assertEqual(multiprocessing.active_children(), [])

    def assert_identical(self, first, second, journals):
        self.assertEqual(len(first), len(second))
        self.assertTrue(any(episode.steps for episode in first))
        for a, b in zip(first, second, strict=True):
            for name in ("status", "reward", "accepted", "floor", "error"):
                self.assertEqual(getattr(a, name), getattr(b, name))
            self.assertEqual(a.termination_reason, b.termination_reason)
            self.assertEqual(len(a.steps), len(b.steps))
            for x, y in zip(a.steps, b.steps, strict=True):
                self.assertEqual(
                    (x.choice, x.logits, x.value), (y.choice, y.logits, y.value)
                )
                self.assertTrue(np.array_equal(x.inputs.ids, y.inputs.ids))
                self.assertTrue(np.array_equal(x.inputs.numbers, y.inputs.numbers))
                self.assertEqual(
                    (x.inputs.count, x.inputs.eligibility, x.inputs.temperature),
                    (y.inputs.count, y.inputs.eligibility, y.inputs.temperature),
                )
        for one, other in zip(*journals, strict=True):
            self.assertEqual(one.read_bytes(), other.read_bytes())

    def test_split_is_contiguous_ordered_and_uneven(self):
        self.assertEqual(split(list(range(7)), 3), [[0, 1], [2, 3], [4, 5, 6]])
        self.assertEqual(split([1, 2], 3), [[], [1], [2]])
        self.assertEqual(split([1, 2, 3], 1), [[1, 2, 3]])

    def test_results_and_journals_do_not_depend_on_worker_count(self):
        directories = [self.directory / str(n) for n in (0, 1, 3)]
        results = [run(d, 3, n) for d, n in zip(directories, (0, 1, 3), strict=True)]
        journals = [sorted(d.glob("*.jsonl")) for d in directories]
        self.assertEqual(len(journals[0]), len(SEEDS))
        for result, paths in zip(results[1:], journals[1:], strict=True):
            self.assert_identical(results[0], result, (journals[0], paths))

    def test_overlapped_inference_equals_serial_collection(self):
        reference = self.directory / "reference"
        expected = run(reference, 1, 0)
        inline = self.directory / "inline"
        actual = collect_many(
            [collection_job(spec, "cpu") for spec in specs(inline)],
            MACRO,
            FrozenCombat(COMBAT, "cpu"),
            width=3,
            overlap_inference=True,
        )
        worker = self.directory / "worker"
        collector = pool(2, 3, worker, overlap_inference=True)
        try:
            pooled = collector.collect(state_bytes(MACRO), specs(worker))
        finally:
            collector.close()
        sharded = run(self.directory / "sharded", 3, 0)
        for result, directory, base, base_directory in (
            (actual, inline, expected, reference),
            (pooled, worker, sharded, self.directory / "sharded"),
        ):
            self.assert_identical(
                base,
                result,
                (
                    sorted(base_directory.glob("*.jsonl")),
                    sorted(directory.glob("*.jsonl")),
                ),
            )

    def legacy(self, device):
        reference = self.directory / "reference"
        expected = collect_many(
            [collection_job(spec, device) for spec in specs(reference)],
            MACRO.to(device),
            FrozenCombat(COMBAT, device),
            width=3,
        )
        actual = run(self.directory / "worker", 1, 1, device)
        self.assert_identical(
            expected,
            actual,
            (
                sorted(reference.glob("*.jsonl")),
                sorted((self.directory / "worker").glob("*.jsonl")),
            ),
        )

    def test_one_shard_in_one_worker_equals_plain_collection(self):
        self.legacy("cpu")

    @unittest.skipUnless(torch.cuda.is_available(), "CUDA unavailable")
    def test_one_shard_in_one_worker_equals_plain_collection_cuda(self):
        try:
            self.legacy("cuda")
        finally:
            MACRO.to("cpu")

    def test_failing_shard_aborts_or_becomes_failed_episode(self):
        seeds = ["11", "12", "-1", "14", "15", "16"]
        collector = pool(2, 3, self.directory)
        with self.assertRaisesRegex(CollectionFailure, "seed is invalid"):
            collector.collect(state_bytes(MACRO), specs(self.directory, seeds))
        collector.close()
        errors = (self.directory / "errors.jsonl").read_text().splitlines()
        self.assertEqual([json.loads(line)["seed"] for line in errors], ["-1"])

        directory = self.directory / "continue"
        collector = pool(2, 3, directory, continue_on_failure=True)
        try:
            result = collector.collect(state_bytes(MACRO), specs(directory, seeds))
        finally:
            collector.close()
        self.assertEqual(
            [episode.status == "error" for episode in result],
            [False, False, True, False, False, False],
        )

    def test_preset_stop_returns_incomplete_batch(self):
        collector = pool(2, 2, self.directory)
        collector.stop.value = 1
        try:
            self.assertEqual(
                collector.collect(state_bytes(MACRO), specs(self.directory)), []
            )
        finally:
            collector.close()


class ParallelTrainerTests(unittest.TestCase):
    def test_trainer_records_settings_and_resume_ignores_worker_count(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            seeds = root / "seeds.json"
            seeds.write_text('["1", "2"]')
            common = [
                "--combat-checkpoint",
                str(COMBAT),
                "--validation-seeds",
                str(seeds),
                "--output-root",
                directory,
                "--encoder",
                "tokens",
                "--model-width",
                "16",
                "--objective",
                "heart_keys_budget",
                "--max-actions",
                "60",
                "--batch-size",
                "2",
                "--collection-width",
                "2",
                "--updates",
                "1",
            ]
            main(
                [
                    *common,
                    "--run-id",
                    "first",
                    "--collection-workers",
                    "2",
                    "--collection-shards",
                    "2",
                    "--collection-worker-device",
                    "cpu",
                ]
            )
            config = json.loads((root / "first/config.json").read_text())
            self.assertEqual(
                (
                    config["collection_workers"],
                    config["collection_shards"],
                    config["collection_worker_device"],
                ),
                (2, 2, "cpu"),
            )
            metrics = [
                json.loads(line)
                for line in (root / "first/metrics.jsonl").read_text().splitlines()
            ]
            self.assertEqual(metrics[1]["train/optimizer_step"], 1.0)
            self.assertEqual(metrics[2]["validation/attempts"], 2.0)
            resume = ["--resume-from", str(root / "first/latest.pt")]
            main(
                [
                    *common,
                    *resume,
                    "--run-id",
                    "second",
                    "--collection-workers",
                    "1",
                    "--collection-shards",
                    "2",
                    "--collection-worker-device",
                    "cpu",
                ]
            )
            self.assertTrue((root / "second/finished.json").exists())
            with self.assertRaisesRegex(ValueError, "mismatch: collection_shards"):
                main(
                    [
                        *common,
                        *resume,
                        "--run-id",
                        "third",
                        "--collection-workers",
                        "2",
                        "--collection-shards",
                        "1",
                    ]
                )
            # A checkpoint from another build resumes only with the explicit flag.
            other = torch.load(root / "first/latest.pt", weights_only=True)
            other["config"]["native_sha256"] = "0" * 64
            torch.save(other, root / "other-build.pt")
            rebuilt = [
                *common,
                "--resume-from",
                str(root / "other-build.pt"),
                "--collection-workers",
                "1",
                "--collection-shards",
                "2",
                "--collection-worker-device",
                "cpu",
            ]
            with self.assertRaisesRegex(ValueError, "mismatch: native_sha256"):
                main([*rebuilt, "--run-id", "fourth"])
            main([*rebuilt, "--run-id", "fifth", "--resume-across-builds"])
            config = json.loads((root / "fifth/config.json").read_text())
            self.assertEqual(
                config["resumed_from_builds"],
                [{"optimizer_updates": 1, "native_sha256": "0" * 64}],
            )
        self.assertEqual(multiprocessing.active_children(), [])


if __name__ == "__main__":
    unittest.main()
