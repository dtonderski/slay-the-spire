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

Useful environment variables:

- `STS_AUTO_SEED_PREFIX`: seed prefix, default `M29`
- `STS_AUTO_START_INDEX`: explicit starting index for one run
- `STS_AUTO_MAX_RUNS`: maximum started runs, default `200`
- `STS_AUTO_TICK_MS`: polling interval, default `500`
- `STS_AUTO_MAX_STATE_POLLS`: repeated identical `state` polls before exiting, default `5`

## Trace Health

- `trace_tools.js validate <trace.jsonl>` checks that every action has a following state or error row for the same step.
- `trace_tools.js trim-valid-prefix <raw.jsonl> <out.jsonl>` writes the valid prefix before the first missing action response and appends metadata explaining the trim.
