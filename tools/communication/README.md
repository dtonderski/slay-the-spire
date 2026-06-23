# Communication Tools

This folder contains the local bridge and helper tools for collecting Slay the Spire traces through CommunicationMod.

## Bridge

- `trace_client.js` is the stdin/stdout bridge used by CommunicationMod. It writes JSONL traces under `verification/corpus/communication_mod/` and publishes current state files under `tools/communication/session/`.
- `run_bridge.cmd` starts the interactive bridge.
- `run_passive_bridge.cmd` starts the bridge in state-polling mode.

## Manual Control

- `send_command.ps1` writes one command to the active bridge session.
- `get_state.ps1` prints the latest bridge state.
- `trace_ui/` contains the browser UI for manual command selection while collecting a trace.

## Overnight Collection

- `overnight_collector.js` watches `session/summary.json` and writes controller commands to `session/next_command.txt`.
- `run_overnight_collector.cmd` starts the autopilot. It persists the next seed index in `session/overnight_collector_state.json`, so restarts keep moving through seeds.
- `overnight_preflight.js` checks whether the current bridge/session is fresh and safe for overnight supervision before starting.
- `overnight_supervisor.js` repeatedly runs the collector, validates the active trace after collector exit, writes a `.valid-prefix.jsonl` salvage file when a trace has a missing action response, writes a `.best-run.jsonl` extracted keeper from valid traces, updates `session/harvest_report.json`, logs compact harvest-quality and best-run lines, and stops with a clear reason if the bridge/session files are stale or the bridge has exited.
- `run_overnight_supervisor.cmd` starts the supervised overnight workflow. Start Slay the Spire with CommunicationMod first.
- `harvest_status.js` reads `session/harvest_report.json` and validates referenced raw, valid-prefix, and best-run artifacts without writing new trace files.
- `overnight_collector.test.js` is a fast Node regression test for command policy edge cases seen in harvested traces.
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
node tools\communication\trace_tools.test.js
```
