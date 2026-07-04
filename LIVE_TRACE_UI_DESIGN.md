# Live Trace UI Design

## Purpose

The live trace UI exists to collect real-game traces from Slay the Spire. Its
first job is not automation, coaching, SlayTheData replay, or combat search. It
is a durable operator console for:

- starting or attaching to a live run
- showing the current game state and legal actions
- sending manually chosen legal actions
- recording an append-only trace
- continuously checking simulator fidelity

Everything else is staged behind this trace-collection contract.

## Lessons From The Prototype

The archived MVP mixed too many responsibilities: browser UI state, bridge
process management, trace capture, simulator replay, combat search, SlayTheData
route selection, and auto-play timing. The replacement should keep hard logic in
the backend and make the frontend a disposable view over typed backend state.

Prototype rules to keep:

- never infer actions from button text
- never rely on frontend timers for game progression
- never guess after fidelity loss or ambiguous live state
- keep recording after failure, but mark the trace clearly
- make every blocked state explainable

## Architecture

Use a Rust backend service with a small web frontend.

The Phase 1 implementation lives in `simulator/crates/sts_live`.

Backend responsibilities:

- bridge process/session discovery
- run start commands
- state request and ingestion
- typed live legal-action extraction
- trace file creation and append
- simulator replay and canonical diffing
- fidelity status
- automation job state, later

Frontend responsibilities:

- render backend state
- render legal actions as buttons
- send operator commands to the backend
- display trace/fidelity/bridge status clearly

The frontend must not implement simulator mechanics, route logic, combat policy,
or action matching.

## Operator Surfaces

The backend must be usable without a browser. Browser automation should be a
visual QA tool, not the primary way for agents or developers to inspect and
drive the system.

Support three clients over the same backend contract:

- web UI for human manual play
- CLI for Codex, scripts, and local debugging
- HTTP API for the web UI, tests, and external tools

The CLI should return compact structured JSON by default:

```powershell
live-trace bridges list
live-trace bridges kill --all
live-trace sessions start --character ironclad --ascension 0 --seed CODEX04
live-trace sessions state --session <id>
live-trace actions list --session <id>
live-trace actions send --session <id> --action <action-id>
live-trace fidelity status --session <id>
live-trace trace path --session <id>
```

Example response:

```json
{
  "session_id": "abc",
  "phase": "combat",
  "fidelity": "ok",
  "legal_actions": [
    {
      "id": "act_12",
      "kind": "play_card",
      "label": "Strike -> Jaw Worm",
      "enabled": true
    }
  ]
}
```

Errors and blocked states should be equally structured:

```json
{
  "status": "blocked",
  "reason_code": "fidelity_lost",
  "message": "Simulator diverged at trace step 42.",
  "first_divergent_step": 42
}
```

The CLI and server should share the same Rust core library. The CLI must not
scrape or drive the web UI.

## Backend Session Model

Each live game session has one backend-owned state machine:

- `not_attached`
- `attached`
- `recording`
- `fidelity_ok`
- `fidelity_lost`
- `blocked`
- `ended`

Important session data:

- session id
- bridge id/process id/client id
- character, ascension, seed string, numeric seed when known
- trace path
- latest observed state
- latest simulator state, if fidelity is available
- legal live actions
- fidelity status and first divergent step
- last bridge heartbeat/state timestamp

Trace writes should be append-only. A trace must never be silently overwritten.

## Stage 1: Manual Trace Collection

Stage 1 is successful when a human can play a run from start to finish through
the UI while collecting a useful trace.

Required features:

- start a run by character, ascension, and seed string or raw numeric seed
- list active bridges
- attach to a bridge
- kill selected bridges and kill all bridges
- manually request state
- show current phase and legal actions
- provide one button for each currently legal live action
- append observed states, sent commands, responses, and errors to a trace
- replay the trace through the simulator when possible
- show fidelity as `unknown`, `ok`, or `lost`
- show the first divergent step and compact diff when fidelity is lost

Mid-run attach is allowed, but it must be honest:

- If the trace includes prior actions/states, replay it to bring the simulator
  up to speed.
- If attaching halfway without prior trace history, create a checkpoint-style
  trace marked `unverified_start`.
- Do not report seed-start fidelity for a halfway attach unless the trace
  genuinely proves it.

## Stage 2: Combat Agent Integration

Combat automation lives in the backend as a session-scoped automation job, not
as frontend clicks or sleeps.

Frontend controls:

- select policy
- select depth and width
- choose which current potions are usable
- show current best plan
- highlight the manual action matching the next planned action
- show predicted final HP
- run one planned action
- auto-play the current combat plan

Backend automation states:

- `idle`
- `planning`
- `waiting_for_fidelity`
- `ready_to_send`
- `sending_action`
- `waiting_for_observed_state`
- `verifying_transition`
- `paused`
- `blocked`
- `done`
- `failed`

Each step must be fidelity-gated:

1. read latest bridge state
2. verify fidelity is still acceptable
3. ask the combat agent for a plan
4. map the next simulator action to exactly one current live legal action
5. send that typed action
6. wait for a new observed state or explicit bridge response
7. replay and verify the transition
8. continue, pause, or block with a reason

If action matching is ambiguous, stale, unsupported, or desynced, automation
must stop in `blocked`. It must not guess.

