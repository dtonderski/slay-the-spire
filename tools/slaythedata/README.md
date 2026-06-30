# SlayTheData Index And Chunk Store

This directory contains a reproducible SQLite indexing pipeline for
`D:\dev\SlayTheData.7z`.

For this machine's current data root, build state, and handoff commands, see
`STATUS.md`.

The archive is large and solid-compressed: about 28 GiB compressed, about
370 GiB uncompressed, and about 44k JSON files. Do not extract it permanently.
The indexer streams JSON files through `7z e -so`, writes a query index, and can
also repack raw run events into local `.jsonl.zst` chunks. The chunk store is the
recommended long-term shape: SQLite finds candidate runs, and chunk export
decompresses only the chunk(s) that contain selected runs.

The chunk store requires the Python `zstandard` package. If it is missing, stop
and install it rather than falling back to weaker stdlib compression.

By default `chunk-build` creates a small locator/filter DB plus compressed raw
run chunks. It does **not** populate the decision child tables. Selected runs
can be exported later with `chunk-export`.

The older `index` command, or `chunk-build --store-decisions`, populates richer
decision child tables. Use that only for smaller analytical subsets unless you
really want a larger/slower full-corpus build.

## Runtime

Use the bundled Codex Python on this machine because `python.exe` on `PATH` is
the Windows Store stub in this shell:

```powershell
$PY = "C:\Users\davton\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe"
$SEVEN_ZIP = "C:\Program Files\Lenovo\Lenovo AI Now\7Zip\7z.exe"
```

The plain `index` command uses only Python standard-library modules. The
recommended chunk store uses `zstandard`.

## Smoke Test

Build a disposable locator DB plus `.jsonl.zst` chunks:

```powershell
& $PY tools\slaythedata\index_slaythedata.py chunk-build `
  --archive D:\dev\SlayTheData.7z `
  --seven-zip $SEVEN_ZIP `
  --db tools\slaythedata\chunk-smoke.sqlite3 `
  --chunks-dir tools\slaythedata\chunks-smoke `
  --manifest-cache tools\slaythedata\manifest-cache.json `
  --limit-files 2 `
  --batch-files 2 `
  --chunk-bytes 1000000 `
  --supported-content tools\slaythedata\supported_content.example.json
```

Export selected runs from chunks without touching the original 7z:

```powershell
& $PY tools\slaythedata\index_slaythedata.py chunk-export `
  --db tools\slaythedata\chunk-smoke.sqlite3 `
  --chunks-dir tools\slaythedata\chunks-smoke `
  --where "character_chosen='IRONCLAD' AND ascension_level=0 AND floor_reached BETWEEN 17 AND 33" `
  --limit 5 `
  --out tools\slaythedata\chunk-export-smoke.jsonl
```

Verified smoke result on the first two archive files, with intentionally tiny
1 MB chunks:

- `2,749` runs indexed.
- Legacy wide/raw-JSON DB: `33,939,456` bytes.
- Lean v4 DB: `13,209,600` bytes.
- Chunk locator DB: `13,361,152` bytes.
- Chunk files: `1,783,035` bytes.
- Chunk payload compressed `14,153,788 -> 1,783,035` bytes, or `0.126x`.

The smoke DB includes fixed manifest overhead for all 44,822 archive files, so
it overstates per-run DB cost. The compressed chunks are the important signal:
they should be far closer to the original 30 GB 7z than the earlier 200+ GB
SQLite-only design, while allowing per-chunk random access.

## Data Location

Keep generated full-corpus artifacts on `D:`, next to the source archive:

```powershell
$SLAYTHEDATA_ROOT = "D:\dev\SlayTheData-index"
```

The repository should contain the reproducible indexing code and docs, not the
multi-GB generated database/chunk store. `tools\slaythedata\.gitignore` still
ignores repo-local DBs, chunks, and exports as a guard for smoke tests or
accidental local builds.

Expected full-corpus outputs:

- `D:\dev\SlayTheData-index\slaythedata-chunks.sqlite3`
- `D:\dev\SlayTheData-index\chunks\*.jsonl.zst`
- `D:\dev\SlayTheData-index\manifest-cache.json`
- `D:\dev\SlayTheData-index\chunk-build.out.log`
- `D:\dev\SlayTheData-index\chunk-build.err.log`

