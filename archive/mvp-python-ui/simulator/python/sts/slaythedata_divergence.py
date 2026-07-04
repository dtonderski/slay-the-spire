"""Known divergent SlayTheData seed filtering."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


DEFAULT_DIVERGENT_SEEDS_PATH = Path(__file__).with_name("slaythedata_divergent_seeds.json")
_STS_SEED_ALPHABET = "0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ"
_UINT64_MASK = (1 << 64) - 1


def load_divergent_seed_set(path: str | Path = DEFAULT_DIVERGENT_SEEDS_PATH) -> set[str]:
    """Load SlayTheData seeds that should be skipped for guided collection."""

    denylist_path = Path(path)
    if not denylist_path.exists():
        return set()
    try:
        data = json.loads(denylist_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"invalid divergent SlayTheData seed file {denylist_path}: {error}") from error

    if isinstance(data, dict):
        seeds = data.get("seeds", [])
    else:
        seeds = data
    if not isinstance(seeds, list):
        raise ValueError(f"divergent SlayTheData seed file {denylist_path} must contain a seed list")

    divergent: set[str] = set()
    for seed in seeds:
        divergent.update(seed_variants(seed))
    return divergent


def is_divergent_seed(seed: Any, path: str | Path = DEFAULT_DIVERGENT_SEEDS_PATH) -> bool:
    return _canonical_seed(seed) in load_divergent_seed_set(path)


def filter_divergent_seed_rows(
    rows: list[dict[str, Any]],
    path: str | Path = DEFAULT_DIVERGENT_SEEDS_PATH,
) -> list[dict[str, Any]]:
    divergent = load_divergent_seed_set(path)
    if not divergent:
        return rows
    return [row for row in rows if _canonical_seed(row.get("seed_played")) not in divergent]


def _canonical_seed(seed: Any) -> str:
    return str(seed or "").strip()


def seed_variants(seed: Any) -> set[str]:
    """Return exact SlayTheData seed strings that should be treated as equivalent."""

    canonical = _canonical_seed(seed)
    if not canonical:
        return set()
    variants = {canonical}
    if _is_integer_seed(canonical):
        playable = _sts_seed_long_to_string(int(canonical))
        if playable:
            variants.add(playable)
    else:
        raw = _sts_seed_string_to_long(canonical)
        if raw is not None:
            variants.add(str(raw))
    return variants


def _is_integer_seed(value: str) -> bool:
    body = value[1:] if value.startswith("-") else value
    return bool(body) and body.isdigit()


def _sts_seed_long_to_string(value: int) -> str:
    remaining = value & _UINT64_MASK
    if remaining == 0:
        return ""
    radix = len(_STS_SEED_ALPHABET)
    encoded = ""
    while remaining > 0:
        encoded = _STS_SEED_ALPHABET[remaining % radix] + encoded
        remaining //= radix
    return encoded


def _sts_seed_string_to_long(value: str) -> int | None:
    total = 0
    for char in value.strip().upper().replace("O", "0"):
        digit = _STS_SEED_ALPHABET.find(char)
        if digit < 0:
            return None
        total = total * len(_STS_SEED_ALPHABET) + digit
    total &= _UINT64_MASK
    if total >= (1 << 63):
        total -= 1 << 64
    return total
