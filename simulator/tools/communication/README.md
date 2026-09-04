# Communication tools

CommunicationMod launches `trace_client.js` as its external process. The bridge
records raw JSONL, publishes current session state, and exposes a guarded local
TCP control socket used by the browser UI and collectors.

## Bridge and manual control

- `run_bridge.cmd`: interactive bridge.
- `run_passive_bridge.cmd`: state-polling bridge.
- `trace_client.js`: stdin/stdout protocol bridge.
- `action_speed.js`: action-gap and settle-poll diagnostics.
- `send_command.ps1` / `get_state.ps1`: legacy manual helpers.
- `trace_ui/`: current Node browser UI; run `npm start` there. It listens on
  `127.0.0.1:8787` by default.

Session files live under `simulator/tools/communication/session/` unless
`STS_BRIDGE_SESSION_DIR` selects another active CommunicationMod session.

Fresh launch scripts set `TRACE_CONTROL_PORT=0`; the assigned localhost port is
published in `session/status.json`. The socket accepts newline-delimited `hello`,
`acquire`, `state`, and `command` messages. Commands require the current owner
token and expected state identity/sequence. Only one command may be in flight.
Accepted commands and stale-owner takeovers are trace-visible.

When TCP control is enabled, legacy `next_command.txt` ingestion is disabled.
Set `TRACE_ALLOW_FILE_COMMANDS=1` only for compatibility diagnostics.

## Random fidelity collection

`random_fidelity_collector.js` starts one real-game run and samples uniformly
from every concrete gameplay command advertised by CommunicationMod. Known
hangs and simulator divergences remain eligible because they are evidence. The
collector writes one immutable trace and does not verify, minimize, or promote
it.

`run_random_fidelity_campaign.js` supervises repeated runs and resumes from the
first policy-seed gap backed by no sealed trace:

```bash
node simulator/tools/communication/run_random_fidelity_campaign.js
```

Important environment variables:

- `STS_BRIDGE_SESSION_DIR`: active bridge session.
- `STS_RANDOM_OUTPUT_DIR`: campaign output directory.
- `STS_RANDOM_MAX_RUNS`: number of runs; non-positive means indefinite.
- `STS_RANDOM_GAME_SEED_PREFIX`: game-seed prefix.
- `STS_RANDOM_SOURCE_VERSION`: declared collection build/schema.
- `STS_RANDOM_RETRY_DELAY_MS` / `STS_RANDOM_MAX_RETRY_DELAY_MS`: infrastructure
  retry backoff.

`collection_overnight_monitor.sh` is the local WSL/Windows watchdog for the game,
bridge, and random campaign. Its host paths are environment-configurable.

Older guided and heuristic collector scripts remain for diagnostics only; they
are not the supported fidelity workflow.

## Diagnostics and tests

- `trace_tools.js validate <trace.jsonl>` checks action/state pairing.
- `trace_tools.js report <trace.jsonl>` summarizes multi-run traces.
- `bridge_probe.js` checks bridge liveness.
- `harvest_status.js` inspects legacy harvest reports without mutation.
- `run_communication_checks.cmd` runs the tool regression suite.

From the repository root, the current Node tests can also be run directly:

```bash
node simulator/tools/communication/trace_client.test.js
node simulator/tools/communication/random_fidelity_collector.test.js
node simulator/tools/communication/run_random_fidelity_campaign.test.js
node simulator/tools/communication/trace_ui/server.test.js
```

Trace collection never establishes parity. Verify immutable output separately
with `sts_verify`.
