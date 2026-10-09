"""Collect an immutable pre-boss root bank from existing natural-run journals.

This is dataset preparation, not a trainer or trace promotion tool. Reconstruct
accepted prefixes on the current native build and report every rejected source.
"""

import argparse
import gzip
import json
import shutil
from dataclasses import asdict
from pathlib import Path

from run_training.contracts import PolicyAction
from run_training.roots import (
    ROOT_PROTOCOL,
    file_hash,
    natural_state_from_setup,
    reconstruct,
    validate_natural_setup,
)
from sts_sim import _native


def find_root(path: Path):
    with gzip.open(path, "rt") as source:
        rows = [json.loads(s) for s in source]
    setup = rows[0]
    validate_natural_setup(setup)
    if not setup["final_act"]:
        raise ValueError("Expected natural A0 Heart-profile source")
    state = natural_state_from_setup(setup)
    decision = state.decision()
    accepted = {r["step"] for r in rows if r["type"] == "accepted"}
    for row in rows:
        if row["type"] != "attempt":
            continue
        ob = decision.observation
        if (
            ob.kind == "rest"
            and ob.context.floor == 15
            and not ob.screen.complete
            and {"rest_heal", "rest_smith"} <= {a.kind for a in decision.actions}
        ):
            return setup["seed"], row["step"]
        if row["step"] not in accepted:
            break
        if decision.revision != row["revision"]:
            raise ValueError("Cross-version prefix revision divergence")
        action = decision.actions[row["index"]]
        if asdict(PolicyAction.from_action(action)) != row["action"]:
            raise ValueError("Cross-version public action divergence")
        decision = state.step(action)
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-run", type=Path, required=True)
    parser.add_argument("--evaluation-iteration", type=int, required=True)
    parser.add_argument("--train-count", type=int, default=128)
    parser.add_argument("--validation-count", type=int, default=32)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if min(args.train_count, args.validation_count) < 1:
        parser.error("Counts must be positive")
    args.out.mkdir(parents=True, exist_ok=False)
    (args.out / "journals").mkdir()
    held_out = set(json.loads((args.source_run / "validation-seeds.json").read_text()))
    cases = []
    seen = set()
    counts = {}
    for split, pattern, count in (
        ("train", "train-*.jsonl.gz", args.train_count),
        (
            "validation",
            f"eval-{args.evaluation_iteration}-*.jsonl.gz",
            args.validation_count,
        ),
    ):
        found = 0
        scanned = 0
        for source in sorted((args.source_run / "journals").glob(pattern)):
            scanned += 1
            try:
                candidate = find_root(source)
                if candidate is None:
                    continue
                seed, step = candidate
                if seed in seen or (seed in held_out) != (split == "validation"):
                    raise ValueError("Duplicate seed or invalid held-out split")
                # Independently validate bank reconstruction and synthetic variants.
                state, _, _ = reconstruct(source, step)
                before = state.observation()
                for fraction in (0.1, 0.5, 1.0):
                    hp = max(1, round(before.context.player_max_hp * fraction))
                    root = state.synthetic_rest_root(hp)
                    assert root.observation().context.player_hp == hp
                    assert state.observation() == before
                target = args.out / "journals" / source.name
                shutil.copyfile(
                    source, target
                )  # Byte-for-byte full original, no truncation.
                cases.append(
                    {
                        "seed": seed,
                        "split": split,
                        "stop_step": step,
                        "journal": str(target.relative_to(args.out)),
                        "journal_sha256": file_hash(target),
                        "original_journal": str(source.resolve()),
                    }
                )
                seen.add(seed)
                found += 1
                if found == count:
                    break
            except (ValueError, RuntimeError, IndexError) as error:
                with (args.out / "rejected-sources.jsonl").open("a") as log:
                    log.write(
                        json.dumps({"source": str(source), "error": str(error)}) + "\n"
                    )
        counts[split] = {"scanned": scanned, "selected": found, "requested": count}
    manifest = {
        "protocol": ROOT_PROTOCOL,
        "native_sha256": file_hash(Path(_native.__file__)),
        "final_act": True,
        "source_run": str(args.source_run.resolve()),
        "source_config_sha256": file_hash(args.source_run / "config.json"),
        "selection": "first unused floor-15 campfire with legal heal and smith; one root per run seed",
        "counts": counts,
        "cases": cases,
    }
    (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (args.out / "validation-seeds.json").write_text(json.dumps(sorted(held_out)) + "\n")
    print(json.dumps(counts), flush=True)
    if any(v["selected"] != v["requested"] for v in counts.values()):
        raise SystemExit(
            "Requested root counts unavailable; partial manifest retained, do not silently shrink"
        )


if __name__ == "__main__":
    main()
