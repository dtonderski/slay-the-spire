from typing import get_args

import torch
from jaxtyping import Float
from sts_sim import MonsterKey
from sts_sim.observations.combat import IntentCategory, Monster, SlimeSize
from torch import Tensor, nn

from .cards import CARD_FEATURE_DIM, CARD_STATE_DIM, CardEncoder, tensorize_cards
from .player import GOLD_SCALE, POWER_TO_INDEX

# Include hidden/none separately from visible categories such as "unknown".
INTENT_TO_INDEX = {key: index for index, key in enumerate(("hidden", "none", *get_args(IntentCategory)))}

SLIME_SIZE_TO_INDEX = {size: index for index, size in enumerate((None, *get_args(SlimeSize)))}

ENEMY_EMBEDDING_DIM = 16
# Basic stats, powers, intent category/numbers, slime size, flags, gold, Stasis presence.
ENEMY_STATE_DIM = 4 + len(POWER_TO_INDEX) + len(INTENT_TO_INDEX) + 4 + len(SLIME_SIZE_TO_INDEX) + 5
# Feature width when using the default enemy and card embedding dimensions.
ENEMY_FEATURE_DIM = ENEMY_EMBEDDING_DIM + ENEMY_STATE_DIM + CARD_FEATURE_DIM
HP_SCALE = 100.0
BLOCK_SCALE = 100.0
INTENT_DAMAGE_SCALE = 100.0

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
ENEMY_TO_INDEX = {key: index for index, key in enumerate(MonsterKey)}


def tensorize_enemies(
    monsters: tuple[Monster, ...], embedding: nn.Embedding, card_embedding: nn.Embedding
) -> Float[Tensor, "n_enemies enemy_features"]:
    """Encode enemies in input slot order, including dead/escaped entries.

    Columns: identity, basic stats, powers, intent category, intent numbers
    (damage/100, hits, damage_present, hits_present), slime-size one-hot,
    escaped/minion/in_defensive_mode, stolen_gold/1000, stasis_present,
    then the held card's features (zeros if absent).

    Pass the same card embedding used for your hand/piles. Powers remain raw;
    hidden/none intents have zero numeric fields.
    """
    indices = torch.tensor(
        [ENEMY_TO_INDEX[monster.content_key] for monster in monsters],
        dtype=torch.long,
        device=embedding.weight.device,
    )
    identities = embedding(indices)
    rows: list[list[float]] = []
    held_card_rows: list[Float[Tensor, " card_features"]] = []
    card_feature_dim = card_embedding.embedding_dim + CARD_STATE_DIM
    for monster in monsters:
        stats = [
            monster.hp / HP_SCALE,
            monster.max_hp / HP_SCALE,
            monster.block / BLOCK_SCALE,
            # Escaping (e.g. a Looter) sets alive=False without zeroing HP.
            # targetable is deliberately omitted: the API currently projects it as alive.
            # TODO: Encode targetable separately if the public API makes it independent.
            float(monster.alive),
        ]
        powers = [0.0] * len(POWER_TO_INDEX)
        for power in monster.powers:
            powers[POWER_TO_INDEX[power.key]] = float(power.amount)
        intent = monster.intent
        if intent.visibility == "visible":
            intent_key = intent.category
            intent_numbers = [
                intent.damage / INTENT_DAMAGE_SCALE if intent.damage is not None else 0.0,
                float(intent.hits) if intent.hits is not None else 0.0,
                float(intent.damage is not None),
                float(intent.hits is not None),
            ]
        else:
            intent_key = intent.visibility
            intent_numbers = [0.0, 0.0, 0.0, 0.0]
        intent_features = [0.0] * len(INTENT_TO_INDEX)
        intent_features[INTENT_TO_INDEX[intent_key]] = 1.0
        slime_size = [0.0] * len(SLIME_SIZE_TO_INDEX)
        slime_size[SLIME_SIZE_TO_INDEX[monster.slime_size]] = 1.0
        extra = [
            float(monster.escaped),
            float(monster.minion),
            float(monster.in_defensive_mode),
            monster.stolen_gold / GOLD_SCALE,
            float(monster.stasis_card is not None),
        ]
        rows.append(stats + powers + intent_features + intent_numbers + slime_size + extra)
        if monster.stasis_card is None:
            held_card_rows.append(identities.new_zeros(card_feature_dim))
        else:
            held_card_rows.append(tensorize_cards((monster.stasis_card,), card_embedding)[0].to(identities))
    state = torch.tensor(rows, dtype=identities.dtype, device=identities.device).reshape(len(monsters), ENEMY_STATE_DIM)
    held_cards = torch.stack(held_card_rows) if held_card_rows else identities.new_zeros((0, card_feature_dim))
    return torch.cat((identities, state, held_cards), dim=1)


class EnemyEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(ENEMY_TO_INDEX), ENEMY_EMBEDDING_DIM)
        self.projection = nn.Linear(ENEMY_FEATURE_DIM, d_model)

    def forward(
        self, monsters: tuple[Monster, ...], cards: CardEncoder
    ) -> tuple[Float[Tensor, "n_enemies enemy_features"], Float[Tensor, "n_enemies d_model"]]:
        """Return enemy features and tokens; Stasis reuses the shared card table."""
        features = tensorize_enemies(monsters, self.embedding, cards.embedding)
        return features, self.projection(features)
