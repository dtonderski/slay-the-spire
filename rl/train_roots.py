"""Frozen multi-combat REINFORCE experiment, split by run seed."""

import argparse
import json
import logging
import math
import random
import time
import traceback
from dataclasses import dataclass
from pathlib import Path

import torch
import wandb
from beam_search import beam_search
from combat_task import action_indices, terminal_reward
from model import CombatModel
from rollout_errors import SimulatorStepError
from sts_sim import Decision, State
from train import (
    Episode,
    episode_loss,
    metrics,
    play_combat,
    play_combats,
    reinforce_loss,
)


@dataclass
class Root:
    state: State
    seed: str
    floor: int
    start_hp: int
    act: int = 1


def key_collection_choices(decision: Decision, allowed: list[int], sapphire_taken: bool) -> list[int]:
    """Prefer legal keys and public routes to the burning elite; otherwise stay random."""
    keys = [i for i in allowed if decision.actions[i].kind in ("rest_recall", "take_sapphire_key", "take_emerald_key")]
    if keys:
        return keys
    obs = decision.observation
    if obs.kind == "treasure" and obs.screen.chest_size != "boss" and not sapphire_taken:
        opened = [i for i in allowed if decision.actions[i].kind == "open_chest"]
        if opened:
            return opened
    if obs.kind == "map":
        reachable = {node.slot for node in obs.screen.nodes if node.burning_elite}
        while True:
            previous = len(reachable)
            reachable.update(
                node.slot for node in obs.screen.nodes if any(child in reachable for child in node.children)
            )
            if len(reachable) == previous:
                break
        paths = [
            i
            for i in allowed
            if decision.actions[i].kind == "choose_map_node" and decision.actions[i].node_slot in reachable
        ]
        if paths:
            return paths
    return allowed


def collect_roots(
    seeds: list[str], floors: int, rng_seed: int, *, synthetic_act1: bool = False, synthetic_act4: bool = False
) -> tuple[list[Root], list[dict]]:
    """Random collector with escape disabled; retain deaths/cutoffs, never replace seeds."""
    if synthetic_act1 and synthetic_act4:
        raise ValueError("Choose one synthetic collection scope")
    synthetic = synthetic_act1 or synthetic_act4
    roots: list[Root] = []
    manifest: list[dict] = []
    for seed in seeds:
        rng = random.Random(f"{rng_seed}:{seed}")
        state = (
            State.new_synthetic(seed, ascension=0, hp=10000, final_act=synthetic_act4)
            if synthetic
            else State.new(seed, ascension=0)
        )
        decision = state.decision()
        choices: list[int] = []
        entries: list[dict] = []
        in_combat = False
        status = "decision_limit"
        failed_action = None
        sapphire_taken = False
        for _ in range(5000):
            obs = decision.observation
            if synthetic_act1 and (obs.context.act > 1 or (obs.context.floor >= 16 and obs.kind != "combat")):
                status = "act1_complete"
                break
            if not synthetic_act4 and obs.context.floor > (16 if synthetic_act1 else floors):
                status = "floor_limit"
                break
            if (obs.kind == "complete" and (obs.context.player_hp <= 0 or not decision.actions)) or (
                obs.kind == "combat" and obs.screen.phase == "lost"
            ):
                status = "death" if obs.context.player_hp <= 0 else "complete"
                break
            if obs.kind == "combat":
                if not in_combat:
                    root = state.synthetic_combat_root(100) if synthetic else state.clone()
                    roots.append(
                        Root(
                            root, seed, obs.context.floor, 100 if synthetic else obs.context.player_hp, obs.context.act
                        )
                    )
                    entries.append(
                        {
                            "act": obs.context.act,
                            "floor": obs.context.floor,
                            "prefix_length": len(choices),
                            "hp": obs.context.player_hp,
                        }
                    )
                in_combat = True
            else:
                in_combat = False
            if not decision.actions:
                raise RuntimeError(f"No legal actions during collection: seed={seed}, screen={obs.kind}")
            allowed = action_indices(decision)
            if not allowed:
                raise RuntimeError(f"No allowed collection actions: seed={seed}, screen={obs.kind}")
            if synthetic_act4:
                allowed = key_collection_choices(decision, allowed, sapphire_taken)
            index = rng.choice(allowed)
            state_action_kind = decision.actions[index].kind
            try:
                decision = state.step(decision.actions[index])
            except ValueError as error:
                if not synthetic:
                    raise
                status = f"error: {error}"
                failed_action = {
                    "index": index,
                    "action": repr(decision.actions[index]),
                    "act": obs.context.act,
                    "floor": obs.context.floor,
                    "screen": obs.kind,
                }
                break
            sapphire_taken |= state_action_kind == "take_sapphire_key"
            choices.append(index)
        manifest.append(
            {
                "seed": seed,
                "status": status,
                "roots": entries,
                "accepted_action_indices": choices,
                "synthetic_initial_hp": 10000 if synthetic else None,
                "synthetic_root_hp": 100 if synthetic else None,
                "synthetic_acts": 4 if synthetic_act4 else (1 if synthetic_act1 else None),
                "final_act_available": synthetic_act4,
                "key_priority": synthetic_act4,
                "last_act": decision.observation.context.act,
                "last_floor": decision.observation.context.floor,
                "failed_action": failed_action,
            }
        )
        print(f"collected seed={seed} roots={len(entries)} status={status}", flush=True)
    return roots, manifest


