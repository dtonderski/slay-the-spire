from encoders.actions import ActionEncoder
from jaxtyping import Float
from observation_encoder import ObservationEncoder
from sts_sim import Action, CombatObservation
from torch import Tensor, nn


class CombatModel(nn.Module):
    """Single-decision policy; actions reuse the observation slices' raw features."""

    def __init__(self, d_model: int = 64, action_dim: int = 64, n_heads: int = 4, n_layers: int = 2) -> None:
        super().__init__()
        self.observation_encoder = ObservationEncoder(d_model, action_dim, n_heads, n_layers)
        self.action_encoder = ActionEncoder(action_dim)

    def forward(self, observation: CombatObservation, actions: tuple[Action, ...]) -> Float[Tensor, " n_actions"]:
        """Return one logit per supplied legal action from the same decision."""
        query, features = self.observation_encoder(observation)
        action_vectors = self.action_encoder(actions, features)
        return action_vectors @ query
