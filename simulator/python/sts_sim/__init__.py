"""Typed fair-policy exposure of the native Rust simulator."""

from __future__ import annotations

from dataclasses import dataclass

from . import _native, observations
from ._native import Action
from .observations import (
    FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION,
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
    OBSERVATION_TYPES,
    Card,
    CardDynamicValues,
    CardKey,
    CardSlot,
    CombatObservation,
    CombatScreen,
    CompleteObservation,
    Counter,
    CounterKey,
    DarkOrb,
    EventChoice,
    EventKey,
    EventObservation,
    EventScreen,
    FrostOrb,
    GridObservation,
    GridScreen,
    HiddenIntent,
    IdleObservation,
    LightningOrb,
    MapNode,
    MapObservation,
    MapScreen,
    MatchAndKeepCard,
    Monster,
    MonsterKey,
    NoneIntent,
    Observation,
    OrbSlot,
    Pile,
    Player,
    PotionKey,
    PotionSlot,
    Power,
    PowerKey,
    QueuedCardReward,
    Relic,
    RelicKey,
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
    RunContext,
    Selection,
    SelectionOption,
    ShopObservation,
    ShopOffer,
    ShopScreen,
    TreasureObservation,
    TreasureScreen,
    VisibleIntent,
    decode_observation,
)

__all__ = [
    "FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION",
    "FAIR_RUN_OBSERVATION_SCHEMA_VERSION",
    "OBSERVATION_TYPES",
    "Action",
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
    "Decision",
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
    "State",
    "TreasureObservation",
    "TreasureScreen",
    "VisibleIntent",
    "observations",
]


@dataclass(frozen=True, slots=True, kw_only=True)
class Decision:
    schema_version: int
    revision: int
    observation: Observation
    actions: tuple[Action, ...]


class State:
    """State-owning fair environment. Observations are concrete typed projections."""

    __slots__ = ("_native",)

    def __init__(self, native: _native.State) -> None:
        self._native = native

    @staticmethod
    def new(seed: str, ascension: int = 0) -> State:
        return State(_native.State.new(seed, ascension))

    def clone(self) -> State:
        return State(self._native.clone())

    @property
    def revision(self) -> int:
        return self._native.revision

    def observation(self) -> Observation:
        return decode_observation(self._native.observation()._to_mapping())

    def legal_actions(self) -> list[Action]:
        return self._native.legal_actions()

    def decision(self) -> Decision:
        return _project_decision(self._native.decision())

    def step(self, action: Action) -> Decision:
        return _project_decision(self._native.step(action))

    def __repr__(self) -> str:
        return f"State(revision={self.revision})"


def _project_decision(native: _native.Decision) -> Decision:
    return Decision(
        schema_version=native.schema_version,
        revision=native.revision,
        observation=decode_observation(native.observation._to_mapping()),
        actions=tuple(native.actions),
    )
