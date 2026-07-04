"""Small helpers for selecting SlayTheData runs for guided collection."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import sqlite3
from typing import Any

from sts.slaythedata_divergence import DEFAULT_DIVERGENT_SEEDS_PATH, load_divergent_seed_set
from sts.slaythedata_policy import build_guided_run_script


DEFAULT_SLAYTHEDATA_ROOT = Path(r"D:\dev\SlayTheData-index")
DEFAULT_SLAYTHEDATA_DB = DEFAULT_SLAYTHEDATA_ROOT / "slaythedata-chunks.sqlite3"
DEFAULT_SLAYTHEDATA_CHUNKS = DEFAULT_SLAYTHEDATA_ROOT / "chunks"
DEFAULT_INDEXER = Path(__file__).resolve().parents[3] / "tools" / "slaythedata" / "index_slaythedata.py"
_LIVE_SEED_LOOKUP_INDEX = "idx_runs_live_seed_lookup"

_GUIDED_SAFE_NEOW_BONUSES = (
    "THREE_ENEMY_KILL",
    "RANDOM_COMMON_RELIC",
    "ONE_RARE_RELIC",
    "TEN_PERCENT_HP_BONUS",
    "TWENTY_PERCENT_HP_BONUS",
    "HUNDRED_GOLD",
    "TWO_FIFTY_GOLD",
    "ONE_RANDOM_RARE_CARD",
    "RANDOM_RARE_CARD",
    "THREE_CARDS",
    "THREE_RARE_CARDS",
    "THREE_SMALL_POTIONS",
    "BOSS_RELIC",
)

_GUIDED_SAFE_NEOW_COSTS = (
    "NONE",
    "CURSE",
    "NO_GOLD",
    "TEN_PERCENT_HP_LOSS",
    "PERCENT_DAMAGE",
)


def select_guided_collection_candidates(
    db_path: str | Path = DEFAULT_SLAYTHEDATA_DB,
    *,
    character: str = "IRONCLAD",
    ascension: int = 0,
    seed_played: str | None = None,
    min_floor_reached: int = 1,
    max_floor_reached: int | None = None,
    min_path_length: int | None = None,
    min_card_choices: int | None = None,
    min_event_choices: int | None = None,
    min_shop_purchases: int | None = None,
    min_potion_usage: int | None = None,
    victory: bool | None = None,
    require_guided_safe_neow: bool = False,
    require_supported: bool = True,
    divergent_seed_path: str | Path = DEFAULT_DIVERGENT_SEEDS_PATH,
    limit: int = 50,
    ranked: bool = True,
) -> list[dict[str, Any]]:
    """Return exportable SlayTheData run candidates from the locator DB."""

    if seed_played:
        ensure_slaythedata_lookup_indexes(db_path)

    where, params = guided_collection_where(
        character=character,
        ascension=ascension,
        seed_played=seed_played,
        min_floor_reached=min_floor_reached,
        max_floor_reached=max_floor_reached,
        min_path_length=min_path_length,
        min_card_choices=min_card_choices,
        min_event_choices=min_event_choices,
        min_shop_purchases=min_shop_purchases,
        min_potion_usage=min_potion_usage,
        victory=victory,
        require_guided_safe_neow=require_guided_safe_neow,
        require_supported=require_supported,
        excluded_seeds=load_divergent_seed_set(divergent_seed_path),
    )
    conn = _connect_readonly(db_path)
    try:
        run_columns = set(_sqlite_table_columns(conn, "runs"))
        if require_guided_safe_neow and not {"neow_bonus", "neow_cost"}.issubset(run_columns):
            missing = sorted({"neow_bonus", "neow_cost"} - run_columns)
            raise ValueError(
                "SlayTheData locator database is missing guided safe-Neow column(s): "
                + ", ".join(missing)
            )
        neow_bonus_expr = "neow_bonus" if "neow_bonus" in run_columns else "NULL AS neow_bonus"
        neow_cost_expr = "neow_cost" if "neow_cost" in run_columns else "NULL AS neow_cost"
        query = f"""
            SELECT runs.id, seed_played, floor_reached, victory, path_length,
                   card_choice_count, event_choice_count, shop_purchase_count,
                   potion_usage_count, {neow_bonus_expr}, {neow_cost_expr},
                   (card_choice_count + event_choice_count * 2 + shop_purchase_count * 3 + potion_usage_count) AS guided_score
            FROM runs
            JOIN chunk_runs ON chunk_runs.run_id = runs.id
            WHERE {where}
            {_candidate_order_clause(ranked)}
            LIMIT ?
        """
        rows = _sqlite_fetchall_with_step_limit(conn, query, [*params, int(limit)])
        if rows is None:
            raise TimeoutError("SlayTheData candidate query timed out")
    finally:
        conn.close()
    return [
        {
            "id": row[0],
            "seed_played": row[1],
            "floor_reached": row[2],
            "victory": bool(row[3]),
            "path_length": row[4],
            "card_choice_count": row[5],
            "event_choice_count": row[6],
            "shop_purchase_count": row[7],
            "potion_usage_count": row[8],
            "neow_bonus": row[9],
            "neow_cost": row[10],
            "guided_score": row[11],
        }
        for row in rows
    ]


def select_seed_matching_candidates(
    db_path: str | Path = DEFAULT_SLAYTHEDATA_DB,
    *,
    seed_played: str,
    character: str = "IRONCLAD",
    ascension: int = 0,
    min_floor_reached: int = 1,
    victory: bool | None = None,
    divergent_seed_path: str | Path = DEFAULT_DIVERGENT_SEEDS_PATH,
    limit: int = 5,
) -> list[dict[str, Any]]:
    """Return exportable runs matching the exact live seed using a seed-first query."""

    if str(seed_played).strip() in load_divergent_seed_set(divergent_seed_path):
        return []

    ensure_slaythedata_lookup_indexes(db_path)
    conn = _connect_readonly(db_path)
    try:
        query = """
            SELECT id, seed_played, floor_reached, victory, path_length,
                   card_choice_count, event_choice_count, shop_purchase_count,
                   potion_usage_count
            FROM runs INDEXED BY idx_runs_live_seed_lookup
            WHERE seed_played = ?
              AND character_chosen = ?
              AND ascension_level = ?
              AND floor_reached >= ?
              {victory_clause}
              AND id IN (SELECT run_id FROM chunk_runs)
            ORDER BY floor_reached DESC, path_length DESC, id ASC
            LIMIT ?
        """
        params: list[Any] = [str(seed_played), character, int(ascension), int(min_floor_reached)]
        if victory is not None:
            params.append(1 if victory else 0)
        rows = _sqlite_fetchall_with_step_limit(
            conn,
            query.format(victory_clause="AND victory = ?" if victory is not None else ""),
            [*params, int(limit)],
            max_steps=2_000,
        )
        if rows is None:
            raise TimeoutError("SlayTheData seed candidate query timed out")
    finally:
        conn.close()
    return [
        {
            "id": row[0],
            "seed_played": row[1],
            "floor_reached": row[2],
            "victory": bool(row[3]),
            "path_length": row[4],
            "card_choice_count": row[5],
            "event_choice_count": row[6],
            "shop_purchase_count": row[7],
            "potion_usage_count": row[8],
            "guided_score": row[5] + row[6] * 2 + row[7] * 3 + row[8],
        }
        for row in rows
    ]


def ensure_slaythedata_lookup_indexes(db_path: str | Path = DEFAULT_SLAYTHEDATA_DB) -> None:
    """Create lightweight lookup indexes required by interactive UI filters."""

    path = Path(db_path)
    if not path.exists():
        raise FileNotFoundError(path)
    conn = sqlite3.connect(path, timeout=30.0)
    try:
        tables = set(_sqlite_table_names(conn))
        if "runs" not in tables:
            raise ValueError("SlayTheData locator database is missing runs table")
        conn.execute(
            f"""
            CREATE INDEX IF NOT EXISTS {_LIVE_SEED_LOOKUP_INDEX}
                ON runs(seed_played, character_chosen, ascension_level, floor_reached, unsupported_any, path_length)
            """
        )
        conn.execute("CREATE INDEX IF NOT EXISTS idx_runs_seed ON runs(seed_played)")
        conn.commit()
    finally:
        conn.close()


def _candidate_order_clause(ranked: bool) -> str:
    if not ranked:
        return ""
    return "ORDER BY path_length DESC, guided_score DESC, floor_reached DESC, id ASC"


def _placeholders(values: tuple[str, ...]) -> str:
    return ", ".join("?" for _ in values)


def slaythedata_index_status(
    db_path: str | Path = DEFAULT_SLAYTHEDATA_DB,
    *,
    character: str = "IRONCLAD",
    ascension: int = 0,
    min_floor_reached: int = 45,
    min_path_length: int | None = 45,
    include_counts: bool = False,
) -> dict[str, Any]:
    """Return a compact readiness summary for guided SlayTheData collection."""

    path = Path(db_path)
    status: dict[str, Any] = {
        "ok": False,
        "db_path": str(path),
        "exists": path.exists(),
        "problems": [],
        "warnings": [],
    }
    if not path.exists():
        status["problems"].append("SlayTheData locator database is missing")
        return status

    try:
        conn = _connect_readonly(path)
    except Exception as error:
        status["problems"].append(f"cannot open SlayTheData locator database: {error}")
        return status

    try:
        tables = set(_sqlite_table_names(conn))
        status["tables"] = sorted(tables)
        required = {"runs", "chunk_runs"}
        missing = sorted(required - tables)
        if missing:
            status["problems"].append(f"missing required table(s): {', '.join(missing)}")
            return status
        run_columns = set(_sqlite_table_columns(conn, "runs"))
        safe_neow_filter_supported = {"neow_bonus", "neow_cost"}.issubset(run_columns)
        decision_tables = {
            "run_card_choices",
            "run_events",
            "run_shop_purchases",
            "run_campfire_choices",
            "run_boss_relic_choices",
            "run_potion_usage",
            "run_potions_obtained",
        }
        offer_tables = {"run_card_offer_cards", "run_boss_relic_offer_relics"}
        status["schema_features"] = {
            "has_neow_columns": safe_neow_filter_supported,
            "has_supported_profile": "unsupported_any" in run_columns,
            "has_decision_tables": decision_tables.issubset(tables),
            "has_offer_tables": offer_tables.issubset(tables),
            "chunk_store_decisions": _sqlite_meta_value(conn, "chunk_store_decisions") == "1"
            if "index_meta" in tables
            else None,
            "store_offers": _sqlite_meta_value(conn, "store_offers") == "1"
            if "index_meta" in tables
            else None,
        }

        status["counts_included"] = bool(include_counts)
        if include_counts:
            status["runs_count"] = _sqlite_count_with_step_limit(conn, "runs")
            status["chunk_runs_count"] = _sqlite_count_with_step_limit(conn, "chunk_runs")
            status["chunk_files_count"] = (
                _sqlite_count_with_step_limit(conn, "chunk_files") if "chunk_files" in tables else None
            )
            if status["runs_count"] is None:
                status["warnings"].append("SlayTheData run count timed out")
            if status["chunk_runs_count"] is None:
                status["warnings"].append("SlayTheData export-row count timed out")
        status["archive_status_counts"] = (
            _archive_status_counts(conn) if "archive_files" in tables else {}
        )
        if "archive_files" in tables:
            pending = int(status["archive_status_counts"].get("pending", 0))
            if pending:
                status["warnings"].append(f"SlayTheData index build is partial: {pending} archive files pending")

        where, params = guided_collection_where(
            character=character.upper(),
            ascension=ascension,
            min_floor_reached=min_floor_reached,
            min_path_length=min_path_length,
            require_guided_safe_neow=safe_neow_filter_supported,
            require_supported=True,
        )
        status["candidate_filters"] = {
            "character": character.upper(),
            "ascension": ascension,
            "min_floor_reached": min_floor_reached,
            "min_path_length": min_path_length,
            "require_guided_safe_neow": safe_neow_filter_supported,
            "require_supported": True,
        }
        candidate_row = _sqlite_fetchone_with_step_limit(
            conn,
            f"""
            SELECT 1
            FROM runs
            JOIN chunk_runs ON chunk_runs.run_id = runs.id
            WHERE {where}
            LIMIT 1
            """,
            params,
        )
        if candidate_row is None:
            status["exportable_candidate_available"] = None
            status["warnings"].append("SlayTheData candidate availability check timed out")
        else:
            status["exportable_candidate_available"] = bool(candidate_row)

        runs_available = _sqlite_table_has_row(conn, "runs")
        chunk_runs_available = _sqlite_table_has_row(conn, "chunk_runs")
        status["runs_available"] = runs_available
        status["chunk_runs_available"] = chunk_runs_available

        if runs_available is False:
            status["problems"].append("SlayTheData locator database has no runs")
        elif runs_available is None:
            status["warnings"].append("SlayTheData run availability check timed out")
        if chunk_runs_available is False:
            status["problems"].append("SlayTheData locator database has no exportable chunk rows")
        elif chunk_runs_available is None:
            status["warnings"].append("SlayTheData export-row availability check timed out")
        if status["exportable_candidate_available"] is False:
            status["warnings"].append("no supported exportable runs match the guided collection filters")
        status["ok"] = not status["problems"]
        return status
    except sqlite3.Error as error:
        status["problems"].append(f"cannot read SlayTheData locator database: {error}")
        return status
    finally:
        conn.close()


def guided_collection_where(
    *,
    character: str = "IRONCLAD",
    ascension: int = 0,
    seed_played: str | None = None,
    min_floor_reached: int = 1,
    max_floor_reached: int | None = None,
    min_path_length: int | None = None,
    min_card_choices: int | None = None,
    min_event_choices: int | None = None,
    min_shop_purchases: int | None = None,
    min_potion_usage: int | None = None,
    victory: bool | None = None,
    require_guided_safe_neow: bool = False,
    require_supported: bool = True,
    excluded_seeds: set[str] | None = None,
) -> tuple[str, list[Any]]:
    clauses = [
        "character_chosen = ?",
        "ascension_level = ?",
        "floor_reached >= ?",
        "is_daily = 0",
        "is_endless = 0",
        "is_trial = 0",
    ]
    params: list[Any] = [character, ascension, min_floor_reached]
    if seed_played:
        clauses.append("seed_played = ?")
        params.append(str(seed_played))
    if excluded_seeds:
        excluded_seed_values = tuple(sorted(excluded_seeds))
        clauses.append(f"seed_played NOT IN ({_placeholders(excluded_seed_values)})")
        params.extend(excluded_seed_values)
    if max_floor_reached is not None:
        clauses.append("floor_reached <= ?")
        params.append(max_floor_reached)
    if victory is not None:
        clauses.append("victory = ?")
        params.append(1 if victory else 0)
    if min_path_length is not None:
        clauses.append("path_length >= ?")
        params.append(min_path_length)
    for column, value in (
        ("card_choice_count", min_card_choices),
        ("event_choice_count", min_event_choices),
        ("shop_purchase_count", min_shop_purchases),
        ("potion_usage_count", min_potion_usage),
    ):
        if value is not None:
            clauses.append(f"{column} >= ?")
            params.append(value)
    if require_guided_safe_neow:
        clauses.append(f"neow_bonus IN ({_placeholders(_GUIDED_SAFE_NEOW_BONUSES)})")
        params.extend(_GUIDED_SAFE_NEOW_BONUSES)
        clauses.append(f"neow_cost IN ({_placeholders(_GUIDED_SAFE_NEOW_COSTS)})")
        params.extend(_GUIDED_SAFE_NEOW_COSTS)
    if require_supported:
        clauses.append("unsupported_any = 0")
    return " AND ".join(clauses), params


def chunk_export_args(
    *,
    db_path: str | Path = DEFAULT_SLAYTHEDATA_DB,
    chunks_dir: str | Path = DEFAULT_SLAYTHEDATA_CHUNKS,
    output_path: str | Path,
    run_ids: list[int] | tuple[int, ...],
    indexer_path: str | Path = Path("tools") / "slaythedata" / "index_slaythedata.py",
) -> list[str]:
    if not run_ids:
        raise ValueError("run_ids must not be empty")
    ids = ",".join(str(int(run_id)) for run_id in run_ids)
    return [
        str(indexer_path),
        "chunk-export",
        "--db",
        str(db_path),
        "--chunks-dir",
        str(chunks_dir),
        "--where",
        f"id IN ({ids})",
        "--out",
        str(output_path),
    ]


def export_guided_run_script(
    run_id: int,
    *,
    db_path: str | Path = DEFAULT_SLAYTHEDATA_DB,
    chunks_dir: str | Path = DEFAULT_SLAYTHEDATA_CHUNKS,
    indexer_path: str | Path = DEFAULT_INDEXER,
    timeout_seconds: float = 30.0,
    runner: Any | None = None,
) -> dict[str, Any]:
    """Export one SlayTheData run from chunks and convert it to a guided script."""

    return build_guided_run_script(
        export_guided_run_row(
            run_id,
            db_path=db_path,
            chunks_dir=chunks_dir,
            indexer_path=indexer_path,
            timeout_seconds=timeout_seconds,
            runner=runner,
        )
    )


def export_guided_run_row(
    run_id: int,
    *,
    db_path: str | Path = DEFAULT_SLAYTHEDATA_DB,
    chunks_dir: str | Path = DEFAULT_SLAYTHEDATA_CHUNKS,
    indexer_path: str | Path = DEFAULT_INDEXER,
    timeout_seconds: float = 30.0,
    runner: Any | None = None,
) -> dict[str, Any]:
    """Export one raw SlayTheData chunk row from the local chunk store."""

    run_id = int(run_id)
    runner = runner or subprocess.run
    with tempfile.TemporaryDirectory(prefix="sts-slaythedata-") as tmp:
        output_path = Path(tmp) / f"run-{run_id}.jsonl"
        args = chunk_export_args(
            db_path=db_path,
            chunks_dir=chunks_dir,
            output_path=output_path,
            run_ids=[run_id],
            indexer_path=indexer_path,
        )
        command = [sys.executable, *args]
        result = runner(
            command,
            cwd=Path(__file__).resolve().parents[3],
            capture_output=True,
            text=True,
            check=False,
            timeout=timeout_seconds,
        )
        if result.returncode != 0:
            detail = (result.stderr or result.stdout or "chunk-export failed").strip()
            raise RuntimeError(detail)
        content = output_path.read_text(encoding="utf-8") if output_path.exists() else ""
        rows = [line for line in content.splitlines() if line.strip()]
        if not rows:
            raise RuntimeError(f"chunk-export produced no rows for run {run_id}")
        return json.loads(rows[0])


def _connect_readonly(db_path: str | Path) -> sqlite3.Connection:
    path = Path(db_path)
    if not path.exists():
        raise FileNotFoundError(path)
    uri = path.resolve().as_uri() + "?mode=ro"
    return sqlite3.connect(uri, uri=True, timeout=1.0)


def _sqlite_table_names(conn: sqlite3.Connection) -> list[str]:
    return [
        row[0]
        for row in conn.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
        ).fetchall()
    ]


def _sqlite_table_columns(conn: sqlite3.Connection, table: str) -> list[str]:
    if not table.replace("_", "").isalnum():
        raise ValueError(f"unsafe table name: {table}")
    return [row[1] for row in conn.execute(f"PRAGMA table_info({table})").fetchall()]


def _sqlite_count(conn: sqlite3.Connection, table: str) -> int:
    if not table.replace("_", "").isalnum():
        raise ValueError(f"unsafe table name: {table}")
    return int(conn.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])


def _sqlite_count_with_step_limit(conn: sqlite3.Connection, table: str) -> int | None:
    row = _sqlite_fetchone_with_step_limit(conn, f"SELECT COUNT(*) FROM {table}", [])
    return None if row is None else int(row[0])


def _sqlite_table_has_row(conn: sqlite3.Connection, table: str) -> bool | None:
    row = _sqlite_fetchone_with_step_limit(conn, f"SELECT 1 FROM {table} LIMIT 1", [])
    return None if row is None else bool(row)


def _archive_status_counts(conn: sqlite3.Connection) -> dict[str, int]:
    return {
        str(row[0]): int(row[1])
        for row in conn.execute("SELECT status, COUNT(*) FROM archive_files GROUP BY status").fetchall()
    }


def _sqlite_meta_value(conn: sqlite3.Connection, key: str) -> str | None:
    row = conn.execute("SELECT value FROM index_meta WHERE key = ?", [key]).fetchone()
    return None if row is None else str(row[0])


def _sqlite_fetchone_with_step_limit(
    conn: sqlite3.Connection,
    query: str,
    params: list[Any],
    *,
    max_steps: int = 2_000,
) -> tuple[Any, ...] | None:
    steps = 0

    def progress() -> int:
        nonlocal steps
        steps += 1
        return 1 if steps > max_steps else 0

    conn.set_progress_handler(progress, 1000)
    try:
        return conn.execute(query, params).fetchone()
    except sqlite3.OperationalError as error:
        if "interrupted" in str(error).lower():
            return None
        raise
    finally:
        conn.set_progress_handler(None, 0)


def _sqlite_fetchall_with_step_limit(
    conn: sqlite3.Connection,
    query: str,
    params: list[Any],
    *,
    max_steps: int = 20_000,
) -> list[tuple[Any, ...]] | None:
    steps = 0

    def progress() -> int:
        nonlocal steps
        steps += 1
        return 1 if steps > max_steps else 0

    conn.set_progress_handler(progress, 1000)
    try:
        return conn.execute(query, params).fetchall()
    except sqlite3.OperationalError as error:
        if "interrupted" in str(error).lower():
            return None
        raise
    finally:
        conn.set_progress_handler(None, 0)
