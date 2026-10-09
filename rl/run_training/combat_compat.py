"""Explicit weights-only adaptation of the frozen public-v1 combat encoder.

This is not optimizer/resume compatibility. v1 public identity indices are kept;
new identity rows and new public-value columns are initialized to zero. Existing
weights keep their semantic columns, including card features inside enemy and
action inputs. Unknown shapes/parameters fail; no partial/non-strict load.
"""

from __future__ import annotations

from encoders.cards import CARD_FEATURE_DIM
from encoders.enemies import ENEMY_FEATURE_DIM
from encoders.potions import POTION_EMBEDDING_DIM
from encoders.vocabulary import LEGACY_V1
from torch import Tensor, nn

STRICT_PROTOCOL = "public_combat_v2_strict_weights"
LEGACY_PROTOCOL = "public_combat_v1_to_v2_zero_extension"
LEGACY_CARD_FEATURE_DIM = 27
ADDED_CARD_COLUMNS = CARD_FEATURE_DIM - LEGACY_CARD_FEATURE_DIM


def frozen_weights(
    model: nn.Module, weights: dict[str, Tensor], *, adapt_legacy: bool
) -> tuple[dict[str, Tensor], str]:
    expected = model.state_dict()
    if weights.keys() != expected.keys():
        raise ValueError("Frozen combat checkpoint parameters do not match")
    if all(weights[key].shape == target.shape for key, target in expected.items()):
        return weights, STRICT_PROTOCOL
    if not adapt_legacy:
        raise ValueError("Legacy combat checkpoint requires explicit --adapt-legacy-combat-checkpoint")
    if ADDED_CARD_COLUMNS != 9:
        raise ValueError("Unsupported public combat feature migration")
    old_enemy = ENEMY_FEATURE_DIM - ADDED_CARD_COLUMNS
    # Offsets are in the OLD concatenated input. Each insertion is nine columns.
    insertions = {
        "observation_encoder.cards.projection.weight": (LEGACY_CARD_FEATURE_DIM,),
        "observation_encoder.enemies.projection.weight": (old_enemy,),
        "action_encoder.encoders.play_hand_slot.0.weight": (
            LEGACY_CARD_FEATURE_DIM,
            LEGACY_CARD_FEATURE_DIM + old_enemy,
        ),
        "action_encoder.encoders.use_potion_slot.0.weight": (POTION_EMBEDDING_DIM + old_enemy,),
        "action_encoder.encoders.toggle_visible_card.0.weight": (LEGACY_CARD_FEATURE_DIM,),
        "action_encoder.encoders.choose_visible_option.0.weight": (LEGACY_CARD_FEATURE_DIM,),
    }
    embedding = "observation_encoder.cards.embedding.weight"
    result: dict[str, Tensor] = {}
    for key, target in expected.items():
        old = weights[key]
        if old.shape == target.shape:
            if key in insertions or key == embedding:
                raise ValueError(f"Mixed combat checkpoint feature protocols: {key}")
            result[key] = old
            continue
        if key == embedding:
            if old.shape != (len(LEGACY_V1["CardKey"]), target.shape[1]):
                raise ValueError("Unknown legacy card embedding vocabulary")
            expanded = old.new_zeros(target.shape)
            expanded[: len(old)] = old
        elif key in insertions:
            positions = insertions[key]
            if old.ndim != 2 or old.shape != (target.shape[0], target.shape[1] - len(positions) * ADDED_CARD_COLUMNS):
                raise ValueError(f"Unknown legacy feature shape: {key}")
            expanded = old.new_zeros(target.shape)
            start = 0
            shift = 0
            for end in (*positions, old.shape[1]):
                expanded[:, start + shift : end + shift] = old[:, start:end]
                start = end
                shift += ADDED_CARD_COLUMNS
        else:
            raise ValueError(f"Unsupported combat checkpoint migration: {key}")
        result[key] = expanded
    return result, LEGACY_PROTOCOL
