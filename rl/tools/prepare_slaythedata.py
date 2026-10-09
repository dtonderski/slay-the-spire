"""Stage original SlayTheData records in SQLite; do not infer missing gameplay history.

Requires py7zr only when --extract is supplied. Source files commit atomically;
rerunning skips committed files after checking their size. Raw archive/JSON files
are retained. Views expand logged arrays on demand rather than duplicating them.
"""

import argparse
import hashlib
import json
import shutil
import sqlite3
import time
from collections import Counter
from pathlib import Path

SCHEMA = """
CREATE TABLE IF NOT EXISTS files (
 id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL, bytes INTEGER NOT NULL,
 sha256 TEXT NOT NULL, records INTEGER NOT NULL, field_counts_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS runs (
 id INTEGER PRIMARY KEY, file_id INTEGER NOT NULL REFERENCES files(id),
 record_index INTEGER NOT NULL, play_id TEXT, character TEXT, ascension INTEGER,
 build_version TEXT, floor_reached INTEGER, victory INTEGER, raw_json TEXT NOT NULL,
 UNIQUE(file_id, record_index)
);
CREATE INDEX IF NOT EXISTS runs_cohort ON runs(character, ascension, build_version, floor_reached);
CREATE INDEX IF NOT EXISTS runs_play_id ON runs(play_id);
CREATE TABLE IF NOT EXISTS import_errors(name TEXT PRIMARY KEY, error TEXT NOT NULL);
"""


def initialize(connection: sqlite3.Connection) -> None:
    connection.execute("PRAGMA foreign_keys=ON")
    connection.execute("PRAGMA journal_mode=WAL")
    connection.execute("PRAGMA synchronous=FULL")
    connection.execute("PRAGMA cache_size=-65536")
    connection.executescript(SCHEMA)
    for name in (
        "card_choices",
        "event_choices",
        "campfire_choices",
        "relics_obtained",
        "potions_obtained",
        "damage_taken",
    ):
        connection.execute(f"""CREATE VIEW IF NOT EXISTS {name} AS
            SELECT r.id AS run_id, CAST(j.key AS INTEGER) AS ordinal,
                   json_extract(j.value, '$.floor') AS floor, j.value AS payload_json
            FROM runs r, json_each(r.raw_json, '$.event.{name}') j""")
    for name in (
        "master_deck",
        "relics",
        "boss_relics",
        "path_per_floor",
        "path_taken",
        "gold_per_floor",
        "current_hp_per_floor",
        "max_hp_per_floor",
        "potions_floor_usage",
        "potions_floor_spawned",
    ):
        connection.execute(f"""CREATE VIEW IF NOT EXISTS {name} AS
            SELECT r.id AS run_id, CAST(j.key AS INTEGER) AS ordinal, j.value AS value
            FROM runs r, json_each(r.raw_json, '$.event.{name}') j""")
    for name, floors in (("items_purchased", "item_purchase_floors"), ("items_purged", "items_purged_floors")):
        connection.execute(f"""CREATE VIEW IF NOT EXISTS {name} AS
            SELECT r.id AS run_id, CAST(j.key AS INTEGER) AS ordinal, j.value AS item,
                   json_extract(r.raw_json, '$.event.{floors}[' || j.key || ']') AS floor
            FROM runs r, json_each(r.raw_json, '$.event.{name}') j""")
    connection.commit()


