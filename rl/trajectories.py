"""Batched REINFORCE bookkeeping; original per-episode losses remain the reference."""

import math
from collections.abc import Iterable
from dataclasses import dataclass
from typing import TYPE_CHECKING

import numpy as np
import torch
from jaxtyping import Float
from torch import Tensor

if TYPE_CHECKING:
    from train import Episode


@dataclass
class DecisionRound:
    owners: tuple[int, ...]
    counts: tuple[int, ...]
    log_probs: Float[Tensor, " active"]
    entropies: Float[Tensor, " active"]
    max_probabilities: Float[Tensor, " active"]


class Trajectories:
    """Keep one graph-bearing tensor per decision round, not per episode/action."""

    def __init__(self) -> None:
        self.rounds: list[DecisionRound] = []

    def losses(self, episodes: list["Episode"], entropy_coef: float) -> tuple[Tensor | None, Tensor | None]:
        """Mean completed-episode objective; defeats count, truncations do not."""
        if entropy_coef < 0 or not math.isfinite(entropy_coef):
            raise ValueError("Entropy coefficient must be finite and nonnegative")
        completed = [episode.reward is not None for episode in episodes]
        count = sum(completed)
        if not count:
            return None, None
        if any(done and episode.decisions == 0 for done, episode in zip(completed, episodes)):
            raise ValueError("Cannot train on an episode without sampled actions")
        owners = np.concatenate([np.asarray(row.owners, dtype=np.int64) for row in self.rounds])
        keep = np.flatnonzero(np.asarray(completed)[owners])
        log_probs = torch.cat([row.log_probs for row in self.rounds])
        indices = torch.tensor(keep, dtype=torch.long, device=log_probs.device)
        rewards = np.asarray([episode.reward if episode.reward is not None else 0.0 for episode in episodes])
        weights = log_probs.new_tensor(rewards[owners[keep]])
        policy_loss = -(log_probs.index_select(0, indices) * weights).sum() / count
        loss = policy_loss
        if entropy_coef:
            entropies = torch.cat([row.entropies for row in self.rounds])
            loss = loss - entropy_coef * entropies.index_select(0, indices).sum() / count
        return loss, policy_loss

    def metrics(self) -> dict[str, float]:
        """Copy detached statistics once, including truncated but excluding forced choices."""
        if not self.rounds:
            return {}
        counts = np.concatenate([np.asarray(row.counts) for row in self.rounds])
        chosen = counts > 1
        if not chosen.any():
            return {}
        values = (
            torch.stack(
                (
                    torch.cat([row.entropies.detach() for row in self.rounds]),
                    torch.cat([row.max_probabilities.detach() for row in self.rounds]),
                )
            )
            .cpu()
            .double()
            .numpy()
        )
        entropy, probability = values[:, chosen]
        return {
            "policy_entropy": float(entropy.mean()),
            "normalized_policy_entropy": float((entropy / np.log(counts[chosen])).mean()),
            "mean_max_action_probability": float(probability.mean()),
            "mean_action_count": float(counts[chosen].mean()),
        }


def validate_gradients(parameters: Iterable[torch.nn.Parameter]) -> None:
    """Preserve elementwise finite checks, with one host synchronization."""
    checks = [torch.isfinite(parameter.grad).all() for parameter in parameters if parameter.grad is not None]
    if checks and not torch.stack(checks).all():
        raise RuntimeError("Non-finite gradient")
