"""Small helpers for selecting SlayTheData runs for guided collection."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import sqlite3
from typing import Any

from sts.slaythedata_policy import load_guided_run_script


DEFAULT_SLAYTHEDATA_ROOT = Path(r"D:\dev\SlayTheData-index")
DEFAULT_SLAYTHEDATA_DB = DEFAULT_SLAYTHEDATA_ROOT / "slaythedata-chunks.sqlite3"
DEFAULT_SLAYTHEDATA_CHUNKS = DEFAULT_SLAYTHEDATA_ROOT / "chunks"
DEFAULT_INDEXER = Path(__file__).resolve().parents[3] / "tools" / "slaythedata" / "index_slaythedata.py"


def select_guided_collection_candidates(
    db_path: str | Path = DEFAULT_SLAYTHEDATA_DB,
    *,
    character: str = "IRONCLAD",
    ascension: int = 0,
    min_floor_reached: int = 1,
    max_floor_reached: int | None = None,
    min_path_length: int | None = None,
    min_card_choices: int | None = None,
    min_event_choices: int | None = None,
    min_shop_purchases: int | None = None,
    min_potion_usage: int | None = None,
    require_supported: bool = True,
    limit: int = 50,
    ranked: bool = True,
) -> list[dict[str, Any]]:
    """Return exportable SlayTheData run candidates from the locator DB."""

    where, params = guided_collection_where(
        character=character,
        ascension=ascension,
        min_floor_reached=min_floor_reached,
        max_floor_reached=max_floor_reached,
        min_path_length=min_path_length,
        min_card_choices=min_card_choices,
        min_event_choices=min_event_choices,
        min_shop_purchases=min_shop_purchases,
        min_potion_usage=min_potion_usage,
        require_supported=require_supported,
    )
    query = f"""
        SELECT id, seed_played, floor_reached, victory, path_length,
               card_choice_count, event_choice_count, shop_purchase_count,
               potion_usage_count,
               (card_choice_count + event_choice_count * 2 + shop_purchase_count * 3 + potion_usage_count) AS guided_score
        FROM runs
        WHERE {where}
        {_candidate_order_clause(ranked)}
        LIMIT ?
    """
    conn = _connect_readonly(db_path)
    try:
        rows = conn.execute(query, [*params, int(limit)]).fetchall()
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
            "guided_score": row[9],
        }
        for row in rows
    ]


def _candidate_order_clause(ranked: bool) -> str:
    if not ranked:
        return ""
    return "ORDER BY path_length DESC, guided_score DESC, floor_reached DESC, id ASC"


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
            require_supported=True,
        )
        status["candidate_filters"] = {
            "character": character.upper(),
            "ascension": ascension,
            "min_floor_reached": min_floor_reached,
            "min_path_length": min_path_length,
            "require_supported": True,
        }
        candidate_row = _sqlite_fetchone_with_step_limit(
            conn,
            f"SELECT 1 FROM runs WHERE {where} LIMIT 1",
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
    min_floor_reached: int = 1,
    max_floor_reached: int | None = None,
    min_path_length: int | None = None,
    min_card_choices: int | None = None,
    min_event_choices: int | None = None,
    min_shop_purchases: int | None = None,
    min_potion_usage: int | None = None,
    require_supported: bool = True,
) -> tuple[str, list[Any]]:
    clauses = [
        "id IN (SELECT run_id FROM chunk_runs)",
        "character_chosen = ?",
        "ascension_level = ?",
        "floor_reached >= ?",
        "is_daily = 0",
        "is_endless = 0",
        "is_trial = 0",
    ]
    params: list[Any] = [character, ascension, min_floor_reached]
    if max_floor_reached is not None:
        clauses.append("floor_reached <= ?")
        params.append(max_floor_reached)
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
        if not output_path.exists() or not output_path.read_text(encoding="utf-8").strip():
            raise RuntimeError(f"chunk-export produced no rows for run {run_id}")
        return load_guided_run_script(output_path)


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
