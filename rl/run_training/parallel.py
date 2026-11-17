"""Sharded run collection, in-process or across spawned worker processes.

Each shard is one `collect_many` call over a contiguous slice of the jobs, so for a
fixed shard count every episode and journal is identical for any worker count.
Changing the shard count changes which episodes share batched forwards, which can
change floating-point rounding.
"""

import gc
import io
import json
import multiprocessing
import signal
import time
from collections.abc import Callable
from concurrent.futures import ProcessPoolExecutor, wait
from dataclasses import dataclass
from pathlib import Path

import torch
from sts_sim import State

from run_training.collector import (
    CollectionFailure,
    CollectionJob,
    FrozenCombat,
    RunEpisode,
    collect_many,
)
from run_training.model import MacroPolicy
from run_training.rewards import Objective


@dataclass(frozen=True)
class JobSpec:
    """A natural-start collection job as plain values that can cross processes."""

    seed: str
    journal: Path
    policy_seed: int
    objective: Objective
    final_act: bool
    avoid_prismatic: bool
    commit_navigation: bool
    exploration_profile: str
    encoder: str
    direct_smith: bool
    final_card_skip: bool
    decision_diagnostics: bool
    max_actions: int
    training_rng_seed: int | None


def collection_job(spec: JobSpec, device: str) -> CollectionJob:
    return CollectionJob(
        spec.seed,
        {
            "objective": spec.objective,
            "final_act": spec.final_act,
            "avoid_prismatic": spec.avoid_prismatic,
            "commit_navigation": spec.commit_navigation,
            "exploration_profile": spec.exploration_profile,
            "encoder": spec.encoder,
            "direct_smith": spec.direct_smith,
            "final_card_skip": spec.final_card_skip,
            "decision_diagnostics": spec.decision_diagnostics,
            "max_actions": spec.max_actions,
            "macro_rng": torch.Generator(device=device).manual_seed(spec.policy_seed),
            "combat_rng": torch.Generator(device=device).manual_seed(
                spec.policy_seed ^ 0x5DEECE66D
            ),
            "journal": spec.journal,
            "state_factory": State.new,
            "training_rng_seed": spec.training_rng_seed,
        },
    )


def record_failure(errors: Path, job: CollectionJob, error: CollectionFailure) -> None:
    with errors.open("a") as log:
        log.write(
            json.dumps(
                {
                    "seed": job.seed,
                    "journal": str(job.options["journal"]),
                    "error": str(error),
                }
            )
            + "\n"
        )


