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


def tensorize_relics(relics: tuple[Relic, ...], embedding: nn.Embedding) -> Float[Tensor, "n_relics relic_features"]:
    """Append three counter slots, sorted by key and zero-padded."""
    indices = torch.tensor(
        [RELIC_TO_INDEX[relic.content_key] for relic in relics],
        dtype=torch.long,
        device=embedding.weight.device,
    )
    identities = embedding(indices)

    counter_rows: list[list[float]] = []
    for relic in relics:
        if len(relic.state) > RELIC_COUNTER_SLOTS:
            raise ValueError(f"Too many counters for {relic.content_key}")
        values = [float(counter.value) for counter in sorted(relic.state, key=lambda counter: counter.key.value)]
        values.extend([0.0] * (RELIC_COUNTER_SLOTS - len(values)))
        counter_rows.append(values)

    counters = torch.tensor(
        counter_rows,
        dtype=identities.dtype,
        device=identities.device,
    ).reshape(len(relics), RELIC_COUNTER_SLOTS)
    return torch.cat((identities, counters), dim=1)


class RelicEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(RELIC_TO_INDEX), RELIC_EMBEDDING_DIM)
        self.projection = nn.Linear(RELIC_FEATURE_DIM, d_model)

    def forward(
        self, relics: tuple[Relic, ...]
    ) -> tuple[Float[Tensor, "n_relics relic_features"], Float[Tensor, "n_relics d_model"]]:
        """Return relic features and projected tokens."""
        features = tensorize_relics(relics, self.embedding)
        return features, self.projection(features)
