"""Strict symbolic combat records tensorized only when loaded."""

from __future__ import annotations

import hashlib
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
from .provenance import RepositoryVersion
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

_LEGACY_RECORD_KEYS = {
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
}
_RECORD_KEYS = _LEGACY_RECORD_KEYS | {
    "record_version",
    "root_manifest_digest",
    "reward_config_digest",
    "source_kind",
    "episode_id",
    "decision_index",
    "value_target_mask",
    "record_id",
}
_RECORD_KEYS_V4 = _RECORD_KEYS | {"search_root_mean_value"}
_ACTION_FIELDS = set(ActionDescriptor.__dataclass_fields__)
_V2_SEARCH_CONFIG_KEYS = {
    "depth",
    "width",
    "transition_budget",
    "max_decisions",
    "max_player_turns",
    "deadline",
    "replan",
    "deduplicate_search_states",
}
_V3_SEARCH_CONFIG_KEYS = {
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
    "checkpoint_file_digest",
    "checkpoint_model_state_digest",
    "checkpoint_config_digest",
    "source_digest",
    "runtime_identity_digest",
    "vocabulary_fingerprint",
    "encoder_contract_digest",
}
_V4_SEARCH_CONFIG_KEYS = _V3_SEARCH_CONFIG_KEYS | {"search_root_mean_name"}
PUCT_VALUE_TARGET_NAME = "privileged_puct_root_mean_v1"
PUCT_SEARCH_ROOT_MEAN_NAME = PUCT_VALUE_TARGET_NAME
COMBAT_PROXY_VALUE_TARGET_NAME = "combat_proxy_v1"
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
_LEGACY_OUTCOME_KEYS = {
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
}
_OUTCOME_KEYS = _LEGACY_OUTCOME_KEYS | {
    "accepted_decisions",
    "player_turns",
    "truncation_trigger",
}
_COUNTER_CHANGE_KEYS = {"owner_kind", "owner_key", "counter_key", "before", "after"}
_V1_ADDITIVE_DYNAMIC_FIELDS = {
    "windmill_retain_damage",
    "steam_barrier_block_reduction",
    "combat_cost_under_turn_override",
}


def _canonical_json(payload: object) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


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
    payload = cast(dict[str, object], json.loads(json.dumps(asdict(observation))))
    if observation.schema_version == 1:
        _remove_v1_absent_additions(payload)
    return payload


def _remove_v1_absent_additions(value: object) -> None:
    if type(value) is dict:
        mapping = cast(dict[str, object], value)
        mapping.pop("orb_slots", None)
        dynamic = mapping.get("dynamic")
        if type(dynamic) is dict:
            dynamic_map = cast(dict[str, object], dynamic)
            for key in _V1_ADDITIVE_DYNAMIC_FIELDS:
                dynamic_map.pop(key, None)
        for child in mapping.values():
            _remove_v1_absent_additions(child)
    elif type(value) is list:
        for child in cast(list[object], value):
            _remove_v1_absent_additions(child)


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
    schema = _integer(source.get("schema_version"), "fair observation schema_version")
    if schema not in {1, 2}:
        raise ValueError("unsupported fair observation schema")
    observation = FairCombatObservation._from_payload(source)
    _validate_runtime_type(observation, FairCombatObservation, "fair observation")
    canonical = fair_observation_payload(observation)
    if schema == 1:
        _remove_v1_absent_additions(canonical)
    if not _type_sensitive_equal(source, canonical):
        raise ValueError("fair observation payload is not canonical for its schema")
    return observation


def fair_observation_digest(observation: FairCombatObservation) -> str:
    return hashlib.sha256(
        _canonical_json(fair_observation_payload(observation)).encode()
    ).hexdigest()


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


def puct_episode_id(root_id: str, search_config: object, reward_config_digest: str) -> str:
    payload = _thaw_json(cast(JsonValue, search_config))
    return hashlib.sha256(
        _canonical_json([root_id, payload, reward_config_digest]).encode()
    ).hexdigest()


def validate_v2_search_config(payload: Mapping[str, object]) -> None:
    if set(payload) != _V2_SEARCH_CONFIG_KEYS:
        raise ValueError("V2 search config has missing or unknown fields")
    for key in ("depth", "width", "transition_budget", "max_decisions", "max_player_turns"):
        if type(payload[key]) is not int or cast(int, payload[key]) <= 0:
            raise TypeError(f"search config {key} must be a positive integer")
    if payload["deadline"] is not None:
        raise ValueError("offline teacher deadline must be null")
    if payload["replan"] != "every_public_decision":
        raise ValueError("unknown teacher replanning policy")
    if type(payload["deduplicate_search_states"]) is not bool:
        raise TypeError("deduplicate_search_states must be boolean")


