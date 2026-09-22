from typing import get_args

import numpy as np
import torch
from jaxtyping import Float
from sts_sim import MonsterKey
from sts_sim.observations.combat import IntentCategory, SlimeSize
from torch import Tensor, nn

from .cards import CARD_FEATURE_DIM, CardEncoder
from .numeric import NumericBatch, tensor
from .player import GOLD_SCALE, POWER_TO_INDEX

# Include hidden/none separately from visible categories such as "unknown".
INTENT_TO_INDEX = {key: index for index, key in enumerate(("hidden", "none", *get_args(IntentCategory)))}
SLIME_SIZE_TO_INDEX = {size: index for index, size in enumerate((None, *get_args(SlimeSize)))}
ENEMY_EMBEDDING_DIM = 16
# Basic stats, powers, intent category/numbers, slime size, flags, gold, Stasis presence.
ENEMY_STATE_DIM = 4 + len(POWER_TO_INDEX) + len(INTENT_TO_INDEX) + 4 + len(SLIME_SIZE_TO_INDEX) + 5
ENEMY_FEATURE_DIM = ENEMY_EMBEDDING_DIM + ENEMY_STATE_DIM + CARD_FEATURE_DIM
HP_SCALE = 100.0
BLOCK_SCALE = 100.0
INTENT_DAMAGE_SCALE = 100.0

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
ENEMY_TO_INDEX = {key: index for index, key in enumerate(MonsterKey)}


class EnemyEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(ENEMY_TO_INDEX), ENEMY_EMBEDDING_DIM)
        self.projection = nn.Linear(ENEMY_FEATURE_DIM, d_model)

    def numeric(
        self, batch: NumericBatch, cards: CardEncoder
    ) -> tuple[Float[Tensor, "n_enemies enemy_features"], Float[Tensor, "n_enemies d_model"], list[int]]:
        """Encode raw enemy tables, retaining dead slots and shared Stasis features."""
        rows = batch.table("enemies", 18)
        raw_ids = rows[:, 1]
        if len(raw_ids) and (int(raw_ids.min()) < 0 or int(raw_ids.max()) >= len(ENEMY_TO_INDEX)):
            raise ValueError("Monster id is outside content vocabulary v1")
        if len(rows) and (
            int(rows[:, 6].min()) < 0
            or int(rows[:, 6].max()) >= len(SLIME_SIZE_TO_INDEX)
            or int(rows[:, 7].min()) < 0
            or int(rows[:, 7].max()) >= len(INTENT_TO_INDEX)
        ):
            raise ValueError("Enemy categorical id is outside content vocabulary v1")
        identities = self.embedding(tensor(self.embedding.weight, raw_ids, integer=True))
        stats = rows[:, 2:6] / np.array([HP_SCALE, HP_SCALE, BLOCK_SCALE, 1.0])
        powers = batch.powers("enemy_powers", len(rows), POWER_TO_INDEX)
        intent = np.eye(len(INTENT_TO_INDEX))[rows[:, 7]]
        numbers = rows[:, 8:12] / np.array([INTENT_DAMAGE_SCALE, 1.0, 1.0, 1.0])
        slime = np.eye(len(SLIME_SIZE_TO_INDEX))[rows[:, 6]]
        extra = rows[:, 12:17] / np.array([1.0, 1.0, 1.0, GOLD_SCALE, 1.0])
        state = tensor(identities, np.concatenate((stats, powers, intent, numbers, slime, extra), axis=1))
        stasis = batch.table("stasis", 18)
        held = identities.new_zeros((len(rows), CARD_FEATURE_DIM)).index_copy(
            0, tensor(identities, stasis[:, 0], integer=True), cards.numeric_features(batch, "stasis")
        )
        features = torch.cat((identities, state, held), dim=1)
        lengths = batch.lengths(rows)
        return features, self.projection(features), lengths
