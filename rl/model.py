import torch
from encoders.actions import ActionEncoder
from jaxtyping import Bool, Float
from observation_encoder import ObservationEncoder
from sts_sim import Action, CombatObservation
from torch import Tensor, nn
from torch.nn.utils.rnn import pad_sequence


class CombatModel(nn.Module):
    """Batched policy; actions reuse each observation's raw features."""

    def __init__(self, d_model: int = 64, action_dim: int = 64, n_heads: int = 4, n_layers: int = 2) -> None:
        super().__init__()
        self.observation_encoder = ObservationEncoder(d_model, action_dim, n_heads, n_layers)
        self.action_encoder = ActionEncoder(action_dim)

    def forward(
        self, observations: list[CombatObservation], actions: list[tuple[Action, ...]]
    ) -> tuple[Float[Tensor, "batch n_actions"], Bool[Tensor, "batch n_actions"]]:
        """Return padded logits and a valid-action mask; every decision needs a candidate."""
        if not observations or len(observations) != len(actions):
            raise ValueError("Observation and action batches must be nonempty and have the same length")
        if any(not candidates for candidates in actions):
            raise ValueError("Batched scoring requires at least one legal action per observation")
        queries, features = self.observation_encoder(observations)
        vectors = pad_sequence(self.action_encoder(actions, features), batch_first=True)
        lengths = torch.tensor([len(candidates) for candidates in actions], device=queries.device)
        valid = torch.arange(vectors.shape[1], device=queries.device).unsqueeze(0) < lengths.unsqueeze(1)
        logits = torch.bmm(vectors, queries.unsqueeze(-1)).squeeze(-1)
        return logits.masked_fill(~valid, float("-inf")), valid
