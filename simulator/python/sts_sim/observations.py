"""Concrete immutable fair observations projected from native records."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass
from enum import StrEnum
from typing import Literal, get_args

from .content_ids import (
    CardKey,
    CounterKey,
    EventKey,
    MonsterKey,
    PotionKey,
    PowerKey,
    RelicKey,
)

FAIR_RUN_OBSERVATION_SCHEMA_VERSION = 4
FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION = 3

Phase = Literal["combat", "reward", "treasure", "rest", "event", "shop", "idle", "complete"]
ObservationKind = Literal[
    "combat", "map", "event", "reward", "treasure", "rest", "shop", "grid", "idle", "complete"
]
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
RoomKind = Literal["combat", "elite", "event", "rest", "shop", "treasure", "boss", "victory"]
ChestSize = Literal["small", "medium", "large"]
CardRewardFlow = Literal["none", "pending", "active"]
GridPurpose = Literal[
    "rest_smith",
    "rest_remove",
    "shop_remove",
    "event_remove",
    "event_obtain_card",
    "event_upgrade",
    "empty_cage",
    "neow_remove",
    "neow_upgrade",
    "bottle",
    "dollys_mirror",
    "calling_bell_curse",
    "pandoras_box",
    "astrolabe",
    "neow_transform",
    "event_transform",
    "bonfire_elementals",
    "designer_remove_and_upgrade",
]


@dataclass(frozen=True, slots=True, kw_only=True)
class CardDynamicValues:
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
class Power:
    key: PowerKey
    amount: int


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
class RunContext:
    ascension: int
    act: int
    floor: int
    gold: int
    player_hp: int
    player_max_hp: int
    deck: tuple[Card, ...]
    relics: tuple[Relic, ...]
    potion_slots: tuple[PotionSlot, ...]


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
class Pile:
    count: int
    cards: tuple[Card, ...]
    known_order: tuple[Card, ...]


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
class MapNode:
    slot: int
    act: int
    room_kind: RoomKind
    children: tuple[int, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class MapScreen:
    act: int
    floor: int
    current_node: int
    reachable_nodes: tuple[int, ...]
    nodes: tuple[MapNode, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class EventChoice:
    slot: int
    label: str


@dataclass(frozen=True, slots=True, kw_only=True)
class MatchAndKeepCard:
    content_key: CardKey | None
    revealed: bool
    matched: bool


@dataclass(frozen=True, slots=True, kw_only=True)
class EventScreen:
    event: EventKey
    choices: tuple[EventChoice, ...]
    match_and_keep: tuple[MatchAndKeepCard, ...] | None


@dataclass(frozen=True, slots=True, kw_only=True)
class QueuedCardReward:
    slot: int
    choice_count: int


@dataclass(frozen=True, slots=True, kw_only=True)
class RewardScreen:
    cards: tuple[CardSlot, ...]
    queued_card_rewards: tuple[QueuedCardReward, ...]
    gold_offer: int
    stolen_gold_offer: int
    potion_offer: PotionKey | None
    potion_offers: tuple[PotionKey, ...]
    relic_offer: RelicKey | None
    boss_relic_choices: tuple[RelicKey, ...]
    card_reward_flow: CardRewardFlow


@dataclass(frozen=True, slots=True, kw_only=True)
class TreasureScreen:
    chest_size: ChestSize
    opened: bool


@dataclass(frozen=True, slots=True, kw_only=True)
class RestHeal:
    kind: Literal["heal"]


@dataclass(frozen=True, slots=True, kw_only=True)
class RestOpenSmith:
    kind: Literal["open_smith"]


@dataclass(frozen=True, slots=True, kw_only=True)
class RestOpenRemove:
    kind: Literal["open_remove"]


@dataclass(frozen=True, slots=True, kw_only=True)
class RestSmith:
    kind: Literal["smith"]
    card_slot: int


@dataclass(frozen=True, slots=True, kw_only=True)
class RestRemoveCard:
    kind: Literal["remove_card"]
    card_slot: int


@dataclass(frozen=True, slots=True, kw_only=True)
class RestLift:
    kind: Literal["lift"]


@dataclass(frozen=True, slots=True, kw_only=True)
class RestDig:
    kind: Literal["dig"]


@dataclass(frozen=True, slots=True, kw_only=True)
class RestRecall:
    kind: Literal["recall"]


@dataclass(frozen=True, slots=True, kw_only=True)
class RestProceed:
    kind: Literal["proceed"]


RestOption = (
    RestHeal
    | RestOpenSmith
    | RestOpenRemove
    | RestSmith
    | RestRemoveCard
    | RestLift
    | RestDig
    | RestRecall
    | RestProceed
)


@dataclass(frozen=True, slots=True, kw_only=True)
class RestScreen:
    complete: bool
    options: tuple[RestOption, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class ShopOffer[IdentityT]:
    slot: int
    content_key: IdentityT
    price: int
    sold: bool


@dataclass(frozen=True, slots=True, kw_only=True)
class ShopScreen:
    merchant_open: bool
    remove_cost: int | None
    cards: tuple[ShopOffer[CardKey], ...]
    relics: tuple[ShopOffer[RelicKey], ...]
    potions: tuple[ShopOffer[PotionKey], ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class GridScreen:
    purpose: GridPurpose
    cards: tuple[CardSlot, ...]
    selected: int | None
    selected_indices: tuple[int, ...]


@dataclass(frozen=True, slots=True, kw_only=True)
class CombatObservation:
    schema_version: int
    phase: Phase
    kind: Literal["combat"]
    context: RunContext
    screen: CombatScreen


@dataclass(frozen=True, slots=True, kw_only=True)
class MapObservation:
    schema_version: int
    phase: Phase
    kind: Literal["map"]
    context: RunContext
    screen: MapScreen


@dataclass(frozen=True, slots=True, kw_only=True)
class EventObservation:
    schema_version: int
    phase: Phase
    kind: Literal["event"]
    context: RunContext
    screen: EventScreen


@dataclass(frozen=True, slots=True, kw_only=True)
class RewardObservation:
    schema_version: int
    phase: Phase
    kind: Literal["reward"]
    context: RunContext
    screen: RewardScreen


@dataclass(frozen=True, slots=True, kw_only=True)
class TreasureObservation:
    schema_version: int
    phase: Phase
    kind: Literal["treasure"]
    context: RunContext
    screen: TreasureScreen


@dataclass(frozen=True, slots=True, kw_only=True)
class RestObservation:
    schema_version: int
    phase: Phase
    kind: Literal["rest"]
    context: RunContext
    screen: RestScreen


@dataclass(frozen=True, slots=True, kw_only=True)
class ShopObservation:
    schema_version: int
    phase: Phase
    kind: Literal["shop"]
    context: RunContext
    screen: ShopScreen


@dataclass(frozen=True, slots=True, kw_only=True)
class GridObservation:
    schema_version: int
    phase: Phase
    kind: Literal["grid"]
    context: RunContext
    screen: GridScreen


@dataclass(frozen=True, slots=True, kw_only=True)
class IdleObservation:
    schema_version: int
    phase: Phase
    kind: Literal["idle"]
    context: RunContext
    screen: None


@dataclass(frozen=True, slots=True, kw_only=True)
class CompleteObservation:
    schema_version: int
    phase: Phase
    kind: Literal["complete"]
    context: RunContext
    screen: None


Observation = (
    CombatObservation
    | MapObservation
    | EventObservation
    | RewardObservation
    | TreasureObservation
    | RestObservation
    | ShopObservation
    | GridObservation
    | IdleObservation
    | CompleteObservation
)

OBSERVATION_TYPES: tuple[type[Observation], ...] = (
    CombatObservation,
    MapObservation,
    EventObservation,
    RewardObservation,
    TreasureObservation,
    RestObservation,
    ShopObservation,
    GridObservation,
    IdleObservation,
    CompleteObservation,
)
REST_OPTION_TYPES: tuple[type[RestOption], ...] = (
    RestHeal,
    RestOpenSmith,
    RestOpenRemove,
    RestSmith,
    RestRemoveCard,
    RestLift,
    RestDig,
    RestRecall,
    RestProceed,
)
ORB_TYPES: tuple[type[Orb], ...] = (LightningOrb, FrostOrb, DarkOrb)
INTENT_TYPES: tuple[type[MonsterIntent], ...] = (HiddenIntent, NoneIntent, VisibleIntent)


def decode_observation(value: object, path: str = "observation") -> Observation:
    """Strictly decode a native fair-run mapping into concrete observation types."""
    data = _mapping(
        value,
        path,
        required=frozenset({"schema_version", "phase", "kind", "context", "screen"}),
    )
    schema_version = _int(data["schema_version"], f"{path}.schema_version")
    if schema_version != FAIR_RUN_OBSERVATION_SCHEMA_VERSION:
        raise ValueError(f"{path}.schema_version: unsupported fair run schema {schema_version}")
    phase = _literal(data["phase"], f"{path}.phase", get_args(Phase))
    kind = _literal(data["kind"], f"{path}.kind", get_args(ObservationKind))
    context = decode_run_context(data["context"], f"{path}.context")
    screen_value = data["screen"]
    if kind == "idle":
        _none(screen_value, f"{path}.screen")
        return IdleObservation(
            schema_version=schema_version,
            phase=phase,
            kind="idle",
            context=context,
            screen=None,
        )
    if kind == "complete":
        _none(screen_value, f"{path}.screen")
        return CompleteObservation(
            schema_version=schema_version,
            phase=phase,
            kind="complete",
            context=context,
            screen=None,
        )
    if kind == "combat":
        return CombatObservation(
            schema_version=schema_version,
            phase=phase,
            kind="combat",
            context=context,
            screen=decode_combat_screen(screen_value, f"{path}.screen"),
        )
    if kind == "map":
        return MapObservation(
            schema_version=schema_version,
            phase=phase,
            kind="map",
            context=context,
            screen=decode_map_screen(screen_value, f"{path}.screen"),
        )
    if kind == "event":
        return EventObservation(
            schema_version=schema_version,
            phase=phase,
            kind="event",
            context=context,
            screen=decode_event_screen(screen_value, f"{path}.screen"),
        )
    if kind == "reward":
        return RewardObservation(
            schema_version=schema_version,
            phase=phase,
            kind="reward",
            context=context,
            screen=decode_reward_screen(screen_value, f"{path}.screen"),
        )
    if kind == "treasure":
        return TreasureObservation(
            schema_version=schema_version,
            phase=phase,
            kind="treasure",
            context=context,
            screen=decode_treasure_screen(screen_value, f"{path}.screen"),
        )
    if kind == "rest":
        return RestObservation(
            schema_version=schema_version,
            phase=phase,
            kind="rest",
            context=context,
            screen=decode_rest_screen(screen_value, f"{path}.screen"),
        )
    if kind == "shop":
        return ShopObservation(
            schema_version=schema_version,
            phase=phase,
            kind="shop",
            context=context,
            screen=decode_shop_screen(screen_value, f"{path}.screen"),
        )
    return GridObservation(
        schema_version=schema_version,
        phase=phase,
        kind="grid",
        context=context,
        screen=decode_grid_screen(screen_value, f"{path}.screen"),
    )


def decode_run_context(value: object, path: str) -> RunContext:
    data = _exact(value, path, RunContext)
    return RunContext(
        ascension=_int(data["ascension"], f"{path}.ascension"),
        act=_int(data["act"], f"{path}.act"),
        floor=_int(data["floor"], f"{path}.floor"),
        gold=_int(data["gold"], f"{path}.gold"),
        player_hp=_int(data["player_hp"], f"{path}.player_hp"),
        player_max_hp=_int(data["player_max_hp"], f"{path}.player_max_hp"),
        deck=_seq(data["deck"], f"{path}.deck", decode_card),
        relics=_seq(data["relics"], f"{path}.relics", decode_relic),
        potion_slots=_seq(data["potion_slots"], f"{path}.potion_slots", decode_potion_slot),
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
        slot=_int(data["slot"], f"{path}.slot"),
        card=decode_card(data["card"], f"{path}.card"),
    )


def decode_power(value: object, path: str) -> Power:
    data = _exact(value, path, Power)
    return Power(
        key=_enum(data["key"], f"{path}.key", PowerKey),
        amount=_int(data["amount"], f"{path}.amount"),
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


def decode_pile(value: object, path: str) -> Pile:
    data = _exact(value, path, Pile)
    return Pile(
        count=_int(data["count"], f"{path}.count"),
        cards=_seq(data["cards"], f"{path}.cards", decode_card),
        known_order=_seq(data["known_order"], f"{path}.known_order", decode_card),
    )


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


def decode_map_screen(value: object, path: str) -> MapScreen:
    data = _exact(value, path, MapScreen)
    return MapScreen(
        act=_int(data["act"], f"{path}.act"),
        floor=_int(data["floor"], f"{path}.floor"),
        current_node=_int(data["current_node"], f"{path}.current_node"),
        reachable_nodes=_seq(data["reachable_nodes"], f"{path}.reachable_nodes", _int),
        nodes=_seq(data["nodes"], f"{path}.nodes", decode_map_node),
    )


def decode_map_node(value: object, path: str) -> MapNode:
    data = _exact(value, path, MapNode)
    return MapNode(
        slot=_int(data["slot"], f"{path}.slot"),
        act=_int(data["act"], f"{path}.act"),
        room_kind=_literal(data["room_kind"], f"{path}.room_kind", get_args(RoomKind)),
        children=_seq(data["children"], f"{path}.children", _int),
    )


def decode_event_screen(value: object, path: str) -> EventScreen:
    data = _exact(value, path, EventScreen)
    return EventScreen(
        event=_enum(data["event"], f"{path}.event", EventKey),
        choices=_seq(data["choices"], f"{path}.choices", decode_event_choice),
        match_and_keep=_optional_seq(
            data["match_and_keep"], f"{path}.match_and_keep", decode_match_and_keep_card
        ),
    )


def decode_event_choice(value: object, path: str) -> EventChoice:
    data = _exact(value, path, EventChoice)
    return EventChoice(
        slot=_int(data["slot"], f"{path}.slot"),
        label=_str(data["label"], f"{path}.label"),
    )


def decode_match_and_keep_card(value: object, path: str) -> MatchAndKeepCard:
    data = _exact(value, path, MatchAndKeepCard)
    return MatchAndKeepCard(
        content_key=_optional_present_enum(data["content_key"], f"{path}.content_key", CardKey),
        revealed=_bool(data["revealed"], f"{path}.revealed"),
        matched=_bool(data["matched"], f"{path}.matched"),
    )


def decode_reward_screen(value: object, path: str) -> RewardScreen:
    data = _exact(value, path, RewardScreen)
    return RewardScreen(
        cards=_seq(data["cards"], f"{path}.cards", decode_card_slot),
        queued_card_rewards=_seq(
            data["queued_card_rewards"], f"{path}.queued_card_rewards", decode_queued_card_reward
        ),
        gold_offer=_int(data["gold_offer"], f"{path}.gold_offer"),
        stolen_gold_offer=_int(data["stolen_gold_offer"], f"{path}.stolen_gold_offer"),
        potion_offer=_optional_present_enum(
            data["potion_offer"], f"{path}.potion_offer", PotionKey
        ),
        potion_offers=_seq(data["potion_offers"], f"{path}.potion_offers", decode_potion_key),
        relic_offer=_optional_present_enum(data["relic_offer"], f"{path}.relic_offer", RelicKey),
        boss_relic_choices=_seq(
            data["boss_relic_choices"], f"{path}.boss_relic_choices", decode_relic_key
        ),
        card_reward_flow=_literal(
            data["card_reward_flow"], f"{path}.card_reward_flow", get_args(CardRewardFlow)
        ),
    )


def decode_queued_card_reward(value: object, path: str) -> QueuedCardReward:
    data = _exact(value, path, QueuedCardReward)
    return QueuedCardReward(
        slot=_int(data["slot"], f"{path}.slot"),
        choice_count=_int(data["choice_count"], f"{path}.choice_count"),
    )


def decode_treasure_screen(value: object, path: str) -> TreasureScreen:
    data = _exact(value, path, TreasureScreen)
    return TreasureScreen(
        chest_size=_literal(data["chest_size"], f"{path}.chest_size", get_args(ChestSize)),
        opened=_bool(data["opened"], f"{path}.opened"),
    )


def decode_rest_screen(value: object, path: str) -> RestScreen:
    data = _exact(value, path, RestScreen)
    return RestScreen(
        complete=_bool(data["complete"], f"{path}.complete"),
        options=_seq(data["options"], f"{path}.options", decode_rest_option),
    )


def decode_rest_option(value: object, path: str) -> RestOption:
    data = _mapping(
        value,
        path,
        required=frozenset({"kind"}),
        optional=frozenset({"card_slot"}),
    )
    kind = _str(data["kind"], f"{path}.kind")
    if kind == "heal":
        _mapping(data, path, required=frozenset({"kind"}))
        return RestHeal(kind="heal")
    if kind == "open_smith":
        _mapping(data, path, required=frozenset({"kind"}))
        return RestOpenSmith(kind="open_smith")
    if kind == "open_remove":
        _mapping(data, path, required=frozenset({"kind"}))
        return RestOpenRemove(kind="open_remove")
    if kind == "smith":
        _mapping(data, path, required=frozenset({"kind", "card_slot"}))
        return RestSmith(kind="smith", card_slot=_int(data["card_slot"], f"{path}.card_slot"))
    if kind == "remove_card":
        _mapping(data, path, required=frozenset({"kind", "card_slot"}))
        return RestRemoveCard(
            kind="remove_card", card_slot=_int(data["card_slot"], f"{path}.card_slot")
        )
    if kind == "lift":
        _mapping(data, path, required=frozenset({"kind"}))
        return RestLift(kind="lift")
    if kind == "dig":
        _mapping(data, path, required=frozenset({"kind"}))
        return RestDig(kind="dig")
    if kind == "recall":
        _mapping(data, path, required=frozenset({"kind"}))
        return RestRecall(kind="recall")
    if kind == "proceed":
        _mapping(data, path, required=frozenset({"kind"}))
        return RestProceed(kind="proceed")
    raise ValueError(f"{path}.kind: unknown rest option {kind!r}")


def decode_shop_screen(value: object, path: str) -> ShopScreen:
    data = _exact(value, path, ShopScreen)
    return ShopScreen(
        merchant_open=_bool(data["merchant_open"], f"{path}.merchant_open"),
        remove_cost=_optional(data["remove_cost"], f"{path}.remove_cost", _int),
        cards=_seq(data["cards"], f"{path}.cards", decode_shop_card_offer),
        relics=_seq(data["relics"], f"{path}.relics", decode_shop_relic_offer),
        potions=_seq(data["potions"], f"{path}.potions", decode_shop_potion_offer),
    )


def decode_shop_card_offer(value: object, path: str) -> ShopOffer[CardKey]:
    return decode_shop_offer(value, path, decode_card_key)


def decode_shop_relic_offer(value: object, path: str) -> ShopOffer[RelicKey]:
    return decode_shop_offer(value, path, decode_relic_key)


def decode_shop_potion_offer(value: object, path: str) -> ShopOffer[PotionKey]:
    return decode_shop_offer(value, path, decode_potion_key)


def decode_shop_offer[IdentityT](
    value: object, path: str, decode_key: Callable[[object, str], IdentityT]
) -> ShopOffer[IdentityT]:
    data = _exact(value, path, ShopOffer)
    return ShopOffer(
        slot=_int(data["slot"], f"{path}.slot"),
        content_key=decode_key(data["content_key"], f"{path}.content_key"),
        price=_int(data["price"], f"{path}.price"),
        sold=_bool(data["sold"], f"{path}.sold"),
    )


def decode_grid_screen(value: object, path: str) -> GridScreen:
    data = _exact(value, path, GridScreen)
    return GridScreen(
        purpose=_literal(data["purpose"], f"{path}.purpose", get_args(GridPurpose)),
        cards=_seq(data["cards"], f"{path}.cards", decode_card_slot),
        selected=_optional(data["selected"], f"{path}.selected", _int),
        selected_indices=_seq(data["selected_indices"], f"{path}.selected_indices", _int),
    )


def _field_names(cls: object) -> frozenset[str]:
    raw = getattr(cls, "__dataclass_fields__", None)
    if not isinstance(raw, dict):
        raise TypeError(f"{cls!r} is not a dataclass type")
    names: list[str] = []
    for key in raw:
        if not isinstance(key, str):
            raise TypeError(f"{cls!r} has a non-string dataclass field name")
        names.append(key)
    return frozenset(names)


def _exact(value: object, path: str, cls: type[object]) -> dict[str, object]:
    return _mapping(value, path, required=_field_names(cls))


def _mapping(
    value: object,
    path: str,
    *,
    required: frozenset[str],
    optional: frozenset[str] = frozenset(),
) -> dict[str, object]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{path}: expected mapping, got {type(value).__name__}")
    typed: dict[str, object] = {}
    for key, child in value.items():
        if not isinstance(key, str):
            raise TypeError(f"{path}: expected string keys, got {type(key).__name__}")
        typed[key] = child
    keys = frozenset(typed)
    missing = required - keys
    extra = keys - required - optional
    if missing or extra:
        raise ValueError(f"{path}: schema mismatch missing={sorted(missing)} extra={sorted(extra)}")
    return typed


def _seq[T](value: object, path: str, decoder: Callable[[object, str], T]) -> tuple[T, ...]:
    if not isinstance(value, (list, tuple)):
        raise TypeError(f"{path}: expected sequence, got {type(value).__name__}")
    return tuple(decoder(item, f"{path}[{index}]") for index, item in enumerate(value))


def _optional[T](value: object, path: str, decoder: Callable[[object, str], T]) -> T | None:
    if value is None:
        return None
    return decoder(value, path)


def _optional_seq[T](
    value: object, path: str, decoder: Callable[[object, str], T]
) -> tuple[T, ...] | None:
    if value is None:
        return None
    return _seq(value, path, decoder)


def _optional_int(data: Mapping[str, object], key: str, path: str) -> int | None:
    if key not in data or data[key] is None:
        return None
    return _int(data[key], f"{path}.{key}")


def decode_card_key(value: object, path: str) -> CardKey:
    return _enum(value, path, CardKey)


def decode_relic_key(value: object, path: str) -> RelicKey:
    return _enum(value, path, RelicKey)


def decode_potion_key(value: object, path: str) -> PotionKey:
    return _enum(value, path, PotionKey)


def _optional_present_enum[T: StrEnum](value: object, path: str, cls: type[T]) -> T | None:
    if value is None:
        return None
    return _enum(value, path, cls)


def _none(value: object, path: str) -> None:
    if value is not None:
        raise TypeError(f"{path}: expected None, got {type(value).__name__}")


def _literal[T](value: object, path: str, allowed: tuple[T, ...]) -> T:
    for candidate in allowed:
        if value == candidate:
            return candidate
    raise ValueError(f"{path}: expected one of {allowed}, got {value!r}")


def _enum[T: StrEnum](value: object, path: str, cls: type[T]) -> T:
    if not isinstance(value, str):
        raise TypeError(f"{path}: expected str, got {type(value).__name__}")
    try:
        return cls(value)
    except ValueError as error:
        raise ValueError(f"{path}: unknown {cls.__name__} {value!r}") from error


def _str(value: object, path: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{path}: expected str, got {type(value).__name__}")
    return value


def _bool(value: object, path: str) -> bool:
    if not isinstance(value, bool):
        raise TypeError(f"{path}: expected bool, got {type(value).__name__}")
    return value


def _int(value: object, path: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{path}: expected int, got {type(value).__name__}")
    return value


__all__ = [
    "FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION",
    "FAIR_RUN_OBSERVATION_SCHEMA_VERSION",
    "INTENT_TYPES",
    "OBSERVATION_TYPES",
    "ORB_TYPES",
    "REST_OPTION_TYPES",
    "Card",
    "CardDynamicValues",
    "CardKey",
    "CardSlot",
    "CombatObservation",
    "CombatScreen",
    "CompleteObservation",
    "Counter",
    "CounterKey",
    "DarkOrb",
    "EventChoice",
    "EventKey",
    "EventObservation",
    "EventScreen",
    "FrostOrb",
    "GridObservation",
    "GridScreen",
    "HiddenIntent",
    "IdleObservation",
    "LightningOrb",
    "MapNode",
    "MapObservation",
    "MapScreen",
    "MatchAndKeepCard",
    "Monster",
    "MonsterKey",
    "NoneIntent",
    "Observation",
    "OrbSlot",
    "Pile",
    "Player",
    "PotionKey",
    "PotionSlot",
    "Power",
    "PowerKey",
    "QueuedCardReward",
    "Relic",
    "RelicKey",
    "RestDig",
    "RestHeal",
    "RestLift",
    "RestObservation",
    "RestOpenRemove",
    "RestOpenSmith",
    "RestProceed",
    "RestRecall",
    "RestRemoveCard",
    "RestScreen",
    "RestSmith",
    "RewardObservation",
    "RewardScreen",
    "RunContext",
    "Selection",
    "SelectionOption",
    "ShopObservation",
    "ShopOffer",
    "ShopScreen",
    "TreasureObservation",
    "TreasureScreen",
    "VisibleIntent",
    "decode_observation",
]
