"""Profile full training updates on distinct multi-combat roots, increasing toward CUDA OOM."""

import argparse
import gc
import hashlib
import json
import random
import statistics
import subprocess
import sys
import time
from collections import Counter, defaultdict
from contextlib import ExitStack, redirect_stdout
from functools import wraps
from pathlib import Path
from unittest.mock import patch

import sts_sim
import torch
from encoders.numeric import NumericBatch
from model import CombatModel
from sts_sim import State
from torch.distributions import Categorical
from train_roots import Root, collect_roots, train_batch


class Timings:
    """Exclusive wall times; GPU phase boundaries synchronize only in the diagnostic pass."""

    def __init__(self) -> None:
        self.seconds = defaultdict(float)
        self.calls = defaultdict(int)
        self.stack = []

    def wrap(self, name, function, gpu=False):
        @wraps(function)
        def timed(*args, **kwargs):
            if gpu:
                torch.cuda.synchronize()
            start = time.perf_counter()
            self.stack.append(0.0)
            try:
                return function(*args, **kwargs)
            finally:
                if gpu:
                    torch.cuda.synchronize()
                elapsed = time.perf_counter() - start
                nested = self.stack.pop()
                self.seconds[name] += elapsed - nested
                self.calls[name] += 1
                if self.stack:
                    self.stack[-1] += elapsed

        return timed


def save_json(path: Path, value: dict) -> None:
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2))
    temporary.replace(path)


def load_roots(manifest: dict) -> tuple[list[Root], list[str]]:
    """Reconstruct roots solely from seeds and accepted-action prefixes."""
    roots, identities = [], []
    held_out = set(manifest["held_out_seeds"])
    seen_seeds = set()
    for entry in manifest["train"]:
        seed = entry["seed"]
        if seed in held_out or seed in seen_seeds:
            raise ValueError(f"Held-out or duplicate seed in training pool: {seed}")
        seen_seeds.add(seed)
        by_prefix = {root["prefix_length"]: root for root in entry["roots"]}
        if len(by_prefix) != len(entry["roots"]):
            raise ValueError("Duplicate combat root prefix")
        if not by_prefix:
            continue
        state = State.new(seed, ascension=0)
        for prefix in range(max(by_prefix) + 1):
            decision = state.decision()
            if prefix in by_prefix:
                expected = by_prefix[prefix]
                obs = decision.observation
                if (
                    obs.kind != "combat"
                    or obs.context.floor != expected["floor"]
                    or obs.context.player_hp != expected["hp"]
                ):
                    raise RuntimeError(f"Root reconstruction mismatch: seed={seed}, prefix={prefix}")
                roots.append(Root(state.clone(), seed, expected["floor"], expected["hp"]))
                identities.append(f"{seed}:{prefix}")
            if prefix < max(by_prefix):
                state.step(decision.actions[entry["accepted_action_indices"][prefix]])
    return roots, identities


def select_roots(roots: list[Root], identities: list[str], size: int):
    if len(roots) != len(identities) or len(set(identities)) != len(identities):
        raise ValueError("Root identities must be unique and aligned")
    if size > len(roots) or size < 1:
        raise ValueError(f"Requested {size} distinct roots, only {len(roots)} available")
    indices = random.Random(123).sample(range(len(roots)), size)
    return [roots[i] for i in indices], [identities[i] for i in indices]


