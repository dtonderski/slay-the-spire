"""CUDA training throughput and synchronized phase timings on the fixed HUMAN1 combat."""

import argparse
import gc
import json
import statistics
import time
from collections import defaultdict
from contextlib import ExitStack
from functools import wraps
from pathlib import Path
from unittest.mock import patch

import sts_sim
import torch
from model import CombatModel
from sts_sim import State
from torch.distributions import Categorical
from train import first_combat
from train_roots import Root, train_batch


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


def benchmark(batch_size: int, warmups: int, repeats: int, max_decisions: int) -> dict:
    torch.manual_seed(123)
    model = CombatModel().cuda()
    initial = {name: value.detach().clone() for name, value in model.state_dict().items()}
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
    state = first_combat("HUMAN1", 0)
    roots = [Root(state, "HUMAN1", 1, state.decision().observation.context.player_hp)] * batch_size

    def reset(seed):
        # Same policy for each measurement; retain warmed Adam buffers but reset their values.
        model.load_state_dict(initial)
        optimizer.zero_grad(set_to_none=True)
        for values in optimizer.state.values():
            for value in values.values():
                if isinstance(value, torch.Tensor):
                    value.zero_()
        torch.manual_seed(seed)
        gc.collect()
        torch.cuda.synchronize()

    for index in range(warmups):
        reset(20000 + index)
        train_batch(roots, model, optimizer, max_decisions)
        torch.cuda.synchronize()
    print(f"batch={batch_size}: {warmups} full-update warmup(s) complete", flush=True)

    def measure():
        torch.cuda.reset_peak_memory_stats()
        torch.cuda.synchronize()
        start = time.perf_counter()
        logs = train_batch(roots, model, optimizer, max_decisions)
        torch.cuda.synchronize()
        seconds = time.perf_counter() - start
        decisions = round(logs["mean_decisions"] * batch_size)
        return {
            "seconds": seconds,
            "decisions": decisions,
            "decisions_per_second": decisions / seconds,
            "peak_allocated_mib": torch.cuda.max_memory_allocated() / 2**20,
            "peak_reserved_mib": torch.cuda.max_memory_reserved() / 2**20,
            "metrics": logs,
        }

    trials = []
    for index in range(repeats):
        reset(30000 + index)
        trial = measure()
        trials.append(trial)
        print(
            f"batch={batch_size} trial={index + 1}: {trial['decisions_per_second']:.1f} decisions/s, "
            f"{trial['seconds']:.2f}s, {trial['peak_allocated_mib']:.0f} MiB",
            flush=True,
        )

    # Separate instrumentation pass. Boundary synchronization changes overlap; not the throughput result.
    reset(30000)
    timing = Timings()
    with ExitStack() as stack:
        for owner, method, name, gpu in (
            (State, "clone", "simulator_clone", False),
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
    print(f"batch={batch_size} synchronized breakdown: {json.dumps(diagnostic['phase_percent'])}", flush=True)
    return {
        "batch_size": batch_size,
        "status": "ok",
        "trials": trials,
        "median_decisions_per_second": statistics.median(trial["decisions_per_second"] for trial in trials),
        "diagnostic": diagnostic,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--batch-sizes", type=int, nargs="+", default=[128, 256, 512, 1024, 2048])
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--max-decisions", type=int, default=256)
    parser.add_argument("--output", type=Path, default=Path("wandb/batch-profile.json"))
    args = parser.parse_args()
    if min(*args.batch_sizes, args.warmups, args.repeats, args.max_decisions) < 1:
        parser.error("Batch sizes, warmups, repeats, and decision limits must be positive")
    if not torch.cuda.is_available():
        parser.error("CUDA is required for this benchmark")
    torch.set_num_threads(1)
    result = {
        "device": torch.cuda.get_device_name(),
        "torch": str(torch.__version__),
        "warmups": args.warmups,
        "repeats": args.repeats,
        "max_decisions": args.max_decisions,
        "seed": "HUMAN1",
        "threads": 1,
        "notes": "Uninstrumented full-update throughput includes clone/decision/step, Python feature building, policy, sampling, backward, optimizer, and cleanup. Setup/reset/W&B excluded. Separate synchronized diagnostic timings include device waits, not pure GPU kernel times. Identical starting weights each trial; sampled trajectories can differ by batch size.",
        "results": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for size in args.batch_sizes:
        try:
            row = benchmark(size, args.warmups, args.repeats, args.max_decisions)
        except torch.cuda.OutOfMemoryError as error:
            row = {"batch_size": size, "status": "cuda_oom", "error": str(error)}
            print(f"batch={size}: CUDA out of memory", flush=True)
        result["results"].append(row)
        args.output.write_text(json.dumps(result, indent=2))
        gc.collect()
        torch.cuda.empty_cache()
    print(f"Saved {args.output}", flush=True)


if __name__ == "__main__":
    main()
