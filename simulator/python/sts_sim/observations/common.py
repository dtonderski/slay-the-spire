"""Shared fair observation types reused across screens."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, get_args

from ..content_ids import CardKey, CounterKey, PotionKey, RelicKey
from ._decode import (
    _bool,
    _enum,
    _exact,
    _field_names,
    _int,
    _literal,
    _mapping,
    _optional_int,
    _optional_present_enum,
    _seq,
)

FAIR_RUN_OBSERVATION_SCHEMA_VERSION = 8

RunOutcome = Literal["ongoing", "death", "act3_clear", "heart_clear", "unknown_complete"]

BossEncounter = Literal[
    "Hexaghost",
    "Slime Boss",
    "The Guardian",
    "Automaton",
    "Collector",
    "Champ",
    "Awakened One",
    "Time Eater",
    "Donu and Deca",
    "Corrupt Heart",
]

Phase = Literal["combat", "reward", "treasure", "rest", "event", "shop", "idle", "complete"]
ObservationKind = Literal[
    "combat", "map", "event", "reward", "treasure", "rest", "shop", "grid", "idle", "complete"
]


@dataclass(frozen=True, slots=True, kw_only=True)
class CardDynamicValues:
    misc_bonus: int | None = None
    base_damage_delta: int | None = None
    base_block_delta: int | None = None
    x_magic: int | None = None
    retain_once: bool = False
    rampage_damage_bonus: int | None = None
    ritual_dagger_damage_bonus: int | None = None
    windmill_retain_damage: int | None = None
    steam_barrier_block_reduction: int | None = None
    combat_cost_under_turn_override: int | None = None


@dataclass(frozen=True, slots=True, kw_only=True)
class Card:
    content_key: CardKey
    cost: int
    cost_is_modified: bool
    cost_resets_next_turn: bool
    upgrade_level: int
    bottled: bool
    temporary: bool
    dynamic: CardDynamicValues


@dataclass(frozen=True, slots=True, kw_only=True)
class CardSlot:
    slot: int
    card: Card


@dataclass(frozen=True, slots=True, kw_only=True)
class Counter:
    key: CounterKey
    value: int


@dataclass(frozen=True, slots=True, kw_only=True)
class Relic:
    slot: int
    content_key: RelicKey
    state: tuple[Counter, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class PotionSlot:
    slot: int
    content_key: PotionKey | None


@dataclass(frozen=True, slots=True, kw_only=True)
class RunKeys:
    ruby: bool
    emerald: bool
    sapphire: bool


@dataclass(frozen=True, slots=True, kw_only=True)
class RunContext:
    ascension: int
    outcome: RunOutcome
    act: int
    act_boss: BossEncounter | None
    final_act_available: bool
    keys: RunKeys
    floor: int
    gold: int
    player_hp: int
    player_max_hp: int
    deck: tuple[Card, ...]
    relics: tuple[Relic, ...]
    potion_slots: tuple[PotionSlot, ...]


def decode_run_context(value: object, path: str) -> RunContext:
    data = _exact(value, path, RunContext)
    return RunContext(
        ascension=_int(data["ascension"], f"{path}.ascension"),
        outcome=_literal(data["outcome"], f"{path}.outcome", get_args(RunOutcome)),
        act=_int(data["act"], f"{path}.act"),
        act_boss=(
            None
            if data["act_boss"] is None
            else _literal(data["act_boss"], f"{path}.act_boss", get_args(BossEncounter))
        ),
        final_act_available=_bool(data["final_act_available"], f"{path}.final_act_available"),
        keys=decode_run_keys(data["keys"], f"{path}.keys"),
        floor=_int(data["floor"], f"{path}.floor"),
        gold=_int(data["gold"], f"{path}.gold"),
        player_hp=_int(data["player_hp"], f"{path}.player_hp"),
        player_max_hp=_int(data["player_max_hp"], f"{path}.player_max_hp"),
        deck=_seq(data["deck"], f"{path}.deck", decode_card),
        relics=_seq(data["relics"], f"{path}.relics", decode_relic),
        potion_slots=_seq(data["potion_slots"], f"{path}.potion_slots", decode_potion_slot),
    )


def decode_run_keys(value: object, path: str) -> RunKeys:
    data = _exact(value, path, RunKeys)
    return RunKeys(
        ruby=_bool(data["ruby"], f"{path}.ruby"),
        emerald=_bool(data["emerald"], f"{path}.emerald"),
        sapphire=_bool(data["sapphire"], f"{path}.sapphire"),
    )


def decode_card(value: object, path: str) -> Card:
    data = _exact(value, path, Card)
    return Card(
        content_key=_enum(data["content_key"], f"{path}.content_key", CardKey),
        cost=_int(data["cost"], f"{path}.cost"),
        cost_is_modified=_bool(data["cost_is_modified"], f"{path}.cost_is_modified"),
        cost_resets_next_turn=_bool(data["cost_resets_next_turn"], f"{path}.cost_resets_next_turn"),
        upgrade_level=_int(data["upgrade_level"], f"{path}.upgrade_level"),
        bottled=_bool(data["bottled"], f"{path}.bottled"),
        temporary=_bool(data["temporary"], f"{path}.temporary"),
        dynamic=decode_card_dynamic(data["dynamic"], f"{path}.dynamic"),
    )


def decode_card_dynamic(value: object, path: str) -> CardDynamicValues:
    names = _field_names(CardDynamicValues)
    data = _mapping(value, path, required=frozenset(), optional=names)
    return CardDynamicValues(
        misc_bonus=_optional_int(data, "misc_bonus", path),
        base_damage_delta=_optional_int(data, "base_damage_delta", path),
        base_block_delta=_optional_int(data, "base_block_delta", path),
        x_magic=_optional_int(data, "x_magic", path),
        retain_once=_bool(data.get("retain_once", False), f"{path}.retain_once"),
        rampage_damage_bonus=_optional_int(data, "rampage_damage_bonus", path),
        ritual_dagger_damage_bonus=_optional_int(data, "ritual_dagger_damage_bonus", path),
        windmill_retain_damage=_optional_int(data, "windmill_retain_damage", path),
        steam_barrier_block_reduction=_optional_int(data, "steam_barrier_block_reduction", path),
        combat_cost_under_turn_override=_optional_int(
            data, "combat_cost_under_turn_override", path
        ),
    )


def decode_card_slot(value: object, path: str) -> CardSlot:
    data = _exact(value, path, CardSlot)
    return CardSlot(
        slot=_int(data["slot"], f"{path}.slot"), card=decode_card(data["card"], f"{path}.card")
    )


def decode_counter(value: object, path: str) -> Counter:
    data = _exact(value, path, Counter)
    return Counter(
        key=_enum(data["key"], f"{path}.key", CounterKey),
        value=_int(data["value"], f"{path}.value"),
    )


def decode_relic(value: object, path: str) -> Relic:
    data = _exact(value, path, Relic)
    return Relic(
        slot=_int(data["slot"], f"{path}.slot"),
        content_key=_enum(data["content_key"], f"{path}.content_key", RelicKey),
        state=_seq(data["state"], f"{path}.state", decode_counter),
    )


def decode_potion_slot(value: object, path: str) -> PotionSlot:
    data = _exact(value, path, PotionSlot)
    return PotionSlot(
        slot=_int(data["slot"], f"{path}.slot"),
        content_key=_optional_present_enum(data["content_key"], f"{path}.content_key", PotionKey),
    )


def decode_card_key(value: object, path: str) -> CardKey:
    return _enum(value, path, CardKey)


def decode_relic_key(value: object, path: str) -> RelicKey:
    return _enum(value, path, RelicKey)


def decode_potion_key(value: object, path: str) -> PotionKey:
    return _enum(value, path, PotionKey)
