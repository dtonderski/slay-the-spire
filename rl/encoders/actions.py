from collections.abc import Iterator
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


def _slot(features: Float[Tensor, "n_slots feature_dim"], slot: int | None) -> Float[Tensor, " feature_dim"]:
    return features[_slot_index(features.shape[0], slot)]


def tensorize_actions(
    actions: tuple[Action, ...],
    hand_features: Float[Tensor, "n_hand card_features"],
    potion_features: Float[Tensor, "n_potion_slots potion_features"],
    enemy_features: Float[Tensor, "n_enemies enemy_features"],
    *,
    selection_features: Float[Tensor, "n_options selection_features"] | None = None,
) -> tuple[tuple[str, Float[Tensor, " ?action_features"]], ...]:
    """Gather (kind, raw features) pairs in candidate order; widths vary by kind.

    Use same-decision rows in visible slot order, with matching device/dtype.
    Selection rows come from SelectionEncoder. Play/use append target features
    and a presence bit; no-object actions have empty vectors. No learned layers.
    """
    result: list[tuple[str, Float[Tensor, " ?action_features"]]] = []
    for action in actions:
        if action.kind in ("play_hand_slot", "use_potion_slot"):
            if action.target_slot is None:
                target = enemy_features.new_zeros(enemy_features.shape[1])
            else:
                target = _slot(enemy_features, action.target_slot)
            target_present = target.new_tensor([float(action.target_slot is not None)])
            if action.kind == "play_hand_slot":
                card = _slot(hand_features, action.hand_slot)
                features = torch.cat((card, target, target_present))
            else:
                potion = _slot(potion_features, action.potion_slot)
                features = torch.cat((potion, target, target_present))
        elif action.kind == "discard_potion_slot":
            features = _slot(potion_features, action.potion_slot)
        elif action.kind in ("toggle_visible_card", "choose_visible_option"):
            if selection_features is None:
                raise ValueError("Selection action requires selection_features")
            features = _slot(selection_features, action.option_slot)
        elif action.kind in (
            "end_turn",
            "confirm_selection",
            "confirm_selection_without_retrieval",
            "skip_selection",
        ):
            features = hand_features.new_empty(0)
        else:
            raise NotImplementedError(f"Unsupported action kind: {action.kind}")
        result.append((action.kind, features))
    return tuple(result)


@dataclass
class FlatActionFeatures:
    """Numeric feature groups stay flat; per-observation views are only for inspection."""

    rows: dict[str, Float[Tensor, "?n_rows ?feature_dim"]]
    lengths: dict[str, list[int]]

    def __len__(self) -> int:
        return len(self.lengths["hand"])

    def __iter__(self) -> Iterator[dict[str, Tensor]]:
        offsets = dict.fromkeys(self.rows, 0)
        for index in range(len(self)):
            yield {
                name: rows[offsets[name] : offsets[name] + self.lengths[name][index]]
                for name, rows in self.rows.items()
            }
            for name in offsets:
                offsets[name] += self.lengths[name][index]

    def __getitem__(self, index: int) -> dict[str, Tensor]:
        if index < 0:
            index += len(self)
        if not 0 <= index < len(self):
            raise IndexError(index)
        return {
            name: rows[sum(self.lengths[name][:index]) : sum(self.lengths[name][: index + 1])]
            for name, rows in self.rows.items()
        }


type ActionFeatures = list[dict[str, Float[Tensor, "?n_rows ?feature_dim"]]] | FlatActionFeatures


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
        features: ActionFeatures,
        *,
        padded: bool = False,
    ) -> list[Float[Tensor, "?n_actions action_dim"]] | Float[Tensor, "batch n_actions action_dim"]:
        """Encode by kind; numeric scoring pads with one scatter, not per-observation copies."""
        vectors = self._flat(actions, features)
        lengths = [len(candidates) for candidates in actions]
        if not padded:
            return list(vectors.split(lengths))
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

    def _flat(self, actions: list[tuple[Action, ...]], features: ActionFeatures) -> Float[Tensor, "actions action_dim"]:
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
        sizes = (
            features.lengths
            if isinstance(features, FlatActionFeatures)
            else {name: [rows[name].shape[0] for rows in features] for name in offsets}
        )
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
        packed = dict(features.rows) if isinstance(features, FlatActionFeatures) else {}
        for kind, entries in grouped.items():
            indices = reference.new_tensor(entries, dtype=torch.long)
            if kind in self.encoders:
                group, _ = object_groups[kind]
                if group not in packed:
                    packed[group] = torch.cat([rows[group] for rows in features])
                inputs = packed[group].index_select(0, indices[:, 1])
                if kind in ("play_hand_slot", "use_potion_slot"):
                    if "targets" not in packed:
                        enemies = (
                            packed["enemies"]
                            if "enemies" in packed
                            else torch.cat([rows["enemies"] for rows in features])
                        )
                        packed["targets"] = torch.cat((enemies, enemies.new_zeros((1, ENEMY_FEATURE_DIM))))
                    targets = packed["targets"].index_select(0, indices[:, 2])
                    present = (indices[:, 2:3] != no_target).to(inputs.dtype)
                    inputs = torch.cat((inputs, targets, present), dim=1)
                encoded = self.encoders[kind](inputs)
            else:
                encoded = self.constants[kind](torch.zeros_like(indices[:, 0]))
            vectors = vectors.index_copy(0, indices[:, 0], encoded)
        return vectors
