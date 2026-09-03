"""Strict symbolic combat records tensorized only when loaded."""

from __future__ import annotations

import json
import math
import types
from collections.abc import Iterable, Iterator, Mapping, Sequence
from dataclasses import asdict, dataclass, fields, is_dataclass
from pathlib import Path
from types import MappingProxyType
from typing import Any, Literal, cast, get_args, get_origin, get_type_hints

import torch

from ..fair import FairCombatObservation
from ..run import ActionDescriptor
from .experiment import write_scientific_artifact
from .provenance import RepositoryVersion, canonical_bytes, read_regular_file_bytes, sha256_bytes
from .tensor import (
    BatchedCombatDecision,
    TensorizedCombatDecision,
    Vocabularies,
    collate_combat_tensors,
    tensorize_combat,
)

OutcomeStatus = Literal["won", "lost", "escaped", "truncated"]
CounterOwnerKind = Literal["card", "relic"]
type JsonValue = (
    None | bool | int | float | str | tuple["JsonValue", ...] | Mapping[str, "JsonValue"]
)

RECORD_VERSION = 4
_RECORD_KEYS = {
    "observation",
    "actions",
    "chosen_action_index",
    "chosen_action",
    "teacher_visit_counts",
    "target_value",
    "value_target_name",
    "outcome",
    "planner_name",
    "planner_version",
    "search_config",
    "root_id",
    "split_group_id",
    "teacher_pair_id",
    "repository",
    "observation_digest",
    "record_version",
    "root_manifest_digest",
    "reward_config_digest",
    "source_kind",
    "episode_id",
    "decision_index",
    "value_target_mask",
    "record_id",
    "search_root_mean_value",
}
_ACTION_FIELDS = set(ActionDescriptor.__dataclass_fields__)
_BEAM_SEARCH_CONFIG_KEYS = {
    "depth",
    "width",
    "transition_budget",
    "max_decisions",
    "max_player_turns",
    "deadline",
    "replan",
    "deduplicate_search_states",
}
_PUCT_SEARCH_CONFIG_KEYS = {
    "c_puct",
    "simulation_budget",
    "transition_budget",
    "max_decisions",
    "max_player_turns",
    "deadline",
    "replan",
    "privileged",
    "leaf_schema",
    "value_target_name",
    "search_root_mean_name",
    "checkpoint_file_digest",
    "checkpoint_model_state_digest",
    "checkpoint_config_digest",
    "source_digest",
    "runtime_identity_digest",
    "vocabulary_fingerprint",
    "encoder_contract_digest",
}
PUCT_SEARCH_ROOT_MEAN_NAME = "privileged_puct_root_mean_v1"
COMBAT_PROXY_VALUE_TARGET_NAME = "combat_proxy_v1"
BEAM_TEACHER_NAME = "public_decision_replanning_beam"
PUCT_TEACHER_NAME = "privileged_puct"
PUCT_TEACHER_VERSION = "synchronous_batch1_v3"
FAIR_LEAF_BATCH_SCHEMA = "fair_leaf_batch_v1"
_COMBAT_ACTION_SLOTS: dict[str, tuple[set[str], set[str]]] = {
    "play_hand_slot": ({"hand_slot"}, {"target_slot"}),
    "end_turn": (set(), set()),
    "use_potion_slot": ({"potion_slot"}, {"target_slot"}),
    "discard_potion_slot": ({"potion_slot"}, set()),
    "toggle_visible_card": ({"option_slot"}, set()),
    "choose_visible_option": ({"option_slot"}, set()),
    "confirm_selection": (set(), set()),
    "skip_selection": (set(), set()),
}
_OUTCOME_KEYS = {
    "status",
    "terminal_hp",
    "terminal_max_hp",
    "hp_change",
    "max_hp_change",
    "gold_change",
    "potion_slots",
    "counter_changes",
    "terminal",
    "truncated",
    "accepted_decisions",
    "player_turns",
    "truncation_trigger",
}
_COUNTER_CHANGE_KEYS = {"owner_kind", "owner_key", "counter_key", "before", "after"}


def _dict(value: object, label: str, expected_keys: set[str] | None = None) -> dict[str, object]:
    if type(value) is not dict:
        raise TypeError(f"{label} must be an object")
    result = cast(dict[str, object], value)
    if any(type(key) is not str for key in result):
        raise TypeError(f"{label} keys must be strings")
    if expected_keys is not None and set(result) != expected_keys:
        raise ValueError(f"{label} has missing or unknown fields")
    return result


def _list(value: object, label: str) -> list[object]:
    if type(value) is not list:
        raise TypeError(f"{label} must be an array")
    return cast(list[object], value)


def _string(value: object, label: str, *, optional: bool = False) -> str | None:
    if value is None and optional:
        return None
    if type(value) is not str or not value:
        raise TypeError(f"{label} must be a nonempty string")
    return cast(str, value)


