"""Train on fresh synthetic A0 batches; evaluate immutable main/stress specifications."""

import argparse
import hashlib
import json
import logging
import math
import random
import time
from pathlib import Path

import torch
import wandb
from loadout_sampling import LoadoutSampler
from model import CombatModel
from rollout_errors import SimulatorStepError
from scenarios import ScenarioConfig
from synthetic_roots import SyntheticRoot, sample_root
from train_roots import Root, evaluate, train_batch
from validation_set import load_validation


def evaluate_sets(
    groups: dict[str, list[Root]],
    model: CombatModel,
    repeats: int,
    max_decisions: int,
    error_dir: Path | None = None,
) -> dict[str, float]:
    """Keep main and deliberately hard validation scores separate."""
    return {
        f"val_{label}/{key}": value
        for label, roots in groups.items()
        for key, value in evaluate(
            roots,
            model,
            repeats,
            max_decisions,
            error_path=error_dir / f"{label}.jsonl" if error_dir is not None else None,
        ).items()
    }


def fresh_batch(
    rng: random.Random,
    sampler: LoadoutSampler,
    batch_size: int,
    config: ScenarioConfig,
    excluded_seeds: frozenset[int] = frozenset(),
) -> tuple[list[Root], list[SyntheticRoot]]:
    """Generate exactly batch_size fresh roots, without a finite training dataset."""
    if batch_size < 1:
        raise ValueError("batch_size must be positive")
    roots, specifications = [], []
    for _ in range(batch_size * 2):
        sampled = sample_root(rng, sampler, config=config)
        seed = json.loads(sampled.spec_json)["seed"]
        if seed in excluded_seeds:
            continue
        obs = sampled.state.observation()
        roots.append(
            Root(sampled.state, str(seed), sampled.encounter.floor, obs.context.player_hp, sampled.encounter.act)
        )
        specifications.append(sampled)
        if len(roots) == batch_size:
            return roots, specifications
    raise RuntimeError("Could not sample a batch disjoint from validation seeds")


