"""Canonical typed content choices for debug run mutations.

The native simulator owns the authoritative catalogues. These Python enums are
constructed from those catalogues so an enum member always carries the exact
public content key accepted by the Rust state transition.
"""

from __future__ import annotations

import json
import re
from collections.abc import Mapping
from dataclasses import dataclass
from enum import StrEnum
from types import MappingProxyType
from typing import Literal, cast

from . import _native

__all__ = [
    "CARD_CATALOGUE",
    "CARD_DEFINITIONS",
    "Card",
    "CardDefinition",
    "CardKeywords",
    "CardValues",
    "Potion",
    "Relic",
]


type CardType = Literal["attack", "skill", "power", "status"]
type CardRarity = Literal["common", "uncommon", "rare"]
type CardTarget = Literal["enemy", "all_enemies", "none"]


@dataclass(frozen=True, slots=True)
class CardValues:
    damage: int | None
    block: int | None
    vulnerable: int | None


@dataclass(frozen=True, slots=True)
class CardKeywords:
    innate: bool
    ethereal: bool
    exhaust: bool
    retain: bool
    unplayable: bool


@dataclass(frozen=True, slots=True)
class CardDefinition:
    content_key: str
    display_name: str
    printed_cost: int
    card_type: CardType
    rarity: CardRarity | None
    target: CardTarget
    values: CardValues
    keywords: CardKeywords
    is_curse: bool

    @classmethod
    def _from_payload(cls, payload: object) -> CardDefinition:
        record = cast(dict[str, object], payload)
        values = cast(dict[str, object], record["values"])
        keywords = cast(dict[str, object], record["keywords"])
        return cls(
            content_key=cast(str, record["content_key"]),
            display_name=cast(str, record["display_name"]),
            printed_cost=cast(int, record["printed_cost"]),
            card_type=cast(CardType, record["card_type"]),
            rarity=cast(CardRarity | None, record["rarity"]),
            target=cast(CardTarget, record["target"]),
            values=CardValues(
                damage=cast(int | None, values["damage"]),
                block=cast(int | None, values["block"]),
                vulnerable=cast(int | None, values["vulnerable"]),
            ),
            keywords=CardKeywords(
                innate=cast(bool, keywords["innate"]),
                ethereal=cast(bool, keywords["ethereal"]),
                exhaust=cast(bool, keywords["exhaust"]),
                retain=cast(bool, keywords["retain"]),
                unplayable=cast(bool, keywords["unplayable"]),
            ),
            is_curse=cast(bool, record["is_curse"]),
        )


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


_card_catalogue_payload = json.loads(_native.card_catalogue_json())
if not isinstance(_card_catalogue_payload, list):
    raise TypeError("native card catalogue must be an array")
CARD_CATALOGUE = tuple(
    CardDefinition._from_payload(record) for record in _card_catalogue_payload
)
CARD_DEFINITIONS: Mapping[str, CardDefinition] = MappingProxyType(
    {definition.content_key: definition for definition in CARD_CATALOGUE}
)

Card = _enum("Card", _native.card_keys())
Relic = _enum("Relic", _native.relic_names())
Potion = _enum(
    "Potion",
    _native.potion_names(),
    aliases={"GAMBLE": "GamblersBrew"},
)
