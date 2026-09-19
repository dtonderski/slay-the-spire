"""Synthetic combat training: rollout, update, evaluation, and one training loop."""

import argparse
import hashlib
import json
import logging
import math
import random
import shutil
import time
from dataclasses import dataclass
from pathlib import Path
from typing import cast

import torch
import wandb
from beam_search import beam_search
from combat_task import action_indices, combat_outcome, terminal_reward
from loadout_sampling import LoadoutSampler
from model import CombatValueModel
from rollout_errors import SimulatorStepError
from scenarios import ScenarioConfig
from sts_sim import State
from synthetic_roots import SyntheticRoot, sample_root
from torch import Tensor
from torch.distributions import Categorical
from trajectories import DecisionRound, Trajectories, validate_gradients
from validation_set import Root, load_validation


@dataclass
class Episode:
    reward: float | None  # Terminal HP / starting max HP; None means truncation.
    won: bool | None
    hp: int
    decisions: int


def play_combats(
    roots: list[State],
    model: CombatValueModel | None,
    *,
    max_decisions: int,
    training: bool = False,
    rng: random.Random,
    trajectories: Trajectories | None = None,
) -> list[Episode]:
    """One numeric rollout path. Clone roots; errors never retry partially advanced states."""
    if not roots or max_decisions < 0:
        raise ValueError("Rollouts need roots and a nonnegative decision limit")
    if training and (model is None or trajectories is None):
        raise ValueError("Training requires a model and trajectory collector")
    if trajectories is not None and trajectories.rounds:
        raise ValueError("Use a fresh trajectory collector for each batch")
    from encoders.numeric import NumericBatch

    states = [root.clone() for root in roots]
    batch = NumericBatch(State.numeric_decisions(states))
    starting_max_hp = batch.table("header", 5)[:, 4].tolist()
    remaining = list(range(len(roots)))
    action_prefixes: list[list[int]] = [[] for _ in roots]
    episodes: list[Episode | None] = [None] * len(roots)
    for step in range(max_decisions + 1):
        active = []
        for row, index in enumerate(remaining):
            kind, phase, combat_phase, hp, _ = batch.table("header", 5)[row]
            kind, phase = batch.symbols[kind], batch.symbols[phase]
            combat_phase = batch.symbols[combat_phase] if combat_phase >= 0 else None
            won = (
                True
                if phase == "reward" or combat_phase == "won" or (kind == "complete" and hp > 0)
                else (False if combat_phase == "lost" or (kind == "complete" and hp <= 0) else None)
            )
            if kind == "event":
                # Rare terminal event return (Colosseum); validate through the typed public screen.
                won = combat_outcome(states[index].decision().observation)
            if won is not None:
                hp = int(hp) if won else 0
                episodes[index] = Episode(hp / starting_max_hp[index], won, hp, step)
            elif kind != "combat":
                raise RuntimeError(f"Unexpected screen after combat: {kind}/{phase}")
            elif step == max_decisions:
                episodes[index] = Episode(None, None, int(hp), step)
            else:
                if not batch.actions[row]:
                    raise SimulatorStepError(
                        "Unsettled combat decision or empty legal-action list",
                        step,
                        [index],
                        [list(action_prefixes[index])],
                    )
                active.append(row)
                continue
        if not active:
            assert all(episode is not None for episode in episodes)
            return [episode for episode in episodes if episode is not None]
        if active != batch.model_rows:
            raise RuntimeError("Numeric combat rows do not match settled decisions")
        potions = batch.table("potions", 3)
        offsets = [0]
        for length in batch.lengths(potions):
            offsets.append(offsets[-1] + length)
        actions = []
        for position, row in enumerate(active):
            candidates = []
            for action in batch.actions[row]:
                if action.kind == "use_potion_slot" and action.potion_slot is not None:
                    code = potions[offsets[position] + action.potion_slot, 1]
                    if code >= 0 and batch.symbols[code] == "smoke_bomb":
                        continue
                candidates.append(action)
            if not candidates:
                raise RuntimeError("No allowed combat actions after disabling escape")
            actions.append(tuple(candidates))
        if model is None:
            choices = [rng.randrange(len(candidates)) for candidates in actions]
        else:
            with torch.set_grad_enabled(training):
                logits, values, _ = model(batch, actions)
                distribution = Categorical(logits=logits)
                sampled = distribution.sample()
                if trajectories is not None:
                    trajectories.rounds.append(
                        DecisionRound(
                            tuple(remaining[row] for row in active),
                            tuple(len(candidates) for candidates in actions),
                            distribution.log_prob(sampled),
                            distribution.entropy(),
                            cast(Tensor, distribution.probs).max(dim=1).values.detach(),
                            values.squeeze(-1),
                        )
                    )
                choices = sampled.tolist()
        chosen = [candidates[choice] for candidates, choice in zip(actions, choices)]
        for row, action in zip(active, chosen, strict=True):
            action_prefixes[remaining[row]].append(batch.actions[row].index(action))
        remaining = [remaining[row] for row in active]
        try:
            payload = State.numeric_steps([states[index] for index in remaining], chosen)
        except ValueError as error:
            # Native batches are not atomic. Never retry/reapply actions to these clones.
            raise SimulatorStepError(
                str(error), step, remaining, [action_prefixes[index] for index in remaining]
            ) from error
        batch = NumericBatch(payload)
    raise AssertionError("Unreachable")


