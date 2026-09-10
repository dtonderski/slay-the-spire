"""Combat fair observation types and strict decoders."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, get_args

from ..content_ids import MonsterKey, PowerKey
from ._decode import (
    _bool,
    _enum,
    _exact,
    _int,
    _literal,
    _mapping,
    _optional,
    _optional_int,
    _seq,
    _str,
)
from .common import (
    Card,
    CardSlot,
    Counter,
    Phase,
    RunContext,
    decode_card,
    decode_card_slot,
    decode_counter,
)

FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION = 4

CombatPhase = Literal["waiting_for_player", "monster_turn", "won", "lost"]
SlimeSize = Literal["Small", "Medium", "Large"]
IntentCategory = Literal[
    "unknown",
    "attack",
    "attack_buff",
    "attack_debuff",
    "attack_defend",
    "buff",
    "debuff",
    "strong_debuff",
    "defend",
    "defend_buff",
    "escape",
    "sleep",
    "stun",
]
SelectionKind = Literal[
    "potion_attack_reward",
    "potion_skill_reward",
    "potion_power_reward",
    "potion_colorless_reward",
    "toolbox_reward",
    "discovery_reward",
    "warcry_put_on_draw",
    "armaments_upgrade",
    "forethought_put_on_draw",
    "forethought_put_any_on_draw",
    "thinking_ahead_put_on_draw",
    "prepared_discard",
    "dual_wield_copy",
    "secret_technique_skill_to_hand",
    "secret_weapon_attack_to_hand",
    "scry",
    "liquid_memories_return_to_hand",
    "headbutt_put_on_draw",
    "hologram_return_to_hand",
    "exhaust",
    "gambling_chip",
    "exhume_return_to_hand",
    "purity_exhaust_up_to_three",
    "burning_pact_draw_two",
    "burning_pact_draw_three",
    "true_grit_exhaust_one",
    "recycle_exhaust_one",
]


@dataclass(frozen=True, slots=True, kw_only=True)
class Power:
    key: PowerKey
    amount: int


@dataclass(frozen=True, slots=True, kw_only=True)
class Player:
    hp: int
    max_hp: int
    block: int
    energy: int
    max_energy: int
    powers: tuple[Power, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class LightningOrb:
    type: Literal["lightning"]


@dataclass(frozen=True, slots=True, kw_only=True)
class FrostOrb:
    type: Literal["frost"]


@dataclass(frozen=True, slots=True, kw_only=True)
class DarkOrb:
    type: Literal["dark"]
    evoke: int


Orb = LightningOrb | FrostOrb | DarkOrb


@dataclass(frozen=True, slots=True, kw_only=True)
class OrbSlot:
    slot: int
    orb: Orb | None


@dataclass(frozen=True, slots=True, kw_only=True)
class KnownPosition:
    position: int
    card: Card


@dataclass(frozen=True, slots=True, kw_only=True)
class Pile:
    cards: tuple[Card, ...]
    known_positions: tuple[KnownPosition, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class HiddenIntent:
    visibility: Literal["hidden"]


@dataclass(frozen=True, slots=True, kw_only=True)
class NoneIntent:
    visibility: Literal["none"]


@dataclass(frozen=True, slots=True, kw_only=True)
class VisibleIntent:
    visibility: Literal["visible"]
    category: IntentCategory
    damage: int | None = None
    hits: int | None = None


MonsterIntent = HiddenIntent | NoneIntent | VisibleIntent


@dataclass(frozen=True, slots=True, kw_only=True)
class Monster:
    slot: int
    content_key: MonsterKey
    slime_size: SlimeSize | None
    hp: int
    max_hp: int
    block: int
    powers: tuple[Power, ...]
    stolen_gold: int
    stasis_card: Card | None
    intent: MonsterIntent
    alive: bool
    escaped: bool
    minion: bool
    targetable: bool
    in_defensive_mode: bool


@dataclass(frozen=True, slots=True, kw_only=True)
class SelectionOption:
    slot: int
    card: Card


@dataclass(frozen=True, slots=True, kw_only=True)
class Selection:
    kind: SelectionKind
    options: tuple[SelectionOption, ...]
    selected_slots: tuple[int, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class CombatScreen:
    schema_version: int
    phase: CombatPhase
    player: Player
    orb_slots: tuple[OrbSlot, ...]
    hand: tuple[CardSlot, ...]
    draw_pile: Pile
    discard_pile: Pile
    exhaust_pile: Pile
    monsters: tuple[Monster, ...]
    selection: Selection | None
    public_counters: tuple[Counter, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class CombatObservation:
    schema_version: int
    phase: Phase
    kind: Literal["combat"]
    context: RunContext
    screen: CombatScreen


ORB_TYPES: tuple[type[Orb], ...] = (LightningOrb, FrostOrb, DarkOrb)
INTENT_TYPES: tuple[type[MonsterIntent], ...] = (HiddenIntent, NoneIntent, VisibleIntent)


def decode_combat_screen(value: object, path: str) -> CombatScreen:
    data = _exact(value, path, CombatScreen)
    schema_version = _int(data["schema_version"], f"{path}.schema_version")
    if schema_version != FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION:
        raise ValueError(f"{path}.schema_version: unsupported fair combat schema {schema_version}")
    return CombatScreen(
        schema_version=schema_version,
        phase=_literal(data["phase"], f"{path}.phase", get_args(CombatPhase)),
        player=decode_player(data["player"], f"{path}.player"),
        orb_slots=_seq(data["orb_slots"], f"{path}.orb_slots", decode_orb_slot),
        hand=_seq(data["hand"], f"{path}.hand", decode_card_slot),
        draw_pile=decode_pile(data["draw_pile"], f"{path}.draw_pile"),
        discard_pile=decode_pile(data["discard_pile"], f"{path}.discard_pile"),
        exhaust_pile=decode_pile(data["exhaust_pile"], f"{path}.exhaust_pile"),
        monsters=_seq(data["monsters"], f"{path}.monsters", decode_monster),
        selection=_optional(data["selection"], f"{path}.selection", decode_selection),
        public_counters=_seq(data["public_counters"], f"{path}.public_counters", decode_counter),
    )


def decode_player(value: object, path: str) -> Player:
    data = _exact(value, path, Player)
    return Player(
        hp=_int(data["hp"], f"{path}.hp"),
        max_hp=_int(data["max_hp"], f"{path}.max_hp"),
        block=_int(data["block"], f"{path}.block"),
        energy=_int(data["energy"], f"{path}.energy"),
        max_energy=_int(data["max_energy"], f"{path}.max_energy"),
        powers=_seq(data["powers"], f"{path}.powers", decode_power),
    )


def decode_orb_slot(value: object, path: str) -> OrbSlot:
    data = _exact(value, path, OrbSlot)
    return OrbSlot(
        slot=_int(data["slot"], f"{path}.slot"),
        orb=_optional(data["orb"], f"{path}.orb", decode_orb),
    )


def decode_orb(value: object, path: str) -> Orb:
    data = _mapping(value, path, required=frozenset({"type"}), optional=frozenset({"evoke"}))
    orb_type = _str(data["type"], f"{path}.type")
    if orb_type == "lightning":
        _mapping(data, path, required=frozenset({"type"}))
        return LightningOrb(type="lightning")
    if orb_type == "frost":
        _mapping(data, path, required=frozenset({"type"}))
        return FrostOrb(type="frost")
    if orb_type == "dark":
        _mapping(data, path, required=frozenset({"type", "evoke"}))
        return DarkOrb(type="dark", evoke=_int(data["evoke"], f"{path}.evoke"))
    raise ValueError(f"{path}.type: unknown orb type {orb_type!r}")


def decode_known_position(value: object, path: str) -> KnownPosition:
    data = _exact(value, path, KnownPosition)
    position = _int(data["position"], f"{path}.position")
    if position < 0:
        raise ValueError(f"{path}.position: expected >= 0, got {position}")
    return KnownPosition(position=position, card=decode_card(data["card"], f"{path}.card"))


def decode_pile(value: object, path: str) -> Pile:
    data = _exact(value, path, Pile)
    cards = _seq(data["cards"], f"{path}.cards", decode_card)
    known_positions = _seq(
        data["known_positions"], f"{path}.known_positions", decode_known_position
    )
    _validate_known_positions(known_positions, cards, f"{path}.known_positions")
    return Pile(cards=cards, known_positions=known_positions)


def _validate_known_positions(
    known_positions: tuple[KnownPosition, ...], cards: tuple[Card, ...], path: str
) -> None:
    positions = [entry.position for entry in known_positions]
    if positions != sorted(positions):
        raise ValueError(f"{path}: positions must be sorted ascending")
    if len(set(positions)) != len(positions):
        raise ValueError(f"{path}: positions must be unique")
    remaining = list(cards)
    for entry in known_positions:
        if entry.position >= len(cards):
            raise ValueError(
                f"{path}: position {entry.position} is out of bounds for {len(cards)} cards"
            )
        try:
            remaining.remove(entry.card)
        except ValueError as error:
            raise ValueError(f"{path}: known card is not a subset of pile membership") from error


def decode_monster(value: object, path: str) -> Monster:
    data = _exact(value, path, Monster)
    slime_size_value = data["slime_size"]
    slime_size = (
        None
        if slime_size_value is None
        else _literal(slime_size_value, f"{path}.slime_size", get_args(SlimeSize))
    )
    return Monster(
        slot=_int(data["slot"], f"{path}.slot"),
        content_key=_enum(data["content_key"], f"{path}.content_key", MonsterKey),
        slime_size=slime_size,
        hp=_int(data["hp"], f"{path}.hp"),
        max_hp=_int(data["max_hp"], f"{path}.max_hp"),
        block=_int(data["block"], f"{path}.block"),
        powers=_seq(data["powers"], f"{path}.powers", decode_power),
        stolen_gold=_int(data["stolen_gold"], f"{path}.stolen_gold"),
        stasis_card=_optional(data["stasis_card"], f"{path}.stasis_card", decode_card),
        intent=decode_intent(data["intent"], f"{path}.intent"),
        alive=_bool(data["alive"], f"{path}.alive"),
        escaped=_bool(data["escaped"], f"{path}.escaped"),
        minion=_bool(data["minion"], f"{path}.minion"),
        targetable=_bool(data["targetable"], f"{path}.targetable"),
        in_defensive_mode=_bool(data["in_defensive_mode"], f"{path}.in_defensive_mode"),
    )


def decode_intent(value: object, path: str) -> MonsterIntent:
    data = _mapping(
        value,
        path,
        required=frozenset({"visibility"}),
        optional=frozenset({"category", "damage", "hits"}),
    )
    visibility = _str(data["visibility"], f"{path}.visibility")
    if visibility == "hidden":
        _mapping(data, path, required=frozenset({"visibility"}))
        return HiddenIntent(visibility="hidden")
    if visibility == "none":
        _mapping(data, path, required=frozenset({"visibility"}))
        return NoneIntent(visibility="none")
    if visibility == "visible":
        visible = _mapping(
            data,
            path,
            required=frozenset({"visibility", "category"}),
            optional=frozenset({"damage", "hits"}),
        )
        return VisibleIntent(
            visibility="visible",
            category=_literal(visible["category"], f"{path}.category", get_args(IntentCategory)),
            damage=_optional_int(visible, "damage", path),
            hits=_optional_int(visible, "hits", path),
        )
    raise ValueError(f"{path}.visibility: unknown intent visibility {visibility!r}")


def decode_selection(value: object, path: str) -> Selection:
    data = _exact(value, path, Selection)
    return Selection(
        kind=_literal(data["kind"], f"{path}.kind", get_args(SelectionKind)),
        options=_seq(data["options"], f"{path}.options", decode_selection_option),
        selected_slots=_seq(data["selected_slots"], f"{path}.selected_slots", _int),
    )


def decode_selection_option(value: object, path: str) -> SelectionOption:
    data = _exact(value, path, SelectionOption)
    return SelectionOption(
        slot=_int(data["slot"], f"{path}.slot"),
        card=decode_card(data["card"], f"{path}.card"),
    )


def decode_power(value: object, path: str) -> Power:
    data = _exact(value, path, Power)
    return Power(
        key=_enum(data["key"], f"{path}.key", PowerKey),
        amount=_int(data["amount"], f"{path}.amount"),
    )
