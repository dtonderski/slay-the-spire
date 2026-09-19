"""Batched policy/value learning from completed-fight returns."""

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
    values: Float[Tensor, " active"]


class Trajectories:
    """Keep one graph-bearing tensor per decision round, not per episode/action."""

    def __init__(self) -> None:
        self.rounds: list[DecisionRound] = []
        self.value_loss: Tensor | None = None  # Detached scalar for logging only.

    def losses(
        self, episodes: list["Episode"], entropy_coef: float, value_coef: float = 0.1
    ) -> tuple[Tensor | None, Tensor | None]:
        """Mean completed-episode objective; defeats count, truncations do not."""
        if entropy_coef < 0 or not math.isfinite(entropy_coef):
            raise ValueError("Entropy coefficient must be finite and nonnegative")
        # Shapes below:
        # B = number of fights (len(episodes)).
        # A_t = active fights making a decision in round t; varies by round.
        # D = sum(A_t): total recorded decisions, including unfinished fights.
        # K = decisions belonging to completed fights (K <= D).
        # Each row has A_t owners, log_probs, values, and entropies.

        # [B] booleans. Defeat (reward=0) counts; truncation (reward=None) doesn't.
        if value_coef < 0 or not math.isfinite(value_coef):
            raise ValueError("Value coefficient must be finite and nonnegative")
        completed = [episode.reward is not None for episode in episodes]
        count = sum(completed)  # Scalar: completed FIGHTS, not decisions.
        if not count:
            return None, None
        if any(done and episode.decisions == 0 for done, episode in zip(completed, episodes)):
            raise ValueError("Cannot train on an episode without sampled actions")
        # [D] integer fight IDs, in round order. Concatenation uses axis 0 throughout.
        owners = np.concatenate([np.asarray(row.owners, dtype=np.int64) for row in self.rounds])
        # completed[owners]: [D] booleans, one per DECISION, not per fight.
        # keep: [K] positions in the flattened decision arrays.
        # Example: owners=[0,1,2,0,2], completed=[True,True,False] -> keep=[0,1,3].
        keep = np.flatnonzero(np.asarray(completed)[owners])
        # [D] floats, with the gradient graph attached; same order as owners.
        log_probs = torch.cat([row.log_probs for row in self.rounds])
        # [K] integer positions: same as keep, but on the model's CPU/GPU device.
        indices = torch.tensor(keep, dtype=torch.long, device=log_probs.device)
        # [B] final rewards. Zeros for unfinished fights are placeholders, never used.
        rewards = np.asarray([episode.reward if episode.reward is not None else 0.0 for episode in episodes])

        # owners[keep]: [K] fight IDs for retained decisions (example: [0,1,0]).
        # rewards[owners[keep]]: [K] final rewards (example: [0.8,0.0,0.8]).
        # new_tensor copies these targets to log_probs' device/dtype, without gradients.
        returns = log_probs.new_tensor(rewards[owners[keep]])  # [K]
        values = torch.cat([row.values for row in self.rounds])  # [D], graph attached
        values = values.index_select(0, indices)  # [K], same selected decisions as returns

        # [K]: one advantage per decision. Both operands must be 1-D, or broadcasting
        # could incorrectly produce [K,K] instead of pairing each prediction with its target.
        advantages = returns - values
        # Selected log_probs: [K]. Elementwise product: [K]. sum()/count: scalar [].
        # detach prevents the policy objective from also changing value predictions.
        policy_loss = -(log_probs.index_select(0, indices) * advantages.detach()).sum() / count
        # Squared errors: [K] -> scalar []. NO detach: this trains the value head.
        # Both objectives sum decisions within each fight, then average over completed fights.
        value_loss = advantages.square().sum() / count
        self.value_loss = value_loss.detach()
        loss = policy_loss + value_coef * value_loss  # Scalar [] used for backward().

        if entropy_coef:
            entropies = torch.cat([row.entropies for row in self.rounds])  # [D]
            # Select [K], then reduce to scalar []; higher entropy lowers the loss.
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
