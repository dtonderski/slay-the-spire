# Communication Tools

This folder contains the local bridge and helper tools for collecting Slay the Spire traces through CommunicationMod.

## Bridge

- `trace_client.js` is the stdin/stdout bridge used by CommunicationMod. It writes JSONL traces under `verification/corpus/communication_mod/` and publishes current state files under `tools/communication/session/`.
- `run_bridge.cmd` starts the interactive bridge.
- `run_passive_bridge.cmd` starts the bridge in state-polling mode.
- Fresh bridge launch scripts enable the optional localhost TCP JSONL control
  socket with `TRACE_CONTROL_PORT=0`. The assigned port is advertised in
  `session/status.json` under `control`.

### TCP JSONL Control

The TCP control socket is an in-memory control plane beside the legacy files.
It does not replace CommunicationMod stdin/stdout; it sends commands through
the same bridge process and records accepted commands in the same trace.

Messages are newline-delimited JSON objects:

- `{ "type": "hello" }` returns protocol metadata.
- `{ "type": "state" }` returns the latest state, summary, trace path, step,
  and bridge-advertised `state_id`.
- `{ "type": "command", "command": "CHOOSE 0", "expected_state_id": "...",
  "expected_state_seq": 7, "owner_token": "...", "metadata": { ... } }`
  enqueues one command only if the latest state id/seq still matches, the
  bridge is ready, the active controller owns the socket, and the command verb
  is currently available.
- Add `"wait_for_state_update": true` to a command to keep the TCP response open
  until the next observed game state arrives, or until `"update_timeout_ms"`
  elapses. The response still records the accepted state, plus an
  `observed_update` object when the bridge saw the post-command state.

`BridgeMirror.send_command(...)` prefers this socket when advertised and falls
back to `next_command.txt` for older bridge clients. When a trace client is
launched with `TRACE_CONTROL_PORT` set, legacy `next_command.txt` ingestion is
disabled by default so file commands cannot bypass state id/seq guards or
controller ownership. Set `TRACE_ALLOW_FILE_COMMANDS=1` only for explicit
compatibility diagnostics. The legacy session files remain the read model and
old-client fallback; the socket is the preferred write path because it gives
accepted/rejected acknowledgements, can wait for post-command state updates,
and does not rely on file polling for command submission.

## Manual Control

- `send_command.ps1` writes one command to the active bridge session.
- `get_state.ps1` prints the latest bridge state.
- `trace_ui/` contains the browser UI for manual command selection while collecting a trace.

## Overnight Collection

- `run_auto_collect.cmd` is the recommended one-shot launcher for the current
  SlayTheData-guided auto-collection path. It runs `run_guided_collect.cmd`,
  then prints `run_guided_collect_status.cmd`, while preserving the guided
  collector exit code.
