import torch
from encoders.cards import CardEncoder
from encoders.enemies import EnemyEncoder
from encoders.player import PlayerEncoder
from encoders.potions import PotionEncoder
from encoders.relics import RelicEncoder
from encoders.selection import SelectionEncoder
from jaxtyping import Bool, Float
from sts_sim import CombatObservation
from torch import Tensor, nn
from torch.nn.utils.rnn import pad_sequence

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
        self, observations: list[CombatObservation]
    ) -> tuple[
        Float[Tensor, "batch n_tokens d_model"],
        Bool[Tensor, "batch n_tokens"],
        list[dict[str, Float[Tensor, "?n_rows ?feature_dim"]]],
    ]:
        """Concatenate real tokens per observation, then pad once; True marks padding."""
        if not observations:
            raise ValueError("Cannot encode an empty observation batch")
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
        # TODO: Represent meaningful public hand/enemy/relic/potion slot order; attention currently
        # treats each group as a set. Keep action-slot lookup separate and unordered piles unordered.
        sequences = [
            torch.cat(
                [self.summary_embedding.weight]
                + [
                    groups[name][i] + self.group_embedding.weight[index]
                    for index, name in enumerate(OBSERVATION_GROUPS)
                ]
            )
            for i in range(len(observations))
        ]
        tokens = pad_sequence(sequences, batch_first=True)
        lengths = torch.tensor([len(sequence) for sequence in sequences], device=tokens.device)
        padding_mask = torch.arange(tokens.shape[1], device=tokens.device).unsqueeze(0) >= lengths.unsqueeze(1)
        action_features = [
            {"hand": hand[i], "enemies": enemies[i], "potions": potions[i], "selection": selection[i]}
            for i in range(len(observations))
        ]
        return tokens, padding_mask, action_features

    def forward(
        self, observations: list[CombatObservation]
    ) -> tuple[Float[Tensor, "batch action_dim"], list[dict[str, Float[Tensor, "?n_rows ?feature_dim"]]]]:
        """Return one summary query per observation; padding never participates as a key/value."""
        tokens, padding_mask, features = self.prepare_batch(observations)
        encoded = self.transformer(tokens, src_key_padding_mask=padding_mask)
        return self.query(encoded[:, 0]), features
