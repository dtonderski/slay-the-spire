"""Explicit public vocabulary v2; enum declaration order is not a tensor ABI.

v1 identities retain their old indices. Newly registered identities append in
lexical order. The native exporter uses the same legacy-prefix rule. No hidden
simulator identity or state is involved.
"""

from __future__ import annotations

import json
from enum import StrEnum
from pathlib import Path
from typing import TypeVar

E = TypeVar("E", bound=StrEnum)
LEGACY_V1: dict[str, list[str]] = json.loads(Path(__file__).with_name("legacy_vocabulary_v1.json").read_text())


def public_vocabulary(enum: type[E]) -> dict[E, int]:
    legacy = LEGACY_V1[enum.__name__]
    if len(legacy) != len(set(legacy)):
        raise ValueError("Duplicate legacy public identities")
    keys = {str(key) for key in enum}
    if not set(legacy) <= keys:
        raise ValueError("Current catalog removed a legacy public identity")
    ordered = [*legacy, *sorted(keys - set(legacy))]
    return {enum(key): index for index, key in enumerate(ordered)}
