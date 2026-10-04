"""Experimental A0 Monte Carlo actor-critic, invoked only by train.py --task run.

Completed episodes, gamma=1, one on-policy update, fresh run value head. No PPO,
privileged search, synthetic HP, or optimizer step after incomplete collection.
"""

import argparse
import hashlib
import json
import math
import os
import random
import shutil
import signal
import time
from contextlib import contextmanager
from pathlib import Path
from threading import Event

import torch
import wandb
from sts_sim import FAIR_RUN_OBSERVATION_SCHEMA_VERSION, _native
from torch.distributions import Categorical

from run_training.collector import CollectionFailure, FrozenCombat, RunEpisode, collect
from run_training.metrics import behavior_scores
from run_training.model import FEATURE_VERSION, MacroModel
from run_training.rewards import (
    ACT1_BOSS_FLOOR,
    ACT1_CLEAR_BONUS,
    REWARD_PROTOCOL,
    succeeded,
    terminal_parts,
)

PROTOCOL = "a0_macro_mc_frozen_sampled_combat_v3"


@contextmanager
def graceful_stop():
    requested = Event()
    previous = {sig: signal.getsignal(sig) for sig in (signal.SIGTERM, signal.SIGINT)}
    for sig in previous:
        signal.signal(sig, lambda _signum, _frame: requested.set())
    try:
        yield requested
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def load_warm_start(model: MacroModel, path: Path, settings: dict) -> dict:
    """Explicit weights-only transfer across simulator/source revisions, not resume."""
    source = torch.load(path, map_location="cpu", weights_only=True)
    for key in (
        "protocol",
        "feature_version",
        "reward_protocol",
        "model_width",
        "objective",
        "observation_schema",
        "final_act",
        "ascension",
        "gamma",
        "combat_sha256",
    ):
        if source["config"].get(key) != settings.get(key):
            raise ValueError(f"Run warm-start mismatch: {key}")
    model.load_state_dict(source["model"], strict=True)
    if not all(torch.isfinite(p).all() for p in model.parameters()):
        raise ValueError("Nonfinite macro checkpoint")
    return {
        "parent_iteration": source["iteration"],
        "parent_optimizer_updates": source["optimizer_updates"],
        "parent_native_sha256": source["config"]["native_sha256"],
    }


def update(
    episodes: list[RunEpisode],
    model: MacroModel,
    optimizer: torch.optim.Optimizer,
    *,
    entropy_coef: float,
    value_coef: float,
) -> dict[str, float]:
    """Validate a whole batch before backward; recompute from stored public inputs."""
    if not episodes or any(ep.reward is None for ep in episodes):
        raise CollectionFailure(
            "Incomplete batch: no optimizer update, no cutoff-as-death targets"
        )
    if any(
        ep.reward
        != terminal_parts(ep.objective, ep.status, ep.furthest_act1_floor).total
        for ep in episodes
    ):
        raise ValueError("Terminal returns must match the declared reward protocol")
    count = sum(len(ep.steps) for ep in episodes)
    optimizer.zero_grad(set_to_none=True)
    if not count:
        return {"optimizer_step": 0.0, "macro_decisions": 0.0}
    # Check all inference/recomputation pairs before accumulating any gradients.
    with torch.no_grad():
        for episode in episodes:
            for step in episode.steps:
                logits, value = model(step.inputs)
                expected = torch.tensor(step.logits, device=logits.device)
                if not torch.allclose(
                    logits, expected, atol=1e-6, rtol=1e-5
                ) or not math.isclose(
                    float(value), step.value, abs_tol=1e-6, rel_tol=1e-5
                ):
                    raise ValueError(
                        "On-policy recomputation mismatch; refusing optimizer update"
                    )
    total_loss = 0.0
    for episode in episodes:
        for step in episode.steps:
            logits, value = model(step.inputs)
            distribution = Categorical(logits=logits)
            choice = torch.tensor(step.choice, device=logits.device)
            assert episode.reward is not None
            target = episode.reward
            # Normalize policy sums by episodes: unbiased gradient of episode
            # return, rather than changing objective with variable run length.
            actor = -distribution.log_prob(choice) * (target - step.value)
            loss = (
                actor / len(episodes)
                + (
                    value_coef * (value - target).square()
                    - entropy_coef * distribution.entropy()
                )
                / count
            )
            if not torch.isfinite(loss):
                optimizer.zero_grad(set_to_none=True)
                raise ValueError("Nonfinite run loss")
            loss.backward()
            total_loss += float(loss.detach())
    gradients = [p.grad for p in model.parameters() if p.grad is not None]
    if not all(torch.isfinite(grad).all() for grad in gradients):
        optimizer.zero_grad(set_to_none=True)
        raise ValueError("Nonfinite run gradient")
    torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0, error_if_nonfinite=True)
    optimizer.step()
    return {"loss": total_loss, "optimizer_step": 1.0, "macro_decisions": float(count)}


