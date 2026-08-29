"""Versioned terminal-only combat value targets."""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from typing import Protocol


class OutcomeLike(Protocol):
    @property
    def status(self) -> str: ...

    @property
    def terminal_hp(self) -> int: ...

    @property
    def terminal_max_hp(self) -> int: ...

    @property
    def max_hp_change(self) -> int: ...

    @property
    def gold_change(self) -> int: ...

    @property
    def potion_slots(self) -> tuple[str | None, ...]: ...


@dataclass(frozen=True, slots=True)
class CombatRewardConfig:
    """Bounded survival-dominant combat proxy.

    Status bands do not overlap: every win outranks every escape, and every
    escape outranks a loss. Resource terms only order outcomes within a band.
    """

    name: str = "combat_proxy_v1"
    version: int = 1
    win_base: float = 0.75
    escape_base: float = 0.25
    loss_value: float = -1.0
    hp_fraction_weight: float = 0.20
    max_hp_change_per_ten_weight: float = 0.01
    gold_change_per_hundred_weight: float = 0.01
    potion_weight: float = 0.01
    resource_clip: float = 0.20

    def __post_init__(self) -> None:
        values = asdict(self)
        if self.name != "combat_proxy_v1" or self.version != 1:
            raise ValueError("unsupported combat reward contract")
        numeric = [value for key, value in values.items() if key not in {"name", "version"}]
        if any(type(value) not in {int, float} for value in numeric):
            raise TypeError("reward coefficients must be numeric")
        if not all(float("-inf") < float(value) < float("inf") for value in numeric):
            raise ValueError("reward coefficients must be finite")
        if not 0.0 < self.resource_clip < 0.25:
            raise ValueError("resource clip must preserve disjoint status bands")
        if self.win_base - self.resource_clip <= self.escape_base + self.resource_clip:
            raise ValueError("win and escape reward bands overlap")
        if not -1.0 <= self.loss_value < self.escape_base - self.resource_clip:
            raise ValueError("loss reward must remain below escape")

    def to_dict(self) -> dict[str, object]:
        return asdict(self)

    @property
    def digest(self) -> str:
        payload = json.dumps(self.to_dict(), sort_keys=True, separators=(",", ":")).encode()
        return hashlib.sha256(payload).hexdigest()

    def value(self, outcome: OutcomeLike) -> float | None:
        if outcome.status == "truncated":
            return None
        if outcome.status == "lost":
            return self.loss_value
        if outcome.status not in {"won", "escaped"}:
            raise ValueError(f"unknown combat outcome: {outcome.status}")
        if outcome.terminal_max_hp <= 0:
            raise ValueError("terminal max HP must be positive")
        hp_fraction = outcome.terminal_hp / outcome.terminal_max_hp
        resource = (
            hp_fraction * self.hp_fraction_weight
            + (outcome.max_hp_change / 10.0) * self.max_hp_change_per_ten_weight
            + (outcome.gold_change / 100.0) * self.gold_change_per_hundred_weight
            + sum(slot is not None for slot in outcome.potion_slots) * self.potion_weight
        )
        resource = max(-self.resource_clip, min(self.resource_clip, resource))
        value = (self.win_base if outcome.status == "won" else self.escape_base) + resource
        return max(-1.0, min(1.0, value))


COMBAT_PROXY_V1 = CombatRewardConfig()
