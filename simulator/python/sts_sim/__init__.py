"""Typed fair-policy exposure of the native Rust simulator."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

from . import _native, observations
from ._native import Action
from .observations import (
    FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION,
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
    OBSERVATION_TYPES,
    BossEncounter,
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
    RelicOffer,
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
    RunKeys,
    RunOutcome,
    Selection,
    SelectionOption,
    ShopCardOffer,
    ShopObservation,
    ShopOffer,
    ShopScreen,
    TreasureObservation,
    TreasureScreen,
    VisibleIntent,
    decode_observation,
)
from .observations._decode import _bool, _enum, _int

__all__ = [
    "ACTION_KINDS",
    "FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION",
    "FAIR_RUN_OBSERVATION_SCHEMA_VERSION",
    "OBSERVATION_TYPES",
    "Action",
    "BossEncounter",
    "Card",
    "CardDynamicValues",
    "CardKey",
    "CardSlot",
    "CombatObservation",
    "CombatScreen",
    "CombatStep",
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
    "RelicOffer",
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
    "RunKeys",
    "RunOutcome",
    "Selection",
    "SelectionOption",
    "ShopCardOffer",
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


@dataclass(frozen=True, slots=True, kw_only=True)
class CombatStep:
    """Run-level public fields of an ongoing combat decision, from `State.advance`.

    It omits the typed combat screen and the deck, relics, potions and map of the
    run context, for drivers whose combat policy reads `numeric_decisions` tables.
    Values come from the same fair projection as `Decision`; `monsters` lists the
    public content key of every monster slot.
    """

    schema_version: int
    revision: int
    actions: tuple[Action, ...]
    act: int
    floor: int
    player_hp: int
    keys: RunKeys
    monsters: tuple[MonsterKey, ...]
    kind: Literal["combat"] = "combat"
    outcome: Literal["ongoing"] = "ongoing"


class State:
    """State-owning fair environment. Observations are concrete typed projections."""

    __slots__ = ("_native", "_previous")

    def __init__(self, native: _native.State) -> None:
        self._native = native
        # Last (payload, observation) decoded here; unchanged parts are shared.
        self._previous: tuple[object, Observation] | None = None

    @staticmethod
    def new(
        seed: str,
        ascension: int = 0,
        *,
        final_act: bool = False,
        training_rng_seed: int | None = None,
    ) -> State:
        """Natural starting state with optional initial Heart/training profiles.

        No keys are granted. ``training_rng_seed`` opts into a private libGDX
        environmental RNG for simulator-only training, not exact real-game replay.
        No profile can be changed mid-run and observations never expose RNG state.
        """
        return State(
            _native.State.new(
                seed, ascension, final_act=final_act, training_rng_seed=training_rng_seed
            )
        )

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

    def synthetic_rest_root(self, hp: int) -> State:
        """Independent synthetic unused A0 Act-1 campfire; current HP only.

        Preserves max HP, inventory, settled entry effects and RNG. Never an
        in-place game-state setter or replay repair; source remains unchanged.
        """
        return State(self._native.synthetic_rest_root(hp))

    def clone(self) -> State:
        return State(self._native.clone())

    @staticmethod
    def numeric_decisions(
        states: list[State],
    ) -> tuple[int, list[str], dict[str, tuple[int, bytes]], list[int]]:
        """Versioned raw public combat tables, without typed observation construction."""
        return _native.numeric_decisions([state._native for state in states])

    @staticmethod
    def numeric_steps(
        states: list[State], indices: list[int], revisions: list[int]
    ) -> tuple[int, list[str], dict[str, tuple[int, bytes]], list[int]]:
        """Step by index into each state's current public legal-action list.

        ``revisions`` are the revisions exported with those indices. A mismatch is
        rejected and does not apply the action. The batch is not atomic: every state is
        attempted (possibly on worker threads), the first error in input order is raised,
        and a state passed twice (or borrowed elsewhere) is rejected before any step.
        """
        if not (len(states) == len(indices) == len(revisions)):
            raise ValueError("State/action batch lengths differ")
        return _native.numeric_steps([state._native for state in states], indices, revisions)

    @property
    def revision(self) -> int:
        return self._native.revision

    def observation(self) -> Observation:
        return self._decode(self._native.observation())

    def player_hp(self) -> int:
        """Public context HP, identical to ``observation().context.player_hp``."""
        return self._native.player_hp()

    def legal_actions(self) -> list[Action]:
        return self._native.legal_actions()

    def decision(self) -> Decision:
        return self._project(self._native.decision())

    def step(self, action: Action) -> Decision:
        return self._project(self._native.step(action))

    def advance(self, action: Action) -> Decision | CombatStep:
        """`step`, except that an ongoing combat successor is a `CombatStep`.

        Skips typed projection where a driver only needs run-level fields; every
        other successor (including combat that ended the run) is a full `Decision`.
        """
        native = self._native.advance(action)
        if native[0] == "decision":
            _, schema_version, revision, actions, mapping = native
            return Decision(
                schema_version=schema_version,
                revision=revision,
                observation=self._decode_mapping(mapping),
                actions=tuple(actions),
            )
        (
            _,
            schema_version,
            revision,
            actions,
            observation_schema,
            act,
            floor,
            player_hp,
            (ruby, emerald, sapphire),
            monsters,
        ) = native
        if observation_schema != FAIR_RUN_OBSERVATION_SCHEMA_VERSION:
            raise ValueError(f"unsupported fair run schema {observation_schema}")
        return CombatStep(
            schema_version=schema_version,
            revision=revision,
            actions=tuple(actions),
            act=_int(act, "context.act"),
            floor=_int(floor, "context.floor"),
            player_hp=_int(player_hp, "context.player_hp"),
            keys=RunKeys(
                ruby=_bool(ruby, "context.keys.ruby"),
                emerald=_bool(emerald, "context.keys.emerald"),
                sapphire=_bool(sapphire, "context.keys.sapphire"),
            ),
            monsters=tuple(
                _enum(monster, "screen.monsters.content_key", MonsterKey) for monster in monsters
            ),
        )

    def __repr__(self) -> str:
        return f"State(revision={self.revision})"

    def _decode(self, native: _native.Observation) -> Observation:
        return self._decode_mapping(native._to_mapping())

    def _decode_mapping(self, mapping: object) -> Observation:
        observation = decode_observation(mapping, previous=self._previous)
        self._previous = (mapping, observation)
        return observation

    def _project(self, native: _native.Decision) -> Decision:
        return Decision(
            schema_version=native.schema_version,
            revision=native.revision,
            observation=self._decode(native.observation),
            actions=tuple(native.actions),
        )
