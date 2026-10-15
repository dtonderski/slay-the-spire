"""Opt-in, public-only policy restrictions; simulator legality is never changed."""

from collections.abc import Sequence

import torch
from sts_sim import Observation, RelicKey
from torch import Tensor

from run_training.contracts import PolicyAction

AVOID_PRISMATIC_PROTOCOL = "avoid_visible_prismatic_v1"
NAVIGATION_PROTOCOL = "commit_room_and_merchant_exit_v1"


def macro_eligibility(
    observation: Observation,
    actions: tuple[PolicyAction, ...],
    *,
    avoid_prismatic: bool,
    commit_navigation: bool = False,
    shop_finished: bool = False,
) -> tuple[bool, ...]:
    """Explicit policy commitments; native legality and candidate indices stay intact.

    Leaving a room commits to advancing. Leaving the merchant commits to finishing
    shopping in this room, even if purchases remain. Public map is already in context.
    """
    if not commit_navigation and (not avoid_prismatic or len(actions) == 1):
        return ()

    def at(items: tuple, slot: int | None):
        if slot is None or not 0 <= slot < len(items):
            raise ValueError("Unresolved public relic reference")
        return items[slot]

    result = []
    for action in actions:
        key = None
        if action.kind == "buy_shop_relic":
            if observation.kind != "shop" or action.shop_slot is None:
                raise ValueError("Unresolved public shop relic reference")
            key = at(observation.screen.relics, action.shop_slot).content_key
        elif action.kind == "take_relic_reward":
            if observation.kind != "reward":
                raise ValueError("Relic acquisition outside public reward screen")
            key = observation.screen.relic_offer
        elif action.kind == "take_relic_reward_at":
            if observation.kind != "reward" or action.reward_slot is None:
                raise ValueError("Unresolved public relic reward reference")
            key = at(observation.screen.relic_offers, action.reward_slot).content_key
        elif action.kind == "choose_boss_relic_reward":
            if observation.kind != "reward" or action.reward_slot is None:
                raise ValueError("Unresolved public boss relic reference")
            key = at(observation.screen.boss_relic_choices, action.reward_slot)
        blocked_shard = (
            avoid_prismatic and len(actions) > 1 and key == RelicKey.PRISMATIC_SHARD
        )
        blocked_navigation = commit_navigation and (
            action.kind == "return_to_room"
            or (shop_finished and action.kind == "enter_shop")
        )
        result.append(not (blocked_shard or blocked_navigation))
    eligibility = tuple(result)
    checked_eligibility(len(actions), eligibility)
    return eligibility


def checked_eligibility(
    size: int, eligibility: tuple[bool, ...], *, choice: int | None = None
) -> tuple[bool, ...]:
    if size < 1 or (
        eligibility
        and (len(eligibility) != size or any(type(v) is not bool for v in eligibility))
    ):
        raise ValueError("Malformed policy eligibility record")
    result = eligibility or (True,) * size
    if not any(result):
        raise ValueError("Policy restriction left no eligible action")
    if choice is not None and (not 0 <= choice < size or not result[choice]):
        raise ValueError("Recorded choice violates policy eligibility")
    return result


def restrict_logits(logits: Tensor, eligibility: tuple[bool, ...]) -> Tensor:
    """Preserve all rows/indices; zero excluded probabilities via negative infinity.

    Callers must validate every actual raw model logit, including excluded rows.
    """
    if logits.ndim != 1:
        raise ValueError("Expected one complete candidate row")
    allowed = checked_eligibility(len(logits), eligibility)
    if all(allowed):
        return logits
    return logits.masked_fill(
        ~torch.tensor(allowed, dtype=torch.bool, device=logits.device), -torch.inf
    )


def restrict_batch_logits(
    logits: Tensor, lengths: Sequence[int], eligibility: Sequence[tuple[bool, ...]]
) -> Tensor:
    if (
        logits.ndim != 2
        or logits.shape[0] != len(lengths)
        or len(eligibility) != len(lengths)
    ):
        raise ValueError("Malformed batched policy eligibility")
    allowed = []
    for size, row in zip(lengths, eligibility, strict=True):
        if size > logits.shape[1]:
            raise ValueError("Candidate row exceeds padded logits")
        allowed.append(
            checked_eligibility(size, row) + (False,) * (logits.shape[1] - size)
        )
    if not any(row for row in eligibility):
        return logits  # Legacy/all-legal protocol: the model already masks padding.
    mask = torch.tensor(allowed, dtype=torch.bool, device=logits.device)
    return logits.masked_fill(~mask, -torch.inf)