def scores(episodes: list[RunEpisode]) -> dict[str, float]:
    complete = [ep for ep in episodes if ep.reward is not None]
    result = {
        "attempts": float(len(episodes)),
        "completed": float(len(complete)),
        "cutoffs": float(sum(ep.status == "cutoff" for ep in episodes)),
        "errors": float(sum(ep.status == "error" for ep in episodes)),
        "successes": float(sum(succeeded(ep.objective, ep.status) for ep in complete)),
        "mean_floor": sum(ep.floor for ep in episodes) / max(1, len(episodes)),
        "accepted_actions": float(sum(ep.accepted for ep in episodes)),
        "mean_furthest_act1_floor": sum(ep.furthest_act1_floor for ep in episodes)
        / max(1, len(episodes)),
        "reward/terminal_samples": float(len(complete)),
        **behavior_scores([ep.behavior for ep in episodes]),
    }
    if complete:
        result["completed_success_rate"] = sum(
            succeeded(ep.objective, ep.status) for ep in complete
        ) / len(complete)
        parts = [
            terminal_parts(ep.objective, ep.status, ep.furthest_act1_floor)
            for ep in complete
        ]
        result["reward/mean"] = sum(p.total for p in parts) / len(parts)
        result["reward/progress_mean"] = sum(p.progress for p in parts) / len(parts)
        result["reward/clear_bonus_mean"] = sum(p.clear_bonus for p in parts) / len(
            parts
        )
    # Lower bound uses all attempts. Never label cutoff as a true game loss.
    if episodes:
        result["success_lower_bound"] = sum(
            succeeded(ep.objective, ep.status) for ep in complete
        ) / len(episodes)
    return result


def validation_seeds(path: Path) -> list[str]:
    values = json.loads(path.read_text())
    if not isinstance(values, list) or not values:
        raise ValueError(
            "Validation seeds must be a nonempty JSON list of decimal strings"
        )
    if any(
        not isinstance(s, str)
        or not s.isascii()
        or not s.isdecimal()
        or not 0 <= int(s) < 2**63
        for s in values
    ):
        raise ValueError("Validation seeds must be decimal strings in [0, 2**63)")
    canonical = [str(int(s)) for s in values]
    if len(set(canonical)) != len(canonical):
        raise ValueError("Duplicate validation seed")
    return canonical


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__, prog="train.py --task run")
    result.add_argument("--run-id", required=True)
    result.add_argument(
        "--combat-checkpoint",
        type=Path,
        required=True,
        help="Trusted combat checkpoint; frozen weights only",
    )
    result.add_argument(
        "--validation-seeds",
        type=Path,
        required=True,
        help="Held-out JSON list of decimal run seeds",
    )
    result.add_argument(
        "--objective", choices=("act1", "act3", "heart"), default="act1"
    )
    result.add_argument(
        "--final-act", action=argparse.BooleanOptionalAction, default=True
    )
    result.add_argument("--updates", type=int, default=1)
    result.add_argument(
        "--batch-size", type=int, default=8, help="Complete natural A0 runs per update"
    )
    result.add_argument(
        "--max-actions",
        type=int,
        default=5000,
        help="Total accepted actions per run, including combat/UI",
    )
    result.add_argument("--eval-every", type=int, default=1)
    result.add_argument(
        "--max-hours",
        type=float,
        help="Cooperative total wall-clock bound, including evaluation",
    )
    result.add_argument(
        "--continue-on-collection-failure",
        action="store_true",
        help="Explicit experimental mode: quarantine whole incomplete batches; never retry an action",
    )
    result.add_argument(
        "--max-consecutive-skips",
        type=int,
        default=8,
        help="Stop rather than repeatedly sampling unusable batches",
    )
    result.add_argument("--compress-journals", action="store_true")
    result.add_argument("--model-width", type=int, default=64)
    result.add_argument("--seed", type=int, default=123)
    result.add_argument("--lr", type=float, default=1e-4)
    result.add_argument("--entropy-coef", type=float, default=0.01)
    result.add_argument("--value-coef", type=float, default=0.1)
    result.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    result.add_argument(
        "--collect-only",
        action="store_true",
        help="Pilot collection without any optimizer updates",
    )
    initialization = result.add_mutually_exclusive_group()
    initialization.add_argument(
        "--resume-from",
        type=Path,
        help="Trusted same-protocol checkpoint; always use a new run ID",
    )
    initialization.add_argument(
        "--warm-start",
        type=Path,
        help="Trusted compatible macro weights only; fresh optimizer/RNG and new run identity, allows new simulator/source",
    )
    result.add_argument("--output-root", type=Path, default=Path("wandb"))
    result.add_argument("--wandb-project", default="sts-run-a0-v1")
    result.add_argument(
        "--wandb-mode", choices=("online", "offline", "disabled"), default="disabled"
    )
    return result