def validate_v3_search_config(payload: Mapping[str, object]) -> None:
    if set(payload) != _V3_SEARCH_CONFIG_KEYS:
        raise ValueError("V3 search config has missing or unknown fields")
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
    if payload["value_target_name"] != PUCT_VALUE_TARGET_NAME:
        raise ValueError("PUCT value target name mismatch")
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


def validate_v4_search_config(payload: Mapping[str, object]) -> None:
    if set(payload) != _V4_SEARCH_CONFIG_KEYS:
        raise ValueError("V4 search config has missing or unknown fields")
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
        raise ValueError("V4 PUCT training value target must be combat_proxy_v1")
    if payload["search_root_mean_name"] != PUCT_SEARCH_ROOT_MEAN_NAME:
        raise ValueError("V4 PUCT search backup name must be privileged_puct_root_mean_v1")
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
        raw = _dict(payload, "combat outcome")
        if set(raw) == _LEGACY_OUTCOME_KEYS:
            source = raw
            accepted_decisions = 0
            player_turns = 1
            truncation_trigger = "accepted_decisions" if source.get("truncated") is True else None
        elif set(raw) == _OUTCOME_KEYS:
            source = raw
            accepted_decisions = _integer(source["accepted_decisions"], "accepted_decisions")
            player_turns = _integer(source["player_turns"], "player_turns")
            truncation_trigger = _string(
                source["truncation_trigger"], "truncation trigger", optional=True
            )
        else:
            raise ValueError("combat outcome has missing or unknown fields")
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
            accepted_decisions=accepted_decisions,
            player_turns=player_turns,
            truncation_trigger=truncation_trigger,
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
    record_version: int = 1
    root_manifest_digest: str | None = None
    reward_config_digest: str | None = None
    source_kind: str = "legacy"
    episode_id: str = "legacy"
    decision_index: int = 0
    value_target_mask: bool = True
    record_id: str | None = None
    search_root_mean_value: float | None = None

    def __post_init__(self) -> None:
        for name in (
            "value_target_name",
            "planner_name",
            "planner_version",
            "root_id",
            "split_group_id",
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
        if self.record_version == 4:
            if self.search_root_mean_value is None:
                raise ValueError("V4 PUCT search root-mean diagnostic must be present")
            if type(self.search_root_mean_value) not in {int, float}:
                raise TypeError("search root-mean value must be numeric")
            root_mean = float(self.search_root_mean_value)
            if not math.isfinite(root_mean) or not -1.0 <= root_mean <= 1.0:
                raise ValueError("search root-mean value must be finite and in [-1, 1]")
            if self.value_target_name != COMBAT_PROXY_VALUE_TARGET_NAME:
                raise ValueError("V4 records must use combat_proxy_v1 training targets")
            if self.outcome.truncated != (not self.value_target_mask):
                raise ValueError("outcome truncation and value target mask disagree")
            if self.chosen_action_index != first_argmax_visits(self.teacher_visit_counts):
                raise ValueError("V4 chosen action is not the first visit-count argmax")
        elif self.search_root_mean_value is not None:
            raise ValueError("pre-V4 records must not include search_root_mean_value")
        elif self.record_version == 3:
            if self.value_target_name != PUCT_VALUE_TARGET_NAME:
                raise ValueError("V3 records must use privileged_puct_root_mean_v1")
            # Truncated PUCT rollouts still keep the backed-up root-mean present
            # and unmasked. Only V2/V4 terminal combat_proxy_v1 values are masked
            # when the episode is truncated.
            if self.target_value is None or not self.value_target_mask:
                raise ValueError("V3 PUCT root-mean targets must be present and unmasked")
            if self.chosen_action_index != first_argmax_visits(self.teacher_visit_counts):
                raise ValueError("V3 chosen action is not the first visit-count argmax")
        elif self.outcome.truncated != (not self.value_target_mask):
            raise ValueError("outcome truncation and value target mask disagree")
        if self.record_version not in {1, 2, 3, 4}:
            raise ValueError("unsupported training record version")
        if type(self.decision_index) is not int or self.decision_index < 0:
            raise ValueError("decision index must be nonnegative")
        for name in ("source_kind", "episode_id"):
            if type(getattr(self, name)) is not str or not getattr(self, name):
                raise TypeError(f"{name} must be a nonempty string")
        if self.record_version == 2:
            validate_v2_search_config(cast(Mapping[str, object], self.search_config))
        elif self.record_version == 3:
            validate_v3_search_config(cast(Mapping[str, object], self.search_config))
        elif self.record_version == 4:
            validate_v4_search_config(cast(Mapping[str, object], self.search_config))
        frozen = _freeze_json(dict(self.search_config), "search config")
        if not isinstance(frozen, Mapping):
            raise TypeError("search config must be an object")
        object.__setattr__(self, "search_config", frozen)
        if self.record_version in {2, 3, 4}:
            for name in ("root_id", "root_manifest_digest", "reward_config_digest"):
                value = getattr(self, name)
                if (
                    type(value) is not str
                    or len(value) != 64
                    or any(character not in "0123456789abcdef" for character in value)
                ):
                    raise ValueError(f"{name} must be a lowercase SHA-256 digest")
            if self.record_version == 4:
                if self.episode_id == "legacy":
                    raise ValueError("V4 records must not use the default legacy episode ID")
                expected_episode = puct_episode_id(
                    self.root_id,
                    self.search_config,
                    cast(str, self.reward_config_digest),
                )
                if self.episode_id != expected_episode:
                    raise ValueError(
                        "V4 episode ID does not match canonical root/search/reward identity"
                    )
            identity = hashlib.sha256(
                _canonical_json(self._substantive_payload()).encode()
            ).hexdigest()
            if self.record_id is None:
                object.__setattr__(self, "record_id", identity)
            elif self.record_id != identity:
                raise ValueError("record ID is invalid")
        if fair_observation_digest(self.observation) != self.observation_digest:
            raise ValueError("fair observation digest is invalid")

    def _substantive_payload(self) -> dict[str, object]:
        payload = self.to_dict()
        payload.pop("record_id", None)
        return payload

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
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
        }
        if self.record_version in {2, 3, 4}:
            payload.update(
                {
                    "record_version": self.record_version,
                    "root_manifest_digest": self.root_manifest_digest,
                    "reward_config_digest": self.reward_config_digest,
                    "source_kind": self.source_kind,
                    "episode_id": self.episode_id,
                    "decision_index": self.decision_index,
                    "value_target_mask": self.value_target_mask,
                    "record_id": self.record_id,
                }
            )
        if self.record_version == 4:
            payload["search_root_mean_value"] = self.search_root_mean_value
        return payload

    @classmethod
    def from_dict(cls, payload: object) -> SymbolicTrainingRecord:
        raw = _dict(payload, "training record")
        search_root_mean_value: float | None = None
        if set(raw) == _LEGACY_RECORD_KEYS:
            source = raw
            record_version = 1
        elif set(raw) == _RECORD_KEYS:
            source = raw
            record_version = _integer(source["record_version"], "record version")
            if record_version not in {2, 3}:
                raise ValueError("training record has missing or unknown fields")
        elif set(raw) == _RECORD_KEYS_V4:
            source = raw
            record_version = _integer(source["record_version"], "record version")
            if record_version != 4:
                raise ValueError("training record has missing or unknown fields")
            search_root_mean_value = _number(
                source["search_root_mean_value"], "search root-mean value"
            )
        else:
            raise ValueError("training record has missing or unknown fields")
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
        assert all(
            value is not None
            for value in (
                value_target_name,
                planner_name,
                planner_version,
                root_id,
                split_group_id,
                observation_digest,
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
            root_manifest_digest=cast(str | None, source.get("root_manifest_digest")),
            reward_config_digest=cast(str | None, source.get("reward_config_digest")),
            source_kind=cast(str, source.get("source_kind", "legacy")),
            episode_id=cast(str, source.get("episode_id", "legacy")),
            decision_index=cast(int, source.get("decision_index", 0)),
            value_target_mask=cast(bool, source.get("value_target_mask", True)),
            record_id=cast(str | None, source.get("record_id")),
            search_root_mean_value=search_root_mean_value,
        )


def write_jsonl(path: Path, records: Iterable[SymbolicTrainingRecord]) -> None:
    with path.open("w", encoding="utf-8") as output:
        for record in records:
            output.write(_canonical_json(record.to_dict()))
            output.write("\n")


def read_jsonl(path: Path) -> Iterator[SymbolicTrainingRecord]:
    with path.open(encoding="utf-8") as source:
        for line_number, line in enumerate(source, 1):
            if not line.strip():
                continue
            try:
                yield SymbolicTrainingRecord.from_dict(json.loads(line))
            except (KeyError, TypeError, ValueError) as error:
                raise ValueError(f"invalid symbolic record at line {line_number}") from error


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
