# Guided Auto-Collection

This note tracks the plan for automated trace collection driven by
SlayTheData run histories plus the omniscient combat agent.

## Goal

Automatically play the real game through the local UI and CommunicationMod:

1. Use SlayTheData for run-level choices: path, card rewards, events, shops,
   campfires, boss relics, and potion-use budgets.
2. Use the simulator combat agent for combat actions.
3. Reconstruct live simulator state from the active trace by strict seed replay.
4. Stop on illegal choices, stale bridge clients, simulator/live prediction
   mismatches, visible character/ascension mismatches, or unsupported screens.
5. Preserve provenance so generated traces can be labeled as guided collection,
   not strict parity proof.

## Current Data Status

The local SlayTheData chunk store lives outside the repo:

- DB: `D:\dev\SlayTheData-index\slaythedata-chunks.sqlite3`
- chunks: `D:\dev\SlayTheData-index\chunks\*.jsonl.zst`

Observed on 2026-06-30:

- `52,666,330` runs in `runs`
- `52,639,808` runs in `chunk_runs`
- supported exportable Ironclad A0 guided candidates are available for the UI
  default filters
- full build still partial: `31,680` archive files indexed,
  `13,142` pending

The chunk store is therefore usable for candidate selection, but status should
still be checked before assuming full-corpus completeness.

The UI now exposes this distinction directly through
`GET /api/slaythedata/status`. The endpoint checks that the locator DB exists,
that `runs` and `chunk_runs` are present, that exportable chunk rows exist, and
that the current guided-collection filters have at least one supported
candidate. It also reports `archive_files` status counts when available, so a
partial build is shown as usable-with-warnings instead of silently looking like
an empty corpus. Exact giant table counts are opt-in with
`?include_counts=1`; the UI uses the fast readiness check by default so page
loads do not block behind active index writes.

## Implemented Slice

`sts.slaythedata_policy` converts one `chunk-export` JSONL row into a
`GuidedRunScript`:

- run config and source metadata
- path fields
- floor-grouped card rewards, relics, events, shops, campfires, and potions
- boss relic choices
- final observed deck/relic/gold summary
- explicit replay policy marking combat actions as unavailable

`POST /api/slaythedata/script` accepts either:

- `{ "exported_run": { ...chunk export row... } }`
- `{ "path": "...jsonl", "line_index": 0 }`

and returns `{ "script": ... }`.

The policy module also includes conservative visible-choice matching for simple
text screens. It returns blockers instead of guessing when the target is
missing or ambiguous.

`sts.guided_collector` owns a loaded script and exposes:

- `POST /api/collector/start`
- `GET /api/collector/status`
- `POST /api/collector/tick`
- `POST /api/collector/stop`

The collector can now:

- list supported candidate runs from the local SlayTheData chunk index
- show SlayTheData index readiness in the guided collector panel before a run
  is loaded, including missing DB/table blockers and partial-build warnings
- rank candidate runs by full path length and guided-decision richness, with UI
  defaults that require card/event/shop decision coverage
- load UI candidates with `ranked=0` by default to avoid global sort latency on
  the huge partial local index; ranked selection remains available for slower
  diagnostics
- filter UI candidates to guided-safe Neow bonuses by default, avoiding starts
  that immediately require unsupported Neow follow-up grids such as remove,
  transform, upgrade, or choose-card screens
- return Neow bonus/cost metadata with candidate rows and default the candidate
  API to guided-safe Neow filtering unless `safe_neow=0` is explicitly passed
- preserve floor-0 SlayTheData card reward rows, enabling guided Neow
  `THREE_CARDS` and `THREE_RARE_CARDS` follow-up card choices when those rows
  provide the picked card and offered alternatives
- export a selected run from local chunks and start the collector from it
- prime the live run start controls from the selected/loaded SlayTheData run
  so the bridge starts the same seed the collector is following
- start the live game directly from the active guided script via
  `POST /api/collector/start-live-run`, sending `START <character> <ascension>
  <seed>` with guided-collector provenance so the first trace action is tied to
  the selected SlayTheData run
- preview the next SlayTheData-guided non-combat choice
- automatically advance scripted decision ordinals after successful sends, so
  repeated auto ticks can handle multiple same-floor shop buys or similar
  multi-step choices without manual ordinal overrides
- send one matched non-combat bridge command when `tick` receives
  `{ "send": true }`, after strict seed replay confirms the command maps to a
  current exact simulator action
