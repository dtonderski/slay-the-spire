"""REINFORCE v1: deliberately overfit the first combat of one fixed seed."""

import argparse
import random
from dataclasses import dataclass

import torch
import wandb
from combat_task import action_indices, combat_outcome, terminal_reward
from jaxtyping import Float
from model import CombatModel
from sts_sim import CombatObservation, State
from torch import Tensor
from torch.distributions import Categorical


@dataclass
class Episode:
    reward: float | None  # Terminal HP / starting max HP; None means truncation.
    won: bool | None
    hp: int
    decisions: int
    log_probs: tuple[Float[Tensor, ""], ...]
    escaped: bool = False


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


def play_combat(
    root: State,
    model: CombatModel | None,
    *,
    max_decisions: int,
    training: bool = False,
    rng: random.Random,
) -> Episode:
    """Evaluate or train one combat through the batched rollout path."""
    return play_combats([root], model, max_decisions=max_decisions, training=training, rng=rng)[0]


def play_combats(
    roots: list[State],
    model: CombatModel | None,
    *,
    max_decisions: int,
    training: bool = False,
    rng: random.Random,
    numeric: bool = False,
) -> list[Episode]:
    """Clone initial roots, batch active decisions, and return episodes in root order."""
    if not roots or max_decisions < 0:
        raise ValueError("Rollouts need nonempty roots and a nonnegative decision limit")
    if numeric:
        return _play_numeric_combats(roots, model, max_decisions=max_decisions, training=training, rng=rng)
    states = [root.clone() for root in roots]
    decisions = [state.decision() for state in states]
    starting_max_hp = [decision.observation.context.player_max_hp for decision in decisions]
    log_probs: list[list[Float[Tensor, ""]]] = [[] for _ in roots]
    episodes: list[Episode | None] = [None for _ in roots]
    for step in range(max_decisions + 1):
        active: list[int] = []
        observations: list[CombatObservation] = []
        for index, decision in enumerate(decisions):
            if episodes[index] is not None:
                continue
            observation = decision.observation
            won = combat_outcome(observation)
            if won is not None:
                hp = observation.context.player_hp if won else 0
                episodes[index] = Episode(
                    terminal_reward(observation, starting_max_hp[index]), won, hp, step, tuple(log_probs[index])
                )
            elif step == max_decisions:
                # Discard truncations, never treat them as terminal defeats.
                episodes[index] = Episode(None, None, observation.context.player_hp, step, ())
            else:
                if observation.kind != "combat" or not decision.actions:
                    raise RuntimeError("Unsettled combat decision or empty legal-action list")
                active.append(index)
                observations.append(observation)
                continue
            log_probs[index].clear()
        if not active:
            assert all(episode is not None for episode in episodes)
            return [episode for episode in episodes if episode is not None]
        indices = [action_indices(decisions[index]) for index in active]
        actions = [[decisions[index].actions[i] for i in allowed] for index, allowed in zip(active, indices)]
        if any(not candidates for candidates in actions):
            raise RuntimeError("No allowed combat actions after disabling escape")
        if model is None:
            choices = [rng.randrange(len(candidates)) for candidates in actions]
        else:
            with torch.set_grad_enabled(training):
                logits, _ = model(observations, actions)
                distribution = Categorical(logits=logits)
                sampled = distribution.sample()
                if training:
                    sampled_log_probs = distribution.log_prob(sampled)
                    for row, index in enumerate(active):
                        log_probs[index].append(sampled_log_probs[row])
                choices = sampled.tolist()
        for index, candidates, choice in zip(active, actions, choices):
            decisions[index] = states[index].step(candidates[choice])
    raise AssertionError("Unreachable")


def _play_numeric_combats(
    roots: list[State], model: CombatModel | None, *, max_decisions: int, training: bool, rng: random.Random
) -> list[Episode]:
    """Same rollout rule with batched native steps and direct public numeric inputs."""
    from encoders.numeric import NumericBatch

    states = [root.clone() for root in roots]
    batch = NumericBatch(State.numeric_decisions(states))
    starting_max_hp = batch.table("header", 5)[:, 4].tolist()
    remaining = list(range(len(roots)))
    episodes: list[Episode | None] = [None] * len(roots)
    log_probs: list[list[Tensor]] = [[] for _ in roots]
    for step in range(max_decisions + 1):
        active = []
        for row, index in enumerate(remaining):
            kind, phase, combat_phase, hp, _ = batch.table("header", 5)[row]
            kind, phase = batch.symbols[kind], batch.symbols[phase]
            combat_phase = batch.symbols[combat_phase] if combat_phase >= 0 else None
            won = (
                True
                if phase == "reward" or combat_phase == "won"
                else (False if combat_phase == "lost" or (kind == "complete" and hp <= 0) else None)
            )
            if won is not None:
                hp = int(hp) if won else 0
                episodes[index] = Episode(hp / starting_max_hp[index], won, hp, step, tuple(log_probs[index]))
            elif kind != "combat":
                raise RuntimeError(f"Unexpected screen after combat: {kind}/{phase}")
            elif step == max_decisions:
                episodes[index] = Episode(None, None, int(hp), step, ())
            else:
                if not batch.actions[row]:
                    raise RuntimeError("Unsettled combat decision or empty legal-action list")
                active.append(row)
                continue
            log_probs[index].clear()
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
        for row in active:
            candidates = []
            for action in batch.actions[row]:
                if action.kind == "use_potion_slot" and action.potion_slot is not None:
                    code = potions[offsets[row] + action.potion_slot, 1]
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
                logits, _ = model(batch, actions)
                distribution = Categorical(logits=logits)
                sampled = distribution.sample()
                if training:
                    values = distribution.log_prob(sampled)
                    for row, value in zip(active, values):
                        log_probs[remaining[row]].append(value)
                choices = sampled.tolist()
        remaining = [remaining[row] for row in active]
        chosen = [candidates[choice] for candidates, choice in zip(actions, choices)]
        batch = NumericBatch(State.numeric_steps([states[index] for index in remaining], chosen))
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
        "escaped": float(sum(episode.escaped for episode in completed)),
        "defeated": float(sum(episode.won is False and not episode.escaped for episode in completed)),
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
    parser.add_argument("--numeric-observations", action=argparse.BooleanOptionalAction, default=True)
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
        config={
            **vars(args),
            "reward": "terminal_hp_over_starting_max_hp",
            "baseline": "none",
            "smoke_bomb_use": False,
        },
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
            episodes = play_combats(
                [root] * args.episodes_per_update,
                model,
                max_decisions=args.max_decisions,
                training=True,
                rng=rng,
                numeric=args.numeric_observations,
            )
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
