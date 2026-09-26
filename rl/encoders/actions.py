from dataclasses import dataclass

import numpy as np
import torch
from jaxtyping import Float
from sts_sim import ACTION_KINDS
from torch import Tensor, nn

from .cards import CARD_FEATURE_DIM
from .enemies import ENEMY_FEATURE_DIM
from .numeric import CANDIDATE_KIND, CANDIDATE_OWNER, CANDIDATE_TARGET, CANDIDATE_WIDTH, upload
from .potions import POTION_EMBEDDING_DIM

KIND_CODE = {name: index for index, name in enumerate(ACTION_KINDS)}


def _require_slots(kind: str, slots: np.ndarray, limits: np.ndarray) -> None:
    if np.any((slots < 0) | (slots >= limits)):
        bad = int(slots[np.flatnonzero((slots < 0) | (slots >= limits))[0]])
        raise ValueError(f"Invalid action slot {bad} for {int(limits.max()) if len(limits) else 0} feature rows")


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
        candidates: np.ndarray,
        features: FlatActionFeatures,
    ) -> Float[Tensor, "batch n_actions action_dim"]:
        """Encode by kind. ``candidates`` are grouped by forward-local observation index."""
        vectors = self._flat(candidates, features)
        counts = np.bincount(candidates[:, CANDIDATE_OWNER].astype(np.int64), minlength=len(features))
        if int(counts.sum()) != len(candidates) or np.any(counts == 0):
            raise ValueError("Padded scoring needs nonempty decisions")
        width = int(counts.max())
        owners = np.repeat(np.arange(len(features)), counts)
        starts = np.cumsum(counts) - counts
        positions = owners * width + np.arange(len(vectors)) - np.repeat(starts, counts)
        indices = upload(positions, torch.long, vectors.device)
        padded_vectors = vectors.new_zeros((len(features) * width, self.action_dim)).index_copy(0, indices, vectors)
        return padded_vectors.reshape(len(features), width, self.action_dim)

    def _flat(self, candidates: np.ndarray, features: FlatActionFeatures) -> Float[Tensor, "actions action_dim"]:
        """Gather raw local slots from integer candidate rows, not Python action objects."""
        if candidates.ndim != 2 or candidates.shape[1] != CANDIDATE_WIDTH:
            raise ValueError("Action candidates must be integer rows")
        if len(candidates) == 0 or len(features) == 0:
            raise ValueError("Action and feature batches must have the same length")
        owners = candidates[:, CANDIDATE_OWNER].astype(np.int64)
        if int(owners.min()) < 0 or int(owners.max()) >= len(features) or np.any(np.diff(owners) < 0):
            raise ValueError("Action candidates must be grouped by observation")
        kinds = candidates[:, CANDIDATE_KIND].astype(np.int64)
        known = {KIND_CODE[name] for name in (*self.encoders, *self.constants)}
        if np.any(~np.isin(kinds, list(known))):
            missing = int(kinds[~np.isin(kinds, list(known))][0])
            name = ACTION_KINDS[missing] if 0 <= missing < len(ACTION_KINDS) else str(missing)
            raise NotImplementedError(f"Unsupported action kind: {name}")
        sizes = {name: np.asarray(lengths, dtype=np.int64) for name, lengths in features.lengths.items()}
        bases = {name: np.cumsum(values) - values for name, values in sizes.items()}
        no_target = int(sizes["enemies"].sum())
        reference = next(self.parameters())
        vectors = reference.new_zeros((len(candidates), self.action_dim))
        packed = dict(features.rows)
        object_column = {
            "play_hand_slot": (2, "hand"),
            "use_potion_slot": (3, "potions"),
            "discard_potion_slot": (3, "potions"),
            "toggle_visible_card": (4, "selection"),
            "choose_visible_option": (4, "selection"),
        }
        for kind, (column, group) in object_column.items():
            selected = np.flatnonzero(kinds == KIND_CODE[kind])
            if len(selected) == 0:
                continue
            slots = candidates[selected, column].astype(np.int64)
            limits = sizes[group][owners[selected]]
            _require_slots(kind, slots, limits)
            inputs = packed[group].index_select(
                0, upload(bases[group][owners[selected]] + slots, torch.long, reference.device)
            )
            if kind in ("play_hand_slot", "use_potion_slot"):
                target_slots = candidates[selected, CANDIDATE_TARGET].astype(np.int64)
                present = target_slots >= 0
                if np.any(present):
                    _require_slots(kind, target_slots[present], sizes["enemies"][owners[selected][present]])
                target_index = np.full(len(selected), no_target, dtype=np.int64)
                if np.any(present):
                    target_index[present] = bases["enemies"][owners[selected][present]] + target_slots[present]
                if "targets" not in packed:
                    enemies = packed["enemies"]
                    packed["targets"] = torch.cat((enemies, enemies.new_zeros((1, ENEMY_FEATURE_DIM))))
                targets = packed["targets"].index_select(0, upload(target_index, torch.long, reference.device))
                flag = upload(present, inputs.dtype, reference.device).unsqueeze(1)
                inputs = torch.cat((inputs, targets, flag), dim=1)
            encoded = self.encoders[kind](inputs)
            vectors = vectors.index_copy(0, upload(selected, torch.long, reference.device), encoded)
        for kind in self.constants:
            selected = np.flatnonzero(kinds == KIND_CODE[kind])
            if len(selected) == 0:
                continue
            encoded = self.constants[kind](torch.zeros(len(selected), dtype=torch.long, device=reference.device))
            vectors = vectors.index_copy(0, upload(selected, torch.long, reference.device), encoded)
        return vectors