def main(argv: list[str] | None = None) -> None:
    cli = parser()
    args = cli.parse_args(argv)
    if (
        min(
            args.updates,
            args.batch_size,
            args.max_actions,
            args.eval_every,
            args.model_width,
            args.max_consecutive_skips,
        )
        < 1
    ):
        cli.error("Counts must be positive")
    if (
        not args.run_id
        or Path(args.run_id).name != args.run_id
        or args.run_id in (".", "..")
    ):
        cli.error("run-id must be a single directory name")
    if (
        not math.isfinite(args.lr)
        or args.lr <= 0
        or any(
            not math.isfinite(x) or x < 0 for x in (args.entropy_coef, args.value_coef)
        )
    ):
        cli.error(
            "Expected positive finite lr and nonnegative finite loss coefficients"
        )
    if args.max_hours is not None and (
        not math.isfinite(args.max_hours) or args.max_hours <= 0
    ):
        cli.error("max-hours must be finite and positive")
    if args.objective == "heart" and not args.final_act:
        cli.error("Heart objective requires --final-act")
    if args.device == "cuda" and not torch.cuda.is_available():
        cli.error("CUDA unavailable")
    held_out = validation_seeds(args.validation_seeds)
    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    rng = random.Random(args.seed)
    model = MacroModel(args.model_width).to(args.device)
    combat = FrozenCombat(args.combat_checkpoint, args.device)
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr)
    settings = {
        key: str(value) if isinstance(value, Path) else value
        for key, value in vars(args).items()
    }
    source_root = Path(__file__).resolve().parents[1]
    source_files = sorted(
        set(source_root.glob("*.py"))
        | set((source_root / "encoders").glob("*.py"))
        | set(Path(__file__).parent.glob("*.py"))
    )
    source_bytes = b"".join(
        str(p.relative_to(source_root)).encode() + b"\0" + p.read_bytes()
        for p in source_files
    )
    settings.update(
        initialization="warm_start_weights_only"
        if args.warm_start
        else ("exact_resume" if args.resume_from else "fresh"),
        parent_checkpoint_sha256=digest(args.warm_start or args.resume_from)
        if (args.warm_start or args.resume_from)
        else None,
        protocol=PROTOCOL,
        reward_protocol=REWARD_PROTOCOL,
        act1_progress_floor_normalizer=ACT1_BOSS_FLOOR,
        act1_clear_bonus=ACT1_CLEAR_BONUS,
        behavior_metric_protocol="accepted_rest_decisions_hp_quartiles_and_map_entries_v1",
        torch_version=str(torch.__version__),
        cuda_version=torch.version.cuda,
        combat_execution="frozen sampled FP32 weights-only; all legal candidates retained",
        failure_policy="quarantine whole batches; conditional supported training"
        if args.continue_on_collection_failure
        else "abort",
        feature_version=FEATURE_VERSION,
        ascension=0,
        gamma=1.0,
        observation_schema=FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
        native_sha256=digest(Path(_native.__file__)),
        combat_sha256=digest(args.combat_checkpoint),
        validation_sha256=digest(args.validation_seeds),
        source_sha256=hashlib.sha256(source_bytes).hexdigest(),
    )
    iteration = 0
    optimizer_updates = 0
    skipped_batches = 0
    consecutive_skips = 0
    committed_iteration = None
    if args.warm_start:
        settings.update(load_warm_start(model, args.warm_start, settings))
    if args.resume_from:
        source = torch.load(args.resume_from, map_location="cpu", weights_only=True)
        ignored = {
            "run_id",
            "updates",
            "resume_from",
            "output_root",
            "wandb_project",
            "wandb_mode",
            "combat_checkpoint",
            "validation_seeds",
            "max_hours",
            "warm_start",
            "initialization",
            "parent_checkpoint_sha256",
        }
        for key, value in settings.items():
            if key not in ignored and source["config"].get(key) != value:
                raise ValueError(f"Run checkpoint continuation mismatch: {key}")
        model.load_state_dict(source["model"], strict=True)
        if not all(torch.isfinite(p).all() for p in model.parameters()):
            raise ValueError("Nonfinite macro checkpoint")
        optimizer.load_state_dict(source["optimizer"])
        rng.setstate(source["sampling_rng"])
        torch.set_rng_state(source["torch_rng"])
        if args.device == "cuda":
            torch.cuda.set_rng_state_all(source["cuda_rng"])
        iteration = source["iteration"]
        optimizer_updates = source["optimizer_updates"]
        skipped_batches = source["skipped_batches"]
        consecutive_skips = source["consecutive_skips"]
    output = args.output_root / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    (output / "trainer.pid").write_text(str(os.getpid()) + "\n")
    (output / "config.json").write_text(json.dumps(settings, indent=2) + "\n")
    shutil.copyfile(args.validation_seeds, output / "validation-seeds.json")
    for path in source_files:
        destination = output / "source" / path.relative_to(source_root)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
    started = time.monotonic()
    deadline = (
        started + args.max_hours * 3600 if args.max_hours is not None else math.inf
    )
    suffix = ".jsonl.gz" if args.compress_journals else ".jsonl"

    def checkpoint() -> None:
        nonlocal committed_iteration
        temporary = output / "latest.tmp"
        torch.save(
            {
                "model": model.state_dict(),
                "optimizer": optimizer.state_dict(),
                "iteration": iteration,
                "optimizer_updates": optimizer_updates,
                "skipped_batches": skipped_batches,
                "consecutive_skips": consecutive_skips,
                "sampling_rng": rng.getstate(),
                "torch_rng": torch.get_rng_state(),
                "cuda_rng": torch.cuda.get_rng_state_all()
                if args.device == "cuda"
                else [],
                "config": settings,
            },
            temporary,
        )
        temporary.replace(output / "latest.pt")
        committed_iteration = iteration

    def stopped() -> bool:
        return stop_flag.is_set() or time.monotonic() >= deadline

    def episode(seed: str, directory: Path, policy_seed: int) -> RunEpisode:
        try:
            return collect(
                seed,
                model,
                combat,
                objective=args.objective,
                final_act=args.final_act,
                max_actions=args.max_actions,
                macro_rng=torch.Generator(device=args.device).manual_seed(policy_seed),
                combat_rng=torch.Generator(device=args.device).manual_seed(
                    policy_seed ^ 0x5DEECE66D
                ),
                journal=directory,
                stop_requested=stopped,
            )
        except CollectionFailure as error:
            if not args.continue_on_collection_failure:
                raise
            # This seed/action is NOT retried and no part of this batch can train.
            with (output / "collection-errors.jsonl").open("a") as errors:
                errors.write(
                    json.dumps(
                        {"seed": seed, "journal": str(directory), "error": str(error)}
                    )
                    + "\n"
                )
            return RunEpisode(
                "error",
                None,
                (),
                error.accepted,
                error.floor,
                str(error),
                objective=args.objective,
                furthest_act1_floor=error.furthest_act1_floor,
                behavior=error.behavior,
            )

    with (
        graceful_stop() as stop_flag,
        wandb.init(
            project=args.wandb_project,
            id=args.run_id,
            name=args.run_id,
            config=settings,
            mode=args.wandb_mode,
        ) as run,
        (output / "metrics.jsonl").open("x") as metrics,
    ):
        run.define_metric("iteration")
        run.define_metric("train/*", step_metric="iteration")
        run.define_metric("validation/*", step_metric="iteration")
        (output / "tracking.json").write_text(
            json.dumps({"mode": args.wandb_mode, "url": run.url}, indent=2)
        )

        def log(values: dict, step: int) -> None:
            values = {
                **values,
                "elapsed_seconds": time.monotonic() - started,
                "optimizer_updates": optimizer_updates,
                "skipped_batches": skipped_batches,
            }
            metrics.write(json.dumps({"iteration": step, **values}) + "\n")
            metrics.flush()
            # Custom iteration axis lets train and validation commit separate rows
            # at the same iteration without W&B dropping a repeated internal step.
            run.log({"iteration": step, **values}, commit=True)
            print(json.dumps({"iteration": step, **values}), flush=True)

        def evaluate(step: int) -> None:
            evaluated = []
            for i, seed in enumerate(held_out):
                if stopped():
                    break
                evaluated.append(
                    episode(
                        seed,
                        output / "journals" / f"eval-{step}-{i}{suffix}",
                        100000 + i,
                    )
                )
            log(
                {
                    **{f"validation/{k}": v for k, v in scores(evaluated).items()},
                    "validation/scheduled": len(held_out),
                    "validation/unattempted": len(held_out) - len(evaluated),
                },
                step,
            )
            shutil.copyfile(output / "latest.pt", output / f"checkpoint-{step:08d}.pt")

        checkpoint()  # Last valid boundary remains recoverable after collection failure.
        try:
            evaluate(iteration)
            for _ in range(args.updates):
                if stopped():
                    break
                batch = []
                next_iteration = iteration + 1
                for index in range(args.batch_size):
                    if stopped():
                        break
                    seed = str(rng.getrandbits(63))
                    while seed in held_out:
                        seed = str(rng.getrandbits(63))
                    batch.append(
                        episode(
                            seed,
                            output
                            / "journals"
                            / f"train-{next_iteration}-{index}{suffix}",
                            rng.getrandbits(63),
                        )
                    )
                incomplete = len(batch) != args.batch_size or any(
                    ep.reward is None for ep in batch
                )
                if args.collect_only:
                    learned = {
                        "optimizer_step": 0.0,
                        "batch_discarded": float(incomplete),
                    }
                elif incomplete:
                    if not args.continue_on_collection_failure and not stopped():
                        raise CollectionFailure("Incomplete batch: optimizer unchanged")
                    skipped_batches += 1
                    consecutive_skips += 1
                    learned = {"optimizer_step": 0.0, "batch_discarded": 1.0}
                else:
                    learned = update(
                        batch,
                        model,
                        optimizer,
                        entropy_coef=args.entropy_coef,
                        value_coef=args.value_coef,
                    )
                    optimizer_updates += int(learned["optimizer_step"])
                    consecutive_skips = 0
                iteration = next_iteration
                checkpoint()  # Commit learning before fallible telemetry publishing.
                log(
                    {f"train/{k}": v for k, v in {**scores(batch), **learned}.items()},
                    iteration,
                )
                if stopped():
                    break
                if consecutive_skips >= args.max_consecutive_skips:
                    raise CollectionFailure(
                        "Consecutive incomplete-batch limit reached; stopping safely"
                    )
                if iteration % args.eval_every == 0:
                    evaluate(iteration)
            if iteration % args.eval_every and not stopped():
                evaluate(iteration)
            checkpoint()
            (output / "finished.json").write_text(
                json.dumps(
                    {
                        "iteration": iteration,
                        "optimizer_updates": optimizer_updates,
                        "skipped_batches": skipped_batches,
                        "reason": "budget_or_signal"
                        if stopped()
                        else "updates_complete",
                        "elapsed_seconds": time.monotonic() - started,
                    },
                    indent=2,
                )
            )
        except Exception as error:
            (output / "failure.json").write_text(
                json.dumps(
                    {
                        "iteration": iteration,
                        "error": f"{type(error).__name__}: {error}",
                        "last_committed_iteration": committed_iteration,
                        "resume": "latest.pt is the last committed update; journals retain attempted actions",
                    },
                    indent=2,
                )
            )
            raise
