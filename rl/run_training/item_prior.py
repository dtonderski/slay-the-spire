"""Shared offline/live conditional item head. No historical simulator-state construction.

Only ascension, a known card-reward floor, and visible item identities enter this
head. Boss floor is deliberately unknown even online. Unsupported/OOV/duplicate
slates get zero residuals, never a reduced legal-action list.
"""

from dataclasses import dataclass

import torch
from sts_sim import Observation
from torch import Tensor, nn

from run_training.contracts import PolicyAction

ITEM_PRIOR_PROTOCOL = "conditional_reward_identity_ascension_card_floor_v1"
ITEM_PRIOR_FORMAT = "run_item_prior_bc_v1"
ITEM_PRIOR_WIDTH = 32


@dataclass(frozen=True)
class ItemSlate:
    kind: str
    ascension: int
    floor: int | None
    items: tuple[str, ...]


@dataclass(frozen=True)
class RewardPriorInput:
    ascension: int
    floor: int
    items: tuple[str | None, ...]


def live_prior_input(
    observation: Observation, actions: tuple[PolicyAction, ...]
) -> RewardPriorInput | None:
    if observation.kind != "reward":
        return None
    items = []
    for action in actions:
        slot = action.reward_slot
        if action.kind == "take_card_reward":
            if slot is None or not 0 <= slot < len(observation.screen.cards):
                raise ValueError("Unresolved public card reward")
            card = observation.screen.cards[slot].card
            items.append(
                f"card:{card.content_key.removesuffix('+')}+{card.upgrade_level}"
            )
        elif action.kind == "choose_boss_relic_reward":
            if slot is None or not 0 <= slot < len(
                observation.screen.boss_relic_choices
            ):
                raise ValueError("Unresolved public boss relic reward")
            items.append("relic:" + observation.screen.boss_relic_choices[slot])
        else:
            items.append(None)
    return RewardPriorInput(
        observation.context.ascension, observation.context.floor, tuple(items)
    )


def validate_items(items: tuple[str, ...]) -> None:
    if not items or tuple(sorted(set(items))) != items:
        raise ValueError("Item vocabulary must be nonempty, sorted and distinct")
    if any(
        not isinstance(item, str) or not item.startswith(("card:", "relic:"))
        for item in items
    ):
        raise ValueError("Unsupported item identity")


class ItemPrior(nn.Module):
    """Train-only vocabulary; zero-output initialization also defines scratch control."""

    def __init__(self, items: tuple[str, ...]) -> None:
        super().__init__()
        validate_items(items)
        self.items = items
        self.indices = {item: index + 1 for index, item in enumerate(items)}
        self.item = nn.Embedding(len(items) + 1, ITEM_PRIOR_WIDTH, padding_idx=0)
        self.bias = nn.Embedding(len(items) + 1, 1, padding_idx=0)
        self.ascension = nn.Embedding(21, ITEM_PRIOR_WIDTH)
        self.family = nn.Embedding(2, ITEM_PRIOR_WIDTH)
        self.query = nn.Linear(ITEM_PRIOR_WIDTH + 2, ITEM_PRIOR_WIDTH)
        nn.init.zeros_(self.bias.weight)
        nn.init.zeros_(self.query.weight)
        nn.init.zeros_(self.query.bias)

    def supported(self, row: ItemSlate) -> bool:
        return (
            len(row.items) >= 2
            and len(set(row.items)) == len(row.items)
            and all(i in self.indices for i in row.items)
        )

    def forward(self, rows: list[ItemSlate]) -> Tensor:
        if not rows:
            raise ValueError("Nonempty item batch required")
        for row in rows:
            if (
                row.kind not in ("card", "relic")
                or not 0 <= row.ascension <= 20
                or not row.items
            ):
                raise ValueError("Invalid public item context")
            if any(not item.startswith(row.kind + ":") for item in row.items):
                raise ValueError("Mixed item families")
            if row.kind == "card" and (row.floor is None or not 0 <= row.floor <= 60):
                raise ValueError("Card floor must be known")
            if row.kind == "relic" and row.floor is not None:
                raise ValueError("Boss floor must remain unknown in the shared head")
        device = self.item.weight.device
        size = max(len(row.items) for row in rows)
        ids = torch.tensor(
            [
                [self.indices.get(item, 0) for item in row.items]
                + [0] * (size - len(row.items))
                for row in rows
            ],
            dtype=torch.long,
            device=device,
        )
        valid = torch.tensor(
            [[j < len(row.items) for j in range(size)] for row in rows], device=device
        )
        allowed = torch.tensor([self.supported(row) for row in rows], device=device)
        ascensions = torch.tensor([r.ascension for r in rows], device=device)
        families = torch.tensor(
            [0 if r.kind == "card" else 1 for r in rows], device=device
        )
        # Zero payload for unknown boss floor has an explicit false presence bit.
        floors = self.item.weight.new_tensor(
            [[0.0, 0.0] if r.floor is None else [r.floor / 60, 1.0] for r in rows]
        )
        context = self.ascension(ascensions) + self.family(families)
        query = self.query(torch.cat((context, floors), dim=1)).tanh()
        scores = (self.item(ids) * query[:, None, :]).sum(-1) + self.bias(ids).squeeze(
            -1
        )
        scores = torch.where(allowed[:, None], scores, torch.zeros_like(scores))
        return scores.masked_fill(~valid, -torch.inf)

    def combine(
        self,
        base: Tensor,
        inputs: tuple[RewardPriorInput | None, ...],
        lengths: tuple[int, ...],
    ) -> Tensor:
        """Redistribute within each family, preserving its base probability mass.

        Other actions (including Skip/Bowl) keep exactly their base logits. The
        log-partition correction prevents conditional demonstrations from silently
        teaching take-vs-skip. No choice is removed, even on unsupported slates.
        """
        rows, indices, offset = [], [], 0
        for item_input, length in zip(inputs, lengths, strict=True):
            if item_input is not None:
                if len(item_input.items) != length:
                    raise ValueError("Reward prior candidate count mismatch")
                for kind in ("card", "relic"):
                    selected = [
                        (i, token)
                        for i, token in enumerate(item_input.items)
                        if token is not None and token.startswith(kind + ":")
                    ]
                    if len(selected) >= 2:
                        rows.append(
                            ItemSlate(
                                kind,
                                item_input.ascension,
                                item_input.floor if kind == "card" else None,
                                tuple(token for _, token in selected),
                            )
                        )
                        indices.append([offset + i for i, _ in selected])
            offset += length
        if not rows:
            return base
        scores = self(rows)
        additions, destinations = [], []
        for row, positions in enumerate(indices):
            index = torch.tensor(positions, device=base.device)
            previous = base[index]
            biased = previous + scores[row, : len(positions)]
            correction = biased.logsumexp(0) - previous.logsumexp(0)
            additions.append(biased - correction - previous)
            destinations.append(index)
        return base.index_add(0, torch.cat(destinations), torch.cat(additions))
