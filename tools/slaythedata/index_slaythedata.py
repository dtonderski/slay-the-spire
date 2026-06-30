#!/usr/bin/env python3
"""Lean SQLite indexer for SlayTheData run-history archives.

The full SlayTheData archive is a large solid-compressed 7z file. This tool
streams JSON files through 7-Zip stdout and stores a normalized query index.
It does not store full run JSON by default; selected candidates can be
materialized later from the archive.
"""

from __future__ import annotations

import argparse
import json
import os
import sqlite3
import subprocess
import time
import zstandard as zstd
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence


SCHEMA_VERSION = "5"
DEFAULT_ARCHIVE = r"D:\dev\SlayTheData.7z"
DEFAULT_DB = "slaythedata.sqlite3"
DEFAULT_7Z_CANDIDATES = [
    r"C:\Program Files\Lenovo\Lenovo AI Now\7Zip\7z.exe",
    r"C:\Program Files\NVIDIA Corporation\NVIDIA app\7z.exe",
    "7z",
]


@dataclass(frozen=True)
class ArchiveFile:
    ordinal: int
    name: str
    size: int | None


def compact_json(value: Any) -> str:
    return json.dumps(value if value is not None else [], separators=(",", ":"), ensure_ascii=True)


def truthy(value: Any) -> int:
    return 1 if bool(value) else 0


def parse_int(value: Any) -> int | None:
    if value is None:
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def strip_upgrade_suffix(card_name: Any) -> str:
    text = str(card_name)
    return text.split("+", 1)[0] if "+" in text else text


def first_existing_7z() -> str:
    for candidate in DEFAULT_7Z_CANDIDATES:
        if os.path.isabs(candidate):
            if os.path.exists(candidate):
                return candidate
        else:
            return candidate
    return DEFAULT_7Z_CANDIDATES[0]


