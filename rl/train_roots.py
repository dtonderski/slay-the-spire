"""Frozen multi-combat REINFORCE experiment, split by run seed."""

import argparse
import json
import random
import time
from dataclasses import dataclass, replace
from pathlib import Path

import torch
import wandb
from model import CombatModel
from sts_sim import State
from train import Episode, metrics, play_combat, reinforce_loss


@dataclass
class Root:
    state: State
    seed: str
    floor: int
    start_hp: int


def collect_roots(seeds: list[str], floors: int, rng_seed: int) -> tuple[list[Root], list[dict]]:
    """Uniform random legal collector; retain deaths/cutoffs, never replace seeds."""
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
            index = rng.randrange(len(decision.actions))
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
    with torch.random.fork_rng(devices=[]):
        for index, root in enumerate(roots):
            for repeat in range(repeats):
                seed = 90000 + index * repeats + repeat
                torch.manual_seed(seed)
                ep = play_combat(root.state, model, max_decisions=max_decisions, rng=random.Random(seed))
                episodes.append(ep)
                if ep.reward is not None:
                    hp_changes.append(ep.hp - root.start_hp)
    result = metrics(episodes)
    if hp_changes:
        result["mean_hp_change_completed"] = sum(hp_changes) / len(hp_changes)
    return result


def train_batch(
    roots: list[Root], model: CombatModel, optimizer: torch.optim.Optimizer, max_decisions: int
) -> dict[str, float]:
    """Accumulate one episode at a time to avoid retaining a batch of long graphs."""
    model.train()
    optimizer.zero_grad(set_to_none=True)
    episodes: list[Episode] = []
    total_loss = 0.0
    completed = 0
    for root in roots:
        ep = play_combat(root.state, model, max_decisions=max_decisions, training=True, rng=random.Random(0))
        if ep.reward is not None:
            loss = reinforce_loss(ep.log_probs, ep.reward)
            if not torch.isfinite(loss):
                raise RuntimeError("Non-finite loss")
            total_loss += loss.detach().item()
            loss.backward()
            completed += 1
            del loss
        episodes.append(replace(ep, log_probs=()))
        del ep
    if completed:
        for parameter in model.parameters():
            if parameter.grad is not None:
                parameter.grad.div_(completed)
                if not torch.isfinite(parameter.grad).all():
                    raise RuntimeError("Non-finite gradient")
        optimizer.step()
    result = metrics(episodes)
    result["optimizer_step"] = float(completed > 0)
    if completed:
        result["loss"] = total_loss / completed
    return result


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
        )
        <= 0
    ):
        parser.error("Counts and durations must be positive")
    torch.set_num_threads(1)
    torch.manual_seed(123)
    shuffle_rng = random.Random(123)
    seeds = [str(1000000 + index) for index in range(args.train_seeds + args.val_seeds)]
    shuffle_rng.shuffle(seeds)
    train_seeds, val_seeds = seeds[: args.train_seeds], seeds[args.train_seeds :]
    output = Path("wandb") / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()
    model = CombatModel()
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)
    epoch, update = 0, 0
    with wandb.init(
        id=args.run_id,
        name="overnight-40train-10val-hp",
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
            "collector": "uniform_random_legal",
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
            run.log(
                {
                    **{
                        f"random_val/{k}": v
                        for k, v in evaluate(val, None, args.eval_repeats, args.max_decisions).items()
                    },
                    **{
                        f"initial_val/{k}": v
                        for k, v in evaluate(val, model, args.eval_repeats, args.max_decisions).items()
                    },
                },
                step=0,
            )
            next_eval = time.monotonic() + args.eval_minutes * 60
            while time.monotonic() - start < args.hours * 3600:
                order = list(train)
                shuffle_rng.shuffle(order)
                epoch += 1
                for offset in range(0, len(order), args.batch_size):
                    if time.monotonic() - start >= args.hours * 3600:
                        break
                    logs = train_batch(order[offset : offset + args.batch_size], model, optimizer, args.max_decisions)
                    update += 1
                    run.log({"epoch": epoch, **{f"train/{k}": v for k, v in logs.items()}}, step=update)
                    print(f"update={update} epoch={epoch} metrics={logs}", flush=True)
                    if time.monotonic() >= next_eval:
                        scores = evaluate(val, model, args.eval_repeats, args.max_decisions)
                        # Distinct W&B step avoids dropping metrics after the training log.
                        update += 1
                        run.log({f"val/{k}": v for k, v in scores.items()}, step=update)
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
            run.log({f"final_val/{k}": v for k, v in final.items()}, step=update + 1)
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