def benchmark(
    roots: list[Root],
    warmups: int,
    repeats: int,
    max_decisions: int,
    result: dict,
    output: Path,
    *,
    numeric: bool = False,
) -> None:
    torch.manual_seed(123)
    model = CombatModel().cuda()
    initial = {name: value.detach().clone() for name, value in model.state_dict().items()}
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
    batch_size = len(roots)

    def phase(name):
        result["stage"] = name
        save_json(output, result)

    def reset(seed):
        # Same initial policy each trial; retain warmed Adam buffers but reset their values.
        model.load_state_dict(initial)
        optimizer.zero_grad(set_to_none=True)
        for values in optimizer.state.values():
            for value in values.values():
                if isinstance(value, torch.Tensor):
                    value.zero_()
        torch.manual_seed(seed)
        gc.collect()
        torch.cuda.synchronize()

    def measure():
        torch.cuda.reset_peak_memory_stats()
        torch.cuda.synchronize()
        start = time.perf_counter()
        logs = train_batch(roots, model, optimizer, max_decisions, numeric=numeric)
        torch.cuda.synchronize()
        seconds = time.perf_counter() - start
        assert logs["episodes"] == batch_size, "Rollout silently changed the requested batch size"
        decisions = round(logs["mean_decisions"] * batch_size)
        return {
            "seconds": seconds,
            "decisions": decisions,
            "decisions_per_second": decisions / seconds,
            "peak_allocated_mib": torch.cuda.max_memory_allocated() / 2**20,
            "peak_reserved_mib": torch.cuda.max_memory_reserved() / 2**20,
            "metrics": logs,
        }

    result["warmups"] = []
    result["trials"] = []
    for index in range(warmups):
        phase(f"warmup_{index + 1}")
        reset(20000 + index)
        result["warmups"].append(measure())
        save_json(output, result)
    print(f"batch={batch_size}: {warmups} full-update warmup(s) complete", flush=True)

    for index in range(repeats):
        phase(f"trial_{index + 1}")
        reset(30000 + index)
        trial = measure()
        result["trials"].append(trial)
        save_json(output, result)
        print(
            f"batch={batch_size} trial={index + 1}: {trial['decisions_per_second']:.1f} decisions/s, "
            f"{trial['seconds']:.2f}s, {trial['peak_allocated_mib']:.0f} MiB, "
            f"truncated={trial['metrics']['truncated']:.0f}",
            flush=True,
        )

    # Separate instrumentation pass; boundary synchronization changes overlap.
    phase("diagnostic")
    reset(30000)
    timing = Timings()
    with ExitStack() as stack:
        for owner, method, name, gpu in (
            (State, "clone", "simulator_clone", False),
            (State, "numeric_decisions", "native_initial_public_export", False),
            (State, "numeric_steps", "native_step_and_public_export", False),
            (NumericBatch, "__init__", "public_buffer_views", False),
            (State, "decision", "simulator_initial_decision", False),
            (State, "step", "simulator_step_native_and_wrapper", False),
            (sts_sim, "_project_decision", "public_decision_getters_and_mapping", False),
            (sts_sim, "decode_observation", "python_observation_decode", False),
            (model, "forward", "policy_scoring_other", True),
            (model.observation_encoder, "prepare_batch", "observation_features_and_projections", True),
            (model.observation_encoder.transformer, "forward", "transformer", True),
            (model.action_encoder, "forward", "action_gather_and_encode", True),
            (Categorical, "sample", "sampling", True),
            (Categorical, "log_prob", "sample_log_probs", True),
            (torch.Tensor, "backward", "backward", True),
            (optimizer, "step", "optimizer_step", True),
        ):
            stack.enter_context(patch.object(owner, method, timing.wrap(name, getattr(owner, method), gpu)))
        diagnostic = measure()
    timing.seconds["other_rollout_loss_checks_cleanup"] = diagnostic["seconds"] - sum(timing.seconds.values())
    diagnostic["phase_seconds"] = dict(timing.seconds)
    diagnostic["phase_calls"] = dict(timing.calls)
    diagnostic["phase_percent"] = {
        name: 100 * seconds / diagnostic["seconds"] for name, seconds in timing.seconds.items()
    }
    result.update(
        status="ok",
        stage="finished",
        diagnostic=diagnostic,
        median_decisions_per_second=statistics.median(trial["decisions_per_second"] for trial in result["trials"]),
    )
    save_json(output, result)


