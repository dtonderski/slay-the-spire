# Autonomous SlayTheData CLI Collection

`live-trace` is the production collector. The browser UI remains an operator
console, but autonomous collection does not depend on browser interaction.

## Collection command

Run from `simulator/` with Slay the Spire, CommunicationMod, and the bridge
manager running:

```powershell
cargo run -p sts_live --bin live-trace -- `
  --slaythedata-db D:\dev\SlayTheData-index\slaythedata-chunks.sqlite3 `
  slaythedata collect `
  --ascension 0 --victory --min-floor 51 --limit 20 `
  --target-floor 60 `
  --journal SLAYTHEDATA_CLI_COLLECTION_LOG.jsonl `
  --repair-packet latest-repair.json
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

Useful recovery commands are:

```powershell
cargo run -p sts_live --bin live-trace -- sessions state session-N
cargo run -p sts_live --bin live-trace -- actions list session-N
cargo run -p sts_live --bin live-trace -- actions send session-N ACTION_ID
cargo run -p sts_live --bin live-trace -- slaythedata skip-shop session-N
cargo run -p sts_live --bin live-trace -- slaythedata resume session-N --target-floor 60
cargo run -p sts_live --bin live-trace -- trace verify session-N
```

`run_ended_before_target` is a terminal game outcome, not a bridge failure, and
therefore has no recovery command. A fidelity break reproduces with
`sts_verify parity --mode seed-start`. A missing guided shop purchase preserves
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
