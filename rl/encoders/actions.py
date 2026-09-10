import torch
from jaxtyping import Float
from sts_sim import Action
from torch import Tensor, nn

from .cards import CARD_FEATURE_DIM
from .enemies import ENEMY_FEATURE_DIM
from .potions import POTION_EMBEDDING_DIM


def _slot(features: Float[Tensor, "n_slots feature_dim"], slot: int | None) -> Float[Tensor, " feature_dim"]:
    """Look up one row, rejecting missing/negative/out-of-range slot references."""
    if slot is None or not 0 <= slot < features.shape[0]:
        raise ValueError(f"Invalid action slot {slot} for {features.shape[0]} feature rows")
    return features[slot]


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
    Selection rows come from tensorize_selection. Play/use append target features
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
        actions: tuple[Action, ...],
        features: dict[str, Float[Tensor, "?n_rows ?feature_dim"]],
    ) -> Float[Tensor, "n_actions action_dim"]:
        """Gather referenced rows from the same decision, preserving candidate order."""
        action_inputs = tensorize_actions(
            actions,
            features["hand"],
            features["potions"],
            features["enemies"],
            selection_features=features["selection"],
        )
        reference = next(self.parameters())
        vectors: list[Float[Tensor, " action_dim"]] = []
        for kind, row in action_inputs:
            if kind in self.encoders:
                vector = self.encoders[kind](row)
            else:
                vector = self.constants[kind](reference.new_zeros((), dtype=torch.long))
            vectors.append(vector)
        if not vectors:
            return reference.new_empty((0, self.action_dim))
        return torch.stack(vectors)
