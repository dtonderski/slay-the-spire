from typing import get_args

import torch
from jaxtyping import Float
from sts_sim.observations.combat import Selection, SelectionKind
from torch import Tensor, nn

from .cards import CardEncoder, tensorize_cards

SELECTION_TO_INDEX = {kind: index for index, kind in enumerate((None, *get_args(SelectionKind)))}
SELECTION_CONTEXT_DIM = len(SELECTION_TO_INDEX)


def tensorize_selection(
    selection: Selection | None, card_embedding: nn.Embedding
) -> tuple[
    Float[Tensor, " selection_context_features"],
    Float[Tensor, "n_options selection_features"],
]:
    """Return (kind one-hot, option rows) for the public selection overlay.

    Each option row is the shared card representation plus a selected bit.
    Rows retain visible slot order, never underlying hand/pile order. No selection
    has its own kind entry and an empty option matrix. Feed both outputs to the
    state encoder; option rows are also reused by selection action projections.
    """
    options = selection.options if selection is not None else ()
    if any(option.slot != index for index, option in enumerate(options)):
        raise ValueError("Selection options must be in contiguous visible slot order")
    selected = set(selection.selected_slots) if selection is not None else set()
    if any(slot < 0 or slot >= len(options) for slot in selected):
        raise ValueError("Selected slot is outside the visible options")
    cards = tensorize_cards(tuple(option.card for option in options), card_embedding)
    flags = cards.new_tensor([float(option.slot in selected) for option in options])
    context = cards.new_zeros(SELECTION_CONTEXT_DIM)
    context[SELECTION_TO_INDEX[selection.kind if selection is not None else None]] = 1.0
    return context, torch.cat((cards, flags.reshape(len(options), 1)), dim=1)


class SelectionEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.context_projection = nn.Linear(SELECTION_CONTEXT_DIM, d_model)
        self.selected_projection = nn.Linear(1, d_model, bias=False)

    def forward(
        self, selection: Selection | None, cards: CardEncoder
    ) -> tuple[
        Float[Tensor, "n_options selection_features"],
        Float[Tensor, "1 d_model"],
        Float[Tensor, "n_options d_model"],
    ]:
        """Return option features, context token, and options using the shared card projection."""
        context, features = tensorize_selection(selection, cards.embedding)
        tokens = cards.projection(features[:, :-1]) + self.selected_projection(features[:, -1:])
        return features, self.context_projection(context).unsqueeze(0), tokens
