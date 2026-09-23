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
    KnownPosition,
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
    "ACTION_KINDS",
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
    "State",
    "TreasureObservation",
    "TreasureScreen",
    "VisibleIntent",
    "observations",
]


ACTION_KINDS = tuple(_native.action_kind_vocabulary())


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

    @staticmethod
    def new_synthetic(
        seed: str, ascension: int = 0, hp: int = 10000, *, final_act: bool = False
    ) -> State:
        """Opt-in synthetic initial HP/max HP; not a real-game replay constructor."""
        return State(_native.State.new_synthetic(seed, ascension, hp, final_act))

    @staticmethod
    def from_synthetic_spec(spec_json: str) -> State:
        """Create an A0 combat from explicit JSON inputs, before combat-start effects."""
        return State(_native.State.from_synthetic_spec(spec_json))

    def synthetic_combat_root(self, hp: int = 100) -> State:
        """Independent HP/max-HP-normalized combat root; does not mutate this state."""
        return State(self._native.synthetic_combat_root(hp))

    def clone(self) -> State:
        return State(self._native.clone())

    @staticmethod
    def numeric_decisions(states: list[State]) -> tuple:
        """Versioned raw public combat tables, without typed observation construction."""
        return _native.numeric_decisions([state._native for state in states])

    @staticmethod
    def numeric_steps(states: list[State], indices: list[int], revisions: list[int]) -> tuple:
        """Step by index into each state's current public legal-action list.

        ``revisions`` are the revisions exported with those indices. A mismatch is
        rejected and does not apply the action. The batch is not atomic.
        """
        if not (len(states) == len(indices) == len(revisions)):
            raise ValueError("State/action batch lengths differ")
        return _native.numeric_steps([state._native for state in states], indices, revisions)

    @property
    def revision(self) -> int:
        return self._native.revision

    def observation(self) -> Observation:
        return decode_observation(self._native.observation()._to_mapping())

    def player_hp(self) -> int:
        """Public context HP, identical to ``observation().context.player_hp``."""
        return self._native.player_hp()

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
