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
Accepted commands and stale-owner takeovers are trace-visible. A failed client
socket is recorded as `control_socket_error`; it does not release ownership,
cancel or resend an accepted command, or mark the whole game session exited.
Inspect an unresolved accepted command through cached state, never by resending
it. An inspection is not a replacement completion: collectors stop on an
accepted-command observation timeout and require explicit review.

When TCP control is enabled, legacy `next_command.txt` ingestion is disabled.
Set `TRACE_ALLOW_FILE_COMMANDS=1` only for compatibility diagnostics.

## Random fidelity collection

`random_fidelity_collector.js` starts one real-game run and samples uniformly
from every concrete gameplay command advertised by CommunicationMod. Known
hangs and simulator divergences remain eligible because they are evidence. The
collector writes one immutable trace and does not verify, minimize, or promote
it. Startup checks the selected local bridge against HELLO and cached STATE,
refusing existing ownership, pending work, or an in-progress game. It never
requests stale-owner takeover, orphan cancellation, or automatic ABANDON.
Errors and action-limit exhaustion exit nonzero and retain ownership/pending
work for explicit inspection; only natural completion releases ownership.
`STS_RANDOM_ABANDON_EXISTING` no longer enables automatic recovery. Do not
restart a failed collector automatically or treat its incomplete trace as a
successful run. Every gameplay completion must match the submitted command ID
and exact +1 execution/settlement fences; STATE must match its poll ID without
advancing those counters. PROFILE remains typed, transient, and fence-less.
An accepted-command observation timeout fails closed rather than substituting a
later cached state. A campaign stops at the first failure, including indefinite
mode; there is no automatic retry or infrastructure backoff.

`run_random_fidelity_campaign.js` supervises repeated runs and resumes from the
first policy-seed gap backed by no sealed trace. Completion requires a matching
`collected` / `game_over` ledger entry with a verified `trace_sha256`, not merely
a matching metadata line or successful process exit. An exclusive `campaign.lock` prevents concurrent supervisors; it is removed
only after natural batch completion. Existing locks are never stolen by PID.
Incomplete/unsealed traces, missing sealed files, recorded failures, and an
interrupted supervisor stop preflight rather than being retried as policy-seed
gaps. Old unhashed ledgers are not completion evidence. Inspect the failure and
use a fresh qualified output directory rather than rewriting records or
silently recovering:

```bash
node simulator/tools/communication/run_random_fidelity_campaign.js
```

Important environment variables:

- `STS_BRIDGE_SESSION_DIR`: active bridge session.
- `STS_RANDOM_OUTPUT_DIR`: campaign output directory.
- `STS_RANDOM_MAX_RUNS`: non-negative safe integer; only `0` means indefinite.
- `STS_RANDOM_MAX_ACTIONS`, `STS_RANDOM_POLICY_SEED`, `STS_STARTING_HP`: positive
  safe integers. Malformed, fractional, empty, negative, and overflowing values
  fail before acquisition; values are not truncated or silently defaulted.
- `STS_RANDOM_GAME_SEED_PREFIX`: game-seed prefix.
- `STS_RANDOM_SOURCE_VERSION`: declared collection build/schema.
- Legacy `STS_RANDOM_RETRY_DELAY_MS` / `STS_RANDOM_MAX_RETRY_DELAY_MS` no longer
  enable retries.

The legacy `collection_overnight_monitor.sh` and
`random_fidelity_game_watchdog.js` entrypoints are disabled: their automatic
restart and PID-discovery behavior could kill another owner or retry an
interrupted accepted command. Launch the game/bridge explicitly, then start
one collector/campaign. Neither entrypoint performs cleanup or recovery.

Older guided and heuristic collector scripts remain for diagnostics only; they
are not the supported fidelity workflow.

## Diagnostics and tests

- `trace_tools.js validate <trace.jsonl>` checks action/state pairing.
- `trace_tools.js report <trace.jsonl>` summarizes multi-run traces.
- `bridge_probe.js` checks bridge liveness.
- `audit_hand_select_retrieval.py` is a read-only diagnostic. It binds the current
  explicit selection (including UUIDs where captured), rather than combining
  adjacent HAND_SELECT owners. Its legacy fallback stops at owner changes.
  A flag still requires source/capture review; zero flags do not certify a trace.
- `harvest_status.js` inspects legacy harvest reports without mutation.
- `run_communication_checks.cmd` runs the tool regression suite.

From the repository root, the current Node tests can also be run directly:

```bash
node simulator/tools/communication/trace_client.test.js
node simulator/tools/communication/random_fidelity_collector.test.js
node --test simulator/tools/communication/random_fidelity_fail_closed.test.js
node --test simulator/tools/communication/collection_review_regressions.test.js
node --test simulator/tools/communication/choice_contract.test.js
node simulator/tools/communication/run_random_fidelity_campaign.test.js
node simulator/tools/communication/random_fidelity_game_watchdog.test.js
node simulator/tools/communication/trace_ui/server.test.js
```

The audit's synthetic infrastructure regressions run through `uv`:

```bash
uv run python -m unittest discover -s simulator/tools/communication -p 'audit_hand_select_retrieval_test.py'
```

New random collection requires CommunicationMod `choice_index_schema: 1` before
START. `choice_list` retains stable UI offer indices; executable CHOOSE commands
come from the producer's `selectable_choice_indices`, not a policy-side potion
filter. Missing/malformed reward/shop selectability stops collection rather than
silently retrying or reindexing. Schema-6/7 replay bindings keep their original
meaning. See the mod README for the additive contract and synthetic cross-layer
regressions.

Trace collection never establishes parity. Verify immutable output separately
with `sts_verify`, and review capture provenance before corpus promotion.

Collection repairs and simulator parity repairs are separate changes. This
collection-only revision does not carry PR #41's simulator-fidelity changes:
in particular it does not reinterpret `WAIT` frames as gameplay milliseconds,
change Match and Keep rules, or add map overlays. Those changes require separate
source-backed review against the immutable captures. Promotion is also a separate
manual operation; a retained capture or a successful replay alone is not approval
for the permanent corpus.