def ingest(connection: sqlite3.Connection, path: Path) -> int:
    """Keep duplicate play IDs; source file and record ordinal identify each record."""
    previous = connection.execute("SELECT bytes FROM files WHERE name=?", (path.name,)).fetchone()
    if previous:
        if previous[0] != path.stat().st_size:
            raise ValueError(f"Previously imported source size changed: {path}")
        return 0
    data = path.read_bytes()
    records = json.loads(data)
    if not isinstance(records, list):
        raise TypeError("Expected a JSON array of event-wrapped run records")
    fields = Counter()
    with connection:
        cursor = connection.execute(
            "INSERT INTO files(name,bytes,sha256,records,field_counts_json) VALUES(?,?,?,?,?)",
            (path.name, len(data), hashlib.sha256(data).hexdigest(), len(records), "{}"),
        )
        file_id = cursor.lastrowid
        for ordinal, record in enumerate(records):
            if not isinstance(record, dict) or not isinstance(record.get("event"), dict):
                raise TypeError(f"Record {ordinal} has an unsupported wrapper; whole file rolled back")
            event = record["event"]
            fields.update(event.keys())
            connection.execute(
                """INSERT INTO runs(file_id,record_index,play_id,character,ascension,build_version,
                                    floor_reached,victory,raw_json) VALUES(?,?,?,?,?,?,?,?,?)""",
                (
                    file_id,
                    ordinal,
                    event.get("play_id"),
                    event.get("character_chosen"),
                    event.get("ascension_level"),
                    event.get("build_version"),
                    event.get("floor_reached"),
                    event.get("victory"),
                    json.dumps(record, separators=(",", ":"), ensure_ascii=False, allow_nan=False),
                ),
            )
        connection.execute("UPDATE files SET field_counts_json=? WHERE id=?", (json.dumps(fields), file_id))
        connection.execute("DELETE FROM import_errors WHERE name=?", (path.name,))
    return len(records)


def extract(archive: Path, destination: Path) -> None:
    import py7zr

    destination.mkdir(parents=True, exist_ok=True)
    marker = destination / ".extraction-complete.json"
    identity = {
        "archive": str(archive.resolve()),
        "bytes": archive.stat().st_size,
        "mtime_ns": archive.stat().st_mtime_ns,
    }
    if marker.exists():
        if json.loads(marker.read_text()) != identity:
            raise ValueError("Archive differs from completed extraction")
        return
    with py7zr.SevenZipFile(archive) as source:
        for name in source.getnames():
            if Path(name).name != name or not name.endswith(".json"):
                raise ValueError(f"Unexpected archive member path: {name!r}")
        size = source.archiveinfo().uncompressed
        if shutil.disk_usage(destination).free < 2.5 * size:
            raise RuntimeError("Insufficient free space for extraction plus SQLite staging")
        source.extractall(path=destination)
    marker.write_text(json.dumps(identity))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, required=True)
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--extract", type=Path)
    args = parser.parse_args()
    args.database.parent.mkdir(parents=True, exist_ok=True)
    progress = args.database.with_suffix(".progress.json")

    def report(stage: str, **values) -> None:
        payload = {"stage": stage, "timestamp": time.time(), **values}
        temporary = progress.with_suffix(".tmp")
        temporary.write_text(json.dumps(payload, indent=2))
        temporary.replace(progress)
        print(json.dumps(payload), flush=True)

    try:
        if args.extract:
            report("extracting")
            extract(args.extract, args.source_dir)
        paths = sorted(args.source_dir.glob("*.json"))
        paths = [path for path in paths if not path.name.startswith(".")]
        if not paths:
            raise ValueError("No input JSON files found")
        with sqlite3.connect(args.database) as connection:
            initialize(connection)
            count = connection.execute("SELECT coalesce(sum(records),0) FROM files").fetchone()[0]
            for index, path in enumerate(paths):
                try:
                    count += ingest(connection, path)
                except (ValueError, TypeError, sqlite3.IntegrityError) as error:
                    with connection:
                        connection.execute("INSERT OR REPLACE INTO import_errors VALUES(?,?)", (path.name, str(error)))
                    print(f"IMPORT ERROR {path.name}: {error}", flush=True)
                if index % 25 == 0 or index + 1 == len(paths):
                    report("importing", files_visited=index + 1, files_total=len(paths), imported_records=count)
            report("checking_database", imported_records=count)
            check = connection.execute("PRAGMA quick_check").fetchall()
            if check != [("ok",)]:
                raise RuntimeError(f"SQLite quick_check failed: {check}")
            errors = connection.execute("SELECT count(*) FROM import_errors").fetchone()[0]
            connection.execute("PRAGMA wal_checkpoint(TRUNCATE)")
            report("complete_with_errors" if errors else "complete", imported_records=count, failed_files=errors)
    except Exception as error:
        report("failed", error=f"{type(error).__name__}: {error}")
        raise


if __name__ == "__main__":
    main()
