"""Non-combat fair observation screens and strict decoders."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Literal, get_args

from ..content_ids import CardKey, EventKey, PotionKey, RelicKey
from ._decode import (
    _bool,
    _enum,
    _exact,
    _int,
    _literal,
    _mapping,
    _optional,
    _optional_present_enum,
    _optional_seq,
    _seq,
    _str,
)
from .common import (
    CardSlot,
    Phase,
    RunContext,
    decode_card_key,
    decode_card_slot,
    decode_potion_key,
    decode_relic_key,
)

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
