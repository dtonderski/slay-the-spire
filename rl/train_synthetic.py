"""Train on fresh synthetic A0 batches; evaluate immutable main/stress specifications."""

import argparse
import hashlib
import json
import logging
import math
import random
import time
from dataclasses import asdict, dataclass
from pathlib import Path

import torch
import wandb
from loadout_sampling import LoadoutSampler
from model import CombatModel
from rollout_errors import SimulatorStepError
from scenarios import ScenarioConfig
from synthetic_roots import SyntheticRoot, sample_root
from train_roots import Root, evaluate, evaluate_beam, train_batch
from validation_set import load_validation


@dataclass
class RewardBaseline:
    """Action-independent reward estimate from PREVIOUS successful batches only."""

    decay: float = 0.95
    value: float = 0.0
    updates: int = 0

    def observe(self, scores: dict[str, float]) -> None:
        if not scores.get("optimizer_step") or not scores.get("completed"):
            return
        mean = scores["mean_return_completed"]
        self.value = mean if self.updates == 0 else self.decay * self.value + (1 - self.decay) * mean
        self.updates += 1


def cached_references(source: Path, output: Path, settings: dict) -> dict[str, float]:
    """Reuse only references with matching fixed inputs and evaluation budgets."""
    previous = json.loads((source / "config.json").read_text())
    for key in (
        "validation_sha256",
        "validation_native_sha256",
        "evaluation_repeats",
        "evaluation_max_decisions",
        "beam_width",
        "beam_transitions",
    ):
        if previous[key] != settings[key]:
            raise ValueError(f"Reference run mismatch: {key}")
    payload = (source / "baselines.json").read_bytes()
    document = json.loads(payload)
    if set(document["sets"]) != {"main", "stress"} or document.get("beam_privileged") is not True:
        raise ValueError("Incomplete reference cache")
    (output / "baselines.json").write_bytes(payload)
    return {
        f"{prefix}_{label}/{key}": value
        for label, records in document["sets"].items()
        for prefix in ("random", "privileged_beam")
        for key, value in records[prefix].items()
    }


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


def evaluate_baselines(
    groups: dict[str, list[Root]],
    repeats: int,
    max_decisions: int,
    output: Path,
    *,
    beam_width: int,
    beam_transitions: int,
) -> dict[str, float]:
    """Persist fixed random and privileged search references; never feed them to the policy."""
    references: dict[str, float] = {}
    records: dict = {
        "beam_privileged": True,
        "beam_width": beam_width,
        "beam_max_transitions_per_root": beam_transitions,
        "max_decisions": max_decisions,
        "random_repeats": repeats,
        "sets": {},
    }
    for label, roots in groups.items():
        print(f"Computing random baseline: {label}", flush=True)
        random_scores = evaluate(
            roots, None, repeats, max_decisions, error_path=output / "errors" / "random-baseline" / f"{label}.jsonl"
        )
        print(f"Computing privileged beam reference: {label}", flush=True)
        beam_scores, beam_records = evaluate_beam(
            roots,
            width=beam_width,
            max_decisions=max_decisions,
            max_transitions=beam_transitions,
        )
        references.update({f"random_{label}/{key}": value for key, value in random_scores.items()})
        references.update({f"privileged_beam_{label}/{key}": value for key, value in beam_scores.items()})
        records["sets"][label] = {"random": random_scores, "privileged_beam": beam_scores, "beam_roots": beam_records}
        temporary = output / "baselines.tmp"
        temporary.write_text(json.dumps(records, indent=2))
        temporary.replace(output / "baselines.json")
    return references


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
    reward_baseline: float = 0.0,
) -> dict[str, float]:
    """Never retry partially advanced states; preserve failed batch inputs for diagnosis."""
    try:
        return train_batch(
            roots, model, optimizer, max_decisions, entropy_coef, numeric=True, reward_baseline=reward_baseline
        )
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
    parser.add_argument("--reward-baseline", choices=("none", "ema"), default="none")
    parser.add_argument("--baseline-decay", type=float, default=0.95)
    parser.add_argument("--reference-run", type=Path)
    parser.add_argument("--beam-width", type=int, default=64)
    parser.add_argument("--beam-transitions", type=int, default=10000)
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
    if (
        min(args.updates, args.batch_size, args.eval_every, args.max_decisions, args.beam_width, args.beam_transitions)
        < 1
    ):
        parser.error("Counts must be positive")
    if not math.isfinite(args.lr) or args.lr <= 0 or not math.isfinite(args.entropy_coef) or args.entropy_coef < 0:
        parser.error("Expected positive finite learning rate and nonnegative finite entropy coefficient")
    if not 0 <= args.baseline_decay < 1:
        parser.error("Baseline decay must be in [0, 1)")
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
    baseline = RewardBaseline(decay=args.baseline_decay)

    def checkpoint() -> None:
        torch.save(
            {
                "model": model.state_dict(),
                "optimizer": optimizer.state_dict(),
                "iteration": iteration,
                "reward_baseline_state": asdict(baseline),
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
            if args.reference_run is not None:
                references = cached_references(args.reference_run, output, settings)
                print(f"Loaded matching random/privileged references from {args.reference_run}", flush=True)
            else:
                references = evaluate_baselines(
                    groups,
                    repeats,
                    eval_limit,
                    output,
                    beam_width=args.beam_width,
                    beam_transitions=args.beam_transitions,
                )
            run.log(references, step=0, commit=False)
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
                    reward_baseline=baseline.value,
                )
                scores["reward_baseline_used"] = baseline.value
                if args.reward_baseline == "ema":
                    baseline.observe(scores)
                scores["reward_baseline_next"] = baseline.value
                scores["baseline_updates"] = float(baseline.updates)
                scores.update(
                    sampling_seconds=sampling_seconds,
                    rejected_loadouts=float(sum(len(root.rejected_loadouts) for root in specs)),
                    update_seconds=time.monotonic() - started,
                )
                logs = {**references, **{f"train/{key}": value for key, value in scores.items()}}
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