def episode_metrics(episodes: list[Episode]) -> dict[str, float]:
    completed = [episode for episode in episodes if episode.reward is not None]
    result = {
        "episodes": float(len(episodes)),
        "completed": float(len(completed)),
        "truncated": float(len(episodes) - len(completed)),
        "defeated": float(sum(episode.won is False for episode in completed)),
        "mean_decisions": sum(episode.decisions for episode in episodes) / len(episodes),
    }
    if completed:
        result.update(
            {
                "win_rate_completed": sum(episode.won is True for episode in completed) / len(completed),
                "mean_return_completed": sum(episode.reward for episode in completed if episode.reward is not None)
                / len(completed),
                "mean_hp_completed": sum(episode.hp for episode in completed) / len(completed),
            }
        )
    return result


def evaluate(
    roots: list[Root],
    model: CombatValueModel | None,
    repeats: int,
    max_decisions: int,
    *,
    error_path: Path | None = None,
) -> dict[str, float]:
    """Same validation sampling seeds each time; never consume training RNG state."""
    episodes: list[Episode] = []
    hp_changes: list[int] = []
    failed_cases: set[int] = set()
    failures = 0
    if model is not None:
        model.eval()
    # manual_seed also seeds CUDA; preserve its state when a GPU model is evaluated.
    devices = list(range(torch.cuda.device_count())) if model is not None and next(model.parameters()).is_cuda else []
    with torch.random.fork_rng(devices=devices):
        for index, root in enumerate(roots):
            for repeat in range(repeats):
                seed = 90000 + index * repeats + repeat
                if devices:
                    torch.manual_seed(seed)
                else:
                    torch.random.default_generator.manual_seed(seed)
                try:
                    ep = play_combats([root.state], model, max_decisions=max_decisions, rng=random.Random(seed))[0]
                except SimulatorStepError as error:
                    if error_path is None:
                        raise
                    failures += 1
                    failed_cases.add(index)
                    error_path.parent.mkdir(parents=True, exist_ok=True)
                    with error_path.open("a") as handle:
                        handle.write(
                            json.dumps(
                                {
                                    "root_index": index,
                                    "combat_seed": root.combat_seed,
                                    "repeat": repeat,
                                    "policy_seed": seed,
                                    "error": str(error),
                                    "step": error.step,
                                    "attempted_prefixes": error.action_prefixes,
                                }
                            )
                            + "\n"
                        )
                    print(f"VALIDATION SIMULATOR ERROR root={index} repeat={repeat}: {error_path}", flush=True)
                    continue
                episodes.append(ep)
                if ep.reward is not None:
                    hp_changes.append(ep.hp - root.start_hp)
    result = episode_metrics(episodes) if episodes else {"episodes": 0.0, "completed": 0.0, "truncated": 0.0}
    result.update(
        attempted_episodes=float(len(roots) * repeats),
        simulator_error_episodes=float(failures),
        cases_with_errors=float(len(failed_cases)),
        episode_coverage=len(episodes) / (len(roots) * repeats) if roots and repeats else 0.0,
    )
    if hp_changes:
        result["mean_hp_change_completed"] = sum(hp_changes) / len(hp_changes)
        result["mean_hp_lost_completed"] = -result["mean_hp_change_completed"]
    return result


