"""Bounded execution benchmark, not a second production training entry point.

Run under the desired checkout's PYTHONPATH. Each trial resets to identical
weights and policy RNG seeds, collects the same scheduled episodes, and measures
one actual optimizer update only if the entire batch completes. Journals remain
immutable. Report cold and warm trials separately; never infer end-to-end gains
from only a model forward benchmark.
"""

import argparse
import gzip
import hashlib
import inspect
import json
import time
from pathlib import Path

import torch
from run_training import collector, trainer
from run_training.model import HealthMacroModel, MacroModel
from run_training.roots import RootBank
from sts_sim import State, _native


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def journal_fingerprint(path):
    with gzip.open(path, "rt") as source:
        rows = [json.loads(line) for line in source]
    # Float diagnostics may round differently; every actual attempted command,
    # accepted revision and terminal state/result must agree independently.
    public_execution = []
    for row in rows:
        if row["type"] == "attempt":
            public_execution.append(
                {
                    k: row[k]
                    for k in ["type", "step", "revision", "owner", "index", "action"]
                }
            )
        elif row["type"] == "accepted":
            public_execution.append(row)
        elif row["type"] == "result":
            public_execution.append(
                {
                    k: row[k]
                    for k in ["type", "status", "reward", "accepted", "floor", "hp"]
                }
            )
        elif row["type"] == "error":
            public_execution.append({k: row[k] for k in ["type", "accepted", "error"]})
    return hashlib.sha256(
        json.dumps(public_execution, sort_keys=True).encode()
    ).hexdigest()


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--profile", choices=["natural", "campfire"], required=True)
    cli.add_argument("--macro-checkpoint", type=Path, required=True)
    cli.add_argument("--combat-checkpoint", type=Path, required=True)
    cli.add_argument("--seeds", type=Path, required=True)
    cli.add_argument("--root-manifest", type=Path)
    cli.add_argument("--episodes", type=int, default=128)
    cli.add_argument("--trials", type=int, default=4)
    cli.add_argument("--width", type=int, default=1)
    cli.add_argument("--learner-batch-size", type=int, default=0)
    cli.add_argument("--device", choices=["cpu", "cuda"], default="cuda")
    cli.add_argument("--out", type=Path, required=True)
    args = cli.parse_args()
    if min(args.episodes, args.trials, args.width) < 1 or args.learner_batch_size < 0:
        cli.error("Counts must be positive and learner batch size nonnegative")
    if args.profile == "campfire" and args.root_manifest is None:
        cli.error("Campfire benchmark requires a root manifest")
    args.out.mkdir(parents=True, exist_ok=False)
    torch.set_num_threads(1)
    torch.manual_seed(1234)
    saved = torch.load(args.macro_checkpoint, weights_only=True, map_location="cpu")
    kind = (
        HealthMacroModel if saved["config"].get("encoder") == "health" else MacroModel
    )
    model = kind(saved["config"]["model_width"]).to(args.device)
    frozen = collector.FrozenCombat(args.combat_checkpoint, args.device)
    bank = (
        RootBank(args.root_manifest, final_act=True)
        if args.profile == "campfire"
        else None
    )
    seeds = json.loads(args.seeds.read_text())
    if not bank and len(seeds) < args.episodes:
        cli.error("Provide enough distinct natural seeds; do not silently repeat them")
    sources = sorted(set(Path(trainer.__file__).parent.glob("*.py")))
    manifest = {
        **vars(args),
        "torch": str(torch.__version__),
        "native_sha256": digest(Path(_native.__file__)),
        "macro_sha256": digest(args.macro_checkpoint),
        "combat_sha256": digest(args.combat_checkpoint),
        "script_sha256": digest(Path(__file__)),
        "source_files": {str(p): digest(p) for p in sources},
    }
    (args.out / "manifest.json").write_text(
        json.dumps(manifest, default=str, indent=2) + "\n"
    )
    all_trials = []

    def sync():
        if args.device == "cuda":
            torch.cuda.synchronize()

    for trial in range(args.trials):
        model.load_state_dict(saved["model"], strict=True)
        model.eval()
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        if args.device == "cuda":
            torch.cuda.reset_peak_memory_stats()
        paths = []

        def jobs(trial=trial, paths=paths):
            for index in range(args.episodes):
                seed = (
                    bank.training[index % len(bank.training)] if bank else seeds[index]
                )
                factory = State.new
                visible = previous = metadata = None
                if bank:
                    state, visible, previous, metadata = bank.initial(
                        seed, [0.15, 0.5, 0.85][index % 3]
                    )

                    def start_root(*_a, state=state, **_kw):
                        return state

                    factory = start_root
                journal = args.out / f"trial-{trial}-{index}.jsonl.gz"
                paths.append(journal)
                options: collector.CollectionOptions = {
                    "objective": "act1_binary",
                    "final_act": True,
                    "max_actions": 3000,
                    "macro_rng": torch.Generator(device=args.device).manual_seed(
                        100000 + index
                    ),
                    "combat_rng": torch.Generator(device=args.device).manual_seed(
                        (100000 + index) ^ 0x5DEECE66D
                    ),
                    "journal": journal,
                    "state_factory": factory,
                    "initial_visible_map": visible,
                    "initial_previous": previous,
                    "initial_metadata": metadata,
                }
                yield seed, options

        sync()
        start = time.perf_counter()
        if args.width == 1:
            episodes = []
            for seed, options in jobs():
                try:
                    episodes.append(collector.collect(seed, model, frozen, **options))
                except collector.CollectionFailure as error:
                    episodes.append(
                        collector.RunEpisode(
                            "error",
                            None,
                            (),
                            error.accepted,
                            error.floor,
                            str(error),
                            objective="act1_binary",
                            furthest_act1_floor=error.furthest_act1_floor,
                            behavior=error.behavior,
                        )
                    )
        else:
            episodes = collector.collect_many(
                (collector.CollectionJob(seed, options) for seed, options in jobs()),
                model,
                frozen,
                width=args.width,
                continue_on_failure=True,
            )
        sync()
        collection_seconds = time.perf_counter() - start
        learn_start = time.perf_counter()
        learned = {"optimizer_step": 0.0, "batch_discarded": 1.0}
        if all(ep.reward is not None for ep in episodes):
            extra = (
                {"batch_steps": args.learner_batch_size}
                if "batch_steps" in inspect.signature(trainer.update).parameters
                else {}
            )
            if args.learner_batch_size and not extra:
                raise ValueError("Selected source does not support batched learning")
            learned = trainer.update(
                episodes, model, optimizer, entropy_coef=0.01, value_coef=0.1, **extra
            )
        sync()
        update_seconds = time.perf_counter() - learn_start
        # Time checkpoint output as well, using temporary-file + atomic replace.
        checkpoint_start = time.perf_counter()
        temporary = args.out / f"trial-{trial}.pt.tmp"
        torch.save(
            {"model": model.state_dict(), "optimizer": optimizer.state_dict()},
            temporary,
        )
        temporary.replace(args.out / f"trial-{trial}.pt")
        checkpoint_seconds = time.perf_counter() - checkpoint_start
        result = {
            "trial": trial,
            "cold": trial == 0,
            "collection_seconds": collection_seconds,
            "update_seconds": update_seconds,
            "checkpoint_seconds": checkpoint_seconds,
            "total_seconds": collection_seconds + update_seconds + checkpoint_seconds,
            "attempts": len(episodes),
            "completed": sum(ep.reward is not None for ep in episodes),
            "errors": sum(ep.status == "error" for ep in episodes),
            "cutoffs": sum(ep.status == "cutoff" for ep in episodes),
            "successes": sum(ep.status == "act1_clear" for ep in episodes),
            "accepted_actions": sum(ep.accepted for ep in episodes),
            "macro_decisions": sum(len(ep.steps) for ep in episodes),
            "max_allocated_cuda_bytes": torch.cuda.max_memory_allocated()
            if args.device == "cuda"
            else 0,
            "learned": learned,
            "journal_execution_sha256": [journal_fingerprint(p) for p in paths],
        }
        all_trials.append(result)
        (args.out / "results.json").write_text(json.dumps(all_trials, indent=2) + "\n")
        print(
            json.dumps(
                {k: v for k, v in result.items() if k != "journal_execution_sha256"}
            ),
            flush=True,
        )


if __name__ == "__main__":
    main()
