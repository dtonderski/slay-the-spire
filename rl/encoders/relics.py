import numpy as np
import torch
from jaxtyping import Float
from sts_sim import CounterKey, RelicKey
from torch import Tensor, nn

from .numeric import NumericBatch, tensor

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

    def numeric(
        self, batch: NumericBatch
    ) -> tuple[Float[Tensor, "n_relics relic_features"], Float[Tensor, "n_relics d_model"], list[int]]:
        """Embed raw relic keys and assemble alphabetically ordered counter slots."""
        rows = batch.table("relics", 2)
        raw_ids = rows[:, 1]
        if len(raw_ids) and (int(raw_ids.min()) < 0 or int(raw_ids.max()) >= len(RELIC_TO_INDEX)):
            raise ValueError("Relic id is outside content vocabulary v1")
        identities = self.embedding(tensor(self.embedding.weight, raw_ids, integer=True))
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
        features = torch.cat((identities, tensor(identities, values)), dim=1)
        lengths = batch.lengths(rows)
        return features, self.projection(features), lengths
