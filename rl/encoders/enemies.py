from typing import get_args

import numpy as np
import torch
from jaxtyping import Float
from sts_sim import MonsterKey
from sts_sim.observations.combat import IntentCategory, Monster, SlimeSize
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

    def tensorize(self, monsters: tuple[Monster, ...], cards: CardEncoder) -> Float[Tensor, "n_enemies enemy_features"]:
        """Retain dead/escaped slots; Stasis uses shared raw card features."""
        indices = torch.tensor(
            [ENEMY_TO_INDEX[monster.content_key] for monster in monsters],
            dtype=torch.long,
            device=self.embedding.weight.device,
        )
        identities = self.embedding(indices)
        held = iter(cards.tensorize(tuple(m.stasis_card for m in monsters if m.stasis_card is not None)))
        rows: list[list[float]] = []
        held_card_rows: list[Float[Tensor, " card_features"]] = []
        for monster in monsters:
            stats = [
                monster.hp / HP_SCALE,
                monster.max_hp / HP_SCALE,
                monster.block / BLOCK_SCALE,
                # Escaping can set alive=False without zeroing HP.
                # TODO: Encode targetable separately if the API makes it independent of alive.
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
            held_card_rows.append(
                identities.new_zeros(CARD_FEATURE_DIM) if monster.stasis_card is None else next(held).to(identities)
            )
        state = identities.new_tensor(rows).reshape(len(monsters), ENEMY_STATE_DIM)
        held_cards = torch.stack(held_card_rows) if held_card_rows else identities.new_zeros((0, CARD_FEATURE_DIM))
        return torch.cat((identities, state, held_cards), dim=1)

    def numeric(
        self, batch: NumericBatch, cards: CardEncoder
    ) -> tuple[Float[Tensor, "n_enemies enemy_features"], Float[Tensor, "n_enemies d_model"], list[int]]:
        """Encode raw enemy tables, retaining dead slots and shared Stasis features."""
        rows = batch.table("enemies", 18)
        identities = self.embedding(
            tensor(self.embedding.weight, batch.codes(rows[:, 1], ENEMY_TO_INDEX), integer=True)
        )
        stats = rows[:, 2:6] / np.array([HP_SCALE, HP_SCALE, BLOCK_SCALE, 1.0])
        powers = batch.powers("enemy_powers", len(rows), POWER_TO_INDEX)
        intent = np.eye(len(INTENT_TO_INDEX))[batch.codes(rows[:, 7], INTENT_TO_INDEX)]
        numbers = rows[:, 8:12] / np.array([INTENT_DAMAGE_SCALE, 1.0, 1.0, 1.0])
        slime = np.eye(len(SLIME_SIZE_TO_INDEX))[batch.codes(rows[:, 6], SLIME_SIZE_TO_INDEX)]
        extra = rows[:, 12:17] / np.array([1.0, 1.0, 1.0, GOLD_SCALE, 1.0])
        state = tensor(identities, np.concatenate((stats, powers, intent, numbers, slime, extra), axis=1))
        stasis = batch.table("stasis", 18)
        held = identities.new_zeros((len(rows), CARD_FEATURE_DIM)).index_copy(
            0, tensor(identities, stasis[:, 0], integer=True), cards.numeric_features(batch, "stasis")
        )
        features = torch.cat((identities, state, held), dim=1)
        lengths = batch.lengths(rows)
        return features, self.projection(features), lengths

    def forward(
        self, batch: list[tuple[Monster, ...]], cards: CardEncoder
    ) -> tuple[list[Float[Tensor, "?n_enemies enemy_features"]], list[Float[Tensor, "?n_enemies d_model"]]]:
        """Project the flattened batch once, retaining each observation's slot order."""
        lengths = [len(monsters) for monsters in batch]
        features = self.tensorize(tuple(monster for monsters in batch for monster in monsters), cards)
        return list(features.split(lengths)), list(self.projection(features).split(lengths))
