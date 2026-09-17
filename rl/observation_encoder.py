import numpy as np
import torch
from encoders.cards import CardEncoder
from encoders.enemies import EnemyEncoder
from encoders.numeric import NumericBatch, tensor
from encoders.player import PlayerEncoder
from encoders.potions import PotionEncoder
from encoders.relics import RelicEncoder
from encoders.selection import SelectionEncoder
from jaxtyping import Bool, Float
from sts_sim import CombatObservation
from torch import Tensor, nn

OBSERVATION_GROUPS = (
    "player",
    "hand",
    "draw",
    "discard",
    "exhaust",
    "enemies",
    "relics",
    "potions",
    "selection_context",
    "selection_options",
)


class ObservationEncoder(nn.Module):
    """Encode an observation batch, returning queries and per-observation action features."""

    def __init__(self, d_model: int = 64, action_dim: int = 64, n_heads: int = 4, n_layers: int = 2) -> None:
        super().__init__()
        self.cards = CardEncoder(d_model)
        self.enemies = EnemyEncoder(d_model)
        self.player = PlayerEncoder(d_model)
        self.potions = PotionEncoder(d_model)
        self.relics = RelicEncoder(d_model)
        self.selection = SelectionEncoder(d_model)
        self.group_embedding = nn.Embedding(len(OBSERVATION_GROUPS), d_model)
        self.summary_embedding = nn.Embedding(1, d_model)
        layer = nn.TransformerEncoderLayer(
            d_model=d_model,
            nhead=n_heads,
            dim_feedforward=4 * d_model,
            dropout=0.0,
            norm_first=True,
            batch_first=True,
        )
        self.transformer = nn.TransformerEncoder(layer, num_layers=n_layers, enable_nested_tensor=False)
        self.query = nn.Linear(d_model, action_dim)

    def prepare_batch(
        self, observations: list[CombatObservation] | NumericBatch
    ) -> tuple[
        Float[Tensor, "batch n_tokens d_model"],
        Bool[Tensor, "batch n_tokens"],
        list[dict[str, Float[Tensor, "?n_rows ?feature_dim"]]],
    ]:
        """Concatenate real tokens per observation, then pad once; True marks padding."""
        if not observations:
            raise ValueError("Cannot encode an empty observation batch")
        if isinstance(observations, NumericBatch):
            return self.prepare_numeric(observations)
        # TODO: Encode screen.phase and keyed screen.public_counters (combat progress/history).
        # TODO: Encode ordered screen.orb_slots, including empty slots and Dark evoke values.
        # TODO: Encode context.ascension/act/floor and the permanent context.deck, not just combat piles.
        # TODO: Encode each Pile.known_positions; never infer hidden order from Pile.cards.
        # Schema/revision metadata are deliberately not gameplay features.
        groups: dict[str, list[Float[Tensor, "?n_tokens d_model"]]] = {}
        _, groups["player"] = self.player(observations)
        hand, groups["hand"] = self.cards([tuple(entry.card for entry in obs.screen.hand) for obs in observations])
        _, groups["draw"] = self.cards([obs.screen.draw_pile.cards for obs in observations])
        _, groups["discard"] = self.cards([obs.screen.discard_pile.cards for obs in observations])
        _, groups["exhaust"] = self.cards([obs.screen.exhaust_pile.cards for obs in observations])
        enemies, groups["enemies"] = self.enemies([obs.screen.monsters for obs in observations], self.cards)
        _, groups["relics"] = self.relics([obs.context.relics for obs in observations])
        potions, groups["potions"] = self.potions([obs.context.potion_slots for obs in observations])
        selection, groups["selection_context"], groups["selection_options"] = self.selection(
            [obs.screen.selection for obs in observations], self.cards
        )
        return self._assemble(groups, hand, enemies, potions, selection)

    def prepare_numeric(
        self, batch: NumericBatch
    ) -> tuple[
        Float[Tensor, "batch n_tokens d_model"],
        Bool[Tensor, "batch n_tokens"],
        list[dict[str, Float[Tensor, "?n_rows ?feature_dim"]]],
    ]:
        """Keep groups flat through projection; build the final padded layout in arrays."""
        groups = {}
        feature_rows = {}
        for name, encoder in (("player", self.player), ("relics", self.relics), ("potions", self.potions)):
            features, tokens, lengths = encoder.numeric(batch)
            groups[name] = (tokens, lengths)
            if name == "potions":
                feature_rows[name] = features.split(lengths)
        for name in ("hand", "draw", "discard", "exhaust"):
            features, tokens, lengths = self.cards.numeric(batch, name)
            groups[name] = (tokens, lengths)
            if name == "hand":
                feature_rows[name] = features.split(lengths)
        features, tokens, lengths = self.enemies.numeric(batch, self.cards)
        groups["enemies"] = (tokens, lengths)
        feature_rows["enemies"] = features.split(lengths)
        features, context, tokens, lengths = self.selection.numeric(batch, self.cards)
        groups["selection_context"] = (context, [1] * batch.size)
        groups["selection_options"] = (tokens, lengths)
        feature_rows["selection"] = features.split(lengths)
        counts = np.array([groups[name][1] for name in OBSERVATION_GROUPS]).T
        width = int(counts.sum(axis=1).max()) + 1
        layout = np.ones((batch.size, width), dtype=np.int64)
        layout[:, 0] = 0
        positions = np.ones(batch.size, dtype=np.int64)
        reference = self.summary_embedding.weight
        packed = [reference, reference.new_zeros(reference.shape)]
        offset = 2
        for index, name in enumerate(OBSERVATION_GROUPS):
            tokens, lengths = groups[name]
            owners = np.repeat(np.arange(batch.size), lengths)
            starts = np.cumsum(lengths) - lengths
            local = np.arange(len(owners)) - np.repeat(starts, lengths)
            layout[owners, positions[owners] + local] = offset + np.arange(len(owners))
            positions += lengths
            offset += len(owners)
            packed.append(tokens + self.group_embedding.weight[index])
        indices = tensor(reference, layout, integer=True)
        tokens = torch.cat(packed).index_select(0, indices.flatten()).reshape(batch.size, width, -1)
        features = [{name: rows[index] for name, rows in feature_rows.items()} for index in range(batch.size)]
        return tokens, indices == 1, features

    def _assemble(self, groups, hand, enemies, potions, selection):
        # TODO: Represent meaningful public hand/enemy/relic/potion slot order; attention currently
        # treats each group as a set. Keep action-slot lookup separate and unordered piles unordered.
        # Pack by group, adding each location embedding once across all observations.
        # Rows 0/1 are the summary/padding tokens. Integer lists describe each complete
        # observation sequence; one gather replaces per-observation cat/add/pad graphs.
        reference = self.summary_embedding.weight
        packed = [reference, reference.new_zeros(reference.shape)]
        sequences = [[0] for _ in hand]
        offset = 2
        for index, name in enumerate(OBSERVATION_GROUPS):
            rows = groups[name]
            packed.append(torch.cat(rows) + self.group_embedding.weight[index])
            for sequence, row in zip(sequences, rows):
                sequence.extend(range(offset, offset + row.shape[0]))
                offset += row.shape[0]
        width = max(map(len, sequences))
        indices = reference.new_tensor(
            [sequence + [1] * (width - len(sequence)) for sequence in sequences], dtype=torch.long
        )
        tokens = torch.cat(packed).index_select(0, indices.flatten()).reshape(len(hand), width, -1)
        padding_mask = indices == 1
        action_features = [
            {"hand": hand[i], "enemies": enemies[i], "potions": potions[i], "selection": selection[i]}
            for i in range(len(hand))
        ]
        return tokens, padding_mask, action_features

    def forward(
        self, observations: list[CombatObservation] | NumericBatch
    ) -> tuple[Float[Tensor, "batch action_dim"], list[dict[str, Float[Tensor, "?n_rows ?feature_dim"]]]]:
        """Return one summary query per observation; padding never participates as a key/value."""
        tokens, padding_mask, features = self.prepare_batch(observations)
        encoded = self.transformer(tokens, src_key_padding_mask=padding_mask)
        return self.query(encoded[:, 0]), features