## Stage 3: SlayTheData Integration

SlayTheData integration is deliberately out of scope until Stage 1 and Stage 2
are reliable.

When added, SlayTheData should be a separate backend subsystem that proposes
high-level choices. It should not own trace capture, bridge state, simulator
state, or combat automation.

## Action Protocol

The backend and bridge should exchange typed actions, not UI labels.

Examples:

```json
{"kind":"play_card","card_instance_id":"...","target_id":"..."}
{"kind":"use_potion","slot":1,"target_id":"..."}
{"kind":"choose_reward","reward_id":"...","option_index":0}
{"kind":"end_turn"}
```

Live legal actions should carry enough identity to match simulator actions and
enough display metadata for the frontend to render them. Display labels are for
humans only.

## Testing Strategy

Stage 1 tests:

- trace append and recovery tests
- fake bridge tests for start, attach, request state, send action, and kill
- legal-action rendering tests against fixed backend responses
- fidelity status tests for ok, lost, unknown, and unverified-start sessions
- replay tests using permanent traces
- CLI e2e tests against a fake bridge
- HTTP API e2e tests against a fake bridge
- optional browser e2e tests against a fake backend or fake bridge

Stage 2 tests:

- fake bridge auto-play loop tests
- action-matching tests from simulator action to live legal action
- blocked-state tests for ambiguous, stale, and desynced states
- policy configuration serialization tests
- pause/cancel/resume tests for automation jobs

Most e2e tests should use a fake bridge that implements the same observable
contract as the real CommunicationMod bridge. That keeps CI deterministic,
fast, and independent of a running game, graphics stack, Steam install, mod
loader, or local save/profile state.

Real-game smoke tests are still useful, but they should be explicit manual or
quarantined tests. They are not a substitute for fake bridge and replay tests,
and they should never be required for ordinary CI.

The manual real-game smoke checklist lives in `LIVE_TRACE_REAL_GAME_SMOKE.md`.

## Implementation Status

The first implementation slice provides:

- `sts_live` Rust crate
- typed core models for sessions, bridges, legal actions, trace records,
  fidelity status, and blocked states
- fake bridge manager for deterministic tests
- CommunicationMod bridge manager that reads the bridge session files, uses the
  TCP JSONL control socket for guarded commands when available, and requires an
  explicit `STS_LIVE_ALLOW_FILE_COMMANDS=1` opt-in before falling back to legacy
  `next_command.txt`
- backend-generated CommunicationMod legal actions with stable IDs and typed
  action kinds for visible choices, combat card plays, simple commands, request
  state, and abandon run
- abandon run is treated as a bridge operator control: TCP uses the
  `abandon_run` control message rather than ordinary available-command matching,
  while legacy file fallback writes `ABANDON`
- observed-state fidelity checking for live traces that can be converted back
  into CommunicationMod-style state/action JSONL, using `sts_verify` rather
  than a separate UI-side replay interpretation
- seed-start fidelity checking is attempted for traces with live-trace run
  metadata and a real `START` history; expected verifier boundaries remain
  `unknown`, unexpected diffs become `lost`, and only boundary-free seed-start
  reports become `ok`
- fake or incomplete traces stay `unknown` until replay has enough real
  CommunicationMod state/action evidence; checkpoint attaches remain
  `unverified_start`
- append-only trace writer and recovery check
- operator request-state polling is recorded as a trace action before the
  resulting observed state, so manual synchronization steps remain replayable
- bridge command results are recorded as compact `response` trace records before
  the observed state, preserving command/response/state ordering without making
  fidelity replay depend on UI transport details
- session store with start, attach, request state, send action, abandon run, and
  kill bridge flows
- session recovery from existing trace JSONL files; the binary reloads trace
  sessions on startup so one-shot CLI commands can continue a prior session
- session listing through the core store, CLI, HTTP API, and thin UI selector so
  recovered sessions are discoverable after server or CLI restart
- bridge send/request failures are recorded as structured trace errors and move
  the session to `blocked`; fidelity loss records a `fidelity_lost` error,
  exposes the compact diff, and prevents further gameplay sends
- CLI/operator commands backed by the same session store; the binary prints
  compact JSON for successful command results and structured JSON errors for
  failed operator commands
- HTTP API errors use the same structured `{ error: { kind, message } }` shape
  as CLI errors; the UI displays the backend-provided message
- minimal HTTP API and static Stage 1 web shell, including selected-bridge and
  kill-all bridge controls
- fake-bridge tests for core, CLI, and HTTP paths
- static UI contract tests that pin the Stage 1 control IDs and verify the web
  client sends typed backend action ids rather than display labels
- CommunicationMod bridge contract tests using deterministic temporary session
  files, without launching Slay the Spire
- manual/quarantined real-game smoke checklist for validating `sts_live`
  against an actual ModTheSpire + CommunicationMod process

Optional follow-up:

- long-running browser e2e tests

## Non-Goals

Not in Stage 1:

- SlayTheData search
- run-level route automation
- combat planning
- automatic action execution
- database indexing
- browser-driven clicking
- unsupported state repair

Not ever:

- silently continuing after unexplained fidelity loss
- hiding bridge errors behind retry loops
- treating legal divergence as strict parity
- letting frontend state become gameplay truth
