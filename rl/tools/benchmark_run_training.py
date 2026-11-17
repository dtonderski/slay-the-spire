"""Bounded execution benchmark, not a second production training entry point.

Run under the desired checkout's PYTHONPATH. Each trial resets to identical
weights and policy RNG seeds, collects the same scheduled episodes, and measures
one actual optimizer update only if the entire batch completes. Journals remain
immutable. Report cold and warm trials separately; never infer end-to-end gains
from only a model forward benchmark.
"""

import argparse
import cProfile
import gzip
import hashlib
import inspect
import json
import time
from pathlib import Path

import numpy as np
import sts_sim
import torch
from run_training import collector, trainer
from run_training.environment import episode_environment_seed
from run_training.model import HealthMacroModel, MacroModel
from run_training.parallel import (
    JobSpec,
    ParallelCollector,
    collect_sharded,
    collection_job,
    state_bytes,
)
from run_training.roots import RootBank
from run_training.token_model import TokenMacroModel
from sts_sim import _native


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


def episode_fingerprint(episode):
    """Exact policy record: every choice, stored logit/value and public input."""
    digest = hashlib.sha256(
        repr((episode.status, episode.reward, episode.accepted, episode.floor)).encode()
    )
    for step in episode.steps:
        digest.update(repr((step.choice, step.logits, step.value)).encode())
        for value in vars(step.inputs).values():
            digest.update(
                value.tobytes()
                if isinstance(value, np.ndarray)
                else repr(value).encode()
            )
    return digest.hexdigest()


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
    cli.add_argument("--shards", type=int, default=1)
    cli.add_argument(
        "--workers", type=int, default=0, help="Spawned shard processes; 0 in-process"
    )
    cli.add_argument("--learner-batch-size", type=int, default=0)
    cli.add_argument("--device", choices=["cpu", "cuda"], default="cuda")
    cli.add_argument(
        "--worker-device", choices=["cpu", "cuda"], help="Defaults to --device"
    )
    cli.add_argument(
        "--objective",
        choices=["act1", "act1_binary", "heart_keys_budget"],
        default="act1_binary",
    )
    cli.add_argument("--max-actions", type=int, default=3000)
    cli.add_argument("--adapt-legacy-combat-checkpoint", action="store_true")
    cli.add_argument("--avoid-prismatic", action="store_true")
    cli.add_argument("--raw-navigation", action="store_true")
    cli.add_argument(
        "--final-card-skip",
        action="store_true",
        help="Make closing a card reward final; also on if the checkpoint used it",
    )
    cli.add_argument("--training-environment-seed", type=int)
    cli.add_argument(
        "--cprofile",
        action="store_true",
        help="Save per-trial CPU profiles; timings include profiler overhead",
    )
    cli.add_argument(
        "--collect-only", action="store_true", help="Skip the optimizer update"
    )
    cli.add_argument(
        "--overlap-inference",
        action="store_true",
        help="Run each wave's macro forward on a helper thread during combat scoring",
    )
    cli.add_argument("--out", type=Path, required=True)
    args = cli.parse_args()
    if (
        min(args.episodes, args.trials, args.width, args.max_actions, args.shards) < 1
        or min(args.learner_batch_size, args.workers) < 0
    ):
        cli.error("Counts must be positive and learner batch size nonnegative")
    if (args.shards > 1 or args.workers) and args.width == 1:
        cli.error("Sharded collection requires --width > 1")
    if args.workers and args.profile != "natural":
        cli.error("Worker processes support natural starts only")
    if args.profile == "campfire" and args.root_manifest is None:
        cli.error("Campfire benchmark requires a root manifest")
    if args.training_environment_seed is not None:
        if args.profile == "campfire":
            cli.error("Root journals own their environmental inputs")
        episode_environment_seed(args.training_environment_seed, "0")
    args.out.mkdir(parents=True, exist_ok=False)
    torch.set_num_threads(1)
    torch.manual_seed(1234)
    saved = torch.load(args.macro_checkpoint, weights_only=True, map_location="cpu")
    kind = {"health": HealthMacroModel, "tokens": TokenMacroModel}.get(
        saved["config"].get("encoder"), MacroModel
    )
    model = kind(saved["config"]["model_width"]).to(args.device)
    final_card_skip = args.final_card_skip or saved["config"].get(
        "final_card_skip", False
    )
    frozen = collector.FrozenCombat(
        args.combat_checkpoint,
        args.device,
        adapt_legacy=args.adapt_legacy_combat_checkpoint,
    )
    bank = (
        RootBank(args.root_manifest, final_act=True)
        if args.profile == "campfire"
        else None
    )
    seeds = json.loads(args.seeds.read_text())
    if not bank and len(seeds) < args.episodes:
        cli.error("Provide enough distinct natural seeds; do not silently repeat them")
    rl_root = Path(trainer.__file__).parent.parent
    sources = sorted(
        set(Path(trainer.__file__).parent.glob("*.py"))
        | set(Path(sts_sim.__file__).parent.rglob("*.py"))
        | set((rl_root / "encoders").glob("*.py"))
        | {rl_root / "model.py", rl_root / "observation_encoder.py"}
    )
    manifest = {
        **vars(args),
        "final_card_skip": final_card_skip,
        "torch": str(torch.__version__),
        "numpy": np.__version__,
        "device_name": torch.cuda.get_device_name() if args.device == "cuda" else "cpu",
        "torch_threads": torch.get_num_threads(),
        "native_sha256": digest(Path(_native.__file__)),
        "macro_sha256": digest(args.macro_checkpoint),
        "combat_sha256": digest(args.combat_checkpoint),
        "seeds_sha256": digest(args.seeds),
        "script_sha256": digest(Path(__file__)),
        "source_files": {str(p): digest(p) for p in sources},
    }
    (args.out / "manifest.json").write_text(
        json.dumps(manifest, default=str, indent=2) + "\n"
    )
    all_trials = []

    def job_of(job):
        return collection_job(job, args.device) if isinstance(job, JobSpec) else job

    pool = (
        ParallelCollector(
            args.workers,
            args.shards,
            model_type=kind,
            model_width=saved["config"]["model_width"],
            device=args.worker_device or args.device,
            combat=args.combat_checkpoint.read_bytes(),
            adapt_legacy=args.adapt_legacy_combat_checkpoint,
            width=args.width,
            continue_on_failure=True,
            errors=args.out / "collection-errors.jsonl",
            stop_requested=lambda: False,
            overlap_inference=args.overlap_inference,
        )
        if args.workers
        else None
    )

    def sync():
        if args.device == "cuda":
            torch.cuda.synchronize()

    for trial in range(args.trials):
        # Train mode, as in the trainer: eval() can select other transformer kernels.
        model.load_state_dict(saved["model"], strict=True)
        optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
        if args.device == "cuda":
            torch.cuda.reset_peak_memory_stats()
        paths = []
        policy_rngs = []

        def jobs(trial=trial, paths=paths):
            for index in range(args.episodes):
                journal = args.out / f"trial-{trial}-{index}.jsonl.gz"
                paths.append(journal)
                if not bank:
                    seed = seeds[index]
                    yield JobSpec(
                        seed,
                        journal,
                        100000 + index,
                        objective=args.objective,
                        final_act=True,
                        avoid_prismatic=args.avoid_prismatic,
                        commit_navigation=not args.raw_navigation,
                        exploration_profile="none",
                        encoder=saved["config"].get("encoder", "hashed"),
                        direct_smith=saved["config"].get("direct_smith", False),
                        final_card_skip=final_card_skip,
                        decision_diagnostics=False,
                        max_actions=args.max_actions,
                        training_rng_seed=episode_environment_seed(
                            args.training_environment_seed, seed
                        )
                        if args.training_environment_seed is not None
                        else None,
                    )
                    continue
                seed = bank.training[index % len(bank.training)]
                state, _, previous, metadata = bank.initial(
                    seed, [0.15, 0.5, 0.85][index % 3]
                )

                def start_root(*_a, state=state, **_kw):
                    return state

                options: collector.CollectionOptions = {
                    "objective": args.objective,
                    "avoid_prismatic": args.avoid_prismatic,
                    "commit_navigation": not args.raw_navigation,
                    "encoder": saved["config"].get("encoder", "hashed"),
                    "direct_smith": saved["config"].get("direct_smith", False),
                    "final_card_skip": final_card_skip,
                    "final_act": True,
                    "max_actions": args.max_actions,
                    "macro_rng": torch.Generator(device=args.device).manual_seed(
                        100000 + index
                    ),
                    "combat_rng": torch.Generator(device=args.device).manual_seed(
                        (100000 + index) ^ 0x5DEECE66D
                    ),
                    "journal": journal,
                    "state_factory": start_root,
                    "initial_previous": previous,
                    "initial_metadata": metadata,
                }
                yield collector.CollectionJob(seed, options)

        profiler = cProfile.Profile() if args.cprofile else None
        sync()
        if profiler is not None:
            profiler.enable()
        start = time.perf_counter()
        if pool is not None:
            episodes = pool.collect(state_bytes(model), list(jobs()))
        elif args.width == 1:
            episodes = []
            for job in map(job_of, jobs()):
                policy_rngs.extend(
                    (job.options["macro_rng"], job.options["combat_rng"])
                )
                try:
                    episodes.append(
                        collector.collect(job.seed, model, frozen, **job.options)
                    )
                except collector.CollectionFailure as error:
                    episodes.append(
                        collector.RunEpisode(
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
                    )
        else:
            scheduled = [job_of(job) for job in jobs()]
            for job in scheduled:
                policy_rngs.extend(
                    (job.options["macro_rng"], job.options["combat_rng"])
                )
            episodes = collect_sharded(
                scheduled,
                model,
                frozen,
                shards=args.shards,
                width=args.width,
                continue_on_failure=True,
                errors=args.out / "collection-errors.jsonl",
                stop_requested=lambda: False,
                overlap_inference=args.overlap_inference,
            )
        sync()
        collection_seconds = time.perf_counter() - start
        learn_start = time.perf_counter()
        learned = {"optimizer_step": 0.0, "batch_discarded": 1.0}
        if not args.collect_only and all(ep.reward is not None for ep in episodes):
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
        if profiler is not None:
            profiler.disable()
            profiler.dump_stats(str(args.out / f"trial-{trial}.pstats"))
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
            "journal_sha256": [
                hashlib.sha256(gzip.decompress(p.read_bytes())).hexdigest()
                for p in paths
            ],
            "episode_sha256": [episode_fingerprint(ep) for ep in episodes],
            "policy_rng_sha256": [
                hashlib.sha256(rng.get_state().numpy().tobytes()).hexdigest()
                for rng in policy_rngs
            ],
        }
        all_trials.append(result)
        (args.out / "results.json").write_text(json.dumps(all_trials, indent=2) + "\n")
        print(
            json.dumps({k: v for k, v in result.items() if not k.endswith("sha256")}),
            flush=True,
        )
    if pool is not None:
        pool.close()


if __name__ == "__main__":
    main()
