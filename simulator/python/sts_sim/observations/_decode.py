"""Private mapping decoders shared by observation modules."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from enum import StrEnum
from functools import lru_cache
from typing import cast


@lru_cache(maxsize=128)
def _field_names(cls: type[object]) -> frozenset[str]:
    # Public dataclass schemas are fixed at import time. Cache schema metadata,
    # not observations; every payload still gets full field/type validation.
    raw = getattr(cls, "__dataclass_fields__", None)
    if not isinstance(raw, dict):
        raise TypeError(f"{cls!r} is not a dataclass type")
    names: list[str] = []
    for key in raw:
        if not isinstance(key, str):
            raise TypeError(f"{cls!r} has a non-string dataclass field name")
        names.append(key)
    return frozenset(names)


type _RecordKey = tuple[tuple[str, type[object], object], ...]


def _record_key(value: object, depth: int = 1) -> _RecordKey | None:
    """Snapshot small plain records for memoization without type-equality aliases.

    Unknown/custom objects use the ordinary decoder. In particular bool/int and
    mutable subclasses cannot alias a previously validated record. Keys retain
    every field (including unknown ones); only successful decodes are cached.
    """
    if type(value) is not dict:
        return None
    result = []
    for key, child in value.items():
        if type(key) is not str:
            return None
        kind = type(child)
        if kind is dict and depth:
            child = _record_key(child, depth - 1)
            if child is None:
                return None
        elif kind not in (str, int, bool, type(None)):
            return None
        result.append((key, kind, child))
    return tuple(result)


def _record_mapping(key: _RecordKey) -> dict[str, object]:
    """Recreate a decoder input on a cache miss, never simulator state."""
    return {
        name: _record_mapping(cast(_RecordKey, value)) if kind is dict else value
        for name, kind, value in key
    }


def _exact(value: object, path: str, cls: type[object]) -> dict[str, object]:
    return _mapping(value, path, required=_field_names(cls))


def _mapping(
    value: object,
    path: str,
    *,
    required: frozenset[str],
    optional: frozenset[str] = frozenset(),
) -> dict[str, object]:
    if type(value) is dict:
        # Native projections are plain dicts. Copy in C, but still check every
        # key: equality with a string schema alone is not a type check.
        copied = value.copy()
        for key in copied:
            if not isinstance(key, str):
                raise TypeError(f"{path}: expected string keys, got {type(key).__name__}")
        typed = cast(dict[str, object], copied)
    else:
        if not isinstance(value, Mapping):
            raise TypeError(f"{path}: expected mapping, got {type(value).__name__}")
        typed: dict[str, object] = {}
        for key, child in value.items():
            if not isinstance(key, str):
                raise TypeError(f"{path}: expected string keys, got {type(key).__name__}")
            typed[key] = child
    keys = frozenset(typed)
    if keys == required:
        return typed
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