## Full Index

For a full corpus chunk build, stream the solid archive once. The recommended
output location is `D:\dev\SlayTheData-index\`.

```powershell
& $PY tools\slaythedata\index_slaythedata.py chunk-build `
  --archive D:\dev\SlayTheData.7z `
  --seven-zip $SEVEN_ZIP `
  --db "$SLAYTHEDATA_ROOT\slaythedata-chunks.sqlite3" `
  --chunks-dir "$SLAYTHEDATA_ROOT\chunks" `
  --manifest-cache "$SLAYTHEDATA_ROOT\manifest-cache.json" `
  --stream-all `
  --only-missing `
  --chunk-bytes 134217728 `
  --zstd-level 10
```

This default full build is locator-only. It supports fast filters on `runs`
columns such as character, ascension, build, floor reached, victory, path JSON,
seed, potion/shop/event presence, and unsupported flags. It relies on
`chunk-export` for detailed per-run decisions.

`--only-missing` skips files already marked `indexed`. Because the source
archive is solid-compressed, late resumes still need to decompress earlier solid
data again. Prefer one uninterrupted `--stream-all` run for the first full
chunk build.

After chunk build, normal selected-run access should use `chunk-export`, not
the original 7z:

```powershell
& $PY tools\slaythedata\index_slaythedata.py chunk-export `
  --db "$SLAYTHEDATA_ROOT\slaythedata-chunks.sqlite3" `
  --chunks-dir "$SLAYTHEDATA_ROOT\chunks" `
  --where "character_chosen='IRONCLAD' AND ascension_level=0 AND floor_reached BETWEEN 17 AND 33" `
  --limit 100 `
  --out "$SLAYTHEDATA_ROOT\exports\slaythedata-act2-ironclad-a0.jsonl"
```

With `--chunk-bytes 134217728`, each lookup decompresses at most about 128 MB
uncompressed data for the chunk(s) containing selected runs. That is the trade:
much better locality than the solid 7z, while retaining strong zstd
compression.

For small targeted batches, use exact archive filenames or `--limit-files`:

```powershell
& $PY tools\slaythedata\index_slaythedata.py index `
  --archive D:\dev\SlayTheData.7z `
  --seven-zip $SEVEN_ZIP `
  --db tools\slaythedata\sample.sqlite3 `
  --file "2018-10-25-02-34#1352.json"
```

The older `index` command still builds a SQLite-only decision index without
chunks. Prefer default `chunk-build` for the full corpus.

If you need queries over not-picked card offers or not-picked boss relic offers,
add `--store-offers` to `index`. The chunk-build path intentionally does not
store offers by default; raw offers are available when exporting selected runs.

If you want picked card/relic/event/shop/campfire/potion child tables while also
building chunks, add `--store-decisions` to `chunk-build`. This is intentionally
not the default because it is much slower and larger.

## Schema

`runs` is mostly scalar and is the primary candidate table. It includes:

- identity: `id`, `source_file`, `source_file_ordinal`, `source_run_ordinal`,
  `play_id`
- filters: `character_chosen`, `build_version`, `ascension_level`, `victory`,
  `floor_reached`, `is_daily`, `is_endless`, `is_beta`, `is_prod`
- seed/path: `seed_played`, `neow_bonus`, `neow_cost`, `path_taken_json`,
  `path_per_floor_json`
- counts/flags: potion/card/event/shop/rest/combat counts, `unsupported_any`,
  `unsupported_character`, `unsupported_ascension`, `unsupported_build`

Repeated queryable decisions live in child tables when built with `index` or
`chunk-build --store-decisions`:

- `run_card_choices`: one row per card reward choice, picked card only by
  default
- `run_relics_obtained`
- `run_events` and `run_event_items`
- `run_shop_purchases`
- `run_campfire_choices`
- `run_boss_relic_choices`
- `run_potion_usage`, `run_potion_spawned`, `run_potions_obtained`
- `run_damage_taken`
- `run_card_offer_cards` and `run_boss_relic_offer_relics`, only populated
  when indexing with `--store-offers`
- `run_materialized_json`, only populated by `materialize --store`
- `chunk_files` and `chunk_runs`, populated by `chunk-build`

Unsupported-content values are intentionally not stored by default because a
restrictive support profile can create many values per run. The default index
stores unsupported flags for fast candidate filtering.

## Candidate Queries

Ironclad A0 runs that ended in Act 2:

```sql
SELECT id, source_file, source_run_ordinal, seed_played,
       build_version, floor_reached, killed_by