def _integer(value: object, label: str) -> int:
    if type(value) is not int:
        raise TypeError(f"{label} must be an integer")
    return value


def _boolean(value: object, label: str) -> bool:
    if type(value) is not bool:
        raise TypeError(f"{label} must be a boolean")
    return value


def _number(value: object, label: str) -> float:
    if type(value) not in {int, float}:
        raise TypeError(f"{label} must be a number")
    try:
        converted = float(cast(int | float, value))
    except OverflowError as error:
        raise ValueError(f"{label} is not representable as float") from error
    if not math.isfinite(converted):
        raise ValueError(f"{label} must be finite")
    return converted


def _freeze_json(value: object, label: str = "JSON value") -> JsonValue:
    if value is None or type(value) in {bool, int, str}:
        return cast(None | bool | int | str, value)
    if type(value) is float:
        number = cast(float, value)
        if not math.isfinite(number):
            raise ValueError(f"{label} must be finite")
        return number
    if type(value) is list or type(value) is tuple:
        return tuple(_freeze_json(item, label) for item in cast(Sequence[object], value))
    if type(value) is dict:
        source = _dict(value, label)
        return MappingProxyType({key: _freeze_json(item, label) for key, item in source.items()})
    raise TypeError(f"{label} is not JSON-compatible")


def _thaw_json(value: JsonValue) -> object:
    if isinstance(value, Mapping):
        return {key: _thaw_json(cast(JsonValue, item)) for key, item in value.items()}
    if isinstance(value, tuple):
        return [_thaw_json(item) for item in value]
    return value


def _type_sensitive_equal(left: object, right: object) -> bool:
    if type(left) is not type(right):
        return False
    if type(left) is dict:
        left_map = cast(dict[str, object], left)
        right_map = cast(dict[str, object], right)
        return set(left_map) == set(right_map) and all(
            _type_sensitive_equal(left_map[key], right_map[key]) for key in left_map
        )
    if type(left) is list:
        left_items = cast(list[object], left)
        right_items = cast(list[object], right)
        return len(left_items) == len(right_items) and all(
            _type_sensitive_equal(a, b) for a, b in zip(left_items, right_items)
        )
    return left == right


def fair_observation_payload(observation: FairCombatObservation) -> dict[str, object]:
    return cast(dict[str, object], json.loads(canonical_bytes(observation.to_wire_dict())))


def _validate_runtime_type(value: object, annotation: object, path: str) -> None:
    while hasattr(annotation, "__value__"):
        annotation = annotation.__value__
    origin = get_origin(annotation)
    arguments = get_args(annotation)
    if origin is Literal:
        if not any(type(value) is type(option) and value == option for option in arguments):
            raise TypeError(f"{path} has an invalid literal value")
        return
    if origin is types.UnionType:
        for option in arguments:
            try:
                _validate_runtime_type(value, option, path)
                return
            except TypeError:
                pass
        raise TypeError(f"{path} has an invalid union value")
    if origin is tuple:
        if type(value) is not tuple:
            raise TypeError(f"{path} must be a tuple")
        element_type = arguments[0]
        for index, item in enumerate(value):
            _validate_runtime_type(item, element_type, f"{path}[{index}]")
        return
    if annotation is None or annotation is type(None):
        if value is not None:
            raise TypeError(f"{path} must be null")
        return
    if annotation in {bool, int, float, str}:
        if type(value) is not annotation:
            raise TypeError(f"{path} has an invalid scalar type")
        return
    if isinstance(annotation, type) and is_dataclass(annotation):
        if type(value) is not annotation:
            raise TypeError(f"{path} has an invalid object type")
        hints = get_type_hints(annotation)
        for field in fields(annotation):
            _validate_runtime_type(
                getattr(value, field.name), hints[field.name], f"{path}.{field.name}"
            )
        return
    if annotation is Any:
        return
    raise TypeError(f"{path} uses an unsupported symbolic annotation")


def fair_observation_from_payload(payload: object) -> FairCombatObservation:
    source = _dict(payload, "fair observation")
    if "schema_version" not in source:
        raise ValueError("fair observation is missing schema_version")
    schema = _integer(source["schema_version"], "fair observation schema_version")
    if schema != 2:
        raise ValueError("unsupported fair observation schema")
    if "orb_slots" not in source:
        raise ValueError("fair observation is missing orb_slots")
    observation = FairCombatObservation._from_payload(source)
    _validate_runtime_type(observation, FairCombatObservation, "fair observation")
    canonical = fair_observation_payload(observation)
    if not _type_sensitive_equal(source, canonical):
        raise ValueError("fair observation payload is not canonical for its schema")
    return observation


def fair_observation_digest(observation: FairCombatObservation) -> str:
    return sha256_bytes(canonical_bytes(fair_observation_payload(observation)))


def action_descriptor_payload(action: ActionDescriptor) -> dict[str, object]:
    return {key: value for key, value in asdict(action).items() if value is not None}


