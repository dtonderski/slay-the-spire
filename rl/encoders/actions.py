from dataclasses import dataclass

import numpy as np
import torch
from jaxtyping import Float
from sts_sim import Action
from torch import Tensor, nn

from .cards import CARD_FEATURE_DIM
from .enemies import ENEMY_FEATURE_DIM
from .potions import POTION_EMBEDDING_DIM


def _slot_index(size: int, slot: int | None) -> int:
    """Validate local slots before translating them into packed-batch indices."""
    if slot is None or not 0 <= slot < size:
        raise ValueError(f"Invalid action slot {slot} for {size} feature rows")
    return slot


@dataclass
class FlatActionFeatures:
    """Numeric feature groups stay flat; per-observation views are only for inspection."""

    rows: dict[str, Float[Tensor, "?n_rows ?feature_dim"]]
    lengths: dict[str, list[int]]

    def __len__(self) -> int:
        return len(self.lengths["hand"])


class ActionEncoder(nn.Module):
    """Encode legal actions using raw rows already computed by observation slices."""

    def __init__(self, action_dim: int = 64) -> None:
        super().__init__()
        self.action_dim = action_dim
        input_dims = {
            "play_hand_slot": CARD_FEATURE_DIM + ENEMY_FEATURE_DIM + 1,
            "use_potion_slot": POTION_EMBEDDING_DIM + ENEMY_FEATURE_DIM + 1,
            "discard_potion_slot": POTION_EMBEDDING_DIM,
            "toggle_visible_card": CARD_FEATURE_DIM + 1,
            "choose_visible_option": CARD_FEATURE_DIM + 1,
        }
        self.encoders = nn.ModuleDict(
            {
                kind: nn.Sequential(nn.Linear(input_dim, action_dim), nn.ReLU(), nn.Linear(action_dim, action_dim))
                for kind, input_dim in input_dims.items()
            }
        )
        self.constants = nn.ModuleDict(
            {
                kind: nn.Embedding(1, action_dim)
                for kind in ("end_turn", "confirm_selection", "confirm_selection_without_retrieval", "skip_selection")
            }
        )

    def forward(
        self,
        actions: list[tuple[Action, ...]],
        features: FlatActionFeatures,
    ) -> Float[Tensor, "batch n_actions action_dim"]:
        """Encode by kind; numeric scoring pads with one scatter, not per-observation copies."""
        vectors = self._flat(actions, features)
        lengths = [len(candidates) for candidates in actions]
        if not lengths or min(lengths) == 0:
            raise ValueError("Padded scoring needs nonempty decisions")
        counts = np.asarray(lengths)
        width = int(counts.max())
        owners = np.repeat(np.arange(len(actions)), counts)
        starts = np.cumsum(counts) - counts
        positions = owners * width + np.arange(len(vectors)) - np.repeat(starts, counts)
        indices = torch.tensor(positions, dtype=torch.long, device=vectors.device)
        # Unique destinations: backward gathers real rows, never atomically accumulates padding.
        padded_vectors = vectors.new_zeros((len(actions) * width, self.action_dim)).index_copy(0, indices, vectors)
        return padded_vectors.reshape(len(actions), width, self.action_dim)

    def _flat(
        self, actions: list[tuple[Action, ...]], features: FlatActionFeatures
    ) -> Float[Tensor, "actions action_dim"]:
        """Gather raw local slots without splitting already-flat numeric feature groups."""
        if len(actions) != len(features):
            raise ValueError("Action and feature batches must have the same length")
        object_groups = {
            "play_hand_slot": ("hand", "hand_slot"),
            "use_potion_slot": ("potions", "potion_slot"),
            "discard_potion_slot": ("potions", "potion_slot"),
            "toggle_visible_card": ("selection", "option_slot"),
            "choose_visible_option": ("selection", "option_slot"),
        }
        # Entries are integers only: candidate position, packed object row, packed target row.
        grouped: dict[str, list[tuple[int, int, int]]] = {}
        offsets = {name: 0 for name in ("hand", "potions", "enemies", "selection")}
        sizes = features.lengths
        no_target = sum(sizes["enemies"])
        position = 0
        for row, candidates in enumerate(actions):
            for action in candidates:
                object_index, target_index = -1, no_target
                if action.kind in object_groups:
                    group, slot_name = object_groups[action.kind]
                    object_index = offsets[group] + _slot_index(sizes[group][row], getattr(action, slot_name))
                    if action.kind in ("play_hand_slot", "use_potion_slot") and action.target_slot is not None:
                        target_index = offsets["enemies"] + _slot_index(sizes["enemies"][row], action.target_slot)
                elif action.kind not in self.constants:
                    raise NotImplementedError(f"Unsupported action kind: {action.kind}")
                grouped.setdefault(action.kind, []).append((position, object_index, target_index))
                position += 1
            for name in offsets:
                offsets[name] += sizes[name][row]

        reference = next(self.parameters())
        vectors = reference.new_zeros((position, self.action_dim))
        packed = dict(features.rows)
        for kind, entries in grouped.items():
            indices = reference.new_tensor(entries, dtype=torch.long)
            if kind in self.encoders:
                group, _ = object_groups[kind]
                inputs = packed[group].index_select(0, indices[:, 1])
                if kind in ("play_hand_slot", "use_potion_slot"):
                    if "targets" not in packed:
                        enemies = packed["enemies"]
                        packed["targets"] = torch.cat((enemies, enemies.new_zeros((1, ENEMY_FEATURE_DIM))))
                    targets = packed["targets"].index_select(0, indices[:, 2])
                    present = (indices[:, 2:3] != no_target).to(inputs.dtype)
                    inputs = torch.cat((inputs, targets, present), dim=1)
                encoded = self.encoders[kind](inputs)
            else:
                encoded = self.constants[kind](torch.zeros_like(indices[:, 0]))
            vectors = vectors.index_copy(0, indices[:, 0], encoded)
        return vectors
