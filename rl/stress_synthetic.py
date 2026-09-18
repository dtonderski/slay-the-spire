"""CPU-only synthetic rollout diagnostics; retain failures, never repair states."""

import argparse
import hashlib
import json
import random
import time
import traceback
from collections import Counter
from pathlib import Path

import torch
from loadout_sampling import LoadoutSampler
from synthetic_roots import sample_root
from train import play_combats
from validation_set import native_sha256


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--distributions", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--roots", type=int, default=10000)
    parser.add_argument("--max-decisions", type=int, default=512)
    parser.add_argument("--seed", type=int, default=20260919)
    args = parser.parse_args()
    if min(args.roots, args.max_decisions) < 1:
        parser.error("Counts must be positive")
    args.output.mkdir(parents=True, exist_ok=False)
    failures = args.output / "failures"
    failures.mkdir()
    sampler = LoadoutSampler.load(args.distributions)
    rng = random.Random(args.seed)
    torch.set_num_threads(1)
    config = {
        "seed": args.seed,
        "roots": args.roots,
        "max_decisions": args.max_decisions,
        "policy": "uniform legal actions, no escape, numeric rollout path, CPU only",
        "native_sha256": native_sha256(),
        "distributions_sha256": hashlib.sha256(args.distributions.read_bytes()).hexdigest(),
    }
    (args.output / "config.json").write_text(json.dumps(config, indent=2))
    counts: Counter[str] = Counter()
    signatures: Counter[str] = Counter()
    started = time.monotonic()
    with (args.output / "results.jsonl").open("x") as results:
        for index in range(args.roots):
            sampling_state = rng.getstate()
            root = None
            stage = "construction"
            policy_seed = args.seed + index
            record: dict = {"index": index, "policy_seed": policy_seed}
            try:
                root = sample_root(rng, sampler)
                stage = "rollout"
                episode = play_combats(
                    [root.state], None, max_decisions=args.max_decisions, rng=random.Random(policy_seed), numeric=True
                )[0]
                status = "truncated" if episode.reward is None else ("won" if episode.won else "lost")
                record.update(status=status, decisions=episode.decisions, final_hp=episode.hp)
            except Exception as error:  # noqa: BLE001 - diagnostic runner records all exception classes
                # Diagnostics capture all failures, including harness errors; not all are simulator bugs.
                signature = f"{stage}:{type(error).__name__}:{error}"
                status = f"{stage}_error"
                signatures[signature] += 1
                record.update(status=status, signature=signature)
                failure = {
                    **record,
                    "spec": json.loads(root.spec_json) if root is not None else None,
                    "sampling_rng_before": sampling_state,
                    "step": getattr(error, "step", None),
                    "attempted_prefixes": getattr(error, "action_prefixes", None),
                    "traceback": traceback.format_exc(),
                    "note": "Last attempted action may be rejected; never retry the mutated state.",
                }
                with (failures / f"{index:08d}.json").open("x") as handle:
                    json.dump(failure, handle, indent=2)
            if root is not None:
                record.update(
                    floor=root.encounter.floor,
                    act=root.encounter.act,
                    kind=root.encounter.kind,
                    encounter=root.encounter.encounter,
                    rejected_loadouts=list(root.rejected_loadouts),
                )
            counts[status] += 1
            results.write(json.dumps(record) + "\n")
            if (index + 1) % 100 == 0 or index + 1 == args.roots:
                results.flush()
                summary = {
                    "attempted": index + 1,
                    "counts": dict(counts),
                    "signatures": dict(signatures),
                    "elapsed_seconds": time.monotonic() - started,
                }
                temporary = args.output / "summary.tmp"
                temporary.write_text(json.dumps(summary, indent=2))
                temporary.replace(args.output / "summary.json")
                print(json.dumps(summary), flush=True)


if __name__ == "__main__":
    main()
