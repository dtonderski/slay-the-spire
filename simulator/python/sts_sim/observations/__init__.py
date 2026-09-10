"""Concrete immutable fair observations projected from native records."""

from __future__ import annotations

from typing import get_args

from ..content_ids import (
    CardKey,
    CounterKey,
    EventKey,
    MonsterKey,
    PotionKey,
    PowerKey,
    RelicKey,
)
from ._decode import _int, _literal, _mapping, _none
from .combat import (
    FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION,
    INTENT_TYPES,
    ORB_TYPES,
    CombatObservation,
    CombatScreen,
    DarkOrb,
    FrostOrb,
    HiddenIntent,
    KnownPosition,
    LightningOrb,
    Monster,
    NoneIntent,
    OrbSlot,
    Pile,
    Player,
    Power,
    Selection,
    SelectionOption,
    VisibleIntent,
    decode_combat_screen,
)
from .combat import CombatPhase as CombatPhase
from .combat import IntentCategory as IntentCategory
from .combat import MonsterIntent as MonsterIntent
from .combat import Orb as Orb
from .combat import SelectionKind as SelectionKind
from .combat import SlimeSize as SlimeSize
from .common import (
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
    Card,
    CardDynamicValues,
    CardSlot,
    Counter,
    ObservationKind,
    Phase,
    PotionSlot,
    Relic,
    RunContext,
    decode_run_context,
)
from .screens import (
    REST_OPTION_TYPES,
    CompleteObservation,
    EventChoice,
    EventObservation,
    EventScreen,
    GridObservation,
    GridScreen,
    IdleObservation,
    MapNode,
    MapObservation,
    MapScreen,
    MatchAndKeepCard,
    QueuedCardReward,
    RestDig,
    RestHeal,
    RestLift,
    RestObservation,
    RestOpenRemove,
    RestOpenSmith,
    RestProceed,
    RestRecall,
    RestRemoveCard,
    RestScreen,
    RestSmith,
    RewardObservation,
    RewardScreen,
    ShopObservation,
    ShopOffer,
    ShopScreen,
    TreasureObservation,
    TreasureScreen,
    decode_event_screen,
    decode_grid_screen,
    decode_map_screen,
    decode_rest_screen,
    decode_reward_screen,
    decode_shop_screen,
    decode_treasure_screen,
)
from .screens import CardRewardFlow as CardRewardFlow
from .screens import ChestSize as ChestSize
from .screens import GridPurpose as GridPurpose
from .screens import RestOption as RestOption
from .screens import RoomKind as RoomKind

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
    "KnownPosition",
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