- match map choices against SlayTheData `path_per_floor` when visible next-node
  room symbols identify exactly one target
- disambiguate same-symbol map choices when the live CommunicationMod map
  topology proves that exactly one visible node can satisfy the next
  SlayTheData route symbols, while still blocking if multiple paths remain
  compatible
- match campfire rest-site choices by campfire key, then card grids by the
  SlayTheData campfire card target
- match boss relic reward choices by act from SlayTheData boss relic history
- match generic reward screens against SlayTheData floor evidence for relics,
  potions, card rewards, and visible gold
- prefer visible named relic/potion reward identities from SlayTheData before
  falling back to generic `Relic`/`Potion` buckets, so named reward screens do
  not accidentally fall through to gold
- skip card reward screens when SlayTheData records `picked: "SKIP"`, using the
  bridge `SKIP` command and the same strict simulator legality check as other
  guided non-combat sends
- match SlayTheData shop purchases by visible item label and leave the shop
  once scripted purchases are exhausted
- open shop card removal when SlayTheData records a removed card on that shop
  floor, then use the existing grid matcher to select the removed card
- delegate one combat action to the live combat search policy
- store the predicted simulator state after a sent combat or strict non-combat
  action
- block the next tick if strict replay of the live trace does not reach that
  predicted state
- write guided-collector provenance into `next_command.json` and preserve that
  object on trace `action.command_meta`, including collector id, source
  SlayTheData run metadata, replay policy, and compact suggestion details
- run a cooperative UI auto-loop that repeatedly ticks the collector, waits
  while the bridge command is pending or not ready, and pauses on real blockers
- route `/api/collector/tick` live combat and non-combat sends through
  `BridgeMirror.send_command`, preserving the same source-state guard and
  provenance path used by manual UI sends
- run the same collector stack headlessly with `python -m sts.guided_collect`.
  The runner selects or accepts one SlayTheData run id, sends guided `START`,
  repeatedly ticks the collector, and writes a compact JSON report with run id,
  seed, trace path, bridge step/state, stop reason, blocker, and recent tick
  history
- when no run id is supplied, export a small batch of candidate rows and skip
  those whose normalized script is known to be unsupported before selecting a
  run; explicit `--run-id` remains strict and reports `script_blocked` instead
  of silently choosing a different run
- include selection diagnostics in guided collection reports: explicit vs auto
  mode, selected run id, candidates considered, and unsupported candidates
  skipped before start
- block guided scripts before sending `START` when the exported SlayTheData row
  would require an unrecorded Neow follow-up card grid target, such as remove,
  transform, or upgrade, or when Neow card-choice bonuses lack a floor-0 picked
  card row
- arm browser-driven auto collection from the guided collector with a single
  `Start + Auto` control. It sends guided `START` through the TCP-required
  backend path, then lets the existing collector loop wait for ready states and
  tick until a strict blocker appears
- disable guided `START`, `Start + Auto`, send-next, and auto controls when
  collector preflight reports that TCP bridge control is unavailable. Manual
  bridge actions remain separate compatibility tooling, but guided collection
  must use a fresh TCP-enabled bridge
- expose bridge preflight status in the guided collector panel and disable
  collector sends while hard preflight problems are present
- refresh and show that preflight status even before a guided script is loaded,
  so startup blockers are visible before pressing Auto
- provide a guarded UI repair for orphan `next_command.json` metadata when no
  `next_command.txt` command is pending
- cover the composed offline workflow with a temp-bridge smoke test: guided
  script start writes a provenance-tagged `START`, the bridge advances to a
  decision state, the collector sends a strict non-combat choice, records a
  prediction, and clears it after the predicted state is observed
- cover a longer offline collector loop across guided start, Neow talk,
  Neow card reward, topology-disambiguated map choice, delegated combat, and
  card reward skip, verifying command provenance and prediction handoff at
  each step
- verify the 2026-06-30 LIVE01 trace through strict Python replay to trace
  exhaustion after fixing Headbutt discard-grid parity; the report has
  `verified=true`, `stop_reason=trace_exhausted`, `anchor_count=0`, and
  `restoration_count=0`
- run the older Rust seed-start verifier on the same trace with
  `unexpected_diffs=0`; it still reports a documented unsupported `PROCEED`
  boundary in the seed-start harness, separate from the live strict replay
  gate used by the UI
