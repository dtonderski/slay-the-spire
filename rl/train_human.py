"""Minimal training example. Simulator errors stop the run; use train.py for full experiments."""

import random
from pathlib import Path

import torch
from loadout_sampling import LoadoutSampler
from model import CombatValueModel
from synthetic_roots import sample_root
from train import play_combats
from trajectories import Trajectories, validate_gradients

DISTRIBUTIONS = Path(__file__).resolve().parent.parent / "data/slaythedata/loadout-a0-v3/fit.json"


def main(
    updates: int = 100,
    batch_size: int = 32,
    max_decisions: int = 512,
    device: str = "cpu",
    distributions: Path = DISTRIBUTIONS,
) -> CombatValueModel:
    torch.set_num_threads(1)
    torch.manual_seed(123)
    root_rng = random.Random(123)
    sampler = LoadoutSampler.load(distributions)
    model = CombatValueModel().to(device)
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-4)

    for step in range(updates):
        # Fresh fights every update. Only public observations enter the model.
        roots = [sample_root(root_rng, sampler).state for _ in range(batch_size)]
        model.train()
        optimizer.zero_grad(set_to_none=True)
        trajectories = Trajectories()
        episodes = play_combats(
            roots,
            model,
            max_decisions=max_decisions,
            training=True,
            rng=random.Random(0),
            trajectories=trajectories,
        )

        # Loss = mean over completed fights of:
        #   -sum(log_probability * detached_advantage)
        #   + 0.1 * sum((value - final_reward)**2) - 0.01 * sum(entropy)
        # The advantage is final_reward - predicted_value; no separate EMA baseline.
        # Truncated fights are excluded; losing fights have final_reward = 0.
        loss, _ = trajectories.losses(episodes, entropy_coef=0.01)
        if loss is None:
            print(f"update={step + 1}: all fights truncated; no optimizer step")
            continue
        if not torch.isfinite(loss):
            raise RuntimeError("Non-finite loss")
        loss.backward()
        validate_gradients(model.parameters())
        optimizer.step()

        # Report actual completed-fight returns, not predicted values.
        rewards = [episode.reward for episode in episodes if episode.reward is not None]
        mean_reward = sum(rewards) / len(rewards)
        print(f"update={step + 1} completed={len(rewards)} reward={mean_reward:.3f} loss={loss.item():.3f}")

    return model


if __name__ == "__main__":
    main()