def action_descriptor_from_payload(payload: object) -> ActionDescriptor:
    source = _dict(payload, "action descriptor")
    if not {"family", "kind"} <= set(source) or not set(source) <= _ACTION_FIELDS:
        raise ValueError("action descriptor has missing or unknown fields")
    family = _string(source["family"], "action family")
    kind = _string(source["kind"], "action kind")
    assert family is not None and kind is not None
    if family != "combat" or kind not in _COMBAT_ACTION_SLOTS:
        raise ValueError("action descriptor is not tensorizable combat schema")
    required, optional = _COMBAT_ACTION_SLOTS[kind]
    supplied = set(source) - {"family", "kind"}
    if not required <= supplied or not supplied <= required | optional:
        raise ValueError("action descriptor slots disagree with combat action kind")
    for slot in supplied:
        value = _integer(source[slot], slot)
        if value < 0:
            raise ValueError("action descriptor slots must be nonnegative")
    return ActionDescriptor(
        family=family,
        kind=kind,
        hand_slot=_optional_integer(source, "hand_slot"),
        potion_slot=_optional_integer(source, "potion_slot"),
        option_slot=_optional_integer(source, "option_slot"),
        target_slot=_optional_integer(source, "target_slot"),
        card_slot=None,
        node_slot=None,
        reward_slot=None,
        shop_slot=None,
        slot=None,
    )


def first_argmax_visits(counts: Sequence[int]) -> int:
    if not counts:
        raise ValueError("visit counts must be nonempty")
    best_index = 0
    best = counts[0]
    for index, value in enumerate(counts):
        if type(value) is not int or value < 0:
            raise ValueError("visit counts must be nonnegative integers")
        if value > best:
            best = value
            best_index = index
    return best_index


def canonical_episode_id(root_id: str, search_config: object, reward_config_digest: str) -> str:
    payload = _thaw_json(cast(JsonValue, search_config))
    return sha256_bytes(canonical_bytes([root_id, payload, reward_config_digest]))


def validate_beam_search_config(payload: Mapping[str, object]) -> None:
    if set(payload) != _BEAM_SEARCH_CONFIG_KEYS:
        raise ValueError("beam search config has missing or unknown fields")
    for key in ("depth", "width", "transition_budget", "max_decisions", "max_player_turns"):
        if type(payload[key]) is not int or cast(int, payload[key]) <= 0:
            raise TypeError(f"search config {key} must be a positive integer")
    if payload["deadline"] is not None:
        raise ValueError("offline teacher deadline must be null")
    if payload["replan"] != "every_public_decision":
        raise ValueError("unknown teacher replanning policy")
    if type(payload["deduplicate_search_states"]) is not bool:
        raise TypeError("deduplicate_search_states must be boolean")


def validate_puct_search_config(payload: Mapping[str, object]) -> None:
    if set(payload) != _PUCT_SEARCH_CONFIG_KEYS:
        raise ValueError("PUCT search config has missing or unknown fields")
    c_puct = payload["c_puct"]
    if type(c_puct) is int:
        exploration = float(c_puct)
    elif type(c_puct) is float:
        exploration = c_puct
    else:
        raise ValueError("search config c_puct must be finite and positive")
    if not math.isfinite(exploration) or exploration <= 0.0:
        raise ValueError("search config c_puct must be finite and positive")
    for key in ("simulation_budget", "transition_budget", "max_decisions", "max_player_turns"):
        if type(payload[key]) is not int or cast(int, payload[key]) <= 0:
            raise TypeError(f"search config {key} must be a positive integer")
    if payload["deadline"] is not None:
        raise ValueError("offline teacher deadline must be null")
    if payload["replan"] != "every_public_decision":
        raise ValueError("unknown teacher replanning policy")
    if payload["privileged"] is not True:
        raise ValueError("PUCT teacher search must be explicitly privileged")
    if payload["leaf_schema"] != FAIR_LEAF_BATCH_SCHEMA:
        raise ValueError("PUCT leaf schema must be fair_leaf_batch_v1")
    if payload["value_target_name"] != COMBAT_PROXY_VALUE_TARGET_NAME:
        raise ValueError("PUCT training value target must be combat_proxy_v1")
    if payload["search_root_mean_name"] != PUCT_SEARCH_ROOT_MEAN_NAME:
        raise ValueError("PUCT search backup name must be privileged_puct_root_mean_v1")
    for key in (
        "checkpoint_file_digest",
        "checkpoint_model_state_digest",
        "checkpoint_config_digest",
        "source_digest",
        "runtime_identity_digest",
        "vocabulary_fingerprint",
        "encoder_contract_digest",
    ):
        value = payload[key]
        if (
            type(value) is not str
            or len(value) != 64
            or any(character not in "0123456789abcdef" for character in value)
        ):
            raise ValueError(f"search config {key} must be a lowercase SHA-256 digest")


