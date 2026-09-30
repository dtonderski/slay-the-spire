import numpy as np
import torch
from jaxtyping import Float
from sts_sim import CounterKey, RelicKey
from torch import Tensor, nn

from .numeric import FeatureArrays, NumericBatch, upload_features

RELIC_EMBEDDING_DIM = 16
RELIC_COUNTER_SLOTS = 3
RELIC_FEATURE_DIM = RELIC_EMBEDDING_DIM + RELIC_COUNTER_SLOTS

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
RELIC_TO_INDEX = {key: index for index, key in enumerate(RelicKey)}


class RelicEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(RELIC_TO_INDEX), RELIC_EMBEDDING_DIM)
        self.projection = nn.Linear(RELIC_FEATURE_DIM, d_model)

    @staticmethod
    def prepare(batch: NumericBatch) -> FeatureArrays:
        """Assemble public relic ids and alphabetically ordered counter slots."""
        rows = batch.table("relics", 2)
        raw_ids = rows[:, 1]
        if len(raw_ids) and (int(raw_ids.min()) < 0 or int(raw_ids.max()) >= len(RELIC_TO_INDEX)):
            raise ValueError("Relic id is outside content vocabulary v1")
        counters = batch.table("relic_counters", 3)
        counts = (
            np.bincount(counters[:, 0], minlength=len(rows)) if len(counters) else np.zeros(len(rows), dtype=np.int64)
        )
        if len(counts) and np.any(counts > RELIC_COUNTER_SLOTS):
            raise ValueError("Too many public relic counters")
        labels = np.array([key.value for key in CounterKey])
        if len(counters) and (int(counters[:, 1].min()) < 0 or int(counters[:, 1].max()) >= len(labels)):
            raise ValueError("Counter id is outside content vocabulary v1")
        order = np.lexsort((labels[counters[:, 1]], counters[:, 0])) if len(counters) else np.array([], dtype=np.int64)
        counters = counters[order]
        slots = np.arange(len(counters)) - np.repeat(np.cumsum(counts) - counts, counts)
        values = np.zeros((len(rows), RELIC_COUNTER_SLOTS))
        values[counters[:, 0], slots] = counters[:, 2]
        return FeatureArrays({"ids": raw_ids}, {"state": values}, {"relics": batch.lengths(rows)})

    def encode(
        self, inputs: dict[str, Tensor], lengths: dict[str, list[int]]
    ) -> tuple[Float[Tensor, "n_relics relic_features"], Float[Tensor, "n_relics d_model"], list[int]]:
        identities = self.embedding(inputs["ids"])
        features = torch.cat((identities, inputs["state"]), dim=1)
        return features, self.projection(features), lengths["relics"]

    def numeric(
        self, batch: NumericBatch
    ) -> tuple[Float[Tensor, "n_relics relic_features"], Float[Tensor, "n_relics d_model"], list[int]]:
        raw = self.prepare(batch)
        return self.encode(upload_features({"relics": raw}, self.embedding.weight)["relics"], raw.lengths)
