"""Paired public-export and observation-preparation timings on the same frozen roots."""

import argparse
import gc
import json
import random
import statistics
import time
from pathlib import Path

import torch
from encoders.numeric import NumericBatch
from model import CombatModel
from profile_batches import load_roots, save_json, select_roots
from sts_sim import State


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--batch-size", type=int, default=2048)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error("repeats must be positive")
    torch.set_num_threads(1)
    torch.manual_seed(123)
    roots, identities = load_roots(json.loads(args.manifest.read_text()))
    roots, identities = select_roots(roots, identities, args.batch_size)
    states = [root.state for root in roots]
    model = CombatModel().cuda()
    typed = [state.decision().observation for state in states]
    numeric = NumericBatch(State.numeric_decisions(states))
    operations = {
        "typed_export": lambda: [state.decision() for state in states],
        "numeric_export": lambda: NumericBatch(State.numeric_decisions(states)),
        "typed_prepare": lambda: model.observation_encoder.prepare_batch(typed),
        "numeric_prepare": lambda: model.observation_encoder.prepare_batch(numeric),
    }
    samples = {key: [] for key in operations}
    rng = random.Random(123)
    for repeat in range(args.repeats + 1):
        order = list(operations)
        rng.shuffle(order)
        for key in order:
            gc.collect()
            torch.cuda.synchronize()
            start = time.perf_counter()
            value = operations[key]()
            torch.cuda.synchronize()
            elapsed = time.perf_counter() - start
            del value
            if repeat:
                samples[key].append(elapsed)
    result = {
        "batch_size": len(roots),
        "root_ids": identities,
        "device": torch.cuda.get_device_name(),
        "samples_seconds": samples,
        "median_seconds": {key: statistics.median(values) for key, values in samples.items()},
        "scope": "One warmup per operation; randomized paired order. Export includes native fair projection and actions. Prepare includes embeddings, projections, token assembly, and device synchronization, but not transformer/action scoring. Output destruction and GC are outside these microtimings; use profile_batches.py for full updates.",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    save_json(args.output, result)
    print(json.dumps(result["median_seconds"], indent=2))


if __name__ == "__main__":
    main()
