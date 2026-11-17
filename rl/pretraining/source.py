"""Read-only run sampling and provenance, with explicit reproducible splits."""

import hashlib
import json
import random
import sqlite3
from collections import Counter
from pathlib import Path

FLAGS = ("is_beta", "is_daily", "is_trial", "is_endless", "chose_seed")


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def split_for(play_id: str) -> str:
    """Existing 10% historical holdout stays test; carve validation out of fit."""
    value = int.from_bytes(hashlib.sha256(play_id.encode()).digest()[:8], "big")
    if value % 10 == 0:
        return "test"
    return "validation" if value % 100 < 10 else "train"


def ordinary(event: dict) -> None:
    if event.get("character_chosen") != "IRONCLAD":
        raise ValueError("not_ironclad")
    if type(event.get("ascension_level")) is not int or not 0 <= event["ascension_level"] <= 20:
        raise ValueError("invalid_ascension")
    for flag in FLAGS:
        value = event.get(flag)
        if value is not False and not (type(value) is int and value == 0):
            raise ValueError(f"excluded_or_missing_{flag}")
    if not isinstance(event.get("play_id"), str) or not event["play_id"]:
        raise ValueError("missing_play_id")


def shuffled_records(database: Path, *, ascension: int, build: str, seed: int, min_floor: int = 0):
    """Uniform shuffled row IDs before filtering; no raw JSON OFFSET scans.

    Caller stops after its declared acceptance count; filters must not inspect
    map compatibility or model performance. Duplicate play IDs are rejected by
    callers. This is a record sample, not a player sample.
    """
    with sqlite3.connect(f"{database.resolve().as_uri()}?mode=ro", uri=True) as connection:
        connection.execute("PRAGMA query_only=ON")
        ids = [
            row[0]
            for row in connection.execute(
                "SELECT id FROM runs WHERE character='IRONCLAD' AND ascension=? "
                "AND build_version=? AND floor_reached>=? ORDER BY id",
                (ascension, build, min_floor),
            )
        ]
        random.Random(seed).shuffle(ids)
        for row_id in ids:
            file_id, ordinal, raw = connection.execute(
                "SELECT file_id,record_index,raw_json FROM runs WHERE id=?", (row_id,)
            ).fetchone()
            record = json.loads(raw)
            yield (
                record["event"],
                {
                    "row_id": row_id,
                    "file_id": file_id,
                    "record_index": ordinal,
                    "raw_json_sha256": digest(raw.encode()),
                    "candidate_population": len(ids),
                },
            )


def write_json(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n")


def histogram(values) -> dict:
    return dict(sorted(Counter(values).items()))
