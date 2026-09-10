import torch
from encoders.cards import CardEncoder
from encoders.enemies import EnemyEncoder
from encoders.player import PlayerEncoder
from encoders.potions import PotionEncoder
from encoders.relics import RelicEncoder
from encoders.selection import SelectionEncoder
from jaxtyping import Float
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
    """Encode one raw observation, returning its query and reusable action features."""

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
        )
        self.transformer = nn.TransformerEncoder(layer, num_layers=n_layers, enable_nested_tensor=False)
        self.query = nn.Linear(d_model, action_dim)

    def forward(
        self,
        observation: CombatObservation,
    ) -> tuple[Float[Tensor, " action_dim"], dict[str, Float[Tensor, "?n_rows ?feature_dim"]]]:
        """Slices own their tables; selection/Stasis share cards. No batching or positional encoding."""
        # TODO: Encode screen.phase and keyed screen.public_counters (combat progress/history).
        # TODO: Encode ordered screen.orb_slots, including empty slots and Dark evoke values.
        # TODO: Encode context.ascension/act/floor and the permanent context.deck, not just combat piles.
        # TODO: Encode each Pile.known_positions; never infer hidden order from Pile.cards.
        # Schema/revision metadata are deliberately not gameplay features.
        screen, context = observation.screen, observation.context
        groups: dict[str, Float[Tensor, "?n_tokens d_model"]] = {}
        _, groups["player"] = self.player(observation)
        hand, groups["hand"] = self.cards(tuple(entry.card for entry in screen.hand))
        _, groups["draw"] = self.cards(screen.draw_pile.cards)
        _, groups["discard"] = self.cards(screen.discard_pile.cards)
        _, groups["exhaust"] = self.cards(screen.exhaust_pile.cards)
        enemies, groups["enemies"] = self.enemies(screen.monsters, self.cards)
        _, groups["relics"] = self.relics(context.relics)
        potions, groups["potions"] = self.potions(context.potion_slots)
        selection, groups["selection_context"], groups["selection_options"] = self.selection(
            screen.selection, self.cards
        )
        reference = self.summary_embedding.weight
        tokens: list[Float[Tensor, "?n_tokens d_model"]] = [
            self.summary_embedding(reference.new_zeros(1, dtype=torch.long))
        ]
        # TODO: Represent meaningful public hand/enemy/relic/potion slot order; attention currently
        # treats each group as a set. Keep action-slot lookup separate and unordered piles unordered.
        for index, name in enumerate(OBSERVATION_GROUPS):
            group = self.group_embedding(reference.new_tensor(index, dtype=torch.long))
            tokens.append(groups[name] + group)
        encoded = self.transformer(torch.cat(tokens, dim=0))
        action_features = {"hand": hand, "enemies": enemies, "potions": potions, "selection": selection}
        return self.query(encoded[0]), action_features