def validate_search_config(teacher_name: str, payload: Mapping[str, object]) -> None:
    if teacher_name == BEAM_TEACHER_NAME:
        validate_beam_search_config(payload)
        return
    if teacher_name == PUCT_TEACHER_NAME:
        validate_puct_search_config(payload)
        return
    raise ValueError("dataset teacher identity is unsupported")


def _optional_integer(payload: Mapping[str, object], key: str) -> int | None:
    return None if key not in payload else _integer(payload[key], key)


@dataclass(frozen=True, slots=True)
class CounterChange:
    owner_kind: CounterOwnerKind
    owner_key: str
    counter_key: str
    before: int
    after: int

    def __post_init__(self) -> None:
        if type(self.owner_kind) is not str:
            raise TypeError("counter owner kind must be a string")
        if self.owner_kind not in {"card", "relic"}:
            raise ValueError("counter owner kind must be card or relic")
        if type(self.owner_key) is not str or type(self.counter_key) is not str:
            raise TypeError("counter owner and counter keys must be strings")
        if not self.owner_key or not self.counter_key:
            raise ValueError("counter owner and counter keys must be nonempty")
        if type(self.before) is not int or type(self.after) is not int:
            raise TypeError("counter values must be integers")

    def to_dict(self) -> dict[str, object]:
        return asdict(self)

    @classmethod
    def from_dict(cls, payload: object) -> CounterChange:
        source = _dict(payload, "counter change", _COUNTER_CHANGE_KEYS)
        owner_kind = _string(source["owner_kind"], "counter owner kind")
        owner_key = _string(source["owner_key"], "counter owner key")
        counter_key = _string(source["counter_key"], "counter key")
        assert owner_kind is not None and owner_key is not None and counter_key is not None
        return cls(
            cast(CounterOwnerKind, owner_kind),
            owner_key,
            counter_key,
            _integer(source["before"], "counter before"),
            _integer(source["after"], "counter after"),
        )


@dataclass(frozen=True, slots=True)
class CombatOutcome:
    status: OutcomeStatus
    terminal_hp: int
    terminal_max_hp: int
    hp_change: int
    max_hp_change: int
    gold_change: int
    potion_slots: tuple[str | None, ...]
    counter_changes: tuple[CounterChange, ...]
    terminal: bool
    truncated: bool
    accepted_decisions: int = 0
    player_turns: int = 1
    truncation_trigger: str | None = None

    def __post_init__(self) -> None:
        if self.status not in {"won", "lost", "escaped", "truncated"}:
            raise ValueError("unknown combat outcome status")
        for name in ("terminal_hp", "terminal_max_hp", "hp_change", "max_hp_change", "gold_change"):
            if type(getattr(self, name)) is not int:
                raise TypeError(f"{name} must be an integer")
        if self.terminal_max_hp <= 0 or not 0 <= self.terminal_hp <= self.terminal_max_hp:
            raise ValueError("terminal HP must be within [0, terminal_max_hp]")
        if type(self.potion_slots) is not tuple:
            raise TypeError("potion slots must be a tuple")
        if any(
            value is not None and (type(value) is not str or not value)
            for value in self.potion_slots
        ):
            raise TypeError("potion slots must contain nonempty strings or null")
        if type(self.counter_changes) is not tuple:
            raise TypeError("counter changes must be a tuple")
        if any(type(change) is not CounterChange for change in self.counter_changes):
            raise TypeError("counter changes must contain CounterChange values")
        if type(self.terminal) is not bool or type(self.truncated) is not bool:
            raise TypeError("terminal flags must be booleans")
        if type(self.accepted_decisions) is not int or self.accepted_decisions < 0:
            raise ValueError("accepted decisions must be a nonnegative integer")
        if type(self.player_turns) is not int or self.player_turns <= 0:
            raise ValueError("player turns must be a positive integer")
        if self.truncation_trigger is not None and self.truncation_trigger not in {
            "accepted_decisions",
            "player_turns",
        }:
            raise ValueError("unknown truncation trigger")
        if self.status == "truncated":
            if self.terminal or not self.truncated or self.truncation_trigger is None:
                raise ValueError("truncated outcome flags disagree")
        elif not self.terminal or self.truncated or self.truncation_trigger is not None:
            raise ValueError("terminal outcome flags disagree")
        if self.status == "lost" and self.terminal_hp != 0:
            raise ValueError("lost combat must end at zero HP")
        if self.status in {"won", "escaped"} and self.terminal_hp <= 0:
            raise ValueError("won or escaped combat must retain positive HP")

    def to_dict(self) -> dict[str, object]:
        return {
            "status": self.status,
            "terminal_hp": self.terminal_hp,
            "terminal_max_hp": self.terminal_max_hp,
            "hp_change": self.hp_change,
            "max_hp_change": self.max_hp_change,
            "gold_change": self.gold_change,
            "potion_slots": list(self.potion_slots),
            "counter_changes": [change.to_dict() for change in self.counter_changes],
            "terminal": self.terminal,
            "truncated": self.truncated,
            "accepted_decisions": self.accepted_decisions,
            "player_turns": self.player_turns,
            "truncation_trigger": self.truncation_trigger,
        }

    @classmethod
    def from_dict(cls, payload: object) -> CombatOutcome:
        source = _dict(payload, "combat outcome", _OUTCOME_KEYS)
        status = _string(source["status"], "outcome status")
        assert status is not None
        slots = tuple(
            _string(value, "potion slot", optional=True)
            for value in _list(source["potion_slots"], "potion slots")
        )
        changes = tuple(
            CounterChange.from_dict(value)
            for value in _list(source["counter_changes"], "counter changes")
        )
        return cls(
            status=cast(OutcomeStatus, status),
            terminal_hp=_integer(source["terminal_hp"], "terminal_hp"),
            terminal_max_hp=_integer(source["terminal_max_hp"], "terminal_max_hp"),
            hp_change=_integer(source["hp_change"], "hp_change"),
            max_hp_change=_integer(source["max_hp_change"], "max_hp_change"),
            gold_change=_integer(source["gold_change"], "gold_change"),
            potion_slots=slots,
            counter_changes=changes,
            terminal=_boolean(source["terminal"], "terminal"),
            truncated=_boolean(source["truncated"], "truncated"),
            accepted_decisions=_integer(source["accepted_decisions"], "accepted_decisions"),
            player_turns=_integer(source["player_turns"], "player_turns"),
            truncation_trigger=_string(
                source["truncation_trigger"], "truncation trigger", optional=True
            ),
        )