def evaluate(roots: list[Root], model: CombatModel | None, repeats: int, max_decisions: int) -> dict[str, float]:
    """Same validation sampling seeds each time; never consume training RNG state."""
    episodes: list[Episode] = []
    hp_changes: list[int] = []
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
                ep = play_combat(root.state, model, max_decisions=max_decisions, rng=random.Random(seed))
                episodes.append(ep)
                if ep.reward is not None:
                    hp_changes.append(ep.hp - root.start_hp)
    result = metrics(episodes)
    if hp_changes:
        result["mean_hp_change_completed"] = sum(hp_changes) / len(hp_changes)
        result["mean_hp_lost_completed"] = -result["mean_hp_change_completed"]
    return result


def train_batch(
    roots: list[Root],
    model: CombatModel,
    optimizer: torch.optim.Optimizer,
    max_decisions: int,
    entropy_coef: float = 0.0,
    *,
    numeric: bool = False,
    error_path: Path | None = None,
    root_indices: list[int] | None = None,
) -> dict[str, float]:
    """Train one group; optionally log native-step failures and discard the entire group."""
    model.train()
    optimizer.zero_grad(set_to_none=True)
    try:
        episodes = play_combats(
            [root.state for root in roots],
            model,
            max_decisions=max_decisions,
            training=True,
            rng=random.Random(0),
            numeric=numeric,
        )
    except SimulatorStepError as error:
        if error_path is None:
            raise
        logging.getLogger(__name__).critical(
            "!!! CRITICAL: SIMULATOR/INTERFACE FAILURE — ENTIRE TRAINING BATCH DISCARDED !!! "
            "%d episodes excluded; NO optimizer step, NO rewards or defeats assigned. "
            "Training continues but coverage is incomplete. Diagnostics: %s",
            len(roots),
            error_path,
            exc_info=True,
        )
        # Store only diagnostics/provenance, never tensors, private state, or repaired observations.
        report = {
            "error": str(error),
            "traceback": traceback.format_exc(),
            "step": error.step,
            "numeric": numeric,
            "discarded_episodes": len(roots),
            "batch_advancement_may_be_partial": True,
            "roots": [
                {
                    "batch_index": i,
                    "dataset_root_index": None if root_indices is None else root_indices[i],
                    "seed": root.seed,
                    "act": root.act,
                    "floor": root.floor,
                    "starting_hp": root.start_hp,
                }
                for i, root in enumerate(roots)
            ],
            "attempted_prefixes": [
                {"batch_index": i, "native_action_indices": prefix}
                for i, prefix in zip(error.root_indices, error.action_prefixes, strict=True)
            ],
            "note": "Prefixes start at frozen roots and include the current attempted action. "
            "Its acceptance is unknown; never retry these partially advanced clones. "
            "Use dataset_root_index with roots.json to reconstruct the independent frozen root.",
        }
        error_path.parent.mkdir(parents=True, exist_ok=True)
        with error_path.open("x") as handle:
            json.dump(report, handle, indent=2)
        return {
            "episodes": float(len(roots)),
            "completed": 0.0,
            "truncated": 0.0,
            "defeated": 0.0,
            "escaped": 0.0,
            "optimizer_step": 0.0,
            "simulator_error_batches": 1.0,
            "discarded_episodes": float(len(roots)),
        }
    losses = [episode_loss(ep, entropy_coef) for ep in episodes if ep.reward is not None]
    policy_losses = [reinforce_loss(ep.log_probs, ep.reward) for ep in episodes if ep.reward is not None]
    result = metrics(episodes)
    result.update(simulator_error_batches=0.0, discarded_episodes=0.0)
    hp_losses = [root.start_hp - ep.hp for root, ep in zip(roots, episodes, strict=True) if ep.reward is not None]
    if hp_losses:
        result["mean_hp_lost_completed"] = sum(hp_losses) / len(hp_losses)
    result["optimizer_step"] = float(bool(losses))
    if losses:
        loss = torch.stack(losses).mean()
        if not torch.isfinite(loss):
            raise RuntimeError("Non-finite loss")
        loss.backward()
        for parameter in model.parameters():
            if parameter.grad is not None and not torch.isfinite(parameter.grad).all():
                raise RuntimeError("Non-finite gradient")
        optimizer.step()
        result["loss"] = loss.detach().item()
        result["policy_loss"] = torch.stack(policy_losses).mean().detach().item()
        result["entropy_bonus"] = result["policy_loss"] - result["loss"]
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
            "seed": root.seed,
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


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hours", type=float, default=8)
    parser.add_argument("--entropy-coef", type=float, default=0.01)
    scope = parser.add_mutually_exclusive_group()
    scope.add_argument(
        "--synthetic-act4",
        action="store_true",
        help="Collect through Act 4 with legal key priority; freeze roots at 100/100",
    )
    scope.add_argument(
        "--synthetic-act1", action="store_true", help="Collect at 10000 HP; freeze combat roots at 100/100"
    )
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--train-seeds", type=int, default=40)
    parser.add_argument("--val-seeds", type=int, default=10)
    parser.add_argument("--floors", type=int, default=10)
    parser.add_argument("--eval-repeats", type=int, default=3)
    parser.add_argument("--eval-minutes", type=float, default=20)
    parser.add_argument("--batch-size", type=int, default=8)
    parser.add_argument("--beam-width", type=int, default=64)
    parser.add_argument("--beam-transitions", type=int, default=10000)
    parser.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    parser.add_argument("--numeric-observations", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--max-decisions", type=int, default=512)
    parser.add_argument("--wandb-mode", choices=("online", "offline", "disabled"), default="online")
    args = parser.parse_args()
    if (
        min(
            args.hours,
            args.train_seeds,
            args.val_seeds,
            args.floors,
            args.eval_repeats,
            args.eval_minutes,
            args.batch_size,
            args.max_decisions,
            args.beam_width,
            args.beam_transitions,
        )
        <= 0
    ):
        parser.error("Counts and durations must be positive")
    if args.entropy_coef < 0 or not math.isfinite(args.entropy_coef):
        parser.error("Entropy coefficient must be nonnegative")
    if args.device == "cuda" and not torch.cuda.is_available():
        parser.error("CUDA is not available")
    torch.set_num_threads(1)
    torch.manual_seed(123)
    shuffle_rng = random.Random(123)
    seeds = [str(1000000 + index) for index in range(args.train_seeds + args.val_seeds)]
    shuffle_rng.shuffle(seeds)
    train_seeds, val_seeds = seeds[: args.train_seeds], seeds[args.train_seeds :]
    output = Path("wandb") / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()
    model = CombatModel().to(args.device)
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
    epoch, update = 0, 0
    with wandb.init(
        id=args.run_id,
        name=f"overnight-{args.train_seeds}train-{args.val_seeds}val-hp"
        + ("-synthetic-act4" if args.synthetic_act4 else ("-synthetic-act1" if args.synthetic_act1 else "")),
        project="sts-combat-v1",
        mode=args.wandb_mode,
        settings=wandb.Settings(base_url="http://localhost:8080"),
        config={
            **vars(args),
            "policy_seed": 123,
            "ascension": 0,
            "lr": 1e-4,
            "reward": "terminal_hp_over_starting_max_hp",
            "baseline": "none",
            "collector": "random_no_escape_key_priority" if args.synthetic_act4 else "uniform_random_no_escape",
            "smoke_bomb_use": False,
            "simulator_step_failure_policy": "discard_entire_batch_log_and_continue",
            "beam_reference": "privileged_weighted_beam",
            "beam_repeats": 1,
            "train_seeds_list": train_seeds,
            "val_seeds_list": val_seeds,
        },
    ) as run:
        try:
            train, train_manifest = collect_roots(
                train_seeds, args.floors, 123, synthetic_act1=args.synthetic_act1, synthetic_act4=args.synthetic_act4
            )
            val, val_manifest = collect_roots(
                val_seeds, args.floors, 123, synthetic_act1=args.synthetic_act1, synthetic_act4=args.synthetic_act4
            )
            manifest = {
                "train": train_manifest,
                "val": val_manifest,
                "note": "Action indices replay against this code version; roots are initial combat clones, never policy inputs.",
            }
            (output / "roots.json").write_text(json.dumps(manifest, indent=2))
            if not train or not val:
                raise RuntimeError("Empty train or validation roots")
            run.summary.update(
                {
                    "train_roots": len(train),
                    "val_roots": len(val),
                    **{f"train_roots_act_{act}": sum(root.act == act for root in train) for act in range(1, 5)},
                    **{f"val_roots_act_{act}": sum(root.act == act for root in val) for act in range(1, 5)},
                    "collection_deaths": sum(item["status"] == "death" for item in train_manifest + val_manifest),
                    "collection_errors": sum(
                        item["status"].startswith("error:") for item in train_manifest + val_manifest
                    ),
                    "collection_cutoffs": sum(
                        item["status"] == "decision_limit" for item in train_manifest + val_manifest
                    ),
                }
            )
            print(f"ROOTS train={len(train)} val={len(val)}", flush=True)
            print(
                "ROOTS BY ACT",
                {
                    act: {"train": sum(r.act == act for r in train), "val": sum(r.act == act for r in val)}
                    for act in range(1, 5)
                },
                flush=True,
            )
            if args.synthetic_act4 and (not any(r.act == 4 for r in train) or not any(r.act == 4 for r in val)):
                raise RuntimeError(
                    "Act 4 roots missing from train or validation; inspect collection and increase seed coverage"
                )
            random_scores = evaluate(val, None, args.eval_repeats, args.max_decisions)
            beam_scores, beam_records = evaluate_beam(
                val, width=args.beam_width, max_decisions=args.max_decisions, max_transitions=args.beam_transitions
            )
            (output / "baselines.json").write_text(
                json.dumps(
                    {
                        "random_val": random_scores,
                        "beam_val": beam_scores,
                        "beam_roots": beam_records,
                        "config": {
                            "privileged": True,
                            "width": args.beam_width,
                            "max_decisions": args.max_decisions,
                            "max_transitions": args.beam_transitions,
                        },
                    },
                    indent=2,
                )
            )
            references = {
                **{f"random_val/{k}": v for k, v in random_scores.items()},
                **{f"beam_val/{k}": v for k, v in beam_scores.items()},
            }
            run.log(
                {
                    **references,
                    **{
                        f"initial_val/{k}": v
                        for k, v in evaluate(val, model, args.eval_repeats, args.max_decisions).items()
                    },
                },
                step=0,
            )
            # Collection and one-time references do not consume the training time budget.
            start = time.monotonic()
            next_eval = start + args.eval_minutes * 60
            train_indices = {id(root): i for i, root in enumerate(train)}
            error_batches, discarded_episodes = 0.0, 0.0
            while time.monotonic() - start < args.hours * 3600:
                order = list(train)
                shuffle_rng.shuffle(order)
                epoch += 1
                for offset in range(0, len(order), args.batch_size):
                    if time.monotonic() - start >= args.hours * 3600:
                        break
                    logs = train_batch(
                        order[offset : offset + args.batch_size],
                        model,
                        optimizer,
                        args.max_decisions,
                        args.entropy_coef,
                        numeric=args.numeric_observations,
                        error_path=output / "simulator_errors" / f"update-{update + 1:06d}.json",
                        root_indices=[train_indices[id(root)] for root in order[offset : offset + args.batch_size]],
                    )
                    error_batches += logs["simulator_error_batches"]
                    discarded_episodes += logs["discarded_episodes"]
                    logs.update(
                        simulator_error_batches_total=error_batches, discarded_episodes_total=discarded_episodes
                    )
                    update += 1
                    run.log({"epoch": epoch, **{f"train/{k}": v for k, v in logs.items()}}, step=update)
                    print(f"update={update} epoch={epoch} metrics={logs}", flush=True)
                    if time.monotonic() >= next_eval:
                        scores = evaluate(val, model, args.eval_repeats, args.max_decisions)
                        # Distinct W&B step avoids dropping metrics after the training log.
                        update += 1
                        run.log({**references, **{f"val/{k}": v for k, v in scores.items()}}, step=update)
                        torch.save(
                            {
                                "model": model.state_dict(),
                                "optimizer": optimizer.state_dict(),
                                "config": vars(args),
                                "epoch": epoch,
                                "step": update,
                            },
                            output / "latest.pt",
                        )
                        print(f"VALIDATION {scores}", flush=True)
                        next_eval = time.monotonic() + args.eval_minutes * 60
            final = evaluate(val, model, args.eval_repeats, args.max_decisions)
            run.log({**references, **{f"final_val/{k}": v for k, v in final.items()}}, step=update + 1)
            print(f"FINAL {final}", flush=True)
        finally:
            torch.save(
                {
                    "model": model.state_dict(),
                    "optimizer": optimizer.state_dict(),
                    "config": vars(args),
                    "epoch": epoch,
                    "step": update,
                },
                output / "latest.pt",
            )
            print(f"Saved {output / 'latest.pt'}", flush=True)


if __name__ == "__main__":
    main()
