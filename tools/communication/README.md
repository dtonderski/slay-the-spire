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
- `overnight_supervisor.js` repeatedly runs the collector, validates the active trace after collector exit, and stops with a clear reason if the bridge/session files are stale or the bridge has exited.
- `run_overnight_supervisor.cmd` starts the supervised overnight workflow. Start Slay the Spire with CommunicationMod first.
- `overnight_collector.test.js` is a fast Node regression test for command policy edge cases seen in harvested traces.

Useful environment variables:

- `STS_AUTO_SEED_PREFIX`: seed prefix, default `M29`
- `STS_AUTO_START_INDEX`: explicit starting index for one run
- `STS_AUTO_MAX_RUNS`: maximum started runs, default `200`
- `STS_AUTO_TICK_MS`: polling interval, default `500`
- `STS_AUTO_MAX_STATE_POLLS`: repeated identical `state` polls before exiting, default `5`
- `STS_AUTO_MAX_SAME_COMMAND`: repeated identical non-state commands before fallback/exit, default `2`
- `STS_SUPERVISOR_MAX_RESTARTS`: collector restarts before supervisor exits, default `20`
- `STS_SUPERVISOR_STALE_MS`: session summary/status age treated as stale, default `120000`
- `STS_SUPERVISOR_RESTART_DELAY_MS`: delay between collector restarts, default `3000`

## Trace Health

- `trace_tools.js validate <trace.jsonl>` checks that every action has a following state or error row for the same step and prints seeds, starts, rooms, encounters, deaths, and boss metadata.
- `trace_tools.js trim-valid-prefix <raw.jsonl> <out.jsonl>` writes the valid prefix before the first missing action response and appends metadata explaining the trim.
- `trace_tools.js extract-run <raw.jsonl> <run-index> <out.jsonl>` extracts one run from a multi-run trace and rebases steps so the selected `START` is action step 1.

Run collector policy tests with:

```powershell
node tools\communication\overnight_collector.test.js
```