def run_7z(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def load_archive_manifest(seven_zip: str, archive: str, cache_path: Path | None) -> list[ArchiveFile]:
    if cache_path and cache_path.exists():
        rows = json.loads(cache_path.read_text(encoding="utf-8"))
        return [ArchiveFile(row["ordinal"], row["name"], row.get("size")) for row in rows]

    proc = run_7z([seven_zip, "l", "-slt", archive])
    files: list[ArchiveFile] = []
    current: dict[str, str] = {}

    def flush() -> None:
        if not current:
            return
        name = current.get("Path")
        if name and name.lower().endswith(".json"):
            files.append(ArchiveFile(len(files), name, parse_int(current.get("Size"))))
        current.clear()

    for line in proc.stdout.splitlines():
        if not line.strip():
            flush()
            continue
        if " = " in line:
            key, value = line.split(" = ", 1)
            current[key] = value
    flush()

    if cache_path:
        cache_path.parent.mkdir(parents=True, exist_ok=True)
        cache_path.write_text(
            json.dumps([file.__dict__ for file in files], indent=2, ensure_ascii=True),
            encoding="utf-8",
        )

    return files


def init_db(conn: sqlite3.Connection) -> None:
    conn.executescript(
        """
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS index_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS archive_files (
            source_file TEXT PRIMARY KEY,
            file_ordinal INTEGER NOT NULL,
            uncompressed_size INTEGER,
            status TEXT NOT NULL DEFAULT 'pending',
            indexed_runs INTEGER NOT NULL DEFAULT 0,
            indexed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY,
            source_file TEXT NOT NULL,
            source_file_ordinal INTEGER NOT NULL,
            source_run_ordinal INTEGER NOT NULL,
            play_id TEXT,
            local_time TEXT,
            timestamp INTEGER,
            character_chosen TEXT,
            ascension_level INTEGER,
            build_version TEXT,
            is_beta INTEGER,
            is_prod INTEGER,
            is_daily INTEGER,
            is_endless INTEGER,
            is_trial INTEGER,
            chose_seed INTEGER,
            victory INTEGER,
            floor_reached INTEGER,
            killed_by TEXT,
            score INTEGER,
            playtime INTEGER,
            seed_played TEXT,
            seed_source_timestamp TEXT,
            special_seed TEXT,
            neow_bonus TEXT,
            neow_cost TEXT,
            path_taken_json TEXT NOT NULL,
            path_per_floor_json TEXT NOT NULL,
            gold INTEGER,
            final_deck_count INTEGER,
            final_relic_count INTEGER,
            potion_usage_count INTEGER,
            potion_spawn_count INTEGER,
            potion_obtained_count INTEGER,
            card_choice_count INTEGER,
            event_choice_count INTEGER,
            shop_purchase_count INTEGER,
            campfire_choice_count INTEGER,
            boss_relic_choice_count INTEGER,
            combat_count INTEGER,
            path_length INTEGER,
            has_potion_usage INTEGER,
            has_shop_purchase INTEGER,
            has_event_choice INTEGER,
            has_boss_relic_choice INTEGER,
            unsupported_character INTEGER,
            unsupported_ascension INTEGER,
            unsupported_build INTEGER,
            unsupported_any INTEGER,
            UNIQUE(source_file, source_run_ordinal)
        );

        CREATE TABLE IF NOT EXISTS run_relics_obtained (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER,
            relic TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS run_card_choices (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER,
            picked TEXT,
            picked_base TEXT,
            skipped INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS run_card_offer_cards (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            choice_ordinal INTEGER NOT NULL,
            card_ordinal INTEGER NOT NULL,
            card TEXT NOT NULL,
            base_card TEXT NOT NULL,
            picked INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS run_events (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER,
            event_name TEXT,
            player_choice TEXT,
            damage_taken INTEGER,
            damage_healed INTEGER,
            max_hp_gain INTEGER,
            max_hp_loss INTEGER,
            gold_gain INTEGER,
            gold_loss INTEGER
        );

        CREATE TABLE IF NOT EXISTS run_event_items (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            event_ordinal INTEGER NOT NULL,
            kind TEXT NOT NULL,
            item_ordinal INTEGER NOT NULL,
            item TEXT NOT NULL,
            base_item TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS run_shop_purchases (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER,
            item TEXT NOT NULL,
            base_item TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS run_campfire_choices (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER,
            key TEXT,
            data TEXT,
            base_data TEXT
        );

        CREATE TABLE IF NOT EXISTS run_boss_relic_choices (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            picked TEXT
        );

        CREATE TABLE IF NOT EXISTS run_boss_relic_offer_relics (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            choice_ordinal INTEGER NOT NULL,
            relic_ordinal INTEGER NOT NULL,
            relic TEXT NOT NULL,
            picked INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS run_potion_usage (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER
        );

        CREATE TABLE IF NOT EXISTS run_potion_spawned (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER
        );

        CREATE TABLE IF NOT EXISTS run_potions_obtained (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER,
            potion TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS run_damage_taken (
            run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL,
            floor INTEGER,
            enemies TEXT,
            damage INTEGER,
            turns INTEGER
        );

        CREATE TABLE IF NOT EXISTS run_materialized_json (
            run_id INTEGER PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
            raw_event_json TEXT NOT NULL,
            materialized_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS chunk_files (
            chunk_id INTEGER PRIMARY KEY,
            chunk_path TEXT NOT NULL UNIQUE,
            compression TEXT NOT NULL,
            first_run_id INTEGER,
            last_run_id INTEGER,
            run_count INTEGER NOT NULL,
            uncompressed_bytes INTEGER NOT NULL,
            compressed_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS chunk_runs (
            run_id INTEGER PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
            chunk_id INTEGER NOT NULL REFERENCES chunk_files(chunk_id) ON DELETE CASCADE,
            line_number INTEGER NOT NULL,
            line_bytes INTEGER NOT NULL,
            UNIQUE(chunk_id, line_number)
        );

        CREATE INDEX IF NOT EXISTS idx_runs_candidate
            ON runs(character_chosen, ascension_level, floor_reached, build_version, victory);
        CREATE INDEX IF NOT EXISTS idx_runs_seed ON runs(seed_played);
        CREATE INDEX IF NOT EXISTS idx_runs_potions ON runs(has_potion_usage, potion_usage_count);
        CREATE INDEX IF NOT EXISTS idx_runs_unsupported ON runs(unsupported_any);
        CREATE INDEX IF NOT EXISTS idx_runs_source ON runs(source_file, source_run_ordinal);
        CREATE INDEX IF NOT EXISTS idx_relics_obtained_relic ON run_relics_obtained(relic);
        CREATE INDEX IF NOT EXISTS idx_card_choices_picked ON run_card_choices(picked_base);
        CREATE INDEX IF NOT EXISTS idx_card_offer_base ON run_card_offer_cards(base_card);
        CREATE INDEX IF NOT EXISTS idx_events_name ON run_events(event_name);
        CREATE INDEX IF NOT EXISTS idx_shop_item ON run_shop_purchases(base_item);
        CREATE INDEX IF NOT EXISTS idx_campfire_key ON run_campfire_choices(key);
        CREATE INDEX IF NOT EXISTS idx_potions_obtained ON run_potions_obtained(potion);
        CREATE INDEX IF NOT EXISTS idx_damage_enemies ON run_damage_taken(enemies);
        CREATE INDEX IF NOT EXISTS idx_chunk_runs_chunk ON chunk_runs(chunk_id, line_number);
        """
    )


def require_schema(conn: sqlite3.Connection) -> None:
    existing = conn.execute("SELECT value FROM index_meta WHERE key = 'schema_version'").fetchone()
    if existing and existing[0] != SCHEMA_VERSION:
        raise SystemExit(
            f"DB schema_version is {existing[0]}, but this tool expects {SCHEMA_VERSION}. "
            "Create a new DB or rebuild the old one."
        )


def set_meta(conn: sqlite3.Connection, key: str, value: str) -> None:
    conn.execute(
        "INSERT INTO index_meta(key, value) VALUES(?, ?) "
        "ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )


def upsert_manifest(conn: sqlite3.Connection, files: Iterable[ArchiveFile]) -> None:
    conn.executemany(
        "INSERT INTO archive_files(source_file, file_ordinal, uncompressed_size) VALUES(?, ?, ?) "
        "ON CONFLICT(source_file) DO UPDATE SET "
        "file_ordinal = excluded.file_ordinal, uncompressed_size = excluded.uncompressed_size",
        [(file.name, file.ordinal, file.size) for file in files],
    )


def load_supported_content(path: Path | None) -> dict[str, set[str] | set[int]]:
    if not path:
        return {}
    data = json.loads(path.read_text(encoding="utf-8"))
    supported: dict[str, set[str] | set[int]] = {}
    for key, values in data.items():
        if key == "ascension_levels":
            supported[key] = {int(value) for value in values}
        else:
            supported[key] = {str(value) for value in values}
    return supported


def unsupported_values(values: Iterable[Any], supported: set[str] | set[int] | None) -> list[str]:
    if supported is None:
        return []
    return sorted({str(value) for value in values if value is not None and str(value) not in supported})


def card_names_from_choices(card_choices: list[dict[str, Any]]) -> list[str]:
    names: list[str] = []
    for choice in card_choices:
        picked = choice.get("picked")
        if picked and picked != "SKIP":
            names.append(strip_upgrade_suffix(picked))
        for card in choice.get("not_picked") or []:
            names.append(strip_upgrade_suffix(card))
    return names


def list_values(items: Iterable[Any], key: str) -> list[Any]:
    return [item.get(key) for item in items if isinstance(item, dict) and item.get(key) is not None]


def run_scalars(
    event: dict[str, Any],
    source_file: ArchiveFile,
    run_ordinal: int,
    supported: dict[str, set[str] | set[int]],
) -> tuple[tuple[Any, ...], dict[str, list[str]], int]:
    master_deck = event.get("master_deck") or []
    relics = event.get("relics") or []
    card_choices = event.get("card_choices") or []
    event_choices = event.get("event_choices") or []
    items_purchased = event.get("items_purchased") or []
    campfire_choices = event.get("campfire_choices") or []
    boss_relics = event.get("boss_relics") or []
    potions_floor_usage = event.get("potions_floor_usage") or []
    potions_floor_spawned = event.get("potions_floor_spawned") or []
    potions_obtained = event.get("potions_obtained") or []
    damage_taken = event.get("damage_taken") or []
    path_taken = event.get("path_taken") or []

    characters = supported.get("characters")
    build_versions = supported.get("build_versions")
    ascension_levels = supported.get("ascension_levels")
    ascension = parse_int(event.get("ascension_level"))

    unsupported_character = int(characters is not None and str(event.get("character_chosen")) not in characters)
    unsupported_ascension = int(ascension_levels is not None and ascension not in ascension_levels)
    unsupported_build = int(build_versions is not None and str(event.get("build_version")) not in build_versions)

    unsupported = {
        "card": unsupported_values(
            [strip_upgrade_suffix(card) for card in master_deck] + card_names_from_choices(card_choices),
            supported.get("cards"),
        ),
        "relic": unsupported_values(relics, supported.get("relics")),
        "potion": unsupported_values(list_values(potions_obtained, "key"), supported.get("potions")),
        "event": unsupported_values(list_values(event_choices, "event_name"), supported.get("events")),
        "shop_item": unsupported_values(items_purchased, supported.get("shop_items")),
    }
    unsupported_any = int(
        unsupported_character
        or unsupported_ascension
        or unsupported_build
        or any(unsupported.values())
    )

    row = (
        source_file.name,
        source_file.ordinal,
        run_ordinal,
        event.get("play_id"),
        event.get("local_time"),
        parse_int(event.get("timestamp")),
        event.get("character_chosen"),
        ascension,
        event.get("build_version"),
        truthy(event.get("is_beta")),
        truthy(event.get("is_prod")),
        truthy(event.get("is_daily")),
        truthy(event.get("is_endless")),
        truthy(event.get("is_trial")),
        truthy(event.get("chose_seed")),
        truthy(event.get("victory")),
        parse_int(event.get("floor_reached")),
        event.get("killed_by"),
        parse_int(event.get("score")),
        parse_int(event.get("playtime")),
        str(event.get("seed_played")) if event.get("seed_played") is not None else None,
        str(event.get("seed_source_timestamp")) if event.get("seed_source_timestamp") is not None else None,
        str(event.get("special_seed")) if event.get("special_seed") is not None else None,
        event.get("neow_bonus"),
        event.get("neow_cost"),
        compact_json(event.get("path_taken")),
        compact_json(event.get("path_per_floor")),
        parse_int(event.get("gold")),
        len(master_deck),
        len(relics),
        len(potions_floor_usage),
        len(potions_floor_spawned),
        len(potions_obtained),
        len(card_choices),
        len(event_choices),
        len(items_purchased),
        len(campfire_choices),
        len(boss_relics),
        len(damage_taken),
        len(path_taken),
        truthy(potions_floor_usage),
        truthy(items_purchased),
        truthy(event_choices),
        truthy(boss_relics),
        unsupported_character,
        unsupported_ascension,
        unsupported_build,
        unsupported_any,
    )
    return row, unsupported, unsupported_any


RUN_INSERT_SQL = """
INSERT OR REPLACE INTO runs (
    source_file, source_file_ordinal, source_run_ordinal, play_id, local_time, timestamp,
    character_chosen, ascension_level, build_version, is_beta, is_prod, is_daily,
    is_endless, is_trial, chose_seed, victory, floor_reached, killed_by, score,
    playtime, seed_played, seed_source_timestamp, special_seed, neow_bonus, neow_cost,
    path_taken_json, path_per_floor_json, gold, final_deck_count, final_relic_count, potion_usage_count, potion_spawn_count,
    potion_obtained_count, card_choice_count, event_choice_count, shop_purchase_count,
    campfire_choice_count, boss_relic_choice_count, combat_count, path_length,
    has_potion_usage, has_shop_purchase, has_event_choice, has_boss_relic_choice,
    unsupported_character, unsupported_ascension, unsupported_build, unsupported_any
) VALUES (
    ?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?
)
ON CONFLICT(source_file, source_run_ordinal) DO UPDATE SET
    play_id=excluded.play_id,
    local_time=excluded.local_time,
    timestamp=excluded.timestamp,
    character_chosen=excluded.character_chosen,
    ascension_level=excluded.ascension_level,
    build_version=excluded.build_version,
    is_beta=excluded.is_beta,
    is_prod=excluded.is_prod,
    is_daily=excluded.is_daily,
    is_endless=excluded.is_endless,
    is_trial=excluded.is_trial,
    chose_seed=excluded.chose_seed,
    victory=excluded.victory,
    floor_reached=excluded.floor_reached,
    killed_by=excluded.killed_by,
    score=excluded.score,
    playtime=excluded.playtime,
    seed_played=excluded.seed_played,
    seed_source_timestamp=excluded.seed_source_timestamp,
    special_seed=excluded.special_seed,
    neow_bonus=excluded.neow_bonus,
    neow_cost=excluded.neow_cost,
    path_taken_json=excluded.path_taken_json,
    path_per_floor_json=excluded.path_per_floor_json,
    gold=excluded.gold,
    final_deck_count=excluded.final_deck_count,
    final_relic_count=excluded.final_relic_count,
    potion_usage_count=excluded.potion_usage_count,
    potion_spawn_count=excluded.potion_spawn_count,
    potion_obtained_count=excluded.potion_obtained_count,
    card_choice_count=excluded.card_choice_count,
    event_choice_count=excluded.event_choice_count,
    shop_purchase_count=excluded.shop_purchase_count,
    campfire_choice_count=excluded.campfire_choice_count,
    boss_relic_choice_count=excluded.boss_relic_choice_count,
    combat_count=excluded.combat_count,
    path_length=excluded.path_length,
    has_potion_usage=excluded.has_potion_usage,
    has_shop_purchase=excluded.has_shop_purchase,
    has_event_choice=excluded.has_event_choice,
    has_boss_relic_choice=excluded.has_boss_relic_choice,
    unsupported_character=excluded.unsupported_character,
    unsupported_ascension=excluded.unsupported_ascension,
    unsupported_build=excluded.unsupported_build,
    unsupported_any=excluded.unsupported_any
"""


def delete_child_rows(conn: sqlite3.Connection, run_id: int) -> None:
    tables = [
        "run_relics_obtained",
        "run_card_choices",
        "run_card_offer_cards",
        "run_events",
        "run_event_items",
        "run_shop_purchases",
        "run_campfire_choices",
        "run_boss_relic_choices",
        "run_boss_relic_offer_relics",
        "run_potion_usage",
        "run_potion_spawned",
        "run_potions_obtained",
        "run_damage_taken",
        "run_materialized_json",
    ]
    for table in tables:
        conn.execute(f"DELETE FROM {table} WHERE run_id = ?", (run_id,))


def insert_run(
    conn: sqlite3.Connection,
    event: dict[str, Any],
    source_file: ArchiveFile,
    run_ordinal: int,
    supported: dict[str, set[str] | set[int]],
    store_offers: bool,
) -> int:
    row, unsupported, _ = run_scalars(event, source_file, run_ordinal, supported)
    conn.execute(RUN_INSERT_SQL, row)
    run_id = conn.execute(
        "SELECT id FROM runs WHERE source_file = ? AND source_run_ordinal = ?",
        (source_file.name, run_ordinal),
    ).fetchone()[0]
    delete_child_rows(conn, run_id)

    insert_relics_obtained(conn, run_id, event.get("relics_obtained") or [])
    insert_card_choices(conn, run_id, event.get("card_choices") or [], store_offers)
    insert_events(conn, run_id, event.get("event_choices") or [])
    insert_shop_purchases(conn, run_id, event.get("items_purchased") or [], event.get("item_purchase_floors") or [])
    insert_campfire_choices(conn, run_id, event.get("campfire_choices") or [])
    insert_boss_relic_choices(conn, run_id, event.get("boss_relics") or [], store_offers)
    insert_floor_list(conn, "run_potion_usage", run_id, event.get("potions_floor_usage") or [])
    insert_floor_list(conn, "run_potion_spawned", run_id, event.get("potions_floor_spawned") or [])
    insert_potions_obtained(conn, run_id, event.get("potions_obtained") or [])
    insert_damage_taken(conn, run_id, event.get("damage_taken") or [])
    return run_id


def insert_locator_run(
    conn: sqlite3.Connection,
    event: dict[str, Any],
    source_file: ArchiveFile,
    run_ordinal: int,
    supported: dict[str, set[str] | set[int]],
) -> int:
    row, _unsupported, _ = run_scalars(event, source_file, run_ordinal, supported)
    conn.execute(RUN_INSERT_SQL, row)
    return conn.execute(
        "SELECT id FROM runs WHERE source_file = ? AND source_run_ordinal = ?",
        (source_file.name, run_ordinal),
    ).fetchone()[0]


def value_at(values: Sequence[Any], index: int) -> Any:
    return values[index] if index < len(values) else None

def insert_relics_obtained(conn: sqlite3.Connection, run_id: int, relics: list[dict[str, Any]]) -> None:
    rows = []
    for i, relic in enumerate(relics):
        if isinstance(relic, dict) and relic.get("key") is not None:
            rows.append((run_id, i, parse_int(relic.get("floor")), str(relic.get("key"))))
    conn.executemany("INSERT INTO run_relics_obtained VALUES (?, ?, ?, ?)", rows)


def insert_card_choices(conn: sqlite3.Connection, run_id: int, choices: list[dict[str, Any]], store_offers: bool) -> None:
    choice_rows = []
    offer_rows = []
    for i, choice in enumerate(choices):
        if not isinstance(choice, dict):
            continue
        picked = choice.get("picked")
        skipped = picked == "SKIP"
        picked_base = None if skipped or picked is None else strip_upgrade_suffix(picked)
        choice_rows.append((run_id, i, parse_int(choice.get("floor")), picked, picked_base, int(skipped)))

        if store_offers:
            offer_index = 0
            if picked and not skipped:
                offer_rows.append((run_id, i, offer_index, str(picked), strip_upgrade_suffix(picked), 1))
                offer_index += 1
            for card in choice.get("not_picked") or []:
                offer_rows.append((run_id, i, offer_index, str(card), strip_upgrade_suffix(card), 0))
                offer_index += 1
    conn.executemany("INSERT INTO run_card_choices VALUES (?, ?, ?, ?, ?, ?)", choice_rows)
    conn.executemany("INSERT INTO run_card_offer_cards VALUES (?, ?, ?, ?, ?, ?)", offer_rows)


def insert_events(conn: sqlite3.Connection, run_id: int, events: list[dict[str, Any]]) -> None:
    event_rows = []
    item_rows = []
    item_keys = {
        "cards_obtained": "card_obtained",
        "cards_removed": "card_removed",
        "cards_upgraded": "card_upgraded",
        "relics_obtained": "relic_obtained",
    }
    for i, event in enumerate(events):
        if not isinstance(event, dict):
            continue
        event_rows.append(
            (
                run_id,
                i,
                parse_int(event.get("floor")),
                event.get("event_name"),
                event.get("player_choice"),
                parse_int(event.get("damage_taken")),
                parse_int(event.get("damage_healed")),
                parse_int(event.get("max_hp_gain")),
                parse_int(event.get("max_hp_loss")),
                parse_int(event.get("gold_gain")),
                parse_int(event.get("gold_loss")),
            )
        )
        for source_key, kind in item_keys.items():
            for item_i, item in enumerate(event.get(source_key) or []):
                item_rows.append((run_id, i, kind, item_i, str(item), strip_upgrade_suffix(item)))
    conn.executemany("INSERT INTO run_events VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)", event_rows)
    conn.executemany("INSERT INTO run_event_items VALUES (?, ?, ?, ?, ?, ?)", item_rows)


def insert_shop_purchases(conn: sqlite3.Connection, run_id: int, items: list[Any], floors: list[Any]) -> None:
    rows = [
        (run_id, i, parse_int(value_at(floors, i)), str(item), strip_upgrade_suffix(item))
        for i, item in enumerate(items)
    ]
    conn.executemany("INSERT INTO run_shop_purchases VALUES (?, ?, ?, ?, ?)", rows)


def insert_campfire_choices(conn: sqlite3.Connection, run_id: int, choices: list[dict[str, Any]]) -> None:
    rows = []
    for i, choice in enumerate(choices):
        if isinstance(choice, dict):
            data = choice.get("data")
            rows.append((run_id, i, parse_int(choice.get("floor")), choice.get("key"), data, strip_upgrade_suffix(data) if data else None))
    conn.executemany("INSERT INTO run_campfire_choices VALUES (?, ?, ?, ?, ?, ?)", rows)


def insert_boss_relic_choices(conn: sqlite3.Connection, run_id: int, choices: list[dict[str, Any]], store_offers: bool) -> None:
    choice_rows = []
    offer_rows = []
    for i, choice in enumerate(choices):
        if not isinstance(choice, dict):
            continue
        picked = choice.get("picked")
        choice_rows.append((run_id, i, picked))
        if store_offers:
            offer_index = 0
            if picked:
                offer_rows.append((run_id, i, offer_index, str(picked), 1))
                offer_index += 1
            for relic in choice.get("not_picked") or []:
                offer_rows.append((run_id, i, offer_index, str(relic), 0))
                offer_index += 1
    conn.executemany("INSERT INTO run_boss_relic_choices VALUES (?, ?, ?)", choice_rows)
    conn.executemany("INSERT INTO run_boss_relic_offer_relics VALUES (?, ?, ?, ?, ?)", offer_rows)


def insert_floor_list(conn: sqlite3.Connection, table: str, run_id: int, floors: list[Any]) -> None:
    rows = [(run_id, i, parse_int(floor)) for i, floor in enumerate(floors)]
    conn.executemany(f"INSERT INTO {table} VALUES (?, ?, ?)", rows)


def insert_potions_obtained(conn: sqlite3.Connection, run_id: int, potions: list[dict[str, Any]]) -> None:
    rows = []
    for i, potion in enumerate(potions):
        if isinstance(potion, dict) and potion.get("key") is not None:
            rows.append((run_id, i, parse_int(potion.get("floor")), str(potion.get("key"))))
    conn.executemany("INSERT INTO run_potions_obtained VALUES (?, ?, ?, ?)", rows)


def insert_damage_taken(conn: sqlite3.Connection, run_id: int, damage: list[dict[str, Any]]) -> None:
    rows = []
    for i, entry in enumerate(damage):
        if isinstance(entry, dict):
            rows.append(
                (
                    run_id,
                    i,
                    parse_int(entry.get("floor")),
                    entry.get("enemies"),
                    parse_int(entry.get("damage")),
                    parse_int(entry.get("turns")),
                )
            )
    conn.executemany("INSERT INTO run_damage_taken VALUES (?, ?, ?, ?, ?, ?)", rows)


def iter_concatenated_json_arrays(stdout: Any, expected_values: int) -> Iterable[list[Any]]:
    decoder = json.JSONDecoder()
    buffer = ""
    decoded = 0
    while True:
        chunk = stdout.read(1024 * 1024)
        if chunk:
            buffer += chunk.decode("utf-8")

        while buffer:
            buffer = buffer.lstrip()
            try:
                value, end = decoder.raw_decode(buffer)
            except json.JSONDecodeError:
                break
            if not isinstance(value, list):
                raise ValueError(f"expected JSON array from archive stream, got {type(value).__name__}")
            decoded += 1
            yield value
            buffer = buffer[end:]

        if not chunk:
            break

    if buffer.strip():
        raise ValueError("archive stream ended with incomplete JSON data")
    if decoded != expected_values:
        raise ValueError(f"decoded {decoded} JSON arrays, expected {expected_values}")


def index_run_array(
    conn: sqlite3.Connection,
    file: ArchiveFile,
    run_array: list[Any],
    supported: dict[str, set[str] | set[int]],
    store_offers: bool,
) -> int:
    count = 0
    for run_ordinal, wrapper in enumerate(run_array):
        if not isinstance(wrapper, dict):
            continue
        event = wrapper.get("event")
        if not isinstance(event, dict):
            continue
        insert_run(conn, event, file, run_ordinal, supported, store_offers)
        count += 1
    conn.execute(
        "UPDATE archive_files SET status = 'indexed', indexed_runs = ?, indexed_at = datetime('now') WHERE source_file = ?",
        (count, file.name),
    )
    return count


def index_file_batch(
    conn: sqlite3.Connection,
    seven_zip: str,
    archive: str,
    files: list[ArchiveFile],
    supported: dict[str, set[str] | set[int]],
    store_offers: bool,
) -> int:
    if not files:
        return 0
    started = time.time()
    proc = subprocess.Popen([seven_zip, "e", "-so", archive, *[file.name for file in files]], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert proc.stdout is not None
    indexed_runs = 0
    try:
        for file, run_array in zip(files, iter_concatenated_json_arrays(proc.stdout, len(files)), strict=True):
            count = index_run_array(conn, file, run_array, supported, store_offers)
            indexed_runs += count
            conn.commit()
            print(f"indexed {file.name}: {count} runs", flush=True)
    finally:
        stderr = proc.stderr.read().decode("utf-8", errors="replace") if proc.stderr else ""
        return_code = proc.wait()
        if return_code != 0:
            raise RuntimeError(f"7z exited with {return_code}\n{stderr}")
    print(f"batch complete: {len(files)} files, {indexed_runs} runs, {time.time() - started:.1f}s", flush=True)
    return indexed_runs


def index_stream_all(
    conn: sqlite3.Connection,
    seven_zip: str,
    archive: str,
    manifest: list[ArchiveFile],
    selected: list[ArchiveFile],
    supported: dict[str, set[str] | set[int]],
    store_offers: bool,
) -> int:
    selected_names = {file.name for file in selected}
    if not selected_names:
        return 0
    started = time.time()
    proc = subprocess.Popen([seven_zip, "e", "-so", archive, "*.json"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert proc.stdout is not None
    indexed_runs = 0
    indexed_files = 0
    try:
        for file, run_array in zip(manifest, iter_concatenated_json_arrays(proc.stdout, len(manifest)), strict=True):
            if file.name not in selected_names:
                continue
            count = index_run_array(conn, file, run_array, supported, store_offers)
            indexed_runs += count
            indexed_files += 1
            conn.commit()
            print(f"indexed {file.name}: {count} runs", flush=True)
            if indexed_files >= len(selected_names):
                proc.terminate()
                break
    finally:
        stderr = proc.stderr.read().decode("utf-8", errors="replace") if proc.stderr else ""
        return_code = proc.wait()
        if return_code not in (0, 1):
            raise RuntimeError(f"7z exited with {return_code}\n{stderr}")
    print(f"stream complete: {indexed_files} files, {indexed_runs} runs, {time.time() - started:.1f}s", flush=True)
    return indexed_runs


class ChunkWriter:
    def __init__(self, conn: sqlite3.Connection, chunks_dir: Path, target_bytes: int, level: int) -> None:
        self.conn = conn
        self.chunks_dir = chunks_dir
        self.target_bytes = target_bytes
        self.level = level
        self.chunk_index = self._next_chunk_index()
        self.lines: list[tuple[int, bytes]] = []
        self.uncompressed_bytes = 0
        self.total_runs = 0
        self.chunks_dir.mkdir(parents=True, exist_ok=True)

    def _next_chunk_index(self) -> int:
        row = self.conn.execute("SELECT COALESCE(MAX(chunk_id), -1) + 1 FROM chunk_files").fetchone()
        return int(row[0])

    def add(self, run_id: int, event: dict[str, Any]) -> None:
        line = json.dumps(
            {
                "run_id": run_id,
                "event": event,
                "replay_support": {
                    "run_level_choices": True,
                    "exact_combat_actions": False,
                    "potion_usage_has_floor_only": True,
                },
            },
            separators=(",", ":"),
            ensure_ascii=True,
        ).encode("utf-8") + b"\n"
        self.lines.append((run_id, line))
        self.uncompressed_bytes += len(line)
        self.total_runs += 1
        if self.uncompressed_bytes >= self.target_bytes:
            self.flush()

    def flush(self) -> None:
        if not self.lines:
            return

        chunk_id = self.chunk_index
        relative_path = Path("chunks") / f"{chunk_id:06d}.jsonl.zst"
        output_path = self.chunks_dir / relative_path.name
        payload = b"".join(line for _, line in self.lines)
        compressed = zstd.ZstdCompressor(level=self.level).compress(payload)
        output_path.write_bytes(compressed)

        first_run_id = self.lines[0][0]
        last_run_id = self.lines[-1][0]
        self.conn.execute(
            """
            INSERT INTO chunk_files(
                chunk_id, chunk_path, compression, first_run_id, last_run_id,
                run_count, uncompressed_bytes, compressed_bytes
            ) VALUES (?, ?, 'zstd', ?, ?, ?, ?, ?)
            """,
            (
                chunk_id,
                str(relative_path).replace("\\", "/"),
                first_run_id,
                last_run_id,
                len(self.lines),
                self.uncompressed_bytes,
                len(compressed),
            ),
        )
        self.conn.executemany(
            "INSERT OR REPLACE INTO chunk_runs(run_id, chunk_id, line_number, line_bytes) VALUES (?, ?, ?, ?)",
            [(run_id, chunk_id, i, len(line)) for i, (run_id, line) in enumerate(self.lines)],
        )
        self.conn.commit()

        print(
            f"wrote chunk {chunk_id:06d}: {len(self.lines)} runs, "
            f"{self.uncompressed_bytes} -> {len(compressed)} bytes",
            flush=True,
        )
        self.chunk_index += 1
        self.lines = []
        self.uncompressed_bytes = 0


def select_files(
    conn: sqlite3.Connection,
    manifest: list[ArchiveFile],
    explicit_files: list[str],
    limit_files: int | None,
    only_missing: bool,
    start_file_index: int,
) -> list[ArchiveFile]:
    by_name = {file.name: file for file in manifest}
    if explicit_files:
        selected = []
        for name in explicit_files:
            if name not in by_name:
                raise ValueError(f"archive file not found in manifest: {name}")
            selected.append(by_name[name])
    else:
        selected = [file for file in manifest if file.ordinal >= start_file_index]

    if only_missing:
        done = {row[0] for row in conn.execute("SELECT source_file FROM archive_files WHERE status = 'indexed'")}
        selected = [file for file in selected if file.name not in done]
    return selected[:limit_files] if limit_files is not None else selected


def print_summary(conn: sqlite3.Connection) -> None:
    indexed_files = conn.execute(
        "SELECT COUNT(*), COALESCE(SUM(indexed_runs), 0), COALESCE(SUM(uncompressed_size), 0) "
        "FROM archive_files WHERE status = 'indexed'"
    ).fetchone()
    total_files = conn.execute("SELECT COUNT(*) FROM archive_files").fetchone()[0]
    total_runs = conn.execute("SELECT COUNT(*) FROM runs").fetchone()[0]
    page_count, page_size = conn.execute("PRAGMA page_count").fetchone()[0], conn.execute("PRAGMA page_size").fetchone()[0]
    db_bytes = page_count * page_size
    per_run = db_bytes / total_runs if total_runs else 0
    print(f"files indexed: {indexed_files[0]}/{total_files}")
    print(f"runs indexed: {total_runs} (archive_files sum: {indexed_files[1]})")
    print(f"db pages: {db_bytes} bytes ({per_run:.1f} bytes/run)")
    if indexed_files[2]:
        print(f"indexed source bytes: {indexed_files[2]} ({db_bytes / indexed_files[2]:.3f}x indexed source JSON)")
    print("top candidate slices:")
    for row in conn.execute(
        """
        SELECT character_chosen, build_version, ascension_level, COUNT(*) AS runs
        FROM runs
        GROUP BY character_chosen, build_version, ascension_level
        ORDER BY runs DESC
        LIMIT 10
        """
    ):
        print(f"  {row[0]} {row[1]} A{row[2]}: {row[3]}")


def setup_db(db_path: Path, args: argparse.Namespace, manifest: list[ArchiveFile]) -> sqlite3.Connection:
    if db_path.parent != Path(""):
        db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db_path)
    init_db(conn)
    require_schema(conn)
    set_meta(conn, "archive", str(Path(args.archive)))
    set_meta(conn, "seven_zip", args.seven_zip)
    set_meta(conn, "manifest_file_count", str(len(manifest)))
    set_meta(conn, "schema_version", SCHEMA_VERSION)
    set_meta(conn, "store_offers", "1" if getattr(args, "store_offers", False) else "0")
    upsert_manifest(conn, manifest)
    conn.commit()
    return conn


def command_index(args: argparse.Namespace) -> int:
    archive = str(Path(args.archive))
    manifest = load_archive_manifest(args.seven_zip, archive, Path(args.manifest_cache) if args.manifest_cache else None)
    conn = setup_db(Path(args.db), args, manifest)
    print(f"manifest files: {len(manifest)}")
    if args.list_only:
        print_summary(conn)
        return 0

    supported = load_supported_content(Path(args.supported_content) if args.supported_content else None)
    selected = select_files(conn, manifest, args.file, args.limit_files, args.only_missing, args.start_file_index)
    print(f"selected files: {len(selected)}")
    if args.stream_all:
        index_stream_all(conn, args.seven_zip, archive, manifest, selected, supported, args.store_offers)
    else:
        for offset in range(0, len(selected), args.batch_files):
            index_file_batch(conn, args.seven_zip, archive, selected[offset : offset + args.batch_files], supported, args.store_offers)
    conn.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    print_summary(conn)
    return 0


def index_run_array_with_chunks(
    conn: sqlite3.Connection,
    file: ArchiveFile,
    run_array: list[Any],
    supported: dict[str, set[str] | set[int]],
    writer: ChunkWriter,
    store_decisions: bool,
) -> int:
    count = 0
    for run_ordinal, wrapper in enumerate(run_array):
        if not isinstance(wrapper, dict):
            continue
        event = wrapper.get("event")
        if not isinstance(event, dict):
            continue
        if store_decisions:
            run_id = insert_run(conn, event, file, run_ordinal, supported, store_offers=False)
        else:
            run_id = insert_locator_run(conn, event, file, run_ordinal, supported)
        writer.add(run_id, event)
        count += 1
    conn.execute(
        "UPDATE archive_files SET status = 'indexed', indexed_runs = ?, indexed_at = datetime('now') WHERE source_file = ?",
        (count, file.name),
    )
    return count


def chunk_build_batch(
    conn: sqlite3.Connection,
    seven_zip: str,
    archive: str,
    files: list[ArchiveFile],
    supported: dict[str, set[str] | set[int]],
    writer: ChunkWriter,
    store_decisions: bool,
) -> int:
    if not files:
        return 0
    started = time.time()
    proc = subprocess.Popen([seven_zip, "e", "-so", archive, *[file.name for file in files]], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert proc.stdout is not None
    indexed_runs = 0
    try:
        for file, run_array in zip(files, iter_concatenated_json_arrays(proc.stdout, len(files)), strict=True):
            count = index_run_array_with_chunks(conn, file, run_array, supported, writer, store_decisions)
            indexed_runs += count
            conn.commit()
            print(f"chunk-indexed {file.name}: {count} runs", flush=True)
    finally:
        stderr = proc.stderr.read().decode("utf-8", errors="replace") if proc.stderr else ""
        return_code = proc.wait()
        if return_code != 0:
            raise RuntimeError(f"7z exited with {return_code}\n{stderr}")
    print(f"chunk batch complete: {len(files)} files, {indexed_runs} runs, {time.time() - started:.1f}s", flush=True)
    return indexed_runs


def chunk_build_stream_all(
    conn: sqlite3.Connection,
    seven_zip: str,
    archive: str,
    manifest: list[ArchiveFile],
    selected: list[ArchiveFile],
    supported: dict[str, set[str] | set[int]],
    writer: ChunkWriter,
    store_decisions: bool,
) -> int:
    selected_names = {file.name for file in selected}
    if not selected_names:
        return 0
    started = time.time()
    proc = subprocess.Popen([seven_zip, "e", "-so", archive, "*.json"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert proc.stdout is not None
    indexed_runs = 0
    indexed_files = 0
    try:
        for file, run_array in zip(manifest, iter_concatenated_json_arrays(proc.stdout, len(manifest)), strict=True):
            if file.name not in selected_names:
                continue
            count = index_run_array_with_chunks(conn, file, run_array, supported, writer, store_decisions)
            indexed_runs += count
            indexed_files += 1
            conn.commit()
            print(f"chunk-indexed {file.name}: {count} runs", flush=True)
            if indexed_files >= len(selected_names):
                proc.terminate()
                break
    finally:
        stderr = proc.stderr.read().decode("utf-8", errors="replace") if proc.stderr else ""
        return_code = proc.wait()
        if return_code not in (0, 1):
            raise RuntimeError(f"7z exited with {return_code}\n{stderr}")
    print(f"chunk stream complete: {indexed_files} files, {indexed_runs} runs, {time.time() - started:.1f}s", flush=True)
    return indexed_runs


def command_chunk_build(args: argparse.Namespace) -> int:
    archive = str(Path(args.archive))
    manifest = load_archive_manifest(args.seven_zip, archive, Path(args.manifest_cache) if args.manifest_cache else None)
    conn = setup_db(Path(args.db), args, manifest)
    set_meta(conn, "chunk_store", "jsonl.zst")
    set_meta(conn, "chunk_target_bytes", str(args.chunk_bytes))
    set_meta(conn, "zstandard_version", zstd.__version__)
    set_meta(conn, "chunk_store_decisions", "1" if args.store_decisions else "0")
    conn.commit()

    print(f"manifest files: {len(manifest)}")
    supported = load_supported_content(Path(args.supported_content) if args.supported_content else None)
    selected = select_files(conn, manifest, args.file, args.limit_files, args.only_missing, args.start_file_index)
    print(f"selected files: {len(selected)}")
    writer = ChunkWriter(conn, Path(args.chunks_dir), args.chunk_bytes, args.zstd_level)
    if args.stream_all:
        chunk_build_stream_all(conn, args.seven_zip, archive, manifest, selected, supported, writer, args.store_decisions)
    else:
        for offset in range(0, len(selected), args.batch_files):
            chunk_build_batch(conn, args.seven_zip, archive, selected[offset : offset + args.batch_files], supported, writer, args.store_decisions)
    writer.flush()
    conn.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    print_summary(conn)
    chunk_summary(conn)
    return 0


def chunk_summary(conn: sqlite3.Connection) -> None:
    row = conn.execute(
        "SELECT COUNT(*), COALESCE(SUM(run_count), 0), COALESCE(SUM(uncompressed_bytes), 0), COALESCE(SUM(compressed_bytes), 0) FROM chunk_files"
    ).fetchone()
    chunk_count, run_count, raw_bytes, compressed_bytes = row
    ratio = compressed_bytes / raw_bytes if raw_bytes else 0
    print(f"chunks: {chunk_count}, chunked runs: {run_count}")
    print(f"chunk bytes: {raw_bytes} -> {compressed_bytes} ({ratio:.3f}x)")


def chunk_rows_for_targets(conn: sqlite3.Connection, where: str | None, limit: int | None) -> list[tuple[int, int, int, str]]:
    sql = """
        SELECT r.id, cr.chunk_id, cr.line_number, cf.chunk_path
        FROM runs r
        JOIN chunk_runs cr ON cr.run_id = r.id
        JOIN chunk_files cf ON cf.chunk_id = cr.chunk_id
    """
    params: list[Any] = []
    if where:
        sql += f" WHERE {where}"
    sql += " ORDER BY cr.chunk_id, cr.line_number"
    if limit is not None:
        sql += " LIMIT ?"
        params.append(limit)
    return [(int(row[0]), int(row[1]), int(row[2]), str(row[3])) for row in conn.execute(sql, params)]


def command_chunk_export(args: argparse.Namespace) -> int:
    conn = sqlite3.connect(args.db)
    require_schema(conn)
    targets = chunk_rows_for_targets(conn, args.where, args.limit)
    if not targets:
        print("selected chunked runs: 0")
        return 0
    by_chunk: dict[tuple[int, str], dict[int, int]] = {}
    for run_id, chunk_id, line_number, chunk_path in targets:
        by_chunk.setdefault((chunk_id, chunk_path), {})[line_number] = run_id

    chunks_dir = Path(args.chunks_dir)
    total = 0
    if args.out:
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    out = open(args.out, "w", encoding="utf-8") if args.out else None
    try:
        for (_chunk_id, chunk_path), wanted in by_chunk.items():
            path = chunks_dir / Path(chunk_path).name
            payload = zstd.ZstdDecompressor().decompress(path.read_bytes())
            for line_number, line in enumerate(payload.splitlines()):
                if line_number not in wanted:
                    continue
                text = line.decode("utf-8")
                if args.store:
                    data = json.loads(text)
                    conn.execute(
                        "INSERT OR REPLACE INTO run_materialized_json(run_id, raw_event_json, materialized_at) VALUES(?, ?, datetime('now'))",
                        (wanted[line_number], compact_json(data["event"])),
                    )
                if out:
                    out.write(text)
                    out.write("\n")
                total += 1
            conn.commit()
    finally:
        if out:
            out.close()
    print(f"chunk-exported runs: {total}")
    return 0


def selected_runs(conn: sqlite3.Connection, where: str | None, limit: int | None) -> list[tuple[int, str, int]]:
    sql = "SELECT id, source_file, source_run_ordinal FROM runs"
    params: list[Any] = []
    if where:
        sql += f" WHERE {where}"
    sql += " ORDER BY source_file_ordinal, source_run_ordinal"
    if limit is not None:
        sql += " LIMIT ?"
        params.append(limit)
    return [(int(row[0]), str(row[1]), int(row[2])) for row in conn.execute(sql, params)]


def materialize_rows(args: argparse.Namespace) -> int:
    conn = sqlite3.connect(args.db)
    require_schema(conn)
    targets = selected_runs(conn, args.where, args.limit)
    if not targets:
        print("selected runs: 0")
        return 0
    by_file: dict[str, list[tuple[int, int]]] = {}
    for run_id, source_file, run_ordinal in targets:
        by_file.setdefault(source_file, []).append((run_id, run_ordinal))

    archive = str(Path(args.archive))
    total = 0
    if args.out:
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    out = open(args.out, "w", encoding="utf-8") if args.out else None
    try:
        for source_file, wanted in by_file.items():
            wanted_by_ordinal = {ordinal: run_id for run_id, ordinal in wanted}
            proc = subprocess.Popen([args.seven_zip, "e", "-so", archive, source_file], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            assert proc.stdout is not None
            arrays = list(iter_concatenated_json_arrays(proc.stdout, 1))
            stderr = proc.stderr.read().decode("utf-8", errors="replace") if proc.stderr else ""
            return_code = proc.wait()
            if return_code != 0:
                raise RuntimeError(f"7z exited with {return_code}\n{stderr}")
            for ordinal, wrapper in enumerate(arrays[0]):
                run_id = wanted_by_ordinal.get(ordinal)
                if run_id is None or not isinstance(wrapper, dict) or not isinstance(wrapper.get("event"), dict):
                    continue
                raw = compact_json(wrapper["event"])
                if args.store:
                    conn.execute(
                        "INSERT OR REPLACE INTO run_materialized_json(run_id, raw_event_json, materialized_at) "
                        "VALUES(?, ?, datetime('now'))",
                        (run_id, raw),
                    )
                if out:
                    out.write(json.dumps({"run_id": run_id, "source_file": source_file, "source_run_ordinal": ordinal, "event": wrapper["event"]}, separators=(",", ":"), ensure_ascii=True))
                    out.write("\n")
                total += 1
            conn.commit()
    finally:
        if out:
            out.close()
    print(f"materialized/exported runs: {total}")
    return 0


def add_common_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--archive", default=DEFAULT_ARCHIVE, help="Path to SlayTheData.7z")
    parser.add_argument("--seven-zip", default=first_existing_7z(), help="Path to 7z.exe")
    parser.add_argument("--db", default=DEFAULT_DB, help="SQLite DB path")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    index = subparsers.add_parser("index", help="Build or resume the lean index")
    add_common_args(index)
    index.add_argument("--manifest-cache", default=None, help="Optional JSON cache for archive file list")
    index.add_argument("--supported-content", default=None, help="Optional JSON profile used for unsupported flags")
    index.add_argument("--file", action="append", default=[], help="Specific archive JSON file to index; may repeat")
    index.add_argument("--limit-files", type=int, default=None, help="Index only the first N selected files")
    index.add_argument("--batch-files", type=int, default=25, help="Archive files per targeted 7z batch")
    index.add_argument("--start-file-index", type=int, default=0, help="Start at this manifest ordinal")
    index.add_argument("--only-missing", action="store_true", help="Skip files already marked indexed")
    index.add_argument("--stream-all", action="store_true", help="Stream *.json once in archive order; best for full indexing")
    index.add_argument("--store-offers", action="store_true", help="Also index not-picked card and boss relic offer options; disabled by default to keep full-corpus DB lean")
    index.add_argument("--list-only", action="store_true", help="Only list and record archive manifest")
    index.set_defaults(func=command_index)

    materialize = subparsers.add_parser("materialize", help="Export/store full JSON for selected indexed runs")
    add_common_args(materialize)
    materialize.add_argument("--where", default=None, help="SQL WHERE clause over runs, for selected candidate materialization")
    materialize.add_argument("--limit", type=int, default=None, help="Maximum selected runs to materialize")
    materialize.add_argument("--out", default=None, help="Optional JSONL export path")
    materialize.add_argument("--store", action="store_true", help="Also store raw event JSON in run_materialized_json")
    materialize.set_defaults(func=materialize_rows)

    chunk_build = subparsers.add_parser("chunk-build", help="Build locator DB plus compressed chunks/*.jsonl.zst")
    add_common_args(chunk_build)
    chunk_build.add_argument("--chunks-dir", required=True, help="Directory for generated .jsonl.zst chunks")
    chunk_build.add_argument("--manifest-cache", default=None, help="Optional JSON cache for archive file list")
    chunk_build.add_argument("--supported-content", default=None, help="Optional JSON profile used for unsupported flags")
    chunk_build.add_argument("--file", action="append", default=[], help="Specific archive JSON file to chunk-index; may repeat")
    chunk_build.add_argument("--limit-files", type=int, default=None, help="Chunk-index only the first N selected files")
    chunk_build.add_argument("--batch-files", type=int, default=25, help="Archive files per targeted 7z batch")
    chunk_build.add_argument("--start-file-index", type=int, default=0, help="Start at this manifest ordinal")
    chunk_build.add_argument("--only-missing", action="store_true", help="Skip files already marked indexed")
    chunk_build.add_argument("--stream-all", action="store_true", help="Stream *.json once in archive order; best for full chunk builds")
    chunk_build.add_argument("--store-decisions", action="store_true", help="Also populate decision child tables while building chunks; slower and larger than the default locator-only full build")
    chunk_build.add_argument("--chunk-bytes", type=int, default=128 * 1024 * 1024, help="Target uncompressed bytes per chunk")
    chunk_build.add_argument("--zstd-level", type=int, default=10, help="zstd compression level")
    chunk_build.set_defaults(func=command_chunk_build)

    chunk_export = subparsers.add_parser("chunk-export", help="Export/store selected runs from local .jsonl.zst chunks")
    chunk_export.add_argument("--db", default=DEFAULT_DB, help="SQLite DB path")
    chunk_export.add_argument("--chunks-dir", required=True, help="Directory containing generated .jsonl.zst chunks")
    chunk_export.add_argument("--where", default=None, help="SQL WHERE clause over runs")
    chunk_export.add_argument("--limit", type=int, default=None, help="Maximum selected runs to export")
    chunk_export.add_argument("--out", default=None, help="Optional JSONL export path")
    chunk_export.add_argument("--store", action="store_true", help="Also store raw event JSON in run_materialized_json")
    chunk_export.set_defaults(func=command_chunk_export)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