def run_worker(args) -> None:
    torch.set_num_threads(1)
    output = args.output_dir / f"batch-{args.worker_size}.json"
    result = {"requested_batch_size": args.worker_size, "status": "running", "stage": "reconstruct_roots"}
    save_json(output, result)
    try:
        pool, identities = load_roots(json.loads((args.output_dir / "roots.json").read_text()))
        roots, identities = select_roots(pool, identities, args.worker_size)
        del pool
        result.update(
            actual_batch_size=len(roots),
            unique_roots=len(set(identities)),
            root_ids=identities,
            unique_seeds=len({root.seed for root in roots}),
            floors=dict(Counter(root.floor for root in roots)),
        )
        result["stage"] = "initialize_cuda"
        save_json(output, result)
        result.update(
            device=torch.cuda.get_device_name(), total_vram_mib=torch.cuda.get_device_properties(0).total_memory / 2**20
        )
        benchmark(
            roots, args.warmups, args.repeats, args.max_decisions, result, output, numeric=args.numeric_observations
        )
    except torch.cuda.OutOfMemoryError as error:
        result.update(
            status="cuda_oom",
            error=str(error),
            peak_allocated_mib=torch.cuda.max_memory_allocated() / 2**20,
            peak_reserved_mib=torch.cuda.max_memory_reserved() / 2**20,
        )
        save_json(output, result)
        print(f"batch={args.worker_size}: CUDA OOM during {result['stage']}", flush=True)
    except Exception as error:
        result.update(status="error", error=f"{type(error).__name__}: {error}")
        save_json(output, result)
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--batch-sizes", type=int, nargs="+", default=[128, 256, 512, 1024, 2048, 4096, 8192, 16384])
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--max-decisions", type=int, default=512)
    parser.add_argument("--floors", type=int, default=10)
    parser.add_argument("--max-collection-seeds", type=int, default=10000)
    parser.add_argument("--held-out-manifest", type=Path, default=Path("wandb/roots20260910040458/roots.json"))
    parser.add_argument("--output-dir", type=Path, default=Path("wandb/multiroot-batch-profile"))
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--numeric-observations", action="store_true")
    parser.add_argument("--training-manifest", type=Path, help="Reuse an existing collected training pool")
    parser.add_argument("--worker-size", type=int, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if (
        min(*args.batch_sizes, args.warmups, args.repeats, args.max_decisions, args.floors, args.max_collection_seeds)
        < 1
    ):
        parser.error("Counts and sizes must be positive")
    if args.worker_size is not None:
        run_worker(args)
        return
    if not torch.cuda.is_available():
        parser.error("CUDA is required")
    torch.set_num_threads(1)
    native = Path(sts_sim._native.__file__)
    identity = {
        "git_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "native_sha256": hashlib.sha256(native.read_bytes()).hexdigest(),
        "torch": str(torch.__version__),
    }
    source_dir = Path(__file__).resolve().parent
    source_files = sorted([*source_dir.glob("*.py"), *source_dir.glob("encoders/*.py")])
    identity["rl_source_sha256"] = hashlib.sha256(
        b"".join(path.name.encode() + b"\0" + path.read_bytes() for path in source_files)
    ).hexdigest()
    config = {
        "warmups": args.warmups,
        "repeats": args.repeats,
        "max_decisions": args.max_decisions,
        "floors": args.floors,
        "numeric_observations": args.numeric_observations,
    }
    manifest_path = args.output_dir / "roots.json"
    summary_path = args.output_dir / "summary.json"
    manifest: dict
    summary: dict
    if args.resume:
        manifest = json.loads(manifest_path.read_text())
        summary = json.loads(summary_path.read_text())
        if summary["identity"] != identity or summary["config"] != config:
            parser.error("Cannot resume across different code/build/config")
        if any(row["status"] != "ok" for row in summary["results"]):
            parser.error("Previous sweep stopped at a failure; inspect it before starting another sweep")
    else:
        held_out = json.loads(args.held_out_manifest.read_text())
        args.output_dir.mkdir(parents=True, exist_ok=False)
        manifest = {
            "held_out_seeds": sorted({entry["seed"] for entry in held_out["val"]}),
            "train": [],
            "next_seed": 2000000,
            "collector_rng_seed": 123,
            "floors": args.floors,
        }
        if args.training_manifest is not None:
            manifest = json.loads(args.training_manifest.read_text())
            if manifest["floors"] != args.floors or set(manifest["held_out_seeds"]) != {
                entry["seed"] for entry in held_out["val"]
            }:
                parser.error("Training pool floors/held-out split do not match")
        summary = {
            "identity": identity,
            "config": config,
            "notes": "Distinct roots from additional training seeds, no replacement. Same initial policy each trial. Full-update wall time includes simulator, decoding, tensor assembly, forward, backward, optimizer, cleanup; collection/reconstruction/reset/W&B excluded. Diagnostic pass synchronizes GPU boundaries and reports wall time, not pure kernel time. Results apply to this frozen early-floor distribution and initial policy, not all later trained policies.",
            "results": [],
        }
        save_json(manifest_path, manifest)
        save_json(summary_path, summary)
    count = sum(len(entry["roots"]) for entry in manifest["train"])
    done = {row["requested_batch_size"] for row in summary["results"]}
    for size in sorted(set(args.batch_sizes)):
        if size in done:
            continue
        while count < size:
            if len(manifest["train"]) >= args.max_collection_seeds:
                raise RuntimeError(f"Collection limit reached: {count} roots, need {size}")
            seed = str(manifest["next_seed"])
            manifest["next_seed"] += 1
            if seed in manifest["held_out_seeds"]:
                continue
            with (args.output_dir / "collection.log").open("a") as log, redirect_stdout(log):
                roots, entries = collect_roots([seed], args.floors, manifest["collector_rng_seed"])
            count += len(roots)
            del roots
            manifest["train"].extend(entries)
            if len(manifest["train"]) % 25 == 0:
                save_json(manifest_path, manifest)
                print(f"collected {count} roots from {len(manifest['train'])} seeds; target={size}", flush=True)
        save_json(manifest_path, manifest)
        command = [
            sys.executable,
            str(Path(__file__).resolve()),
            "--worker-size",
            str(size),
            "--output-dir",
            str(args.output_dir.resolve()),
            "--warmups",
            str(args.warmups),
            "--repeats",
            str(args.repeats),
            "--max-decisions",
            str(args.max_decisions),
        ]
        if args.numeric_observations:
            command.append("--numeric-observations")
        with (args.output_dir / f"batch-{size}.log").open("w") as log:
            process = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=False)
        result_path = args.output_dir / f"batch-{size}.json"
        row = json.loads(result_path.read_text()) if result_path.exists() else {"requested_batch_size": size}
        if process.returncode != 0:
            row.update(status="worker_failure", returncode=process.returncode)
        summary["results"].append(row)
        summary["collection"] = {
            "roots": count,
            "seeds": len(manifest["train"]),
            "statuses": dict(Counter(entry["status"] for entry in manifest["train"])),
        }
        save_json(summary_path, summary)
        print(f"batch={size}: {row['status']}, decisions/s={row.get('median_decisions_per_second')}", flush=True)
        if row["status"] != "ok":
            print(f"Stopping at first failure; details: {result_path}", flush=True)
            break
    print(f"Saved {summary_path}", flush=True)


if __name__ == "__main__":
    main()
