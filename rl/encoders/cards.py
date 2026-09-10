import torch
from jaxtyping import Float
from sts_sim import CardKey
from sts_sim.observations import Card
from torch import Tensor, nn

CARD_EMBEDDING_DIM = 16
CARD_STATE_DIM = 11
CARD_FEATURE_DIM = CARD_EMBEDDING_DIM + CARD_STATE_DIM

# Current-catalog indices only. Checkpoint compatibility is not implemented yet.
CARD_TO_INDEX = {key: index for index, key in enumerate(CardKey)}


def tensorize_cards(cards: tuple[Card, ...], embedding: nn.Embedding) -> Float[Tensor, "n_cards card_features"]:
    """Embed identities and append raw state, preserving input order.

    State columns: cost, upgrade_level, cost_is_modified, cost_resets_next_turn,
    temporary, Rampage bonus, Ritual Dagger bonus, Windmill retain damage,
    Steam Barrier reduction, underlying combat cost, underlying cost present.
    Bottled status and pile positions are not encoded.
    """
    # TODO: Revisit the omitted public bottled flag when encoding the permanent deck/run context.
    # Optional damage/block bonuses deliberately collapse absent and zero for this v0.
    indices = torch.tensor(
        [CARD_TO_INDEX[card.content_key] for card in cards],
        dtype=torch.long,
        device=embedding.weight.device,
    )
    identities = embedding(indices)
    rows: list[list[float]] = []
    for card in cards:
        dynamic = card.dynamic
        underlying_cost = dynamic.combat_cost_under_turn_override
        rows.append(
            [
                float(card.cost),
                float(card.upgrade_level),
                float(card.cost_is_modified),
                float(card.cost_resets_next_turn),
                float(card.temporary),
                float(dynamic.rampage_damage_bonus or 0),
                float(dynamic.ritual_dagger_damage_bonus or 0),
                float(dynamic.windmill_retain_damage or 0),
                float(dynamic.steam_barrier_block_reduction or 0),
                float(underlying_cost if underlying_cost is not None else 0),
                float(underlying_cost is not None),
            ]
        )
    state = torch.tensor(rows, dtype=identities.dtype, device=identities.device).reshape(len(cards), CARD_STATE_DIM)
    return torch.cat((identities, state), dim=1)


class CardEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(len(CARD_TO_INDEX), CARD_EMBEDDING_DIM)
        self.projection = nn.Linear(CARD_FEATURE_DIM, d_model)

    def forward(
        self, cards: tuple[Card, ...]
    ) -> tuple[Float[Tensor, "n_cards card_features"], Float[Tensor, "n_cards d_model"]]:
        """Return shared card features and projected tokens, preserving order."""
        features = tensorize_cards(cards, self.embedding)
        return features, self.projection(features)
