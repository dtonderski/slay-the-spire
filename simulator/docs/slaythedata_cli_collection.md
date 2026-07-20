# Autonomous SlayTheData CLI Collection

`live-trace` is the production collector. The browser UI remains an operator
console, but autonomous collection does not depend on browser interaction.

## Persistent agent protocol

The production agent surface is a single long-lived `live-trace slaythedata
agent` process. It owns one `SessionStore`, starts with the normal `collect`
filters supplied on the command line, writes newline-delimited JSON events to
stdout, and accepts newline-delimited JSON commands on stdin. Keeping the store
resident avoids process-level session discovery and SlayTheData reattachment at
every decision point.

The protocol is deliberately small. Commands inspect the current decision
packet, send one current legal action by ID, accept an unavailable guided shop
purchase, resume the attached run, start the next unjournaled run, or stop.
Every command carries an optional request ID which is copied to its response.
Progress events are compact and identify run, session, floor, phase, and the
operation that may take time. A decision event contains the richer state needed
by an agent: enabled legal actions, shop inventory and prices, deck, relics,
potions, HP, gold, and route state. The agent—not a generic shop policy—chooses
substitute purchases and purges.

The resident process is only a latency optimization. Its trace remains the
authority for recovery. After a crash, session recovery reconstructs simulator
state by strict seed-start replay of the recorded seed and accepted actions;
live observations are never copied into simulator state.

## Collection command

Run from `simulator/` with Slay the Spire, CommunicationMod, and the bridge
manager running:

```powershell
cargo build -p sts_live --bin live-trace --release

target\release\live-trace.exe `
  --slaythedata-db D:\dev\SlayTheData-index\slaythedata-chunks.sqlite3 `
  slaythedata agent `
  --ascension 0 --victory --min-floor 51 `
  --target-floor 60 `
  --journal SLAYTHEDATA_CLI_COLLECTION_LOG.jsonl `
  --repair-packet latest-repair.json `
  --combat-search-transition-budget 100000 `
  --combat-search-time-budget-ms 30000
```

The agent processes one run per decision cycle even if a larger `--limit` is
supplied. After a terminal or incompatible run, send `{"command":"next_run"}`.
At a recoverable blocker, send one or more `act` commands using IDs from the
decision packet, then `skip_shop` when accepting unavailable shop guidance and
`resume` to continue:

```json
{"request_id":"buy-feed","command":"act","session_id":"session-15","action_id":"choose-1"}
{"request_id":"accept-shop","command":"skip_shop","session_id":"session-15"}
{"request_id":"continue","command":"resume","session_id":"session-15"}
```

The collector resets the bridge before each attempt by default, starts a fresh
seeded run, attaches the selected SlayTheData record, and drives combat and
non-combat actions until it reaches the target or emits a structured terminal
result. Search excludes runs already marked in the permanent corpus; the
journal also excludes every previously attempted run unless
`--retry-journaled` is supplied.

Raw session traces, journals, and repair packets are gitignored. A trace enters
Git only through strict promotion into
`verification/corpus/permanent_traces/`.

## Machine-readable recovery contract

Every command prints one JSON value. A collection terminal includes:

- `status`, `reason`, and `blocker_kind`;
- `session_id`, `trace_path`, floor, phase, and fidelity status;
- the current live-state summary and enabled legal actions;
- the first simulator diff or guidance mapping failure;
- a reproduction or recovery command when recovery is meaningful;
- strict seed-start verification and promotion results in the journal record.

The one-shot commands remain available for diagnostics and crash recovery:

```powershell
target\release\live-trace.exe sessions state session-N
target\release\live-trace.exe actions list session-N
target\release\live-trace.exe actions send session-N ACTION_ID
target\release\live-trace.exe slaythedata skip-shop session-N
target\release\live-trace.exe slaythedata resume session-N --target-floor 60
target\release\live-trace.exe trace verify session-N
```

`run_ended_before_target` is a terminal game outcome, not a bridge failure, and
therefore has no recovery command. A fidelity break reproduces with
`sts_verify parity`. A missing guided shop purchase preserves
the session and points to `skip-shop`; an agent may instead inspect legal
actions, send a chosen action, and resume.

## State-authority invariant

The simulator is always reconstructed from the recorded `START` seed/config and
the accepted action sequence. Real-game observations are expected outputs used
for strict comparison and for exposing currently legal bridge actions. They are
never copied into simulator state.

On process restart, the CLI recovers the trace and its live observation, then
reattaches only the recorded SlayTheData guidance cursor. `sim_run_state` is
produced exclusively by a fresh strict seed-start replay. If replay does not
produce a clean simulator state, guided automation remains blocked.

## Typed guidance and divergence contract

Each preflight step retains its original `SlayTheDataReplayStepKind`. The live
binder reads route symbols, reward picks, event effects, shop items, purge
targets, and campfire targets only from that typed intent. `code` remains a
workflow/status label and `message` is display-only. Session recovery rebuilds
the typed report from the persisted raw run and restores only its recorded
cursor; a production attachment is rejected if any step lacks typed intent.

When a legal current-build run moves past unavailable guidance from the
2020-07-30 dataset, the existing SlayTheData trace event includes a typed
`SlayTheDataGuidedDivergence` with its source build, step, floor, intent, and
reason. These records describe guided-source divergence. They are separate
from strict simulator fidelity status and never convert a simulator mismatch
into a guided success.

## 2026-07-18 validation campaign

The autonomous CLI processed 22 unique source runs that had not previously been
promoted to the permanent corpus (two had earlier failed UI attempts in the
journal, but neither had yielded a collected trace):

`6453, 111730, 57275, 13284, 62651, 188397, 92797, 167636, 249980,
231506, 259382, 130000, 181755, 86425, 284637, 236317, 209667, 254954,
287257, 94930, 179782, 187418`.

A read-only lookup of every ID in the configured SlayTheData index confirmed
all 22 are Ironclad A0 victories with `floor_reached = 51`, build
`2020-07-30`, and a non-empty Neow bonus. The campaign produced 32 autonomous
collector/resume invocations over 22 attempts. Nine attempts reached floor 11
or later and all nine produced a promoted strict-clean full trace or retained
prefix. The deepest new trace, run 187418, reached floor 29; its final strict
replay contained 727 total actions and 726 verified transitions, with zero
unsupported actions and zero unexpected diffs, before clean full-trace
promotion.

### CLI versus browser workflow

The pre-CLI journal contains 75 browser-workflow records and 14 explicitly
started/retried UI sessions. For the 14 sessions with a later journal entry,
the median recorded wall span was 882.4 seconds and the total was 69,441.9
seconds; 10 reached floor 11 or later. These spans include human/agent diagnosis
and code-fix time, so they are an operational reference, not a pure execution
speed benchmark.

For the CLI campaign itself, the 32 autonomous invocations used 3,645.3 seconds
of active command time, with a 71.0-second median and a median collector-reported
rate of 1.49 verified trace actions per second. It processed 22 source runs and
required zero browser interactions. The meaningful result is not a claimed
wall-clock speedup: the CLI replaces UI clicking with reproducible JSON
terminals, safe process-level resume, automatic strict verification, and
automatic clean-prefix promotion.

## Verification gates

The implementation is covered by tests for CLI JSON state/actions, fresh-start
defaults, journal exclusion, structured repair packets, recovered-session
continuation without simulator hydration, shop recovery, game-over terminals,
strict promotion, and refreshing a stable corpus file after a repaired trace is
promoted again. The permanent corpus is replayed in seed-start mode by the
`sts_verify` corpus tests.
