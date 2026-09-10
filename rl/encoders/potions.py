import torch
from jaxtyping import Float
from sts_sim import PotionKey
from sts_sim.observations import PotionSlot
from torch import Tensor, nn

POTION_EMBEDDING_DIM = 16

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
POTION_TO_INDEX: dict[PotionKey | None, int] = {None: 0}
POTION_TO_INDEX.update({key: index + 1 for index, key in enumerate(PotionKey)})


class PotionEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(POTION_TO_INDEX), POTION_EMBEDDING_DIM)
        self.projection = nn.Linear(POTION_EMBEDDING_DIM, d_model)

    def tensorize(self, slots: tuple[PotionSlot, ...]) -> Float[Tensor, "n_potion_slots potion_features"]:
        """Embed potion slots in input order, including empty slots (index 0)."""
        indices = torch.tensor(
            [POTION_TO_INDEX[slot.content_key] for slot in slots],
            dtype=torch.long,
            device=self.embedding.weight.device,
        )
        return self.embedding(indices)

    def forward(
        self, batch: list[tuple[PotionSlot, ...]]
    ) -> tuple[list[Float[Tensor, "?n_potion_slots potion_features"]], list[Float[Tensor, "?n_potion_slots d_model"]]]:
        """Project all slots once and return unpadded per-observation rows."""
        lengths = [len(slots) for slots in batch]
        features = self.tensorize(tuple(slot for slots in batch for slot in slots))
        return list(features.split(lengths)), list(self.projection(features).split(lengths))