@dataclass(frozen=True, slots=True)
class SymbolicTrainingRecord:
    observation: FairCombatObservation
    actions: tuple[ActionDescriptor, ...]
    chosen_action_index: int
    chosen_action: ActionDescriptor
    teacher_visit_counts: tuple[int, ...]
    target_value: float | None
    value_target_name: str
    outcome: CombatOutcome
    planner_name: str
    planner_version: str
    search_config: Mapping[str, JsonValue]
    root_id: str
    split_group_id: str
    teacher_pair_id: str | None
    repository: RepositoryVersion
    observation_digest: str
    record_version: int
    root_manifest_digest: str
    reward_config_digest: str
    source_kind: str
    episode_id: str
    decision_index: int
    value_target_mask: bool
    record_id: str
    search_root_mean_value: float | None

    def __post_init__(self) -> None:
        identity = self._coerce_and_validate()
        if (
            type(self.record_id) is not str
            or len(self.record_id) != 64
            or any(character not in "0123456789abcdef" for character in self.record_id)
            or self.record_id != identity
        ):
            raise ValueError("record ID is invalid")

    def _coerce_and_validate(self) -> str:
        for name in (
            "value_target_name",
            "planner_name",
            "planner_version",
            "root_id",
            "split_group_id",
            "source_kind",
            "episode_id",
        ):
            if type(getattr(self, name)) is not str or not getattr(self, name):
                raise TypeError(f"{name} must be a nonempty string")
        if self.teacher_pair_id is not None and (
            type(self.teacher_pair_id) is not str or not self.teacher_pair_id
        ):
            raise TypeError("teacher_pair_id must be null or a nonempty string")
        if not self.actions:
            raise ValueError("training record must contain legal actions")
        if type(self.chosen_action_index) is not int or not 0 <= self.chosen_action_index < len(
            self.actions
        ):
            raise ValueError("chosen action index is out of range")
        if self.actions[self.chosen_action_index] != self.chosen_action:
            raise ValueError("chosen action descriptor does not match its index")
        if len(self.teacher_visit_counts) != len(self.actions):
            raise ValueError("teacher visit counts do not align with actions")
        if any(type(value) is not int or value < 0 for value in self.teacher_visit_counts):
            raise ValueError("teacher visit counts must be nonnegative integers")
        if sum(self.teacher_visit_counts) <= 0:
            raise ValueError("teacher visit counts must have positive mass")
        if type(self.value_target_mask) is not bool:
            raise TypeError("value target mask must be boolean")
        if self.target_value is None:
            if self.value_target_mask:
                raise ValueError("missing target value must be masked")
        else:
            if type(self.target_value) not in {int, float}:
                raise TypeError("target value must be numeric")
            target = float(self.target_value)
            if not math.isfinite(target) or not -1.0 <= target <= 1.0:
                raise ValueError("target value must be finite and in [-1, 1]")
            if not self.value_target_mask:
                raise ValueError("present target value must not be masked")
        if self.planner_name == PUCT_TEACHER_NAME:
            raw_root_mean = self.search_root_mean_value
            if type(raw_root_mean) is int:
                root_mean = float(raw_root_mean)
            elif type(raw_root_mean) is float:
                root_mean = raw_root_mean
            else:
                raise TypeError("PUCT search root-mean value must be numeric")
            if not math.isfinite(root_mean) or not -1.0 <= root_mean <= 1.0:
                raise ValueError("search root-mean value must be finite and in [-1, 1]")
            object.__setattr__(self, "search_root_mean_value", root_mean)
        elif self.planner_name == BEAM_TEACHER_NAME:
            if self.search_root_mean_value is not None:
                raise ValueError("beam records must not carry a PUCT search root-mean")
        if self.value_target_name != COMBAT_PROXY_VALUE_TARGET_NAME:
            raise ValueError("records must use combat_proxy_v1 training targets")
        if self.outcome.truncated != (not self.value_target_mask):
            raise ValueError("outcome truncation and value target mask disagree")
        if self.chosen_action_index != first_argmax_visits(self.teacher_visit_counts):
            raise ValueError("chosen action is not the first visit-count argmax")
        if self.record_version != RECORD_VERSION:
            raise ValueError("unsupported training record version")
        if type(self.decision_index) is not int or self.decision_index < 0:
            raise ValueError("decision index must be nonnegative")
        validate_search_config(self.planner_name, cast(Mapping[str, object], self.search_config))
        frozen = _freeze_json(dict(self.search_config), "search config")
        if not isinstance(frozen, Mapping):
            raise TypeError("search config must be an object")
        object.__setattr__(self, "search_config", frozen)
        for name in ("root_id", "root_manifest_digest", "reward_config_digest"):
            value = getattr(self, name)
            if (
                type(value) is not str
                or len(value) != 64
                or any(character not in "0123456789abcdef" for character in value)
            ):
                raise ValueError(f"{name} must be a lowercase SHA-256 digest")
        expected_episode = canonical_episode_id(
            self.root_id,
            self.search_config,
            self.reward_config_digest,
        )
        if self.episode_id != expected_episode:
            raise ValueError("episode ID does not match canonical root/search/reward identity")
        if fair_observation_digest(self.observation) != self.observation_digest:
            raise ValueError("fair observation digest is invalid")
        if not self.repository.clean:
            raise ValueError("training records require a clean repository")
        return sha256_bytes(canonical_bytes(self._substantive_payload()))

    @classmethod
    def create(
        cls,
        observation: FairCombatObservation,
        actions: tuple[ActionDescriptor, ...],
        chosen_action_index: int,
        chosen_action: ActionDescriptor,
        teacher_visit_counts: tuple[int, ...],
        target_value: float | None,
        value_target_name: str,
        outcome: CombatOutcome,
        planner_name: str,
        planner_version: str,
        search_config: Mapping[str, JsonValue],
        root_id: str,
        split_group_id: str,
        teacher_pair_id: str | None,
        repository: RepositoryVersion,
        observation_digest: str,
        record_version: int,
        root_manifest_digest: str,
        reward_config_digest: str,
        source_kind: str,
        episode_id: str,
        decision_index: int,
        value_target_mask: bool,
        search_root_mean_value: float | None = None,
    ) -> SymbolicTrainingRecord:
        """Construct a record and assign the canonical record ID."""

        self = object.__new__(cls)
        object.__setattr__(self, "observation", observation)
        object.__setattr__(self, "actions", actions)
        object.__setattr__(self, "chosen_action_index", chosen_action_index)
        object.__setattr__(self, "chosen_action", chosen_action)
        object.__setattr__(self, "teacher_visit_counts", teacher_visit_counts)
        object.__setattr__(self, "target_value", target_value)
        object.__setattr__(self, "value_target_name", value_target_name)
        object.__setattr__(self, "outcome", outcome)
        object.__setattr__(self, "planner_name", planner_name)
        object.__setattr__(self, "planner_version", planner_version)
        object.__setattr__(self, "search_config", search_config)
        object.__setattr__(self, "root_id", root_id)
        object.__setattr__(self, "split_group_id", split_group_id)
        object.__setattr__(self, "teacher_pair_id", teacher_pair_id)
        object.__setattr__(self, "repository", repository)
        object.__setattr__(self, "observation_digest", observation_digest)
        object.__setattr__(self, "record_version", record_version)
        object.__setattr__(self, "root_manifest_digest", root_manifest_digest)
        object.__setattr__(self, "reward_config_digest", reward_config_digest)
        object.__setattr__(self, "source_kind", source_kind)
        object.__setattr__(self, "episode_id", episode_id)
        object.__setattr__(self, "decision_index", decision_index)
        object.__setattr__(self, "value_target_mask", value_target_mask)
        object.__setattr__(self, "record_id", "0" * 64)
        object.__setattr__(self, "search_root_mean_value", search_root_mean_value)
        object.__setattr__(self, "record_id", self._coerce_and_validate())
        return self

    @classmethod
    def create_from(
        cls, record: SymbolicTrainingRecord, **changes: object
    ) -> SymbolicTrainingRecord:
        payload = {
            "observation": record.observation,
            "actions": record.actions,
            "chosen_action_index": record.chosen_action_index,
            "chosen_action": record.chosen_action,
            "teacher_visit_counts": record.teacher_visit_counts,
            "target_value": record.target_value,
            "value_target_name": record.value_target_name,
            "outcome": record.outcome,
            "planner_name": record.planner_name,
            "planner_version": record.planner_version,
            "search_config": record.search_config,
            "root_id": record.root_id,
            "split_group_id": record.split_group_id,
            "teacher_pair_id": record.teacher_pair_id,
            "repository": record.repository,
            "observation_digest": record.observation_digest,
            "record_version": record.record_version,
            "root_manifest_digest": record.root_manifest_digest,
            "reward_config_digest": record.reward_config_digest,
            "source_kind": record.source_kind,
            "episode_id": record.episode_id,
            "decision_index": record.decision_index,
            "value_target_mask": record.value_target_mask,
            "search_root_mean_value": record.search_root_mean_value,
        }
        payload.update(changes)
        payload.pop("record_id", None)
        return cls.create(**cast(Any, payload))

    def _substantive_payload(self) -> dict[str, object]:
        payload = self.to_dict()
        payload.pop("record_id", None)
        return payload

    def to_dict(self) -> dict[str, object]:
        return {
            "observation": fair_observation_payload(self.observation),
            "actions": [action_descriptor_payload(action) for action in self.actions],
            "chosen_action_index": self.chosen_action_index,
            "chosen_action": action_descriptor_payload(self.chosen_action),
            "teacher_visit_counts": list(self.teacher_visit_counts),
            "target_value": self.target_value,
            "value_target_name": self.value_target_name,
            "outcome": self.outcome.to_dict(),
            "planner_name": self.planner_name,
            "planner_version": self.planner_version,
            "search_config": _thaw_json(cast(JsonValue, self.search_config)),
            "root_id": self.root_id,
            "split_group_id": self.split_group_id,
            "teacher_pair_id": self.teacher_pair_id,
            "repository": self.repository.to_dict(),
            "observation_digest": self.observation_digest,
            "record_version": self.record_version,
            "root_manifest_digest": self.root_manifest_digest,
            "reward_config_digest": self.reward_config_digest,
            "source_kind": self.source_kind,
            "episode_id": self.episode_id,
            "decision_index": self.decision_index,
            "value_target_mask": self.value_target_mask,
            "record_id": self.record_id,
            "search_root_mean_value": self.search_root_mean_value,
        }

    @classmethod
    def from_dict(cls, payload: object) -> SymbolicTrainingRecord:
        source = _dict(payload, "training record", _RECORD_KEYS)
        record_version = _integer(source["record_version"], "record version")
        if record_version != RECORD_VERSION:
            raise ValueError("unsupported training record version")
        actions = tuple(
            action_descriptor_from_payload(action) for action in _list(source["actions"], "actions")
        )
        counts = tuple(
            _integer(value, "teacher visit count")
            for value in _list(source["teacher_visit_counts"], "teacher visit counts")
        )
        value_target_name = _string(source["value_target_name"], "value target name")
        planner_name = _string(source["planner_name"], "planner name")
        planner_version = _string(source["planner_version"], "planner version")
        root_id = _string(source["root_id"], "root id")
        split_group_id = _string(source["split_group_id"], "split group id")
        teacher_pair_id = _string(source["teacher_pair_id"], "teacher pair id", optional=True)
        observation_digest = _string(source["observation_digest"], "observation digest")
        root_manifest_digest = _string(source["root_manifest_digest"], "root manifest digest")
        reward_config_digest = _string(source["reward_config_digest"], "reward config digest")
        source_kind = _string(source["source_kind"], "source kind")
        episode_id = _string(source["episode_id"], "episode id")
        record_id = _string(source["record_id"], "record id")
        assert all(
            value is not None
            for value in (
                value_target_name,
                planner_name,
                planner_version,
                root_id,
                split_group_id,
                observation_digest,
                root_manifest_digest,
                reward_config_digest,
                source_kind,
                episode_id,
                record_id,
            )
        )
        repository_payload = _dict(source["repository"], "repository")
        search_payload = _dict(source["search_config"], "search config")
        return cls(
            observation=fair_observation_from_payload(source["observation"]),
            actions=actions,
            chosen_action_index=_integer(source["chosen_action_index"], "chosen action index"),
            chosen_action=action_descriptor_from_payload(source["chosen_action"]),
            teacher_visit_counts=counts,
            target_value=(
                None
                if source["target_value"] is None
                else _number(source["target_value"], "target value")
            ),
            value_target_name=cast(str, value_target_name),
            outcome=CombatOutcome.from_dict(source["outcome"]),
            planner_name=cast(str, planner_name),
            planner_version=cast(str, planner_version),
            search_config=cast(Mapping[str, JsonValue], _freeze_json(search_payload)),
            root_id=cast(str, root_id),
            split_group_id=cast(str, split_group_id),
            teacher_pair_id=teacher_pair_id,
            repository=RepositoryVersion.from_dict(repository_payload),
            observation_digest=cast(str, observation_digest),
            record_version=record_version,
            root_manifest_digest=cast(str, root_manifest_digest),
            reward_config_digest=cast(str, reward_config_digest),
            source_kind=cast(str, source_kind),
            episode_id=cast(str, episode_id),
            decision_index=_integer(source["decision_index"], "decision index"),
            value_target_mask=_boolean(source["value_target_mask"], "value target mask"),
            record_id=cast(str, record_id),
            search_root_mean_value=(
                None
                if source["search_root_mean_value"] is None
                else _number(source["search_root_mean_value"], "search root-mean value")
            ),
        )