- expose an optional trace-client TCP JSONL control socket. Fresh
  `run_bridge.cmd` and `run_passive_bridge.cmd` launches bind an ephemeral
  localhost port and advertise it in `session/status.json`; Python
  `BridgeMirror.send_command` prefers that socket and falls back to the legacy
  `next_command.txt` path when unavailable
- advertise a monotonic bridge `state_seq` alongside each `state_id`, acquire a
  single TCP controller owner token before Python UI commands, and require both
  the expected state id and sequence for TCP command acceptance
- publish accepted in-memory TCP commands into `session/status.json` as pending
  commands, so the UI and collector wait for CommunicationMod to consume them
  even though no legacy `next_command.txt` file exists
- require TCP control for guided collector `START` and tick sends. Manual bridge
  commands may still use the legacy file fallback, but guided auto-collection
  now refuses to send on an old/file-only bridge
- request a post-command observed state for guided `START` as well as guided
  tick sends, so the first live auto-collection command has the same
  acknowledged/observed boundary as later actions
- let TCP command callers request `wait_for_state_update`, which keeps the
  command response open until the next observed CommunicationMod state arrives
  or a bounded timeout fires; the Python mirror normalizes that observed update
  into the usual bridge-status shape
- verify guided combat and non-combat simulator predictions immediately from
  that observed TCP state when it is present, clearing the pending prediction
  in the same tick on a match and blocking loudly on timeout or mismatch

Combat sending is deliberately routed through `SessionManager` so the same
strict live-session attach, stale search guard, prediction, visible bridge slot
mapping, and `BridgeMirror.send_command(..., source_state_id=...)` checks are
used for manual UI play and guided collection.

## Next Infrastructure Slice

Run live end-to-end smoke against a freshly restarted TCP-enabled bridge, then
broaden exact non-combat coverage and polish candidate selection.

Headless smoke command shape:

```powershell
cd D:\dev\slay-the-spire\simulator
uv run python -m sts.guided_collect `
  --run-id <slaythedata-run-id> `
  --max-actions 200 `
  --report-output target\guided-collect\latest.json
```

Without `--run-id`, the runner selects one exportable safe-Neow Ironclad A0
candidate from the local SlayTheData chunk index. Guided collection requires a
fresh TCP-enabled bridge by default; `--allow-file-bridge` exists only for
diagnostics and compatibility tests.

The runner preflights the bridge before exporting a SlayTheData run. If the
bridge is stale, file-only, or has a pending command, it writes a
`preflight_blocked` report instead of sending anything.

Tick algorithm:

1. Read bridge status.
2. Reject changed bridge client identity or pending command.
3. Attach/replay current live session from the active trace.
4. If the current decision is combat:
   - run combat search with the selected policy and allowed potion budget,
   - predict the simulator transition,
   - send the matching bridge command,
   - wait for the next observed state,
   - verify predicted state hash before continuing the cached plan.
5. If the current decision is non-combat:
   - read the current floor and screen,
   - select the next SlayTheData script decision,
   - match it to exact legal simulator action and visible bridge action,
   - send only if both agree.
6. Stop with a blocker if any required choice is illegal, ambiguous,
   unsupported, or would require guessing.

Steps 1, 3, 4, simple visible-choice sending, strict non-combat legal-action
agreement, conservative map path matching including topology-backed
same-symbol disambiguation, boss relic matching, campfire/grid matching,
post-send prediction checks, and generated-trace provenance are implemented,
and the UI can repeatedly call tick until blocked. The bridge write path now
has an acknowledged TCP option with controller ownership, state sequence guards,
and guided-auto enforcement. The browser UI still needs an end-to-end live
smoke against a freshly restarted TCP-enabled bridge. Remaining work is broader
support for Neow bonuses whose follow-up target is not explicitly recorded by
SlayTheData, only if a non-guessing data source is added, plus end-to-end live
bridge smoke coverage.

## Important Boundaries

Strict replay remains the simulator parity proof. Guided collection is allowed
to diverge legally from the SlayTheData source run and must be tagged as such.

SlayTheData potion usage is floor-level only. The combat agent decides timing
and target within the floor budget.

Shop histories record purchased items, not guaranteed full inventories. The
collector should use "buy if legal" semantics and stop if the desired item is
not visible.

Combat actions must never be copied from SlayTheData because they are not
present there.
