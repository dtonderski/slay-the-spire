"""Typed, visibility-safe fair observation objects."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import asdict, dataclass
from typing import Literal, cast, get_args

type DecisionRevision = int
type FairCombatPhase = Literal["waiting_for_player", "monster_turn", "won", "lost"]
type FairRunPhase = Literal[
    "combat", "reward", "treasure", "rest", "event", "shop", "idle", "complete"
]
type FairRunKind = Literal[
    "combat",
    "reward",
    "treasure",
    "rest",
    "event",
    "shop",
    "map",
    "idle",
    "complete",
    "grid",
]
type FairIntentCategory = Literal[
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
type FairSelectionKind = Literal[
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
type FairOrbKind = Literal["lightning", "frost", "dark"]
type FairSlimeSize = Literal["Small", "Medium", "Large"]
type FairIntentVisibility = Literal["hidden", "none", "visible"]
type PlayerChoiceKind = Literal[
    "play_hand_slot",
    "end_turn",
    "use_potion_slot",
    "discard_potion_slot",
    "toggle_visible_card",
    "choose_visible_option",
    "confirm_selection",
    "skip_selection",
    "proceed",
]

_COMBAT_PHASES = frozenset(get_args(FairCombatPhase.__value__))
_RUN_PHASES = frozenset(get_args(FairRunPhase.__value__))
_RUN_KINDS = frozenset(get_args(FairRunKind.__value__))
_IDLE_RUN_KINDS = frozenset({"map", "idle"})
_GRID_OVERLAY_PHASES = frozenset(
    {"combat", "reward", "treasure", "rest", "event", "shop", "idle"}
)
_INTENT_CATEGORIES = frozenset(get_args(FairIntentCategory.__value__))
_SELECTION_KINDS = frozenset(get_args(FairSelectionKind.__value__))
_ORB_KINDS = frozenset(get_args(FairOrbKind.__value__))
_SLIME_SIZES = frozenset(get_args(FairSlimeSize.__value__))
_INTENT_VISIBILITIES = frozenset(get_args(FairIntentVisibility.__value__))
_DYNAMIC_KEYS = frozenset(
    {
        "rampage_damage_bonus",
        "ritual_dagger_damage_bonus",
        "windmill_retain_damage",
        "steam_barrier_block_reduction",
        "combat_cost_under_turn_override",
    }
)
_COMBAT_CONTEXT_KEYS = frozenset({"ascension", "act", "floor", "gold"})
_RUN_CONTEXT_KEYS = frozenset(
    {
        "ascension",
        "act",
        "floor",
        "gold",
        "player_hp",
        "player_max_hp",
        "deck",
        "relics",
        "potion_slots",
    }
)
_COMBAT_OBSERVATION_KEYS = frozenset(
    {
        "schema_version",
        "context",
        "phase",
        "player",
        "orb_slots",
        "hand",
        "draw_pile",
        "discard_pile",
        "exhaust_pile",
        "monsters",
        "relics",
        "potion_slots",
        "selection",
        "public_counters",
    }
)
_RUN_OBSERVATION_KEYS = frozenset({"schema_version", "phase", "kind", "context", "screen"})


def _object(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        raise TypeError(f"{label} must be an object")
    source = cast(dict[str, object], value)
    if any(type(key) is not str for key in source):
        raise TypeError(f"{label} keys must be strings")
    return source


def _require_keys(
    source: Mapping[str, object],
    required: frozenset[str],
    label: str,
    *,
    optional: frozenset[str] = frozenset(),
) -> None:
    keys = frozenset(source)
    unknown = keys - required - optional
    if unknown:
        raise ValueError(f"{label} has unknown field {min(unknown)}")
    missing = required - keys
    if missing:
        raise ValueError(f"{label} is missing {min(missing)}")


def _require_int(value: object, label: str) -> int:
    if type(value) is not int or isinstance(value, bool):
        raise TypeError(f"{label} must be an integer")
    return value


def _require_bool(value: object, label: str) -> bool:
    if type(value) is not bool:
        raise TypeError(f"{label} must be boolean")
    return value


def _require_str(value: object, label: str) -> str:
    if type(value) is not str:
        raise TypeError(f"{label} must be a string")
    return value


def _require_enum(value: object, allowed: frozenset[str], label: str) -> str:
    text = _require_str(value, label)
    if text not in allowed:
        raise ValueError(f"{label} is invalid")
    return text


def _require_run_phase_kind(phase: object, kind: object) -> tuple[FairRunPhase, FairRunKind]:
    run_phase = cast(
        FairRunPhase, _require_enum(phase, _RUN_PHASES, "fair run observation phase")
    )
    run_kind = cast(FairRunKind, _require_enum(kind, _RUN_KINDS, "fair run observation kind"))
    if run_kind == "grid":
        if run_phase not in _GRID_OVERLAY_PHASES:
            raise ValueError("fair run observation grid overlay is invalid for this phase")
        return run_phase, run_kind
    if run_phase == "idle":
        if run_kind not in _IDLE_RUN_KINDS:
            raise ValueError("fair run idle phase only allows map or idle kind")
        return run_phase, run_kind
    if run_kind != run_phase:
        raise ValueError("fair run observation kind does not correspond to phase")
    return run_phase, run_kind


def _array(value: object, label: str) -> tuple[object, ...]:
    if type(value) is not list:
        raise TypeError(f"{label} must be an array")
    return tuple(value)


def _omitted_int(source: Mapping[str, object], key: str, label: str) -> int | None:
    if key not in source:
        return None
    value = source[key]
    if value is None:
        raise TypeError(f"{label} {key} must be omitted rather than null")
    return _require_int(value, f"{label} {key}")


def _omit_serializer_omitted_nulls(payload: object) -> object:
    """Drop only canonical serde-omitted optional nulls from an asdict payload."""

    if type(payload) is dict:
        source = cast(dict[str, object], payload)
        keys = frozenset(source)
        if keys <= _DYNAMIC_KEYS:
            return {
                key: _omit_serializer_omitted_nulls(value)
                for key, value in source.items()
                if value is not None
            }
        if keys <= {"type", "evoke"} and source.get("type") in _ORB_KINDS:
            kind = source["type"]
            if kind == "dark":
                return {
                    "type": "dark",
                    "evoke": _omit_serializer_omitted_nulls(source["evoke"]),
                }
            return {"type": kind}
        if keys <= {"visibility", "category", "damage", "hits"} and source.get(
            "visibility"
        ) in _INTENT_VISIBILITIES:
            visibility = source["visibility"]
            if visibility != "visible":
                return {"visibility": visibility}
            omitted: dict[str, object] = {
                "visibility": "visible",
                "category": source["category"],
            }
            damage = source.get("damage")
            hits = source.get("hits")
            if damage is not None:
                omitted["damage"] = damage
            if hits is not None:
                omitted["hits"] = hits
            return omitted
        return {key: _omit_serializer_omitted_nulls(value) for key, value in source.items()}
    if type(payload) is list or type(payload) is tuple:
        return [
            _omit_serializer_omitted_nulls(item)
            for item in cast(tuple[object, ...] | list[object], payload)
        ]
    return payload


@dataclass(frozen=True, slots=True)
class FairCardDynamicValues:
    rampage_damage_bonus: int | None
    ritual_dagger_damage_bonus: int | None
    windmill_retain_damage: int | None
    steam_barrier_block_reduction: int | None
    combat_cost_under_turn_override: int | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairCardDynamicValues:
        value = _object(payload, "fair card dynamic values")
        _require_keys(value, frozenset(), "fair card dynamic values", optional=_DYNAMIC_KEYS)
        return cls(
            rampage_damage_bonus=_omitted_int(value, "rampage_damage_bonus", "fair card dynamic"),
            ritual_dagger_damage_bonus=_omitted_int(
                value, "ritual_dagger_damage_bonus", "fair card dynamic"
            ),
            windmill_retain_damage=_omitted_int(value, "windmill_retain_damage", "fair card dynamic"),
            steam_barrier_block_reduction=_omitted_int(
                value, "steam_barrier_block_reduction", "fair card dynamic"
            ),
            combat_cost_under_turn_override=_omitted_int(
                value, "combat_cost_under_turn_override", "fair card dynamic"
            ),
        )


@dataclass(frozen=True, slots=True)
class FairCard:
    content_key: str
    cost: int
    cost_is_modified: bool
    cost_resets_next_turn: bool
    upgrade_level: int
    bottled: bool
    temporary: bool
    dynamic: FairCardDynamicValues

    @classmethod
    def _from_payload(cls, payload: object) -> FairCard:
        value = _object(payload, "fair card")
        _require_keys(
            value,
            frozenset(
                {
                    "content_key",
                    "cost",
                    "cost_is_modified",
                    "cost_resets_next_turn",
                    "upgrade_level",
                    "bottled",
                    "temporary",
                    "dynamic",
                }
            ),
            "fair card",
        )
        return cls(
            content_key=_require_str(value["content_key"], "fair card content_key"),
            cost=_require_int(value["cost"], "fair card cost"),
            cost_is_modified=_require_bool(value["cost_is_modified"], "fair card cost_is_modified"),
            cost_resets_next_turn=_require_bool(
                value["cost_resets_next_turn"], "fair card cost_resets_next_turn"
            ),
            upgrade_level=_require_int(value["upgrade_level"], "fair card upgrade_level"),
            bottled=_require_bool(value["bottled"], "fair card bottled"),
            temporary=_require_bool(value["temporary"], "fair card temporary"),
            dynamic=FairCardDynamicValues._from_payload(value["dynamic"]),
        )


@dataclass(frozen=True, slots=True)
class FairHandCard:
    slot: int
    card: FairCard

    @classmethod
    def _from_payload(cls, payload: object) -> FairHandCard:
        value = _object(payload, "fair hand card")
        _require_keys(value, frozenset({"slot", "card"}), "fair hand card")
        return cls(_require_int(value["slot"], "fair hand card slot"), FairCard._from_payload(value["card"]))


@dataclass(frozen=True, slots=True)
class FairPile:
    count: int
    cards: tuple[FairCard, ...]
    known_order: tuple[FairCard, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairPile:
        value = _object(payload, "fair pile")
        _require_keys(value, frozenset({"count", "cards", "known_order"}), "fair pile")
        return cls(
            count=_require_int(value["count"], "fair pile count"),
            cards=tuple(FairCard._from_payload(card) for card in _array(value["cards"], "fair pile cards")),
            known_order=tuple(
                FairCard._from_payload(card)
                for card in _array(value["known_order"], "fair pile known_order")
            ),
        )


@dataclass(frozen=True, slots=True)
class FairPower:
    key: str
    amount: int

    @classmethod
    def _from_payload(cls, payload: object) -> FairPower:
        value = _object(payload, "fair power")
        _require_keys(value, frozenset({"key", "amount"}), "fair power")
        return cls(
            _require_str(value["key"], "fair power key"),
            _require_int(value["amount"], "fair power amount"),
        )


@dataclass(frozen=True, slots=True)
class FairPlayer:
    hp: int
    max_hp: int
    block: int
    energy: int
    max_energy: int
    powers: tuple[FairPower, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairPlayer:
        value = _object(payload, "fair player")
        _require_keys(
            value,
            frozenset({"hp", "max_hp", "block", "energy", "max_energy", "powers"}),
            "fair player",
        )
        return cls(
            hp=_require_int(value["hp"], "fair player hp"),
            max_hp=_require_int(value["max_hp"], "fair player max_hp"),
            block=_require_int(value["block"], "fair player block"),
            energy=_require_int(value["energy"], "fair player energy"),
            max_energy=_require_int(value["max_energy"], "fair player max_energy"),
            powers=tuple(
                FairPower._from_payload(power) for power in _array(value["powers"], "fair player powers")
            ),
        )


@dataclass(frozen=True, slots=True)
class FairOrb:
    type: FairOrbKind
    evoke: int | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairOrb:
        value = _object(payload, "fair orb")
        if "type" not in value:
            raise ValueError("fair orb is missing type")
        kind = _require_enum(value["type"], _ORB_KINDS, "fair orb type")
        if kind == "dark":
            _require_keys(value, frozenset({"type", "evoke"}), "fair orb")
            return cls("dark", _require_int(value["evoke"], "fair orb evoke"))
        _require_keys(value, frozenset({"type"}), "fair orb")
        return cls(cast(FairOrbKind, kind), None)


@dataclass(frozen=True, slots=True)
class FairOrbSlot:
    slot: int
    orb: FairOrb | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairOrbSlot:
        value = _object(payload, "fair orb slot")
        _require_keys(value, frozenset({"slot", "orb"}), "fair orb slot")
        orb = value["orb"]
        return cls(
            slot=_require_int(value["slot"], "fair orb slot"),
            orb=None if orb is None else FairOrb._from_payload(orb),
        )


@dataclass(frozen=True, slots=True)
class FairMonsterIntent:
    visibility: FairIntentVisibility
    category: FairIntentCategory | None
    damage: int | None
    hits: int | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairMonsterIntent:
        value = _object(payload, "fair monster intent")
        if "visibility" not in value:
            raise ValueError("fair monster intent is missing visibility")
        visibility = cast(
            FairIntentVisibility,
            _require_enum(value["visibility"], _INTENT_VISIBILITIES, "fair monster intent visibility"),
        )
        if visibility != "visible":
            _require_keys(value, frozenset({"visibility"}), "fair monster intent")
            return cls(visibility, None, None, None)
        _require_keys(
            value,
            frozenset({"visibility", "category"}),
            "fair monster intent",
            optional=frozenset({"damage", "hits"}),
        )
        return cls(
            visibility,
            cast(
                FairIntentCategory,
                _require_enum(value["category"], _INTENT_CATEGORIES, "fair monster intent category"),
            ),
            _omitted_int(value, "damage", "fair monster intent"),
            _omitted_int(value, "hits", "fair monster intent"),
        )


@dataclass(frozen=True, slots=True)
class FairMonster:
    slot: int
    content_key: str
    slime_size: FairSlimeSize | None
    hp: int
    max_hp: int
    block: int
    powers: tuple[FairPower, ...]
    stolen_gold: int
    stasis_card: FairCard | None
    intent: FairMonsterIntent
    alive: bool
    escaped: bool
    minion: bool
    targetable: bool
    in_defensive_mode: bool

    @classmethod
    def _from_payload(cls, payload: object) -> FairMonster:
        value = _object(payload, "fair monster")
        _require_keys(
            value,
            frozenset(
                {
                    "slot",
                    "content_key",
                    "slime_size",
                    "hp",
                    "max_hp",
                    "block",
                    "powers",
                    "stolen_gold",
                    "stasis_card",
                    "intent",
                    "alive",
                    "escaped",
                    "minion",
                    "targetable",
                    "in_defensive_mode",
                }
            ),
            "fair monster",
        )
        slime = value["slime_size"]
        stasis = value["stasis_card"]
        return cls(
            slot=_require_int(value["slot"], "fair monster slot"),
            content_key=_require_str(value["content_key"], "fair monster content_key"),
            slime_size=None
            if slime is None
            else cast(FairSlimeSize, _require_enum(slime, _SLIME_SIZES, "fair monster slime_size")),
            hp=_require_int(value["hp"], "fair monster hp"),
            max_hp=_require_int(value["max_hp"], "fair monster max_hp"),
            block=_require_int(value["block"], "fair monster block"),
            powers=tuple(
                FairPower._from_payload(power) for power in _array(value["powers"], "fair monster powers")
            ),
            stolen_gold=_require_int(value["stolen_gold"], "fair monster stolen_gold"),
            stasis_card=None if stasis is None else FairCard._from_payload(stasis),
            intent=FairMonsterIntent._from_payload(value["intent"]),
            alive=_require_bool(value["alive"], "fair monster alive"),
            escaped=_require_bool(value["escaped"], "fair monster escaped"),
            minion=_require_bool(value["minion"], "fair monster minion"),
            targetable=_require_bool(value["targetable"], "fair monster targetable"),
            in_defensive_mode=_require_bool(value["in_defensive_mode"], "fair monster in_defensive_mode"),
        )


@dataclass(frozen=True, slots=True)
class FairCounter:
    key: str
    value: int

    @classmethod
    def _from_payload(cls, payload: object) -> FairCounter:
        value = _object(payload, "fair counter")
        _require_keys(value, frozenset({"key", "value"}), "fair counter")
        return cls(
            _require_str(value["key"], "fair counter key"),
            _require_int(value["value"], "fair counter value"),
        )


@dataclass(frozen=True, slots=True)
class FairRelic:
    slot: int
    content_key: str
    state: tuple[FairCounter, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairRelic:
        value = _object(payload, "fair relic")
        _require_keys(value, frozenset({"slot", "content_key", "state"}), "fair relic")
        return cls(
            slot=_require_int(value["slot"], "fair relic slot"),
            content_key=_require_str(value["content_key"], "fair relic content_key"),
            state=tuple(
                FairCounter._from_payload(counter)
                for counter in _array(value["state"], "fair relic state")
            ),
        )


@dataclass(frozen=True, slots=True)
class FairPotionSlot:
    slot: int
    content_key: str | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairPotionSlot:
        value = _object(payload, "fair potion slot")
        _require_keys(value, frozenset({"slot", "content_key"}), "fair potion slot")
        content_key = value["content_key"]
        return cls(
            _require_int(value["slot"], "fair potion slot"),
            None if content_key is None else _require_str(content_key, "fair potion content_key"),
        )


@dataclass(frozen=True, slots=True)
class FairSelectionOption:
    slot: int
    card: FairCard

    @classmethod
    def _from_payload(cls, payload: object) -> FairSelectionOption:
        value = _object(payload, "fair selection option")
        _require_keys(value, frozenset({"slot", "card"}), "fair selection option")
        return cls(
            _require_int(value["slot"], "fair selection option slot"),
            FairCard._from_payload(value["card"]),
        )


@dataclass(frozen=True, slots=True)
class FairSelection:
    kind: FairSelectionKind
    options: tuple[FairSelectionOption, ...]
    selected_slots: tuple[int, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairSelection:
        value = _object(payload, "fair selection")
        _require_keys(value, frozenset({"kind", "options", "selected_slots"}), "fair selection")
        return cls(
            kind=cast(
                FairSelectionKind,
                _require_enum(value["kind"], _SELECTION_KINDS, "fair selection kind"),
            ),
            options=tuple(
                FairSelectionOption._from_payload(option)
                for option in _array(value["options"], "fair selection options")
            ),
            selected_slots=tuple(
                _require_int(slot, "fair selection slot")
                for slot in _array(value["selected_slots"], "fair selection selected_slots")
            ),
        )


@dataclass(frozen=True, slots=True)
class FairCombatContext:
    """Exact combat-visible run coordinates: ascension, act, floor, and gold."""

    ascension: int
    act: int
    floor: int
    gold: int

    @classmethod
    def _from_payload(cls, payload: object) -> FairCombatContext:
        value = _object(payload, "fair combat context")
        _require_keys(value, _COMBAT_CONTEXT_KEYS, "fair combat context")
        return cls(
            ascension=_require_int(value["ascension"], "fair combat context ascension"),
            act=_require_int(value["act"], "fair combat context act"),
            floor=_require_int(value["floor"], "fair combat context floor"),
            gold=_require_int(value["gold"], "fair combat context gold"),
        )


@dataclass(frozen=True, slots=True)
class FairRunRelic:
    slot: int
    content_key: str

    @classmethod
    def _from_payload(cls, payload: object) -> FairRunRelic:
        value = _object(payload, "fair run relic")
        _require_keys(value, frozenset({"slot", "content_key"}), "fair run relic")
        return cls(
            _require_int(value["slot"], "fair run relic slot"),
            _require_str(value["content_key"], "fair run relic content_key"),
        )


@dataclass(frozen=True, slots=True)
class FairRunContext:
    """Exact full run-screen context for non-combat observations."""

    ascension: int
    act: int
    floor: int
    gold: int
    player_hp: int
    player_max_hp: int
    deck: tuple[FairCard, ...]
    relics: tuple[FairRunRelic, ...]
    potion_slots: tuple[FairPotionSlot, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairRunContext:
        value = _object(payload, "fair run context")
        _require_keys(value, _RUN_CONTEXT_KEYS, "fair run context")
        return cls(
            ascension=_require_int(value["ascension"], "fair run context ascension"),
            act=_require_int(value["act"], "fair run context act"),
            floor=_require_int(value["floor"], "fair run context floor"),
            gold=_require_int(value["gold"], "fair run context gold"),
            player_hp=_require_int(value["player_hp"], "fair run context player_hp"),
            player_max_hp=_require_int(value["player_max_hp"], "fair run context player_max_hp"),
            deck=tuple(
                FairCard._from_payload(card) for card in _array(value["deck"], "fair run context deck")
            ),
            relics=tuple(
                FairRunRelic._from_payload(relic)
                for relic in _array(value["relics"], "fair run context relics")
            ),
            potion_slots=tuple(
                FairPotionSlot._from_payload(slot)
                for slot in _array(value["potion_slots"], "fair run context potion_slots")
            ),
        )


@dataclass(frozen=True, slots=True)
class FairCombatObservation:
    schema_version: int
    context: FairCombatContext
    phase: FairCombatPhase
    player: FairPlayer
    orb_slots: tuple[FairOrbSlot, ...]
    hand: tuple[FairHandCard, ...]
    draw_pile: FairPile
    discard_pile: FairPile
    exhaust_pile: FairPile
    monsters: tuple[FairMonster, ...]
    relics: tuple[FairRelic, ...]
    potion_slots: tuple[FairPotionSlot, ...]
    selection: FairSelection | None
    public_counters: tuple[FairCounter, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairCombatObservation:
        value = _object(payload, "fair combat observation")
        _require_keys(value, _COMBAT_OBSERVATION_KEYS, "fair combat observation")
        schema_version = _require_int(value["schema_version"], "fair combat observation schema_version")
        if schema_version != 2:
            raise ValueError("unsupported fair combat observation schema")
        selection = value["selection"]
        return cls(
            schema_version=schema_version,
            context=FairCombatContext._from_payload(value["context"]),
            phase=cast(
                FairCombatPhase,
                _require_enum(value["phase"], _COMBAT_PHASES, "fair combat observation phase"),
            ),
            player=FairPlayer._from_payload(value["player"]),
            orb_slots=tuple(
                FairOrbSlot._from_payload(slot)
                for slot in _array(value["orb_slots"], "fair combat observation orb_slots")
            ),
            hand=tuple(
                FairHandCard._from_payload(card)
                for card in _array(value["hand"], "fair combat observation hand")
            ),
            draw_pile=FairPile._from_payload(value["draw_pile"]),
            discard_pile=FairPile._from_payload(value["discard_pile"]),
            exhaust_pile=FairPile._from_payload(value["exhaust_pile"]),
            monsters=tuple(
                FairMonster._from_payload(monster)
                for monster in _array(value["monsters"], "fair combat observation monsters")
            ),
            relics=tuple(
                FairRelic._from_payload(relic)
                for relic in _array(value["relics"], "fair combat observation relics")
            ),
            potion_slots=tuple(
                FairPotionSlot._from_payload(slot)
                for slot in _array(value["potion_slots"], "fair combat observation potion_slots")
            ),
            selection=None if selection is None else FairSelection._from_payload(selection),
            public_counters=tuple(
                FairCounter._from_payload(counter)
                for counter in _array(value["public_counters"], "fair combat observation public_counters")
            ),
        )

    def __str__(self) -> str:
        from .notebook import format_observation

        return format_observation(self)

    def to_wire_dict(self) -> dict[str, object]:
        return cast(dict[str, object], _omit_serializer_omitted_nulls(asdict(self)))


@dataclass(frozen=True, slots=True)
class FairRunObservation:
    """Visibility-safe observation for a non-combat run decision screen."""

    schema_version: int
    phase: FairRunPhase
    kind: FairRunKind
    context: FairRunContext
    screen: dict[str, object]

    def __post_init__(self) -> None:
        _require_run_phase_kind(self.phase, self.kind)

    @classmethod
    def _from_payload(cls, payload: object) -> FairRunObservation:
        value = _object(payload, "fair run observation")
        _require_keys(value, _RUN_OBSERVATION_KEYS, "fair run observation")
        schema_version = _require_int(value["schema_version"], "fair run observation schema_version")
        if schema_version != 1:
            raise ValueError("unsupported fair run observation schema")
        phase, kind = _require_run_phase_kind(value["phase"], value["kind"])
        return cls(
            schema_version=schema_version,
            phase=phase,
            kind=kind,
            context=FairRunContext._from_payload(value["context"]),
            screen=_object(value["screen"], "fair run observation screen"),
        )

    def __str__(self) -> str:
        from .notebook import format_observation

        return format_observation(self)
