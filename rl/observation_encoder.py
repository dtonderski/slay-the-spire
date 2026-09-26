import math

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
        # Row grouping for the transformer call; parameters and checkpoints are unaffected.
        self.width_group_min_rows = 256
        self.width_group_growth = 1.25

    def prepare_numeric(
        self, batch: NumericBatch
    ) -> tuple[
        Float[Tensor, "batch n_tokens d_model"],
        Bool[Tensor, "batch n_tokens"],
        FlatActionFeatures,
        np.ndarray,
    ]:
        """Keep groups flat through projection; build the final padded layout in arrays.

        Also returns each row's real token count (summary included), computed on the host.
        """
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
        features = FlatActionFeatures(feature_rows, feature_lengths)
        return tokens.reshape(batch.size, width, -1), padding, features, positions

    def forward(self, observations: NumericBatch) -> tuple[Float[Tensor, "batch action_dim"], FlatActionFeatures]:
        """Return one summary query per observation; padding never participates as a key/value."""
        tokens, padding_mask, features, token_counts = self.prepare_numeric(observations)
        return self.query(self._summaries(tokens, padding_mask, token_counts)), features

    def _summaries(
        self, tokens: Float[Tensor, "batch n_tokens d_model"], padding: Bool[Tensor, "batch n_tokens"], counts: np.ndarray
    ) -> Float[Tensor, "batch d_model"]:
        """Encode rows in groups of similar width and return each row's summary token.

        Padding keys are masked, so trimming a group to its widest row does not change the
        result mathematically. It is not bit-identical to one padded call: kernels see
        different shapes. Groups are formed on the host from ``counts``; no device sync.
        """
        groups = width_groups(counts, tokens.shape[1], self.width_group_min_rows, self.width_group_growth)
        if len(groups) == 1:
            return self.transformer(tokens, src_key_padding_mask=padding)[:, 0]
        order = np.argsort(counts, kind="stable")
        inverse = np.empty_like(order)
        inverse[order] = np.arange(len(order))
        index = upload(order, torch.long, tokens.device)
        tokens, padding = tokens.index_select(0, index), padding.index_select(0, index)
        parts = [
            self.transformer(tokens[start:end, :width], src_key_padding_mask=padding[start:end, :width])[:, 0]
            for start, end, width in groups
        ]
        return torch.cat(parts).index_select(0, upload(inverse, torch.long, tokens.device))


def width_groups(counts: np.ndarray, width: int, min_rows: int, growth: float) -> list[tuple[int, int, int]]:
    """Contiguous ``(start, end, width)`` slices of rows sorted by token count.

    A group extends until its widest row exceeds ``growth`` times its narrowest, but always
    holds at least ``min_rows`` rows, so small batches stay one call. Widths are rounded up
    to a multiple of 8 (capped at the padded width); the extra columns are masked padding.
    """
    ordered = np.sort(counts)
    groups = []
    start = 0
    while start < len(ordered):
        end = int(np.searchsorted(ordered, math.ceil(ordered[start] * growth), side="right"))
        end = max(end, min(start + min_rows, len(ordered)))
        if len(ordered) - end < min_rows:
            end = len(ordered)
        groups.append((start, end, min(width, -(-int(ordered[end - 1]) // 8) * 8)))
        start = end
    return groups
