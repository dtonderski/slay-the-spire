import torch
from jaxtyping import Float
from sts_sim import RelicKey
from sts_sim.observations import Relic
from torch import Tensor, nn

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

    def tensorize(self, relics: tuple[Relic, ...]) -> Float[Tensor, "n_relics relic_features"]:
        """Append three counter slots, sorted by key and zero-padded."""
        indices = torch.tensor(
            [RELIC_TO_INDEX[relic.content_key] for relic in relics],
            dtype=torch.long,
            device=self.embedding.weight.device,
        )
        identities = self.embedding(indices)
        counter_rows: list[list[float]] = []
        for relic in relics:
            if len(relic.state) > RELIC_COUNTER_SLOTS:
                raise ValueError(f"Too many counters for {relic.content_key}")
            values = [float(counter.value) for counter in sorted(relic.state, key=lambda counter: counter.key.value)]
            values.extend([0.0] * (RELIC_COUNTER_SLOTS - len(values)))
            counter_rows.append(values)
        counters = identities.new_tensor(counter_rows).reshape(len(relics), RELIC_COUNTER_SLOTS)
        return torch.cat((identities, counters), dim=1)

    def forward(
        self, batch: list[tuple[Relic, ...]]
    ) -> tuple[list[Float[Tensor, "?n_relics relic_features"]], list[Float[Tensor, "?n_relics d_model"]]]:
        """Project all relics once and return unpadded per-observation rows."""
        lengths = [len(relics) for relics in batch]
        features = self.tensorize(tuple(relic for relics in batch for relic in relics))
        return list(features.split(lengths)), list(self.projection(features).split(lengths))
