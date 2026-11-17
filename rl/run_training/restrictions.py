"""Opt-in, public-only policy restrictions; simulator legality is never changed."""

from collections.abc import Sequence
from dataclasses import dataclass

import torch
from sts_sim import Observation, RelicKey
from sts_sim.observations.common import CardSlot
from torch import Tensor

from run_training.contracts import PolicyAction

AVOID_PRISMATIC_PROTOCOL = "avoid_visible_prismatic_v1"
NAVIGATION_PROTOCOL = "commit_room_and_merchant_exit_v1"
DIRECT_SMITH_PROTOCOL = "campfire_direct_smith_v1"
CARD_REWARD_PROTOCOL = "card_reward_close_is_final_v1"
_OPEN_CARD_REWARD = ("open_card_reward", "open_queued_card_reward")


@dataclass(frozen=True, slots=True)
class ClosedCardRewards:
    """Card rewards on the current reward screen closed without taking a card.

    Built only from public reward screens and accepted actions. Opening a reward
    shows its cards in `screen.cards`; closing keeps them there, and
    `open_card_reward` reopens exactly them (or the first queued reward when no
    cards are shown). Queued rewards are known by public slot only, so the queued
    slot currently shown is tracked, and slots shift when a queued reward is used.
    """

    queued: tuple[bool, ...] | None = None  # closed flag per queued slot
    shown_slot: int | None = None  # queued slot whose cards are shown
    closed_cards: tuple[CardSlot, ...] = ()  # closed non-queued cards

    def blocks(self, observation: Observation, action: PolicyAction) -> bool:
        if self.queued is None or observation.kind != "reward":
            return False
        cards = observation.screen.cards
        if action.kind == "open_queued_card_reward":
            return self.queued[action.reward_slot or 0]
        if action.kind != "open_card_reward":
            return False
        if not cards:
            return bool(self.queued) and self.queued[0]
        if self.shown_slot is not None:
            return self.queued[self.shown_slot]
        return cards == self.closed_cards

    def advanced(
        self,
        before: Observation | None,
        action: PolicyAction,
        after: Observation | None,
    ) -> "ClosedCardRewards":
        """Update after an accepted action. Any screen other than rewards, the map
        overlay (`idle`/`map`) or a grid opened from the rewards starts afresh."""
        if after is None or after.kind not in ("reward", "grid", "idle", "map"):
            return ClosedCardRewards()
        if before is None or before.kind != "reward":
            return self
        screen = before.screen
        queued = list(self.queued or (False,) * len(screen.queued_card_rewards))
        if len(queued) != len(screen.queued_card_rewards):
            raise ValueError("Untracked change of public queued card rewards")
        shown_slot, closed_cards = self.shown_slot, self.closed_cards
        if action.kind == "open_queued_card_reward":
            shown_slot = action.reward_slot
        elif action.kind == "open_card_reward" and not screen.cards:
            shown_slot = 0 if queued else None
        cards = after.screen.cards if after.kind == "reward" else screen.cards
        if screen.card_reward_flow == "active" and not cards:
            # Taken, bowled or skipped: the shown reward is gone (queued slots shift).
            if shown_slot is not None:
                del queued[shown_slot]
            shown_slot = None
        elif action.kind == "close_card_reward":
            if shown_slot is not None:
                queued[shown_slot] = True
            else:
                closed_cards = cards
        elif cards != screen.cards and action.kind not in _OPEN_CARD_REWARD:
            shown_slot = None  # e.g. Tiny House rolls new shown choices
        return ClosedCardRewards(tuple(queued), shown_slot, closed_cards)


def macro_eligibility(
    observation: Observation,
    actions: tuple[PolicyAction, ...],
    *,
    avoid_prismatic: bool,
    commit_navigation: bool = False,
    shop_finished: bool = False,
    direct_smith: bool = False,
    closed_card_rewards: ClosedCardRewards | None = None,
) -> tuple[bool, ...]:
    """Explicit policy commitments; native legality and candidate indices stay intact.

    Leaving a room commits to advancing. Leaving the merchant commits to finishing
    shopping in this room, even if purchases remain. Public map is already in context.
    Direct smith removes the campfire's upgrade-screen detour whenever the same
    upgrades are offered as direct campfire actions; no upgrade becomes unavailable.
    With `closed_card_rewards`, closing a card reward without taking a card is final
    for that reward; opens are only masked while another action remains.
    """
    direct_smith = direct_smith and any(a.kind == "rest_smith" for a in actions)
    closed = closed_card_rewards
    if closed is not None and all(a.kind in _OPEN_CARD_REWARD for a in actions):
        closed = None
    if (
        not direct_smith
        and not commit_navigation
        and closed is None
        and (not avoid_prismatic or len(actions) == 1)
    ):
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
        blocked_smith = direct_smith and action.kind == "rest_open_smith"
        blocked_reopen = closed is not None and closed.blocks(observation, action)
        result.append(
            not (blocked_shard or blocked_navigation or blocked_smith or blocked_reopen)
        )
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
