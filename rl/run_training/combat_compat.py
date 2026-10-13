"""Explicit weights-only migration of the frozen 322-card public vocabulary."""

from collections.abc import Mapping

from encoders.vocabulary import CARD_KEYS, CARD_KEYS_V1
from torch import Tensor, nn


def frozen_weights(model: nn.Module, weights: Mapping[str, Tensor], *, adapt_legacy: bool) -> dict[str, Tensor]:
    expected = model.state_dict()
    if set(weights) != set(expected):
        raise ValueError("Frozen combat checkpoint parameter keys differ")
    key = "observation_encoder.cards.embedding.weight"
    result = dict(weights)
    if adapt_legacy:
        if CARD_KEYS != CARD_KEYS_V1 + ("COLD_SNAP", "PIERCING_WAIL"):
            raise ValueError("Unsupported target card vocabulary for legacy migration")
        source = weights[key]
        target = expected[key]
        if source.shape != (len(CARD_KEYS_V1), target.shape[1]):
            raise ValueError("Explicit legacy adaptation requires a v1 card embedding")
        extended = source.new_zeros(target.shape)
        extended[: len(CARD_KEYS_V1)] = source
        result[key] = extended
    for name, value in result.items():
        if value.shape != expected[name].shape:
            if name == key and not adapt_legacy:
                raise ValueError("Legacy combat checkpoint requires explicit --adapt-legacy-combat-checkpoint")
            raise ValueError(f"Frozen combat checkpoint shape mismatch: {name}")
    return result
