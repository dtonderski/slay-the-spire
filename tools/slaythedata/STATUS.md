# SlayTheData Local Status

This file is a handoff note for future Codex sessions. It records the local
machine state; the broader design and schema live in `README.md`.

## Current Data Root

Generated corpus artifacts are on `D:`, not inside the repo:

```powershell
$SLAYTHEDATA_ROOT = "D:\dev\SlayTheData-index"
```

Important paths:

- source archive: `D:\dev\SlayTheData.7z`
- SQLite locator DB: `D:\dev\SlayTheData-index\slaythedata-chunks.sqlite3`
- compressed raw run chunks: `D:\dev\SlayTheData-index\chunks\*.jsonl.zst`
- manifest cache: `D:\dev\SlayTheData-index\manifest-cache.json`
- build logs: `D:\dev\SlayTheData-index\chunk-build.out.log` and
  `D:\dev\SlayTheData-index\chunk-build.err.log`
- build PID file: `D:\dev\SlayTheData-index\chunk-build.pid`

## Build State

The full-corpus `chunk-build` was started on 2026-06-30 and may still be
running. Treat the DB as usable but potentially partial until
`archive_files.status` has no `pending` rows.

The build is locator-only:

- `runs` has scalar filters and counts.
- `chunk_files` and `chunk_runs` locate raw JSONL records inside zstd chunks.
- detailed decision child tables are not populated unless a future build uses
  `chunk-build --store-decisions`.

While the build is running, some newly indexed `runs` rows may not yet be
available in `chunk_runs` until the current chunk is flushed. For export, only
select rows that already have a `chunk_runs` entry.

Latest observed UI-fast status on 2026-06-30:

- `52,666,330` rows in `runs`
- `52,639,808` rows in `chunk_runs`
- `1,949` rows in `chunk_files`
- `31,680` archive files indexed and `13,142` pending
- supported exportable Ironclad A0 guided candidates are available for the UI
  default filters

## Check Progress

```powershell
$PY = "C:\Users\davton\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe"
$SLAYTHEDATA_ROOT = "D:\dev\SlayTheData-index"

$pid = Get-Content "$SLAYTHEDATA_ROOT\chunk-build.pid"
Get-Process -Id $pid -ErrorAction SilentlyContinue

@'
import json, os, sqlite3
root = r"D:\dev\SlayTheData-index"
db = os.path.join(root, "slaythedata-chunks.sqlite3")
manifest = os.path.join(root, "manifest-cache.json")
m = json.load(open(manifest, encoding="utf-8"))
con = sqlite3.connect(db)
indexed = con.execute(
    "select count(*), coalesce(sum(indexed_runs),0), max(file_ordinal) "
    "from archive_files where status='indexed'"
).fetchone()
chunks = con.execute(
    "select count(*), coalesce(sum(run_count),0), "
    "coalesce(sum(compressed_bytes),0), coalesce(sum(uncompressed_bytes),0) "
    "from chunk_files"
).fetchone()
done_bytes = sum((m[i].get("size") or 0) for i in range((indexed[2] or -1) + 1))
total_bytes = sum(x.get("size") or 0 for x in m)
print("indexed_files", indexed[0], "of", len(m), "file_pct", indexed[0] / len(m) * 100)
print("indexed_runs", indexed[1])
print("byte_pct", done_bytes / total_bytes * 100)
print("chunks", chunks[0], "chunked_runs", chunks[1])
print("compressed_GiB", chunks[2] / 1024**3, "uncompressed_GiB", chunks[3] / 1024**3)
print("status_counts", list(con.execute(
    "select status, count(*) from archive_files group by status"
)))
'@ | & $PY -
```

If the progress query times out while the writer is busy, wait a minute and
retry. The build process can still be healthy.

## Export Smoke Test

Use `id IN (SELECT run_id FROM chunk_runs)` while the build is incomplete.
That avoids selecting indexed rows whose raw JSON has not been flushed to a
chunk yet.

```powershell
$PY = "C:\Users\davton\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe"
$SLAYTHEDATA_ROOT = "D:\dev\SlayTheData-index"

& $PY tools\slaythedata\index_slaythedata.py chunk-export `
  --db "$SLAYTHEDATA_ROOT\slaythedata-chunks.sqlite3" `
  --chunks-dir "$SLAYTHEDATA_ROOT\chunks" `
  --where "id IN (SELECT run_id FROM chunk_runs) AND character_chosen='IRONCLAD' AND ascension_level=0 AND floor_reached BETWEEN 17 AND 33" `
  --limit 5 `
  --out "$SLAYTHEDATA_ROOT\exports\handoff-smoke.jsonl"
```

The output is JSONL. Each line contains the original run event JSON plus
`run_id` and `replay_support` metadata.

## Dependencies

Use the bundled Codex Python above. It has `zstandard` installed locally for
this workflow. If a future session is missing `zstandard` or a working `7z.exe`,
stop and tell the user rather than replacing the chunk store with a weaker
workaround.
