import numpy as np
import torch
from jaxtyping import Float
from sts_sim import CardKey
from torch import Tensor, nn

from .numeric import NumericBatch, tensor

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

    def numeric(
        self, batch: NumericBatch
    ) -> dict[str, tuple[Float[Tensor, "n_cards card_features"], Float[Tensor, "n_cards d_model"], list[int]]]:
        """Return flat features/tokens and per-observation lengths for every card table.

        All tables share one embedding lookup and one projection; per-table results
        are row slices of those, so each card is encoded exactly as it would be alone.
        """
        tables = [batch.table(name, 18) for name in CARD_TABLES]
        rows = np.concatenate(tables)
        raw_ids = rows[:, 1]
        if len(raw_ids) and (int(raw_ids.min()) < 0 or int(raw_ids.max()) >= len(CARD_TO_INDEX)):
            raise ValueError("Card id is outside content vocabulary v1")
        identities = self.embedding(tensor(self.embedding.weight, raw_ids, integer=True))
        state = tensor(identities, rows[:, [2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 17]])
        features = torch.cat((identities, state), dim=1)
        sizes = [len(table) for table in tables]
        return {
            name: (table_features, table_tokens, batch.lengths(table))
            for name, table, table_features, table_tokens in zip(
                CARD_TABLES, tables, features.split(sizes), self.projection(features).split(sizes), strict=True
            )
        }
