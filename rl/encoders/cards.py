import numpy as np
import torch
from jaxtyping import Float
from sts_sim import CardKey
from torch import Tensor, nn

from .numeric import FeatureArrays, NumericBatch, upload_features

CARD_EMBEDDING_DIM = 16
CARD_STATE_DIM = 11
CARD_FEATURE_DIM = CARD_EMBEDDING_DIM + CARD_STATE_DIM
# Every public card table, encoded together in one pass.
CARD_TABLES = ("hand", "draw", "discard", "exhaust", "stasis", "selection_cards")

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
CARD_TO_INDEX = {key: index for index, key in enumerate(CardKey)}


class CardEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(CARD_TO_INDEX), CARD_EMBEDDING_DIM)
        self.projection = nn.Linear(CARD_FEATURE_DIM, d_model)

    @staticmethod
    def prepare(batch: NumericBatch) -> FeatureArrays:
        """Gather the public card tables once, before any learned embedding."""
        tables = [batch.table(name, 18) for name in CARD_TABLES]
        rows = np.concatenate(tables)
        raw_ids = rows[:, 1]
        if len(raw_ids) and (int(raw_ids.min()) < 0 or int(raw_ids.max()) >= len(CARD_TO_INDEX)):
            raise ValueError("Card id is outside content vocabulary v1")
        return FeatureArrays(
            {"ids": raw_ids},
            {"state": rows[:, [2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 17]]},
            {name: batch.lengths(table) for name, table in zip(CARD_TABLES, tables, strict=True)},
        )

    def encode(
        self, inputs: dict[str, Tensor], lengths: dict[str, list[int]]
    ) -> dict[str, tuple[Float[Tensor, "n_cards card_features"], Float[Tensor, "n_cards d_model"], list[int]]]:
        identities = self.embedding(inputs["ids"])
        features = torch.cat((identities, inputs["state"]), dim=1)
        sizes = [sum(lengths[name]) for name in CARD_TABLES]
        return {
            name: (table_features, table_tokens, lengths[name])
            for name, table_features, table_tokens in zip(
                CARD_TABLES, features.split(sizes), self.projection(features).split(sizes), strict=True
            )
        }

    def numeric(
        self, batch: NumericBatch
    ) -> dict[str, tuple[Float[Tensor, "n_cards card_features"], Float[Tensor, "n_cards d_model"], list[int]]]:
        raw = self.prepare(batch)
        return self.encode(upload_features({"cards": raw}, self.embedding.weight)["cards"], raw.lengths)
