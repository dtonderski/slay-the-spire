"""Unified state-owning Python environment and player-action API."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from enum import StrEnum
from typing import cast

from . import _native
from .fair import FairCombatObservation, FairRunObservation

type DecisionRevision = int
type Observation = FairCombatObservation | FairRunObservation


def _mapping(value: object) -> dict[str, object]:
    if not isinstance(value, dict):
        raise TypeError("run environment payload must be an object")
    return cast(dict[str, object], value)


@dataclass(frozen=True, slots=True)
class Action:
    """One legal player action bound to the decision that produced it."""

    revision: DecisionRevision
    family: str
    kind: str
    _handle: _native.Action = field(repr=False, compare=False, hash=False)
    hand_slot: int | None = None
    potion_slot: int | None = None
    option_slot: int | None = None
    target_slot: int | None = None
    card_slot: int | None = None
    node_slot: int | None = None
    reward_slot: int | None = None
    shop_slot: int | None = None
    slot: int | None = None

    def _display_text(self) -> str:
        """Return a compact, context-free description for terminals and notebooks.

        Observation-aware names (for example ``Choose Talk``) belong to the
        notebook formatter because resolving them requires the current
        ``Decision``.  The action itself still has enough information to show
        its kind, public slots, and revision without dumping every unset field.
        """

        slots = (
            ("hand", self.hand_slot),
            ("potion", self.potion_slot),
            ("option", self.option_slot),
            ("target", self.target_slot),
            ("card", self.card_slot),
            ("node", self.node_slot),
            ("reward", self.reward_slot),
            ("shop", self.shop_slot),
            ("slot", self.slot),
        )
        details = [f"{name}={value}" for name, value in slots if value is not None]
        details.append(f"revision={self.revision}")
        return f"{self.kind}({', '.join(details)})"

    def __str__(self) -> str:
        return self._display_text()

    def __repr__(self) -> str:
        # Keep lists/tuples of actions readable in a notebook as well as a
        # single action passed to print().  Retain the public type name so a
        # collection is still recognizably a collection of Action objects.
        return f"Action({self._display_text()})"

    @classmethod
    def _from_native(cls, native: _native.Action) -> Action:
        descriptor = _mapping(json.loads(native.public_action_json()))

        def optional_int(name: str) -> int | None:
            value = descriptor.get(name)
            return None if value is None else cast(int, value)

        return cls(
            revision=native.revision(),
            family=native.family(),
            kind=native.kind(),
            hand_slot=optional_int("hand_slot"),
            potion_slot=optional_int("potion_slot"),
            option_slot=optional_int("option_slot"),
            target_slot=optional_int("target_slot"),
            card_slot=optional_int("card_slot"),
            node_slot=optional_int("node_slot"),
            reward_slot=optional_int("reward_slot"),
            shop_slot=optional_int("shop_slot"),
            slot=optional_int("slot"),
            _handle=native,
        )


@dataclass(frozen=True, slots=True)
class Decision:
    """Atomic environment decision with one screen observation and action list."""

    revision: DecisionRevision
    phase: str
    kind: str
    observation: Observation
    actions: tuple[Action, ...]


@dataclass(frozen=True, slots=True)
class Snapshot:
    """Versioned serialized checkpoint suitable for exact restoration."""

    json: str
    hash: str


@dataclass(frozen=True, slots=True)
class StepResult:
    """State of the environment immediately after one accepted action."""

    terminal: bool
    decision: Decision


class RunEnv:
    """One authoritative run with fair, full-state, and snapshot projections."""

    def __init__(self, native: _native.OmniRunEnv) -> None:
        self._native = native

    @classmethod
    def combat_fixture(cls) -> RunEnv:
        return cls(_native.OmniRunEnv.combat_fixture())

    @classmethod
    def map_fixture(cls) -> RunEnv:
        return cls(_native.OmniRunEnv.map_fixture())

    @classmethod
    def new_ironclad(cls, seed: str, ascension: int = 0) -> RunEnv:
        return cls(_native.OmniRunEnv.new_ironclad(seed, ascension))

    @classmethod
    def from_snapshot(cls, snapshot: Snapshot | str) -> RunEnv:
        payload = snapshot.json if isinstance(snapshot, Snapshot) else snapshot
        return cls(_native.OmniRunEnv.from_snapshot_json(payload))

    @classmethod
    def from_state_json_for_debugging(cls, state_json: str) -> RunEnv:
        """Construct a validated privileged state for projection experiments."""

        return cls(_native.OmniRunEnv.from_state_json_for_debugging(state_json))

    def clone(self) -> RunEnv:
        return type(self)(self._native.clone())

    @property
    def revision(self) -> DecisionRevision:
        return self._native.revision()

    @property
    def phase(self) -> str:
        return self._native.phase()

    @property
    def current_decision_kind(self) -> str:
        return self._native.current_decision()

    def observation(self) -> Observation:
        """Return the visibility-safe observation for the current decision screen."""

        payload = _mapping(json.loads(self._native.observation_json()))
        if payload.get("kind") == "combat":
            return FairCombatObservation._from_payload(payload["screen"])
        return FairRunObservation._from_payload(payload)

    def add_card(self, card: StrEnum) -> None:
        """Debug utility: add one canonical card to the run deck.

        This is an explicit privileged mutation for experiments. It advances
        the decision revision, so actions obtained before the mutation become
        stale.
        """

        from .content import Card

        if not isinstance(card, Card):
            raise TypeError("card must be a sts_sim.Card member")
        self._native.debug_add_card(card.value)

    def add_relic(self, relic: StrEnum) -> None:
        """Debug utility: acquire one canonical relic and its modeled effects."""

        from .content import Relic

        if not isinstance(relic, Relic):
            raise TypeError("relic must be a sts_sim.Relic member")
        self._native.debug_add_relic(relic.value)

    def add_potion(self, potion: StrEnum) -> None:
        """Debug utility: add one canonical potion to an open potion slot."""

        from .content import Potion

        if not isinstance(potion, Potion):
            raise TypeError("potion must be a sts_sim.Potion member")
        self._native.debug_add_potion(potion.value)

    def legal_actions(self) -> tuple[Action, ...]:
        """Return the single legal player-action list for the current state."""

        return tuple(Action._from_native(action) for action in self._native.legal_actions())

    def decision(self) -> Decision:
        """Return the current action list together with its fair observation, if supported."""

        actions = self.legal_actions()
        revision = self.revision
        if any(action.revision != revision for action in actions):
            raise RuntimeError("environment revision changed while constructing a decision")
        observation = self.observation()
        return Decision(
            revision=revision,
            phase=self.phase,
            kind=self.current_decision_kind,
            observation=observation,
            actions=actions,
        )

    def step(self, action: Action) -> StepResult:
        """Apply one action returned by this environment or an identical clone."""

        self._native.step_action(action._handle)
        decision = self.decision()
        combat_terminal = (
            isinstance(decision.observation, FairCombatObservation)
            and decision.observation.phase in ("won", "lost")
        )
        return StepResult(
            terminal=decision.phase == "complete"
            or combat_terminal
            or not decision.actions,
            decision=decision,
        )

    def full_state(self) -> dict[str, object]:
        """Return the complete privileged simulator state as a detached dictionary."""

        return _mapping(json.loads(self._native.state_json()))

    def snapshot(self) -> Snapshot:
        """Return the versioned exact-restoration artifact for the current run."""

        return Snapshot(
            json=self._native.snapshot_json(),
            hash=self._native.snapshot_hash(),
        )

    def __repr__(self) -> str:
        return (
            f"RunEnv(phase={self.phase!r}, decision={self.current_decision_kind!r}, "
            f"revision={self.revision})"
        )