def train_batch(
    roots: list[Root],
    model: CombatValueModel,
    optimizer: torch.optim.Optimizer,
    max_decisions: int,
    entropy_coef: float = 0.0,
    *,
    value_coef: float = 0.1,
) -> dict[str, float]:
    """Collect one batch, calculate the batched loss, and take one optimizer step.

    Simulator errors propagate to the experiment's diagnostic/skip handler.
    """
    model.train()
    optimizer.zero_grad(set_to_none=True)
    trajectories = Trajectories()
    episodes = play_combats(
        [root.state for root in roots],
        model,
        max_decisions=max_decisions,
        training=True,
        rng=random.Random(0),
        trajectories=trajectories,
    )
    loss, policy_loss = trajectories.losses(episodes, entropy_coef, value_coef=value_coef)
    result = episode_metrics(episodes)
    result.update(trajectories.metrics())
    result.update(simulator_error_batches=0.0, discarded_episodes=0.0, optimizer_step=float(loss is not None))
    hp_losses = [root.start_hp - ep.hp for root, ep in zip(roots, episodes, strict=True) if ep.reward is not None]
    if hp_losses:
        result["mean_hp_lost_completed"] = sum(hp_losses) / len(hp_losses)
    if loss is not None:
        if not torch.isfinite(loss):
            raise RuntimeError("Non-finite loss")
        loss.backward()
        validate_gradients(model.parameters())
        optimizer.step()
        assert policy_loss is not None
        result["loss"] = loss.detach().item()
        result["policy_loss"] = policy_loss.detach().item()
        assert trajectories.value_loss is not None
        result["value_loss"] = trajectories.value_loss.item()
        result["entropy_bonus"] = result["policy_loss"] + value_coef * result["value_loss"] - result["loss"]
    return result


def evaluate_beam(
    roots: list[Root], *, width: int, max_decisions: int, max_transitions: int
) -> tuple[dict, list[dict]]:
    """Privileged deterministic reference, once per root; never silently discard failures."""
    if not roots:
        raise ValueError("Beam evaluation needs nonempty roots")
    records: list[dict] = []
    start = time.monotonic()
    for index, root in enumerate(roots):
        record: dict = {
            "root_index": index,
            "seed": root.combat_seed,
            "act": root.act,
            "floor": root.floor,
            "starting_hp": root.start_hp,
        }
        try:
            result = beam_search(root.state, width=width, max_decisions=max_decisions, max_transitions=max_transitions)
            record.update(
                reward=result.reward,
                hp=result.hp,
                won=result.won,
                transitions=result.transitions,
                limit_reached=result.limit_reached,
                status="completed" if result.reward is not None else "unfinished",
            )
            # Verify the reported terminal result against the actual accepted-action path.
            replay = root.state.clone()
            indices = []
            for action in result.actions:
                decision = replay.decision()
                choice = next(i for i in action_indices(decision) if repr(decision.actions[i]) == repr(action))
                replay.step(decision.actions[choice])
                indices.append(choice)
            record["action_indices"] = indices
            if result.reward is not None:
                starting_max_hp = root.state.decision().observation.context.player_max_hp
                if terminal_reward(replay.decision().observation, starting_max_hp) != result.reward:
                    raise RuntimeError("Beam plan replay reward mismatch")
                assert result.hp is not None
                record["hp_lost"] = root.start_hp - result.hp
        except Exception as error:
            logging.getLogger(__name__).exception("Beam evaluation failed for root %s", index)
            record.update(status="error", error=f"{type(error).__name__}: {error}")
        records.append(record)
        print(f"BEAM {index + 1}/{len(roots)} status={record['status']}", flush=True)
    completed = [record for record in records if record["status"] == "completed"]
    scores = {
        "roots": len(roots),
        "completed": len(completed),
        "unfinished": sum(record["status"] == "unfinished" for record in records),
        "errors": sum(record["status"] == "error" for record in records),
        "completion_rate": len(completed) / len(roots),
        "found_win_rate_all_roots": sum(record["won"] is True for record in completed) / len(roots),
        "transitions": sum(record.get("transitions", 0) for record in records),
        "limit_reached": sum(record.get("limit_reached", False) for record in records),
        "seconds": time.monotonic() - start,
    }
    if completed:
        scores.update(
            mean_return_completed=sum(record["reward"] for record in completed) / len(completed),
            mean_hp_completed=sum(record["hp"] for record in completed) / len(completed),
            mean_hp_lost_completed=sum(record["hp_lost"] for record in completed) / len(completed),
            win_rate_completed=sum(record["won"] is True for record in completed) / len(completed),
        )
    return scores, records


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
    if "main" not in document["sets"] or document.get("beam_privileged") is not True:
        raise ValueError("Incomplete reference cache")
    (output / "baselines.json").write_bytes(payload)
    return {
        f"{prefix}_main/{key}": value
        for prefix in ("random", "privileged_beam")
        for key, value in document["sets"]["main"][prefix].items()
    }