def parse_jsonl_records(content: bytes) -> tuple[SymbolicTrainingRecord, ...]:
    """Parse canonical nonblank JSONL bytes into realized records."""

    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError("JSONL is not UTF-8") from error
    if "\r" in text:
        raise ValueError("JSONL must use canonical newline separators")
    if not text.endswith("\n"):
        raise ValueError("JSONL must end with a newline")
    records: list[SymbolicTrainingRecord] = []
    for line_number, line in enumerate(text[:-1].split("\n"), 1):
        if not line or line.strip() != line:
            raise ValueError(f"JSONL contains a blank or noncanonical line at {line_number}")
        try:
            loaded = json.loads(line)
            record = SymbolicTrainingRecord.from_dict(loaded)
        except (KeyError, TypeError, ValueError) as error:
            raise ValueError(f"invalid symbolic record at line {line_number}") from error
        if line.encode("utf-8") != canonical_bytes(record.to_dict()):
            raise ValueError(f"JSONL contains a blank or noncanonical line at {line_number}")
        records.append(record)
    return tuple(records)


def write_jsonl(path: Path, records: Iterable[SymbolicTrainingRecord]) -> None:
    payload = b"".join(canonical_bytes(record.to_dict()) + b"\n" for record in records)
    write_scientific_artifact(path, payload)


