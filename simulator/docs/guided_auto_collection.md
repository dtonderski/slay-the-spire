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
   mismatches, or unsupported screens.
5. Preserve provenance so generated traces can be labeled as guided collection,
   not strict parity proof.

## Current Data Status

The local SlayTheData chunk store lives outside the repo:

- DB: `D:\dev\SlayTheData-index\slaythedata-chunks.sqlite3`
- chunks: `D:\dev\SlayTheData-index\chunks\*.jsonl.zst`

Observed on 2026-06-30:

- `35,716,837` runs in `runs`
- `35,703,387` runs in `chunk_runs`
- `5,143,603` exportable supported Ironclad A0 runs
- full build still partial: `17,727` archive files indexed,
  `27,095` pending

The chunk store is therefore usable for candidate selection, but status should
still be checked before assuming full-corpus completeness.

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

## Next Infrastructure Slice

Add a backend collector coordinator beside `SessionManager`.

Suggested endpoints:

- `POST /api/collector/start`
- `GET /api/collector/status`
- `POST /api/collector/tick`
- `POST /api/collector/stop`

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

