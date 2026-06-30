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
        ORDER BY path_length DESC, guided_score DESC, floor_reached DESC, id ASC
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