def read_jsonl(path: Path) -> Iterator[SymbolicTrainingRecord]:
    yield from parse_jsonl_records(read_regular_file_bytes(path))


@dataclass(frozen=True, slots=True)
class TensorizedTrainingExample:
    decision: TensorizedCombatDecision
    policy_target: torch.Tensor
    value_target: torch.Tensor
    value_target_mask: torch.Tensor
    value_target_name: str
    outcome: CombatOutcome
    record: SymbolicTrainingRecord


@dataclass(frozen=True, slots=True)
class BatchedTrainingExamples:
    decision: BatchedCombatDecision
    policy_target: torch.Tensor
    value_target: torch.Tensor
    value_target_mask: torch.Tensor
    value_target_name: str
    outcomes: tuple[CombatOutcome, ...]
    records: tuple[SymbolicTrainingRecord, ...]


class SymbolicCombatDataset:
    def __init__(
        self,
        records: Sequence[SymbolicTrainingRecord],
        vocabularies: Vocabularies,
    ) -> None:
        self._records = tuple(records)
        if not self._records:
            raise ValueError("symbolic combat dataset must be nonempty")
        names = {record.value_target_name for record in self._records}
        if len(names) > 1:
            raise ValueError("dataset mixes value target names")
        self.value_target_name = next(iter(names))
        self._vocabularies = vocabularies

    def __len__(self) -> int:
        return len(self._records)

    def __getitem__(self, index: int) -> TensorizedTrainingExample:
        record = self._records[index]
        decision = tensorize_combat(record.observation, record.actions, self._vocabularies)
        total = sum(record.teacher_visit_counts)
        probabilities = [count / total for count in record.teacher_visit_counts]
        policy = torch.tensor(probabilities, dtype=torch.float32)
        return TensorizedTrainingExample(
            decision,
            policy,
            torch.tensor(
                0.0 if record.target_value is None else record.target_value,
                dtype=torch.float32,
            ),
            torch.tensor(record.value_target_mask, dtype=torch.bool),
            record.value_target_name,
            record.outcome,
            record,
        )


def collate_training_examples(
    items: Sequence[TensorizedTrainingExample],
) -> BatchedTrainingExamples:
    if not items:
        raise ValueError("cannot collate an empty training batch")
    names = {item.value_target_name for item in items}
    if len(names) != 1:
        raise ValueError("batch mixes value target names")
    decision = collate_combat_tensors(tuple(item.decision for item in items))
    policy = torch.zeros(decision.action_mask.shape, dtype=torch.float32)
    for index, item in enumerate(items):
        policy[index, : item.policy_target.shape[0]] = item.policy_target
    return BatchedTrainingExamples(
        decision=decision,
        policy_target=policy,
        value_target=torch.stack(tuple(item.value_target for item in items)),
        value_target_mask=torch.stack(tuple(item.value_target_mask for item in items)),
        value_target_name=next(iter(names)),
        outcomes=tuple(item.outcome for item in items),
        records=tuple(item.record for item in items),
    )