def split[T](items: list[T], shards: int) -> list[list[T]]:
    n = len(items)
    return [items[n * k // shards : n * (k + 1) // shards] for k in range(shards)]


def collect_sharded(
    jobs: list[CollectionJob],
    macro: MacroPolicy,
    combat: FrozenCombat,
    *,
    shards: int,
    width: int,
    continue_on_failure: bool,
    errors: Path,
    stop_requested: Callable[[], bool],
    overlap_inference: bool = False,
) -> list[RunEpisode]:
    return [
        episode
        for shard in split(jobs, shards)
        for episode in collect_many(
            shard,
            macro,
            combat,
            width=width,
            continue_on_failure=continue_on_failure,
            stop_requested=stop_requested,
            on_failure=lambda job, error: record_failure(errors, job, error),
            overlap_inference=overlap_inference,
        )
    ]


def state_bytes(model: MacroPolicy) -> bytes:
    buffer = io.BytesIO()
    torch.save(model.state_dict(), buffer)
    return buffer.getvalue()


@dataclass(frozen=True)
class _Worker:
    macro: MacroPolicy
    combat: FrozenCombat
    device: str
    width: int
    continue_on_failure: bool
    errors: Path
    stop: Callable[[], bool]
    overlap_inference: bool


_WORKER: _Worker | None = None  # Set once per worker process by `_start_worker`.


def tune_worker_runtime() -> None:
    """Collector-process garbage collection settings; no effect on any result.

    Models and imports are frozen out of collection passes, and generation-0
    passes run every 10,000 net allocations instead of 700. With the defaults a
    worker spent about 4% of collection time in the collector.
    """
    gc.freeze()
    gc.set_threshold(10_000, 10, 10)


def _start_worker(
    model_type: type[MacroPolicy],
    model_width: int,
    device: str,
    combat: bytes,
    adapt_legacy: bool,
    width: int,
    continue_on_failure: bool,
    errors: Path,
    stop,
    overlap_inference: bool,
) -> None:
    global _WORKER
    # Only the trainer decides to stop; it sets the shared flag.
    signal.signal(signal.SIGINT, signal.SIG_IGN)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    torch.set_num_threads(1)
    _WORKER = _Worker(
        model_type(model_width).to(device),
        FrozenCombat(io.BytesIO(combat), device, adapt_legacy=adapt_legacy),
        device,
        width,
        continue_on_failure,
        errors,
        lambda: bool(stop.value),
        overlap_inference,
    )
    tune_worker_runtime()


def _collect_shard(
    weights: bytes, specs: list[JobSpec]
) -> tuple[list[RunEpisode], float]:
    """One shard's episodes and the seconds this worker spent on it."""
    started = time.perf_counter()
    worker = _WORKER
    assert worker is not None
    worker.macro.load_state_dict(
        torch.load(io.BytesIO(weights), map_location=worker.device, weights_only=True),
        strict=True,
    )
    episodes = collect_sharded(
        [collection_job(spec, worker.device) for spec in specs],
        worker.macro,
        worker.combat,
        shards=1,
        width=worker.width,
        continue_on_failure=worker.continue_on_failure,
        errors=worker.errors,
        stop_requested=worker.stop,
        overlap_inference=worker.overlap_inference,
    )
    return episodes, time.perf_counter() - started


class ParallelCollector:
    """Persistent spawned workers, each a full collector copy running whole shards.

    `last_busy_seconds` holds each shard's in-worker seconds from the last batch.
    """

    def __init__(
        self,
        workers: int,
        shards: int,
        *,
        model_type: type[MacroPolicy],
        model_width: int,
        device: str,
        combat: bytes,
        adapt_legacy: bool,
        width: int,
        continue_on_failure: bool,
        errors: Path,
        stop_requested: Callable[[], bool],
        overlap_inference: bool = False,
    ) -> None:
        # spawn: forking a process that has initialized CUDA is unsafe.
        context = multiprocessing.get_context("spawn")
        self.stop = context.Value("b", 0, lock=False)
        self.workers = workers
        self.shards = shards
        self.stop_requested = stop_requested
        self.last_busy_seconds: list[float] = []
        self.executor = ProcessPoolExecutor(
            workers,
            mp_context=context,
            initializer=_start_worker,
            initargs=(
                model_type,
                model_width,
                device,
                combat,
                adapt_legacy,
                width,
                continue_on_failure,
                errors,
                self.stop,
                overlap_inference,
            ),
        )

    def collect(self, weights: bytes, specs: list[JobSpec]) -> list[RunEpisode]:
        futures = [
            self.executor.submit(_collect_shard, weights, shard)
            for shard in split(specs, self.shards)
        ]
        pending = set(futures)
        while pending:
            done, pending = wait(pending, timeout=0.1)
            if self.stop_requested() or any(f.exception() for f in done):
                self.stop.value = 1
        # The first failed shard in shard order, after every shard has stopped.
        results = [future.result() for future in futures]
        self.last_busy_seconds = [seconds for _, seconds in results]
        return [episode for episodes, _ in results for episode in episodes]

    def idle_fraction(self, wall_seconds: float) -> float:
        """Share of the workers' wall time with no shard running in the last batch.

        Includes start-up, weight loading, transfer and the uneven shard tail.
        """
        busy = sum(self.last_busy_seconds)
        return max(0.0, 1 - busy / (self.workers * max(wall_seconds, 1e-9)))

    def close(self) -> None:
        self.executor.shutdown(cancel_futures=True)
