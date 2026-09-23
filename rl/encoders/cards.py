import torch
from jaxtyping import Float
from sts_sim import CardKey
from torch import Tensor, nn

from .numeric import NumericBatch, tensor

CARD_EMBEDDING_DIM = 16
CARD_STATE_DIM = 11
CARD_FEATURE_DIM = CARD_EMBEDDING_DIM + CARD_STATE_DIM

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
CARD_TO_INDEX = {key: index for index, key in enumerate(CardKey)}


class CardEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(CARD_TO_INDEX), CARD_EMBEDDING_DIM)
        self.projection = nn.Linear(CARD_FEATURE_DIM, d_model)

    def numeric_features(self, batch: NumericBatch, name: str) -> Float[Tensor, "n_cards card_features"]:
        rows = batch.table(name, 18)
        raw_ids = rows[:, 1]
        if len(raw_ids) and (int(raw_ids.min()) < 0 or int(raw_ids.max()) >= len(CARD_TO_INDEX)):
            raise ValueError("Card id is outside content vocabulary v1")
        ids = tensor(self.embedding.weight, raw_ids, integer=True)
        identities = self.embedding(ids)
        state = tensor(identities, rows[:, [2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 17]])
        return torch.cat((identities, state), dim=1)

    def numeric(
        self, batch: NumericBatch, name: str
    ) -> tuple[Float[Tensor, "n_cards card_features"], Float[Tensor, "n_cards d_model"], list[int]]:
        """Return flat card features/tokens and per-observation lengths."""
        features = self.numeric_features(batch, name)
        lengths = batch.lengths(batch.table(name, 18))
        return features, self.projection(features), lengths
