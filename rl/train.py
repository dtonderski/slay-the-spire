"""REINFORCE v1: deliberately overfit the first combat of one fixed seed."""

import argparse
import random
from dataclasses import dataclass

import torch
import wandb
from jaxtyping import Float
from model import CombatModel
from sts_sim import Observation, State
from torch import Tensor
from torch.distributions import Categorical


@dataclass
class Episode:
    reward: float | None  # Terminal HP / starting max HP; None means truncation.
    won: bool | None
    hp: int
    decisions: int
    log_probs: tuple[Float[Tensor, ""], ...]


def first_combat(seed: str, ascension: int) -> State:
    """Prepare a fixed reset state; seed and State never enter the policy."""
    state = State.new(seed, ascension=ascension)
    for _ in range(100):
        decision = state.decision()
        if decision.observation.kind == "combat":
            if decision.observation.screen.phase != "waiting_for_player":
                raise RuntimeError("Initial combat is not awaiting a player decision")
            return state
        if not decision.actions:
            raise RuntimeError("No actions available before reaching combat")
        state.step(decision.actions[0])
    raise RuntimeError("Could not reach the first combat within 100 setup decisions")


def combat_outcome(observation: Observation) -> bool | None:
    """Recognize combat outcomes before selecting any postcombat action."""
    if observation.kind == "combat":
        if observation.screen.phase == "won":
            return True
        if observation.screen.phase == "lost":
            return False
        return None
    if observation.phase == "reward":
        return True
    if observation.kind == "complete" and observation.context.player_hp <= 0:
        return False
    raise RuntimeError(f"Unexpected screen after combat: {observation.kind}/{observation.phase}")


def play_combat(
    root: State,
    model: CombatModel | None,
    *,
    max_decisions: int,
    training: bool = False,
    rng: random.Random,
) -> Episode:
    """Reset by cloning the initial combat only; None model is the random baseline."""
    state = root.clone()
    decision = state.decision()
    starting_max_hp = decision.observation.context.player_max_hp
    log_probs: list[Float[Tensor, ""]] = []
    for step in range(max_decisions + 1):
        observation = decision.observation
        won = combat_outcome(observation)
        if won is not None:
            hp = observation.context.player_hp if won else 0
            return Episode(hp / starting_max_hp, won, hp, step, tuple(log_probs))
        if step == max_decisions:
            # Drop truncated trajectories rather than inventing a terminal reward.
            return Episode(None, None, observation.context.player_hp, step, ())
        if observation.kind != "combat" or not decision.actions:
            raise RuntimeError("Unsettled combat decision or empty legal-action list")
        if model is None:
            index = rng.randrange(len(decision.actions))
        else:
            with torch.set_grad_enabled(training):
                logits, _ = model([observation], [decision.actions])
                distribution = Categorical(logits=logits[0])
                sampled = distribution.sample()
                if training:
                    log_probs.append(distribution.log_prob(sampled))
                index = int(sampled.item())
        decision = state.step(decision.actions[index])
    raise AssertionError("Unreachable")


def reinforce_loss(log_probs: tuple[Float[Tensor, ""], ...], reward: float) -> Float[Tensor, ""]:
    """Undiscounted terminal return: -reward * sum(log pi(action | state))."""
    if not log_probs:
        raise ValueError("Cannot train on an episode without sampled actions")
    return -reward * torch.stack(log_probs).sum()


def metrics(episodes: list[Episode]) -> dict[str, float]:
    completed = [episode for episode in episodes if episode.reward is not None]
    result = {
        "episodes": float(len(episodes)),
        "completed": float(len(completed)),
        "truncated": float(len(episodes) - len(completed)),
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


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", default="HUMAN1")
    parser.add_argument("--ascension", type=int, choices=range(21), default=0)
    parser.add_argument("--policy-seed", type=int, default=123)
    parser.add_argument("--updates", type=int, default=100)
    parser.add_argument("--episodes-per-update", type=int, default=4)
    parser.add_argument("--eval-episodes", type=int, default=8)
    parser.add_argument("--max-decisions", type=int, default=256)
    parser.add_argument("--lr", type=float, default=1e-4)
    parser.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    parser.add_argument("--wandb-project", default="sts-combat-v1")
    parser.add_argument("--wandb-base-url", default="http://localhost:8080")
    parser.add_argument("--wandb-mode", choices=("online", "offline", "disabled"), default="online")
    args = parser.parse_args()
    if min(args.updates, args.episodes_per_update, args.eval_episodes, args.max_decisions) < 1 or args.lr <= 0:
        parser.error("Counts and learning rate must be positive")

    if args.device == "cuda" and not torch.cuda.is_available():
        parser.error("CUDA is not available")

    torch.set_num_threads(1)
    torch.manual_seed(args.policy_seed)
    rng = random.Random(args.policy_seed)
    root = first_combat(args.seed, args.ascension)
    model = CombatModel().to(args.device)  # Fixed-root overfitting, not generalization.
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr)
    with wandb.init(
        project=args.wandb_project,
        mode=args.wandb_mode,
        settings=wandb.Settings(base_url=args.wandb_base_url),
        config={**vars(args), "reward": "terminal_hp_over_starting_max_hp", "baseline": "none"},
    ) as run:
        baseline = [
            play_combat(root, None, max_decisions=args.max_decisions, rng=rng) for _ in range(args.eval_episodes)
        ]
        model.eval()
        initial = [
            play_combat(root, model, max_decisions=args.max_decisions, rng=rng) for _ in range(args.eval_episodes)
        ]
        run.log(
            {
                **{f"random/{key}": value for key, value in metrics(baseline).items()},
                **{f"initial/{key}": value for key, value in metrics(initial).items()},
            },
            step=0,
        )
        for update in range(1, args.updates + 1):
            model.train()
            episodes = [
                play_combat(root, model, max_decisions=args.max_decisions, training=True, rng=rng)
                for _ in range(args.episodes_per_update)
            ]
            losses = [reinforce_loss(ep.log_probs, ep.reward) for ep in episodes if ep.reward is not None]
            logs = {f"train/{key}": value for key, value in metrics(episodes).items()}
            if not losses:
                run.log(logs, step=update)
                raise RuntimeError("Every episode was truncated; increase --max-decisions before training")
            loss = torch.stack(losses).mean()
            if not torch.isfinite(loss):
                raise RuntimeError("Non-finite policy loss")
            optimizer.zero_grad()
            loss.backward()
            optimizer.step()
            logs["train/loss"] = loss.detach().item()
            run.log(logs, step=update)
            print(
                f"update={update} loss={logs['train/loss']:.3f} "
                f"win_rate={logs['train/win_rate_completed']:.2f} truncated={int(logs['train/truncated'])}"
            )
            # Release trajectory graphs before collecting the next update.
            del episodes, losses, loss
        model.eval()
        final = [play_combat(root, model, max_decisions=args.max_decisions, rng=rng) for _ in range(args.eval_episodes)]
        run.log({f"final/{key}": value for key, value in metrics(final).items()}, step=args.updates + 1)
        print("Final stochastic evaluation:", metrics(final))


if __name__ == "__main__":
    main()
