import numpy as np
from jaxtyping import Float
from sts_sim import PowerKey
from torch import Tensor, nn

from .numeric import FeatureArrays, NumericBatch

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

    @staticmethod
    def prepare(batch: NumericBatch) -> FeatureArrays:
        """Scale public stats and powers without running learned modules."""
        stats = batch.table("player", 6) / np.array(
            [HP_SCALE, HP_SCALE, BLOCK_SCALE, ENERGY_SCALE, ENERGY_SCALE, GOLD_SCALE]
        )
        powers = batch.powers("player_powers", batch.size, POWER_TO_INDEX)
        # Preserve the typed reference's float32 rounding for double models too.
        values = np.concatenate((stats, powers), axis=1).astype(np.float32)
        return FeatureArrays({}, {"state": values}, {"player": [1] * batch.size})

    def encode(
        self, inputs: dict[str, Tensor], lengths: dict[str, list[int]]
    ) -> tuple[Float[Tensor, "batch player_features"], Float[Tensor, "batch d_model"], list[int]]:
        features = inputs["state"]
        return features, self.projection(features), lengths["player"]
