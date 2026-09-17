"""Frozen multi-combat REINFORCE experiment, split by run seed."""

import argparse
import json
import logging
import random
import time
from dataclasses import dataclass
from pathlib import Path

import torch
import wandb
from beam_search import beam_search
from combat_task import action_indices, terminal_reward
from model import CombatModel
from sts_sim import State
from train import Episode, metrics, play_combat, play_combats, reinforce_loss


@dataclass
class Root:
    state: State
    seed: str
    floor: int
    start_hp: int


def collect_roots(seeds: list[str], floors: int, rng_seed: int) -> tuple[list[Root], list[dict]]:
    """Random collector with escape disabled; retain deaths/cutoffs, never replace seeds."""
    roots: list[Root] = []
    manifest: list[dict] = []
    for seed in seeds:
        rng = random.Random(f"{rng_seed}:{seed}")
        state = State.new(seed, ascension=0)
        decision = state.decision()
        choices: list[int] = []
        entries: list[dict] = []
        in_combat = False
        status = "decision_limit"
        for _ in range(5000):
            obs = decision.observation
            if obs.context.floor > floors:
                status = "floor_limit"
                break
            if obs.kind == "complete" or (obs.kind == "combat" and obs.screen.phase == "lost"):
                status = "death" if obs.context.player_hp <= 0 else "complete"
                break
            if obs.kind == "combat":
                if not in_combat:
                    roots.append(Root(state.clone(), seed, obs.context.floor, obs.context.player_hp))
                    entries.append(
                        {"floor": obs.context.floor, "prefix_length": len(choices), "hp": obs.context.player_hp}
                    )
                in_combat = True
            else:
                in_combat = False
            if not decision.actions:
                raise RuntimeError(f"No legal actions during collection: seed={seed}, screen={obs.kind}")
            allowed = action_indices(decision)
            if not allowed:
                raise RuntimeError(f"No allowed collection actions: seed={seed}, screen={obs.kind}")
            index = rng.choice(allowed)
            decision = state.step(decision.actions[index])
            choices.append(index)
        manifest.append({"seed": seed, "status": status, "roots": entries, "accepted_action_indices": choices})
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
    return result


def train_batch(
    roots: list[Root],
    model: CombatModel,
    optimizer: torch.optim.Optimizer,
    max_decisions: int,
    *,
    numeric: bool = False,
) -> dict[str, float]:
    """Roll out a fixed group, then backpropagate once through its shared graphs."""
    model.train()
    optimizer.zero_grad(set_to_none=True)
    episodes = play_combats(
        [root.state for root in roots],
        model,
        max_decisions=max_decisions,
        training=True,
        rng=random.Random(0),
        numeric=numeric,
    )
    losses = [reinforce_loss(ep.log_probs, ep.reward) for ep in episodes if ep.reward is not None]
    result = metrics(episodes)
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
        record: dict = {"root_index": index, "seed": root.seed, "floor": root.floor}
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
            win_rate_completed=sum(record["won"] is True for record in completed) / len(completed),
        )
    return scores, records


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hours", type=float, default=8)
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
        name=f"overnight-{args.train_seeds}train-{args.val_seeds}val-hp",
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
            "collector": "uniform_random_no_escape",
            "smoke_bomb_use": False,
            "beam_reference": "privileged_weighted_beam",
            "beam_repeats": 1,
            "train_seeds_list": train_seeds,
            "val_seeds_list": val_seeds,
        },
    ) as run:
        try:
            train, train_manifest = collect_roots(train_seeds, args.floors, 123)
            val, val_manifest = collect_roots(val_seeds, args.floors, 123)
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
                    "collection_deaths": sum(item["status"] == "death" for item in train_manifest + val_manifest),
                    "collection_cutoffs": sum(
                        item["status"] == "decision_limit" for item in train_manifest + val_manifest
                    ),
                }
            )
            print(f"ROOTS train={len(train)} val={len(val)}", flush=True)
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
                        numeric=args.numeric_observations,
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