- `run_guided_collect.cmd` starts the SlayTheData-guided headless collector.
  It writes the latest JSON report to
  `simulator\target\guided-collect\latest.json`, archives timestamped attempts
  under `simulator\target\guided-collect\reports\`, and forwards any extra
  command line arguments to `python -m sts.guided_collect`. Start Slay the
  Spire and a fresh TCP-enabled bridge first; the launcher waits briefly for
  bridge preflight to pass, then exits nonzero with a `preflight_blocked`
  report instead of sending into a stale or file-only bridge.
- `guided_collect_status.js` prints a compact summary of the latest guided
  collection report, its producer/freshness, strict replay validation status,
  recent archived reports, and validates the referenced trace when one exists.
- `run_guided_collect_status.cmd` runs that status check from Windows shells.
- `overnight_collector.js` is the legacy heuristic collector. It watches
  `session/summary.json` and writes controller commands to
  `session/next_command.txt`; use it only for diagnostics or fallback trace
  harvesting, not for the SlayTheData-guided automation milestone.
- Its map policy scores only currently visible choices, preferring elites, fights, chests, events, shops, then rests. It does not do route lookahead yet.
- Its combat policy is intentionally simple, but now prefers blocking over a basic attack when low HP faces heavy incoming damage.
- It persists pending `START` state so it waits for in-game confirmation before sending another seed, proceeds out of `SHOP_ROOM` after leaving the shop screen, and handles `HAND_SELECT` choose/confirm flows.
- `run_overnight_collector.cmd` starts the autopilot. It persists the next seed index in `session/overnight_collector_state.json`, so restarts keep moving through seeds.
- `overnight_preflight.js` checks whether the current bridge/session is fresh and safe for overnight supervision before starting.
- It also fails when `next_command.json` exists without `next_command.txt`,
  because orphan metadata could be attached to the next raw bridge command.
- `run_overnight_preflight.cmd` runs the preflight check.
- `bridge_probe.js` writes one temporary `state` command and verifies that the active CommunicationMod bridge consumes it. If the command is not consumed, it removes the probe command and exits nonzero.
- Session `summary.json` and `status.json` include `client_pid`; use this to catch duplicate bridge clients writing conflicting session files.
- `overnight_supervisor.js` repeatedly runs the collector, validates the active trace after collector exit, writes a `.valid-prefix.jsonl` salvage file when a trace has a missing action response, writes a `.best-run.jsonl` extracted keeper from valid traces, updates `session/harvest_report.json`, logs compact harvest-quality and best-run lines, and stops with a clear reason if the bridge/session files are stale or the bridge has exited.
- `run_overnight_supervisor.cmd` starts the legacy heuristic supervised
  workflow. It requires `STS_LEGACY_HEURISTIC_COLLECTOR=1` so it is not
  confused with guided auto-collection. Start Slay the Spire with
  CommunicationMod first.
- `run_overnight_guarded.cmd` runs preflight first and only starts the legacy
  heuristic supervised workflow if the bridge/session is fresh and
  `STS_LEGACY_HEURISTIC_COLLECTOR=1` is set.
- `harvest_status.js` reads `session/harvest_report.json` and validates referenced raw, valid-prefix, and best-run artifacts without writing new trace files.
- `run_communication_checks.cmd` runs the communication tool regression tests.
- `overnight_collector.test.js` is a fast Node regression test for command policy edge cases seen in harvested traces.
- `bridge_probe.test.js` is a fast Node regression test for bridge liveness probe result handling.
- `overnight_preflight.test.js` is a fast Node regression test for stale-session and pending-command detection before overnight runs.
- `overnight_supervisor.test.js` is a fast Node regression test for stale-session and trace-path decisions in the supervisor.
- `harvest_status.test.js` is a fast Node regression test for non-mutating harvest report inspection.
- `trace_tools.test.js` is a fast Node regression test for trace validation and harvest coverage summaries.

Useful environment variables:

- `STS_AUTO_SEED_PREFIX`: seed prefix, default `M29`
- `STS_AUTO_START_INDEX`: explicit starting index for one run
- `STS_AUTO_MAX_RUNS`: maximum started runs, default `200`
- `STS_AUTO_TICK_MS`: polling interval, default `500`
- `STS_AUTO_MAX_STATE_POLLS`: repeated identical `state` polls before exiting, default `5`
- `STS_AUTO_MAX_SAME_COMMAND`: repeated identical non-state commands before fallback/exit, default `2`
- `STS_AUTO_MAX_IDLE_MS`: stale session/status age before the collector exits for supervisor recovery, default `120000`
- `STS_SUPERVISOR_MAX_RESTARTS`: collector restarts before supervisor exits, default `20`
- `STS_SUPERVISOR_STALE_MS`: session summary/status age treated as stale, default `120000`
- `STS_SUPERVISOR_RESTART_DELAY_MS`: delay between collector restarts, default `3000`
- `STS_PREFLIGHT_STALE_MS`: session summary/status age treated as stale by preflight, default `120000`
- `TRACE_CONTROL_PORT`: optional trace-client TCP JSONL control port. Use `0`
  to bind any free localhost port and publish it in `session/status.json`.
- `TRACE_ALLOW_FILE_COMMANDS`: set to `1` to allow legacy
  `session/next_command.txt` command ingestion even when `TRACE_CONTROL_PORT`
  is enabled. Leave unset for guided auto-collection and normal UI use.

## Trace Health

- `trace_tools.js validate <trace.jsonl>` checks that every action has a following state or error row for the same step and prints seeds, starts, rooms, encounters, deaths, terminal state, elite/boss room coverage, and a simple harvest score.
- `trace_tools.js report <trace.jsonl>` adds per-run summaries for multi-run overnight traces and identifies the best run by harvest score.
- `trace_tools.js trim-valid-prefix <raw.jsonl> <out.jsonl>` writes the valid prefix before the first missing action response and appends metadata explaining the trim.
- `trace_tools.js extract-run <raw.jsonl> <run-index> <out.jsonl>` extracts one run from a multi-run trace and rebases steps so the selected `START` is action step 1.
- `trace_tools.js extract-best-run <trace.jsonl> <out.jsonl>` extracts the highest-scoring run and adds metadata with the selected run index and coverage score.
- `trace_tools.js collapse-card-reward-loop <trace.jsonl> <out.jsonl>` removes no-progress `SKIP` / reopen-same-card-reward loops from old autopilot traces while preserving the eventual card pick.

Run collector policy tests with:

```powershell
node tools\communication\overnight_collector.test.js
node tools\communication\overnight_preflight.test.js
node tools\communication\overnight_supervisor.test.js
node tools\communication\harvest_status.test.js
node tools\communication\guided_collect_status.test.js
node tools\communication\trace_tools.test.js
```

Before a guided auto-collection run:

```powershell
tools\communication\run_auto_collect.cmd
tools\communication\run_guided_collect_status.cmd
```

To bias the selected SlayTheData source run toward potion-budget coverage,
forward the guided collector filter through the wrapper:

```powershell
tools\communication\run_auto_collect.cmd --min-potion-usage 1
```

Before a legacy heuristic overnight run:

```powershell
$env:STS_LEGACY_HEURISTIC_COLLECTOR = "1"
tools\communication\run_overnight_guarded.cmd
node tools\communication\bridge_probe.js
node tools\communication\harvest_status.js
```