def evaluate_baselines(
    roots: list[Root],
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
    print("Computing random baseline: main", flush=True)
    random_scores = evaluate(
        roots, None, repeats, max_decisions, error_path=output / "errors" / "random-baseline" / "main.jsonl"
    )
    print("Computing privileged beam reference: main", flush=True)
    beam_scores, beam_records = evaluate_beam(
        roots,
        width=beam_width,
        max_decisions=max_decisions,
        max_transitions=beam_transitions,
    )
    references.update({f"random_main/{key}": value for key, value in random_scores.items()})
    references.update({f"privileged_beam_main/{key}": value for key, value in beam_scores.items()})
    records["sets"]["main"] = {"random": random_scores, "privileged_beam": beam_scores, "beam_roots": beam_records}
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


def update_with_diagnostics(
    roots: list[Root],
    specifications: list[SyntheticRoot],
    model: CombatValueModel,
    optimizer: torch.optim.Optimizer,
    max_decisions: int,
    entropy_coef: float,
    failure_path: Path,
    *,
    continue_on_error: bool = False,
    value_coef: float = 0.1,
) -> dict[str, float]:
    """Never retry partially advanced states; preserve failed batch inputs for diagnosis."""
    try:
        return train_batch(roots, model, optimizer, max_decisions, entropy_coef, value_coef=value_coef)
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
    parser.add_argument("--max-hours", type=float, help="Stop after this training/validation wall-clock budget")
    parser.add_argument("--batch-size", type=int, default=2048)
    parser.add_argument("--eval-every", type=int, default=100)
    parser.add_argument("--value-coef", type=float, default=0.1)
    parser.add_argument("--reference-run", type=Path)
    initialization = parser.add_mutually_exclusive_group()
    initialization.add_argument("--warm-start", type=Path, help="Trusted checkpoint: weights only; new optimizer/RNG")
    initialization.add_argument(
        "--resume-from", type=Path, help="Trusted same-protocol checkpoint: restore optimizer/RNG"
    )
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
    if not math.isfinite(args.value_coef) or args.value_coef < 0:
        parser.error("Value coefficient must be finite and nonnegative")
    if args.max_hours is not None and (not math.isfinite(args.max_hours) or args.max_hours <= 0):
        parser.error("Maximum hours must be finite and positive")
    config = ScenarioConfig(min_floor=args.min_floor, max_floor=args.max_floor)
    if args.device == "cuda" and not torch.cuda.is_available():
        parser.error("CUDA is unavailable")
    output = Path("wandb") / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    validation_bytes = args.validation_manifest.read_bytes()
    document, groups = load_validation(args.validation_manifest)
    sampler = LoadoutSampler.load(args.distributions)
    (output / "validation.json").write_bytes(validation_bytes)
    validation = groups["main"]
    repeats = document["evaluation"]["repeats"]
    eval_limit = document["evaluation"]["max_decisions"]
    # Keep ALL frozen seeds held out, even cases no longer evaluated by this trainer.
    excluded = frozenset(int(root.combat_seed) for roots in groups.values() for root in roots)
    settings = {key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()}
    settings.update(
        validation_sha256=hashlib.sha256(validation_bytes).hexdigest(),
        distributions_sha256=hashlib.sha256(args.distributions.read_bytes()).hexdigest(),
        validation_native_sha256=document["native_sha256"],
        evaluation_repeats=repeats,
        evaluation_max_decisions=eval_limit,
        validation_roots=len(validation),
        validation_subset="main",
        validation_acts=sorted({root.act for root in validation}),
        training_protocol="fresh_independent_A0_roots_per_update",
        validation_protocol=document["protocol"],
    )
    (output / "config.json").write_text(json.dumps(settings, indent=2))
    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    rng = random.Random(args.seed)
    model = CombatValueModel().to(args.device)
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr)
    initial_checkpoint = args.resume_from or args.warm_start
    if initial_checkpoint is not None:
        # Always use a new output directory. Cross-native warm starts load weights
        # only; optimizer/RNG continuation requires the same gameplay/training contract.
        source = torch.load(initial_checkpoint, map_location="cpu", weights_only=False)
        if args.resume_from is not None:
            for key in (
                "validation_sha256",
                "validation_native_sha256",
                "distributions_sha256",
                "training_protocol",
                "min_floor",
                "max_floor",
                "max_decisions",
                "lr",
                "entropy_coef",
                "value_coef",
                "device",
            ):
                if source["config"].get(key) != settings[key]:
                    raise ValueError(f"Checkpoint continuation mismatch: {key}")
        weights = source["model"]
        if not all(torch.isfinite(value).all() for value in weights.values()):
            raise ValueError("Warm-start checkpoint contains non-finite weights")
        model.load_state_dict(weights, strict=True)
        if args.resume_from is not None:
            optimizer.load_state_dict(source["optimizer"])
            rng.setstate(source["sampling_rng"])
            torch.set_rng_state(source["torch_rng"])
            if args.device == "cuda":
                torch.cuda.set_rng_state_all(source["cuda_rng"])
        settings["initialization"] = {
            "mode": "optimizer_rng_continuation" if args.resume_from else "weights_only_new_optimizer_and_rng",
            "checkpoint_sha256": hashlib.sha256(initial_checkpoint.read_bytes()).hexdigest(),
            "source_iteration": source["iteration"],
            "source_native_sha256": source["config"].get("validation_native_sha256"),
        }
        del source, weights
        (output / "config.json").write_text(json.dumps(settings, indent=2))
        print(f"Initialized from {initial_checkpoint}: {settings['initialization']['mode']}", flush=True)
    iteration = 0

    def checkpoint() -> None:
        temporary = output / "latest.tmp"
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
            temporary,
        )
        temporary.replace(output / "latest.pt")
        archived = output / f"checkpoint-{iteration:08d}.pt"
        if not archived.exists():
            shutil.copyfile(output / "latest.pt", archived)

    with wandb.init(
        project=args.wandb_project, id=args.run_id, name=args.run_id, config=settings, mode=args.wandb_mode
    ) as run:
        try:
            if args.reference_run is not None:
                references = cached_references(args.reference_run, output, settings)
                print(f"Loaded matching random/privileged references from {args.reference_run}", flush=True)
            else:
                references = evaluate_baselines(
                    validation,
                    repeats,
                    eval_limit,
                    output,
                    beam_width=args.beam_width,
                    beam_transitions=args.beam_transitions,
                )
            run.log(references, step=0, commit=False)
            print("Starting fixed initial validation", flush=True)
            validation_scores = evaluate(
                validation, model, repeats, eval_limit, error_path=output / "errors" / "validation-00000000.jsonl"
            )
            run.log({f"val_main/{key}": value for key, value in validation_scores.items()}, step=0)
            checkpoint()
            print("Initial validation logged; starting training updates", flush=True)
            deadline = time.monotonic() + args.max_hours * 3600 if args.max_hours is not None else math.inf
            for iteration in range(1, args.updates + 1):
                started = time.monotonic()
                roots, specs = fresh_batch(rng, sampler, args.batch_size, config, excluded)
                sampling_seconds = time.monotonic() - started
                scores = update_with_diagnostics(
                    roots,
                    specs,
                    model,
                    optimizer,
                    args.max_decisions,
                    args.entropy_coef,
                    output / "errors" / f"update-{iteration:08d}.json",
                    continue_on_error=args.continue_on_simulator_error,
                    value_coef=args.value_coef,
                )
                if args.device == "cuda":
                    scores["peak_cuda_allocated_gib"] = torch.cuda.max_memory_allocated() / 2**30
                    scores["peak_cuda_reserved_gib"] = torch.cuda.max_memory_reserved() / 2**30
                scores.update(
                    sampling_seconds=sampling_seconds,
                    rejected_loadouts=float(sum(len(root.rejected_loadouts) for root in specs)),
                    update_seconds=time.monotonic() - started,
                )
                logs = {**references, **{f"train/{key}": value for key, value in scores.items()}}
                finished = iteration == args.updates or time.monotonic() >= deadline
                if iteration % args.eval_every == 0 or finished:
                    validation_scores = evaluate(
                        validation,
                        model,
                        repeats,
                        eval_limit,
                        error_path=output / "errors" / f"validation-{iteration:08d}.jsonl",
                    )
                    logs.update({f"val_main/{key}": value for key, value in validation_scores.items()})
                    checkpoint()
                run.log(logs, step=iteration)
                print(f"update={iteration} roots={len(roots)} optimizer_step={scores['optimizer_step']}", flush=True)
                if finished or time.monotonic() >= deadline:
                    print("Training budget reached; saving final checkpoint", flush=True)
                    break
        finally:
            checkpoint()


if __name__ == "__main__":
    main()
