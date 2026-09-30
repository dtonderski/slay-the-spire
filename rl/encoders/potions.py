from jaxtyping import Float
from sts_sim import PotionKey
from torch import Tensor, nn

from .numeric import FeatureArrays, NumericBatch, upload_features

POTION_EMBEDDING_DIM = 16

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
POTION_TO_INDEX: dict[PotionKey | None, int] = {None: 0}
POTION_TO_INDEX.update({key: index + 1 for index, key in enumerate(PotionKey)})


class PotionEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(POTION_TO_INDEX), POTION_EMBEDDING_DIM)
        self.projection = nn.Linear(POTION_EMBEDDING_DIM, d_model)

    @staticmethod
    def prepare(batch: NumericBatch) -> FeatureArrays:
        """Validate public potion ids, including real empty slots."""
        rows = batch.table("potions", 3)
        raw_ids = rows[:, 1]
        if len(raw_ids) and (int(raw_ids.min()) < 0 or int(raw_ids.max()) >= len(POTION_TO_INDEX)):
            raise ValueError("Potion id is outside content vocabulary v1")
        return FeatureArrays({"ids": raw_ids}, {}, {"potions": batch.lengths(rows)})

    def encode(
        self, inputs: dict[str, Tensor], lengths: dict[str, list[int]]
    ) -> tuple[Float[Tensor, "n_potions potion_features"], Float[Tensor, "n_potions d_model"], list[int]]:
        features = self.embedding(inputs["ids"])
        return features, self.projection(features), lengths["potions"]

    def numeric(
        self, batch: NumericBatch
    ) -> tuple[Float[Tensor, "n_potions potion_features"], Float[Tensor, "n_potions d_model"], list[int]]:
        raw = self.prepare(batch)
        return self.encode(upload_features({"potions": raw}, self.embedding.weight)["potions"], raw.lengths)