def update(
    roots: list[Root],
    specifications: list[SyntheticRoot],
    model: CombatModel,
    optimizer: torch.optim.Optimizer,
    max_decisions: int,
    entropy_coef: float,
    failure_path: Path,
    *,
    continue_on_error: bool = False,
) -> dict[str, float]:
    """Never retry partially advanced states; preserve failed batch inputs for diagnosis."""
    try:
        return train_batch(roots, model, optimizer, max_decisions, entropy_coef, numeric=True)
    except SimulatorStepError as error:
        failure_path.parent.mkdir(parents=True, exist_ok=True)
        with failure_path.open("x") as handle:
            json.dump(
                {
                    "error": str(error),
                    "step": error.step,
                    "specifications": [json.loads(root.spec_json) for root in specifications],
                    "root_indices": error.root_indices,
                    "attempted_prefixes": error.action_prefixes,
                    "note": "Last attempted action may not have been accepted. Reconstruct NEW roots; never retry clones.",
                },
                handle,
                indent=2,
            )
        logging.getLogger(__name__).critical(
            "SIMULATOR FAILURE: discarded %d episodes, no optimizer step. %s", len(roots), failure_path
        )
        if not continue_on_error:
            raise
        return {
            "episodes": float(len(roots)),
            "optimizer_step": 0.0,
            "simulator_error_batches": 1.0,
            "discarded_episodes": float(len(roots)),
        }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--distributions", type=Path, required=True)
    parser.add_argument("--validation-manifest", type=Path, required=True)
    parser.add_argument("--updates", type=int, default=10000)
    parser.add_argument("--batch-size", type=int, default=2048)
    parser.add_argument("--eval-every", type=int, default=100)
    parser.add_argument("--max-decisions", type=int, default=512)
    parser.add_argument("--min-floor", type=int, default=1)
    parser.add_argument("--max-floor", type=int, default=55)
    parser.add_argument("--seed", type=int, default=123)
    parser.add_argument("--lr", type=float, default=1e-4)
    parser.add_argument("--entropy-coef", type=float, default=0.01)
    parser.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    parser.add_argument("--continue-on-simulator-error", action="store_true")
    parser.add_argument("--wandb-project", default="sts-combat-v1")
    parser.add_argument("--wandb-mode", choices=("online", "offline", "disabled"), default="online")
    args = parser.parse_args()
    if min(args.updates, args.batch_size, args.eval_every, args.max_decisions) < 1:
        parser.error("Counts must be positive")
    if not math.isfinite(args.lr) or args.lr <= 0 or not math.isfinite(args.entropy_coef) or args.entropy_coef < 0:
        parser.error("Expected positive finite learning rate and nonnegative finite entropy coefficient")
    config = ScenarioConfig(min_floor=args.min_floor, max_floor=args.max_floor)
    if args.device == "cuda" and not torch.cuda.is_available():
        parser.error("CUDA is unavailable")
    output = Path("wandb") / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    validation_bytes = args.validation_manifest.read_bytes()
    document, groups = load_validation(args.validation_manifest)
    sampler = LoadoutSampler.load(args.distributions)
    (output / "validation.json").write_bytes(validation_bytes)
    validation = [root for roots in groups.values() for root in roots]
    repeats = document["evaluation"]["repeats"]
    eval_limit = document["evaluation"]["max_decisions"]
    excluded = frozenset(int(root.seed) for root in validation)
    settings = {key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()}
    settings.update(
        validation_sha256=hashlib.sha256(validation_bytes).hexdigest(),
        distributions_sha256=hashlib.sha256(args.distributions.read_bytes()).hexdigest(),
        validation_native_sha256=document["native_sha256"],
        evaluation_repeats=repeats,
        evaluation_max_decisions=eval_limit,
        validation_roots=len(validation),
        validation_acts=sorted({root.act for root in validation}),
        training_protocol="fresh_independent_A0_roots_per_update",
        validation_protocol=document["protocol"],
    )
    (output / "config.json").write_text(json.dumps(settings, indent=2))
    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    rng = random.Random(args.seed)
    model = CombatModel().to(args.device)
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr)
    iteration = 0

    def checkpoint() -> None:
        torch.save(
            {
                "model": model.state_dict(),
                "optimizer": optimizer.state_dict(),
                "iteration": iteration,
                "sampling_rng": rng.getstate(),
                "torch_rng": torch.get_rng_state(),
                "cuda_rng": torch.cuda.get_rng_state_all() if args.device == "cuda" else [],
                "config": settings,
            },
            output / "latest.pt",
        )

    with wandb.init(
        project=args.wandb_project, id=args.run_id, name=args.run_id, config=settings, mode=args.wandb_mode
    ) as run:
        try:
            print("Starting fixed initial validation", flush=True)
            run.log(
                evaluate_sets(groups, model, repeats, eval_limit, output / "errors" / "validation-00000000"), step=0
            )
            checkpoint()
            print("Initial validation logged; starting training updates", flush=True)
            for iteration in range(1, args.updates + 1):
                started = time.monotonic()
                roots, specs = fresh_batch(rng, sampler, args.batch_size, config, excluded)
                sampling_seconds = time.monotonic() - started
                scores = update(
                    roots,
                    specs,
                    model,
                    optimizer,
                    args.max_decisions,
                    args.entropy_coef,
                    output / "errors" / f"update-{iteration:08d}.json",
                    continue_on_error=args.continue_on_simulator_error,
                )
                scores.update(
                    sampling_seconds=sampling_seconds,
                    rejected_loadouts=float(sum(len(root.rejected_loadouts) for root in specs)),
                    update_seconds=time.monotonic() - started,
                )
                logs = {f"train/{key}": value for key, value in scores.items()}
                if iteration % args.eval_every == 0 or iteration == args.updates:
                    logs.update(
                        evaluate_sets(
                            groups,
                            model,
                            repeats,
                            eval_limit,
                            output / "errors" / f"validation-{iteration:08d}",
                        )
                    )
                    checkpoint()
                run.log(logs, step=iteration)
                print(f"update={iteration} roots={len(roots)} optimizer_step={scores['optimizer_step']}", flush=True)
        finally:
            checkpoint()


if __name__ == "__main__":
    main()
