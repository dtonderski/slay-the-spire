"""Typed, visibility-safe fair observation objects and legacy combat bindings."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Literal, cast

from . import _native

type DecisionRevision = int
type FairCombatPhase = Literal["waiting_for_player", "monster_turn", "won", "lost"]
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
type FairSelectionKind = str
type FairOrbKind = Literal["lightning", "frost", "dark"]
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


def _mapping(value: object) -> dict[str, object]:
    if not isinstance(value, dict):
        raise TypeError("fair API payload must be an object")
    return cast(dict[str, object], value)


def _items(value: object) -> tuple[object, ...]:
    if not isinstance(value, list):
        raise TypeError("fair API payload collection must be an array")
    return tuple(value)


@dataclass(frozen=True, slots=True)
class FairCardDynamicValues:
    rampage_damage_bonus: int | None
    ritual_dagger_damage_bonus: int | None
    windmill_retain_damage: int | None
    steam_barrier_block_reduction: int | None
    combat_cost_under_turn_override: int | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairCardDynamicValues:
        value = _mapping(payload)
        return cls(
            rampage_damage_bonus=cast(int | None, value.get("rampage_damage_bonus")),
            ritual_dagger_damage_bonus=cast(
                int | None, value.get("ritual_dagger_damage_bonus")
            ),
            windmill_retain_damage=cast(
                int | None, value.get("windmill_retain_damage")
            ),
            steam_barrier_block_reduction=cast(
                int | None, value.get("steam_barrier_block_reduction")
            ),
            combat_cost_under_turn_override=cast(
                int | None, value.get("combat_cost_under_turn_override")
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
        value = _mapping(payload)
        return cls(
            content_key=cast(str, value["content_key"]),
            cost=cast(int, value["cost"]),
            cost_is_modified=cast(bool, value["cost_is_modified"]),
            cost_resets_next_turn=cast(bool, value["cost_resets_next_turn"]),
            upgrade_level=cast(int, value["upgrade_level"]),
            bottled=cast(bool, value["bottled"]),
            temporary=cast(bool, value["temporary"]),
            dynamic=FairCardDynamicValues._from_payload(value["dynamic"]),
        )


@dataclass(frozen=True, slots=True)
class FairHandCard:
    slot: int
    card: FairCard

    @classmethod
    def _from_payload(cls, payload: object) -> FairHandCard:
        value = _mapping(payload)
        return cls(cast(int, value["slot"]), FairCard._from_payload(value["card"]))


@dataclass(frozen=True, slots=True)
class FairPile:
    count: int
    cards: tuple[FairCard, ...]
    known_order: tuple[FairCard, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairPile:
        value = _mapping(payload)
        return cls(
            count=cast(int, value["count"]),
            cards=tuple(FairCard._from_payload(card) for card in _items(value["cards"])),
            known_order=tuple(
                FairCard._from_payload(card) for card in _items(value["known_order"])
            ),
        )


@dataclass(frozen=True, slots=True)
class FairPower:
    key: str
    amount: int

    @classmethod
    def _from_payload(cls, payload: object) -> FairPower:
        value = _mapping(payload)
        return cls(cast(str, value["key"]), cast(int, value["amount"]))


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
        value = _mapping(payload)
        return cls(
            hp=cast(int, value["hp"]),
            max_hp=cast(int, value["max_hp"]),
            block=cast(int, value["block"]),
            energy=cast(int, value["energy"]),
            max_energy=cast(int, value["max_energy"]),
            powers=tuple(FairPower._from_payload(power) for power in _items(value["powers"])),
        )


@dataclass(frozen=True, slots=True)
class FairOrb:
    type: FairOrbKind
    evoke: int | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairOrb:
        value = _mapping(payload)
        return cls(
            type=cast(FairOrbKind, value["type"]),
            evoke=cast(int | None, value.get("evoke")),
        )


@dataclass(frozen=True, slots=True)
class FairOrbSlot:
    slot: int
    orb: FairOrb | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairOrbSlot:
        value = _mapping(payload)
        orb = value.get("orb")
        return cls(
            slot=cast(int, value["slot"]),
            orb=None if orb is None else FairOrb._from_payload(orb),
        )


@dataclass(frozen=True, slots=True)
class FairMonsterIntent:
    visibility: Literal["hidden", "none", "visible"]
    category: FairIntentCategory | None
    damage: int | None
    hits: int | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairMonsterIntent:
        value = _mapping(payload)
        visibility = cast(Literal["hidden", "none", "visible"], value["visibility"])
        return cls(
            visibility=visibility,
            category=cast(FairIntentCategory | None, value.get("category")),
            damage=cast(int | None, value.get("damage")),
            hits=cast(int | None, value.get("hits")),
        )


@dataclass(frozen=True, slots=True)
class FairMonster:
    slot: int
    content_key: str
    slime_size: str | None
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
        value = _mapping(payload)
        stasis = value.get("stasis_card")
        return cls(
            slot=cast(int, value["slot"]),
            content_key=cast(str, value["content_key"]),
            slime_size=cast(str | None, value.get("slime_size")),
            hp=cast(int, value["hp"]),
            max_hp=cast(int, value["max_hp"]),
            block=cast(int, value["block"]),
            powers=tuple(FairPower._from_payload(power) for power in _items(value["powers"])),
            stolen_gold=cast(int, value["stolen_gold"]),
            stasis_card=None if stasis is None else FairCard._from_payload(stasis),
            intent=FairMonsterIntent._from_payload(value["intent"]),
            alive=cast(bool, value["alive"]),
            escaped=cast(bool, value["escaped"]),
            minion=cast(bool, value["minion"]),
            targetable=cast(bool, value["targetable"]),
            in_defensive_mode=cast(bool, value["in_defensive_mode"]),
        )


@dataclass(frozen=True, slots=True)
class FairCounter:
    key: str
    value: int

    @classmethod
    def _from_payload(cls, payload: object) -> FairCounter:
        value = _mapping(payload)
        return cls(cast(str, value["key"]), cast(int, value["value"]))


@dataclass(frozen=True, slots=True)
class FairRelic:
    slot: int
    content_key: str
    state: tuple[FairCounter, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairRelic:
        value = _mapping(payload)
        return cls(
            slot=cast(int, value["slot"]),
            content_key=cast(str, value["content_key"]),
            state=tuple(FairCounter._from_payload(counter) for counter in _items(value["state"])),
        )


@dataclass(frozen=True, slots=True)
class FairPotionSlot:
    slot: int
    content_key: str | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairPotionSlot:
        value = _mapping(payload)
        return cls(cast(int, value["slot"]), cast(str | None, value.get("content_key")))


@dataclass(frozen=True, slots=True)
class FairSelectionOption:
    slot: int
    card: FairCard

    @classmethod
    def _from_payload(cls, payload: object) -> FairSelectionOption:
        value = _mapping(payload)
        return cls(cast(int, value["slot"]), FairCard._from_payload(value["card"]))


@dataclass(frozen=True, slots=True)
class FairSelection:
    kind: FairSelectionKind
    options: tuple[FairSelectionOption, ...]
    selected_slots: tuple[int, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairSelection:
        value = _mapping(payload)
        return cls(
            kind=cast(str, value["kind"]),
            options=tuple(
                FairSelectionOption._from_payload(option) for option in _items(value["options"])
            ),
            selected_slots=tuple(
                cast(int, slot) for slot in _items(value["selected_slots"])
            ),
        )


@dataclass(frozen=True, slots=True)
class FairContext:
    ascension: int
    act: int
    floor: int
    gold: int
    player_hp: int | None = None
    player_max_hp: int | None = None
    deck: tuple[FairCard, ...] = ()
    relics: tuple[str, ...] = ()
    potion_slots: tuple[FairPotionSlot, ...] = ()

    @classmethod
    def _from_payload(cls, payload: object) -> FairContext:
        value = _mapping(payload)
        relic_payload = value.get("relics", [])
        return cls(
            ascension=cast(int, value["ascension"]),
            act=cast(int, value["act"]),
            floor=cast(int, value["floor"]),
            gold=cast(int, value["gold"]),
            player_hp=cast(int | None, value.get("player_hp")),
            player_max_hp=cast(int | None, value.get("player_max_hp")),
            deck=tuple(
                FairCard._from_payload(card) for card in _items(value.get("deck", []))
            ),
            relics=tuple(
                cast(str, _mapping(relic)["content_key"])
                for relic in _items(relic_payload)
            ),
            potion_slots=tuple(
                FairPotionSlot._from_payload(slot)
                for slot in _items(value.get("potion_slots", []))
            ),
        )


@dataclass(frozen=True, slots=True)
class FairCombatObservation:
    schema_version: int
    context: FairContext
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
        value = _mapping(payload)
        schema_version = cast(int, value["schema_version"])
        if schema_version != 2:
            raise ValueError("unsupported fair combat observation schema")
        if "orb_slots" not in value:
            raise ValueError("fair combat observation is missing orb_slots")
        selection = value.get("selection")
        return cls(
            schema_version=schema_version,
            context=FairContext._from_payload(value["context"]),
            phase=cast(FairCombatPhase, value["phase"]),
            player=FairPlayer._from_payload(value["player"]),
            orb_slots=tuple(
                FairOrbSlot._from_payload(slot)
                for slot in _items(value["orb_slots"])
            ),
            hand=tuple(FairHandCard._from_payload(card) for card in _items(value["hand"])),
            draw_pile=FairPile._from_payload(value["draw_pile"]),
            discard_pile=FairPile._from_payload(value["discard_pile"]),
            exhaust_pile=FairPile._from_payload(value["exhaust_pile"]),
            monsters=tuple(
                FairMonster._from_payload(monster) for monster in _items(value["monsters"])
            ),
            relics=tuple(FairRelic._from_payload(relic) for relic in _items(value["relics"])),
            potion_slots=tuple(
                FairPotionSlot._from_payload(slot) for slot in _items(value["potion_slots"])
            ),
            selection=None if selection is None else FairSelection._from_payload(selection),
            public_counters=tuple(
                FairCounter._from_payload(counter)
                for counter in _items(value["public_counters"])
            ),
        )

    def __str__(self) -> str:
        from .notebook import format_observation

        return format_observation(self)


@dataclass(frozen=True, slots=True)
class FairRunObservation:
    """Visibility-safe observation for a non-combat run decision screen."""

    schema_version: int
    phase: str
    kind: str
    context: FairContext
    screen: dict[str, object]

    @classmethod
    def _from_payload(cls, payload: object) -> FairRunObservation:
        value = _mapping(payload)
        return cls(
            schema_version=cast(int, value["schema_version"]),
            phase=cast(str, value["phase"]),
            kind=cast(str, value["kind"]),
            context=FairContext._from_payload(value["context"]),
            screen=_mapping(value["screen"]),
        )

    def __str__(self) -> str:
        from .notebook import format_observation

        return format_observation(self)


@dataclass(frozen=True, slots=True)
class PlayerChoice:
    kind: PlayerChoiceKind
    hand_slot: int | None = None
    potion_slot: int | None = None
    option_slot: int | None = None
    target_slot: int | None = None

    @classmethod
    def _from_payload(cls, payload: object) -> PlayerChoice:
        value = _mapping(payload)
        return cls(
            kind=cast(PlayerChoiceKind, value["kind"]),
            hand_slot=cast(int | None, value.get("hand_slot")),
            potion_slot=cast(int | None, value.get("potion_slot")),
            option_slot=cast(int | None, value.get("option_slot")),
            target_slot=cast(int | None, value.get("target_slot")),
        )

    @classmethod
    def play_hand_slot(cls, hand_slot: int, target_slot: int | None = None) -> PlayerChoice:
        return cls("play_hand_slot", hand_slot=hand_slot, target_slot=target_slot)

    @classmethod
    def end_turn(cls) -> PlayerChoice:
        return cls("end_turn")

    @classmethod
    def use_potion_slot(cls, potion_slot: int, target_slot: int | None = None) -> PlayerChoice:
        return cls("use_potion_slot", potion_slot=potion_slot, target_slot=target_slot)

    @classmethod
    def discard_potion_slot(cls, potion_slot: int) -> PlayerChoice:
        return cls("discard_potion_slot", potion_slot=potion_slot)

    @classmethod
    def toggle_visible_card(cls, option_slot: int) -> PlayerChoice:
        return cls("toggle_visible_card", option_slot=option_slot)

    @classmethod
    def choose_visible_option(cls, option_slot: int) -> PlayerChoice:
        return cls("choose_visible_option", option_slot=option_slot)

    @classmethod
    def confirm_selection(cls) -> PlayerChoice:
        return cls("confirm_selection")

    @classmethod
    def skip_selection(cls) -> PlayerChoice:
        return cls("skip_selection")

    def _to_payload(self) -> dict[str, object]:
        payload: dict[str, object] = {"kind": self.kind}
        for name in ("hand_slot", "potion_slot", "option_slot", "target_slot"):
            value = getattr(self, name)
            if value is not None:
                payload[name] = value
        return payload


@dataclass(frozen=True, slots=True)
class PlayerChoiceRequest:
    decision_revision: DecisionRevision
    choice: PlayerChoice

    def _to_payload(self) -> dict[str, object]:
        return {
            "decision_revision": self.decision_revision,
            "choice": self.choice._to_payload(),
        }


@dataclass(frozen=True, slots=True)
class FairDecision:
    schema_version: int
    decision_revision: DecisionRevision
    observation: FairCombatObservation
    choices: tuple[PlayerChoice, ...]

    @classmethod
    def _from_payload(cls, payload: object) -> FairDecision:
        value = _mapping(payload)
        return cls(
            schema_version=cast(int, value["schema_version"]),
            decision_revision=cast(int, value["decision_revision"]),
            observation=FairCombatObservation._from_payload(value["observation"]),
            choices=tuple(
                PlayerChoice._from_payload(choice) for choice in _items(value["choices"])
            ),
        )


@dataclass(frozen=True, slots=True)
class FairStepResult:
    terminal: bool
    decision: FairDecision | None

    @classmethod
    def _from_payload(cls, payload: object) -> FairStepResult:
        value = _mapping(payload)
        decision = value.get("decision")
        return cls(
            terminal=cast(bool, value["terminal"]),
            decision=None if decision is None else FairDecision._from_payload(decision),
        )


class FairCombatEnv:
    """State-owning fair combat environment with no hidden-state accessors."""

    def __init__(self, native: _native.FairCombatEnv) -> None:
        self._native = native

    @classmethod
    def combat_fixture(cls) -> FairCombatEnv:
        return cls(_native.FairCombatEnv.combat_fixture())

    @classmethod
    def from_snapshot_for_testing(cls, snapshot_json: str) -> FairCombatEnv:
        return cls(_native.FairCombatEnv.from_snapshot_for_testing(snapshot_json))

    def clone(self) -> FairCombatEnv:
        return type(self)(self._native.clone())

    def decision(self) -> FairDecision:
        payload = json.loads(self._native.decision_json())
        return FairDecision._from_payload(payload)

    def step(self, request: PlayerChoiceRequest) -> FairStepResult:
        payload = json.loads(self._native.step_json(json.dumps(request._to_payload())))
        return FairStepResult._from_payload(payload)
