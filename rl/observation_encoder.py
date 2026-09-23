import numpy as np
import torch
from encoders.actions import FlatActionFeatures
from encoders.cards import CardEncoder
from encoders.enemies import EnemyEncoder
from encoders.numeric import NumericBatch, tensor, upload
from encoders.player import PlayerEncoder
from encoders.potions import PotionEncoder
from encoders.relics import RelicEncoder
from encoders.selection import SelectionEncoder
from jaxtyping import Bool, Float
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

    def prepare_numeric(
        self, batch: NumericBatch
    ) -> tuple[
        Float[Tensor, "batch n_tokens d_model"],
        Bool[Tensor, "batch n_tokens"],
        FlatActionFeatures,
    ]:
        """Keep groups flat through projection; build the final padded layout in arrays."""
        groups = {}
        feature_rows = {}
        feature_lengths = {}
        for name, encoder in (("player", self.player), ("relics", self.relics), ("potions", self.potions)):
            features, tokens, lengths = encoder.numeric(batch)
            groups[name] = (tokens, lengths)
            if name == "potions":
                feature_rows[name] = features
                feature_lengths[name] = lengths
        cards = self.cards.numeric(batch)
        for name in ("hand", "draw", "discard", "exhaust"):
            features, tokens, lengths = cards[name]
            groups[name] = (tokens, lengths)
            if name == "hand":
                feature_rows[name] = features
                feature_lengths[name] = lengths
        features, tokens, lengths = self.enemies.numeric(batch, cards["stasis"][0])
        groups["enemies"] = (tokens, lengths)
        feature_rows["enemies"] = features
        feature_lengths["enemies"] = lengths
        features, context, tokens, lengths = self.selection.numeric(batch, cards["selection_cards"])
        groups["selection_context"] = (context, [1] * batch.size)
        groups["selection_options"] = (tokens, lengths)
        feature_rows["selection"] = features
        feature_lengths["selection"] = lengths
        counts = np.array([groups[name][1] for name in OBSERVATION_GROUPS]).T
        width = int(counts.sum(axis=1).max()) + 1
        positions = np.ones(batch.size, dtype=np.int64)
        reference = self.summary_embedding.weight
        destinations = [np.arange(batch.size) * width]
        for name in OBSERVATION_GROUPS:
            lengths = groups[name][1]
            owners = np.repeat(np.arange(batch.size), lengths)
            starts = np.cumsum(lengths) - lengths
            local = np.arange(len(owners)) - np.repeat(starts, lengths)
            destinations.append(owners * width + positions[owners] + local)
            positions += lengths
        # One lookup adds each row's group embedding; the summary row has none.
        kinds = np.repeat(np.arange(len(OBSERVATION_GROUPS)), counts.sum(axis=0))
        grouped = torch.cat([groups[name][0] for name in OBSERVATION_GROUPS])
        grouped = grouped + self.group_embedding(tensor(reference, kinds, integer=True))
        packed = torch.cat((reference.expand(batch.size, -1), grouped))
        indices = tensor(reference, np.concatenate(destinations), integer=True)
        # Each real row has one destination; padding has no source row or backward accumulation.
        tokens = reference.new_zeros((batch.size * width, reference.shape[1])).index_copy(0, indices, packed)
        padding = upload(np.arange(width)[None, :] >= positions[:, None], torch.bool, reference.device)
        return tokens.reshape(batch.size, width, -1), padding, FlatActionFeatures(feature_rows, feature_lengths)

    def forward(self, observations: NumericBatch) -> tuple[Float[Tensor, "batch action_dim"], FlatActionFeatures]:
        """Return one summary query per observation; padding never participates as a key/value."""
        tokens, padding_mask, features = self.prepare_numeric(observations)
        encoded = self.transformer(tokens, src_key_padding_mask=padding_mask)
        return self.query(encoded[:, 0]), features
