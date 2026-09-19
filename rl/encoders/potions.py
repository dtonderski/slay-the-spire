from jaxtyping import Float
from sts_sim import PotionKey
from torch import Tensor, nn

from .numeric import NumericBatch, tensor

POTION_EMBEDDING_DIM = 16

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
POTION_TO_INDEX: dict[PotionKey | None, int] = {None: 0}
POTION_TO_INDEX.update({key: index + 1 for index, key in enumerate(PotionKey)})


class PotionEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(POTION_TO_INDEX), POTION_EMBEDDING_DIM)
        self.projection = nn.Linear(POTION_EMBEDDING_DIM, d_model)

    def numeric(
        self, batch: NumericBatch
    ) -> tuple[Float[Tensor, "n_potions potion_features"], Float[Tensor, "n_potions d_model"], list[int]]:
        """Embed raw potion keys, including real empty slots."""
        rows = batch.table("potions", 3)
        features = self.embedding(tensor(self.embedding.weight, batch.codes(rows[:, 1], POTION_TO_INDEX), integer=True))
        lengths = batch.lengths(rows)
        return features, self.projection(features), lengths
