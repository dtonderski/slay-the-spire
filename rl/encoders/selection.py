from typing import get_args

import numpy as np
import torch
from jaxtyping import Float
from sts_sim.observations.combat import SelectionKind
from torch import Tensor, nn

from .cards import CardEncoder
from .numeric import NumericBatch, tensor

SELECTION_TO_INDEX = {kind: index for index, kind in enumerate((None, *get_args(SelectionKind)))}
SELECTION_CONTEXT_DIM = len(SELECTION_TO_INDEX)


class SelectionEncoder(nn.Module):
    def __init__(self, d_model: int = 64) -> None:
        super().__init__()
        self.context_projection = nn.Linear(SELECTION_CONTEXT_DIM, d_model)
        self.selected_projection = nn.Linear(1, d_model, bias=False)

    def numeric(
        self, batch: NumericBatch, cards: CardEncoder
    ) -> tuple[
        Float[Tensor, "n_options selection_features"],
        Float[Tensor, "batch d_model"],
        Float[Tensor, "n_options d_model"],
        list[int],
    ]:
        """Encode context and option flags without per-observation tensor operations."""
        features, tokens, lengths_list = cards.numeric(batch, "selection_cards")
        lengths = np.array(lengths_list)
        starts = np.cumsum(lengths) - lengths
        options = batch.table("selection_options", 2)
        if not np.array_equal(options[:, 1], np.arange(len(options)) - np.repeat(starts, lengths)):
            raise ValueError("Selection options must be in contiguous visible slot order")
        selected = batch.table("selected_slots", 2)
        if np.any(selected[:, 1] < 0) or np.any(selected[:, 1] >= lengths[selected[:, 0]]):
            raise ValueError("Selected slot is outside the visible options")
        values = np.zeros((len(options), 1))
        values[starts[selected[:, 0]] + selected[:, 1], 0] = 1
        reference = self.context_projection.weight
        flags = tensor(reference, values)
        context = tensor(
            reference, np.eye(SELECTION_CONTEXT_DIM)[batch.codes(batch.table("selection", 1)[:, 0], SELECTION_TO_INDEX)]
        )
        return (
            torch.cat((features, flags), dim=1),
            self.context_projection(context),
            tokens + self.selected_projection(flags),
            lengths_list,
        )
