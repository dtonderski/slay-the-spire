"""Canonical typed content choices for debug run mutations.

The native simulator owns the authoritative catalogues. These Python enums are
constructed from those catalogues so an enum member always carries the exact
public content key accepted by the Rust state transition.
"""

from __future__ import annotations

import re
from enum import StrEnum

from . import _native

__all__ = ["Card", "Potion", "Relic"]


def _member_name(value: str) -> str:
    value = re.sub(r"\+$", "_PLUS", value)
    value = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", value)
    value = re.sub(r"[^A-Za-z0-9]+", "_", value).strip("_").upper()
    return value if value and not value[0].isdigit() else f"_{value}"


def _enum(name: str, values: list[str], *, aliases: dict[str, str] | None = None) -> type[StrEnum]:
    members: dict[str, str] = {_member_name(value): value for value in values}
    for alias, value in (aliases or {}).items():
        members[alias] = value
    return StrEnum(name, members, module=__name__)


Card = _enum("Card", _native.card_keys())
Relic = _enum("Relic", _native.relic_names())
Potion = _enum(
    "Potion",
    _native.potion_names(),
    aliases={"GAMBLE": "GamblersBrew"},
)
