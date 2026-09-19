import torch
from encoders.actions import ActionEncoder
from encoders.numeric import NumericBatch
from jaxtyping import Bool, Float
from observation_encoder import ObservationEncoder
from sts_sim import Action
from torch import Tensor, nn


class CombatValueModel(nn.Module):
    """Shared numeric observation encoder with policy and scalar value heads."""

    def __init__(self, d_model: int = 64, action_dim: int = 64, n_heads: int = 4, n_layers: int = 2) -> None:
        super().__init__()
        self.observation_encoder = ObservationEncoder(d_model, action_dim, n_heads, n_layers)
        self.policy_head = nn.Linear(action_dim, action_dim)
        self.value_head = nn.Linear(action_dim, 1)
        self.action_encoder = ActionEncoder(action_dim)

    def forward(
        self, observations: NumericBatch, actions: list[tuple[Action, ...]]
    ) -> tuple[Float[Tensor, "batch n_actions"], Float[Tensor, "batch 1"], Bool[Tensor, "batch n_actions"]]:
        """Return padded action logits, one value per observation, and an action mask."""
        if not observations or len(observations) != len(actions):
            raise ValueError("Observation and action batches must be nonempty and have the same length")
        if any(not candidates for candidates in actions):
            raise ValueError("Batched scoring requires at least one legal action per observation")
        state, features = self.observation_encoder(observations)
        policy_query = self.policy_head(state)
        values = self.value_head(state)
        vectors = self.action_encoder(actions, features)
        lengths = torch.tensor([len(candidates) for candidates in actions], device=policy_query.device)
        valid = torch.arange(vectors.shape[1], device=policy_query.device).unsqueeze(0) < lengths.unsqueeze(1)
        logits = torch.bmm(vectors, policy_query.unsqueeze(-1)).squeeze(-1)
        return logits.masked_fill(~valid, float("-inf")), values, valid
