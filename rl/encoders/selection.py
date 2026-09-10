from typing import get_args

import torch
from jaxtyping import Float
from sts_sim.observations.combat import Selection, SelectionKind
from torch import Tensor, nn

from .cards import CardEncoder

SELECTION_TO_INDEX = {kind: index for index, kind in enumerate((None, *get_args(SelectionKind)))}
SELECTION_CONTEXT_DIM = len(SELECTION_TO_INDEX)


class SelectionEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.context_projection = nn.Linear(SELECTION_CONTEXT_DIM, d_model)
        self.selected_projection = nn.Linear(1, d_model, bias=False)

    def forward(
        self, batch: list[Selection | None], cards: CardEncoder
    ) -> tuple[
        list[Float[Tensor, "?n_options selection_features"]],
        list[Float[Tensor, "1 d_model"]],
        list[Float[Tensor, "?n_options d_model"]],
    ]:
        """Return option rows, context tokens, and options using the shared card encoder."""
        options = [selection.options if selection is not None else () for selection in batch]
        selected = [set(selection.selected_slots) if selection is not None else set() for selection in batch]
        for slots, selected_slots in zip(options, selected):
            if any(option.slot != index for index, option in enumerate(slots)):
                raise ValueError("Selection options must be in contiguous visible slot order")
            if any(slot < 0 or slot >= len(slots) for slot in selected_slots):
                raise ValueError("Selected slot is outside the visible options")

        features, tokens = cards([tuple(option.card for option in slots) for slots in options])
        reference = self.context_projection.weight
        context = reference.new_zeros((len(batch), SELECTION_CONTEXT_DIM))
        for index, selection in enumerate(batch):
            context[index, SELECTION_TO_INDEX[selection.kind if selection is not None else None]] = 1.0
        flags = reference.new_tensor(
            [
                float(option.slot in selected_slots)
                for slots, selected_slots in zip(options, selected)
                for option in slots
            ]
        ).reshape(-1, 1)
        lengths = [len(slots) for slots in options]
        flag_rows = flags.split(lengths)
        flag_tokens = self.selected_projection(flags).split(lengths)
        return (
            [torch.cat((rows, flag), dim=1) for rows, flag in zip(features, flag_rows)],
            list(self.context_projection(context).split([1] * len(batch))),
            [rows + flag for rows, flag in zip(tokens, flag_tokens)],
        )