FROM runs
WHERE character_chosen = 'IRONCLAD'
  AND ascension_level = 0
  AND victory = 0
  AND floor_reached BETWEEN 17 AND 33
ORDER BY floor_reached DESC
LIMIT 100;
```

Replay-friendly Ironclad A0 candidates:

```sql
SELECT id, source_file, source_run_ordinal, seed_played, floor_reached
FROM runs
WHERE character_chosen = 'IRONCLAD'
  AND ascension_level = 0
  AND unsupported_any = 0
  AND is_daily = 0
  AND is_endless = 0
  AND is_trial = 0
ORDER BY floor_reached DESC
LIMIT 50;
```

Potion usage floors:

```sql
SELECT r.id, r.seed_played, group_concat(p.floor, ',') AS potion_floors
FROM runs r
JOIN run_potion_usage p ON p.run_id = r.id
WHERE r.character_chosen = 'IRONCLAD'
  AND r.ascension_level = 0
GROUP BY r.id
ORDER BY r.floor_reached DESC;
```

For a locator-only chunk DB, use the scalar count/flag columns first, then
`chunk-export` selected runs and inspect the raw JSONL:

```sql
SELECT id, source_file, source_run_ordinal, seed_played, potion_usage_count
FROM runs
WHERE character_chosen = 'IRONCLAD'
  AND ascension_level = 0
  AND has_potion_usage = 1
ORDER BY floor_reached DESC
LIMIT 100;
```

Runs that picked a specific card:

```sql
SELECT r.id, r.seed_played, c.floor, c.picked
FROM runs r
JOIN run_card_choices c ON c.run_id = r.id
WHERE r.character_chosen = 'IRONCLAD'
  AND c.picked_base = 'Inflame';
```

Runs with a specific event choice:

```sql
SELECT r.id, r.seed_played, e.floor, e.event_name, e.player_choice
FROM runs r
JOIN run_events e ON e.run_id = r.id
WHERE e.event_name = 'Scrap Ooze'
  AND e.player_choice = 'Success';
```

Shop purchases:

```sql
SELECT r.id, r.seed_played, s.floor, s.item
FROM runs r
JOIN run_shop_purchases s ON s.run_id = r.id
WHERE s.base_item IN ('Membership Card', 'Offering', 'Shrug It Off')
ORDER BY r.id, s.ordinal;
```

Runs that were offered a card but did not pick it require an index built with
`--store-offers`:

```sql
SELECT DISTINCT r.id, r.seed_played, o.choice_ordinal
FROM runs r
JOIN run_card_offer_cards o ON o.run_id = r.id
WHERE o.base_card = 'Feed'
  AND o.picked = 0;
```

## Materialize Selected Runs

Prefer `chunk-export` after building chunks. The older `materialize` command is
kept as a fallback for DBs that were built without chunks; it re-reads selected
source files from the original solid 7z.

Fallback materialization from the original archive:

```powershell
& $PY tools\slaythedata\index_slaythedata.py materialize `
  --archive D:\dev\SlayTheData.7z `
  --seven-zip $SEVEN_ZIP `
  --db "$SLAYTHEDATA_ROOT\slaythedata-chunks.sqlite3" `
  --where "character_chosen='IRONCLAD' AND ascension_level=0 AND floor_reached BETWEEN 17 AND 33" `
  --limit 25 `
  --out "$SLAYTHEDATA_ROOT\exports\slaythedata-act2-ironclad-a0.jsonl"
```

Add `--store` to also save selected raw events in `run_materialized_json`.

The exported data is still run-history data, not exact combat traces:

```json
{
  "run_level_choices": true,
  "exact_combat_actions": false,
  "potion_usage_has_floor_only": true
}
```

Use the exported run histories as run-level scripts; use CommunicationMod-style
traces for exact combat action verification.
