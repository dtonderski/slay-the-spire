import torch
from jaxtyping import Float
from sts_sim import CombatObservation, PowerKey
from torch import Tensor, nn

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
POWER_TO_INDEX = {key: index for index, key in enumerate(PowerKey)}
PLAYER_STAT_DIM = 6
PLAYER_FEATURE_DIM = PLAYER_STAT_DIM + len(POWER_TO_INDEX)

HP_SCALE = 100.0
BLOCK_SCALE = 100.0
ENERGY_SCALE = 10.0
GOLD_SCALE = 1000.0


class PlayerEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.projection = nn.Linear(PLAYER_FEATURE_DIM, d_model)

    def tensorize(self, observation: CombatObservation) -> Float[Tensor, " player_features"]:
        """Return scaled [hp, max_hp, block, energy, max_energy, gold], then raw powers."""
        player = observation.screen.player
        stats = [
            player.hp / HP_SCALE,
            player.max_hp / HP_SCALE,
            player.block / BLOCK_SCALE,
            player.energy / ENERGY_SCALE,
            player.max_energy / ENERGY_SCALE,
            observation.context.gold / GOLD_SCALE,
        ]
        powers = [0] * len(POWER_TO_INDEX)
        for power in player.powers:
            powers[POWER_TO_INDEX[power.key]] = power.amount
        return torch.tensor(stats + powers, dtype=torch.float32, device=self.projection.weight.device).to(
            self.projection.weight
        )

    def forward(
        self, batch: list[CombatObservation]
    ) -> tuple[list[Float[Tensor, "1 player_features"]], list[Float[Tensor, "1 d_model"]]]:
        """Return one player row and token per observation."""
        features = (
            torch.stack([self.tensorize(obs) for obs in batch])
            if batch
            else self.projection.weight.new_empty((0, PLAYER_FEATURE_DIM))
        )
        lengths = [1] * len(batch)
        return list(features.split(lengths)), list(self.projection(features).split(lengths))
