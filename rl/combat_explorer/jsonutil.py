"""JSON-safe encoding, hashing, and seed string conversion."""

from __future__ import annotations

import hashlib
import json
import math
from dataclasses import asdict, is_dataclass
from enum import Enum
from typing import Any

from combat_explorer.errors import ExplorerError, InvalidSeedError


def json_safe(value: object) -> object:
    """Convert nested dataclasses/enums/tuples into JSON-encodable values."""
    if value is None or isinstance(value, (bool, str)):
        return value
    if isinstance(value, int) and not isinstance(value, bool):
        return value
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError("Non-finite float cannot be stored in explorer JSON")
        return value
    if isinstance(value, Enum):
        raw = value.value
        return json_safe(raw) if not isinstance(raw, Enum) else str(value)
    if isinstance(value, dict):
        return {str(key): json_safe(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [json_safe(item) for item in value]
    if is_dataclass(value) and not isinstance(value, type):
        return json_safe(asdict(value))
    raise TypeError(f"Cannot JSON-encode {type(value).__name__}")


def canonical_dumps(value: object) -> str:
    return json.dumps(json_safe(value), sort_keys=True, separators=(",", ":"), allow_nan=False)


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def observation_sha256(observation: object) -> str:
    payload = asdict(observation) if is_dataclass(observation) and not isinstance(observation, type) else observation
    return sha256_text(canonical_dumps(payload))


def seed_to_int(seed: object) -> int:
    if isinstance(seed, bool):
        raise ValueError("Seed must be an integer or integer string")
    if isinstance(seed, int):
        return seed
    if not isinstance(seed, str) or not seed:
        raise ValueError("Seed must be an integer or integer string")
    try:
        return int(seed, 10)
    except ValueError as error:
        raise ValueError("Seed must be an integer or integer string") from error


def parse_seed(seed: object, *, field: str = "seed") -> int:
    try:
        return seed_to_int(seed)
    except ValueError as error:
        raise InvalidSeedError(f"Invalid {field}: seed must be an integer or integer string") from error


def seed_to_str(seed: object) -> str:
    return str(seed_to_int(seed))


def finite_floats(values: list[float], label: str) -> list[float]:
    out: list[float] = []
    for value in values:
        number = float(value)
        if not math.isfinite(number):
            raise ValueError(f"Non-finite {label}")
        out.append(number)
    return out


def require_mapping(value: object, path: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ExplorerError(f"{path} must be an object", code="invalid_session")
    return value
