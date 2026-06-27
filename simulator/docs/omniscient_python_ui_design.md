# Omniscient Python API And Simulator UI Design

## Purpose

This document designs the first Python-facing simulator API and the UI/control
surface that should sit on top of it.

The first implementation target is intentionally omniscient. It is for:

- simulator-only play
- branchable MCTS/search
- combat advice
- debug inspection
- trace replay
- optional real-game bridge control through CommunicationMod

It is not the fair RL API described in `rl_python_api_design.md`,
`fair_action_schema.md`, and `rl_visibility_matrix.md`.

The near-term product should let us play the simulator without the real game,
optionally mirror or drive the real game when CommunicationMod is running, and
ask an omniscient searcher for combat actions. The Python API is the critical
foundation: UI, MCTS, scripts, and future notebooks should all call the same
Python package instead of reimplementing simulator mechanics.

## Design Position

The first Python API should bind the simulator as an explicit debug/planning
surface. It may expose hidden state, exact legal actions, branchable snapshots,
state hashes, internal IDs, RNG counters, and transition logs.

That is a feature for search and debugging, not a fairness mistake.

The separation is:

- `sts_core`: authoritative Rust simulator mechanics and state.
- `sts_verify`: trace import, normalization, CommunicationMod parity, and
  verifier reports.
- `py-sts` / `sts_python`: Python bindings for explicit omniscient control.
- future `sts_rl` / fair facade: visibility-filtered API for fair RL.
- future `sts_gym`: Gymnasium wrapper over the fair facade, not over the
  omniscient API.

The omniscient API is optimized for branchability and inspection. The fair API
is optimized for observational non-interference. They are sibling products, not
layers on top of each other.

## This API Is Intentionally Unfair

The omniscient API may expose:

- full `CombatState` and `RunState`
- ordered draw, discard, and exhaust piles
- internal `CardId`, `MonsterId`, `MapNodeId`, and generated object identity
- exact legal actions and exact validation errors
- full snapshots, clone, restore, and branching rollout
- state hashes and canonical debug serialization
- RNG seeds, counters, streams, and logs
- reward, relic, shop, event, potion, and map internals
- transition event logs and internal action queues
- verifier traces, diffs, unsupported boundaries, and resync metadata

Agents trained or evaluated through this interface should be labeled
omniscient, oracle, planner, debugger, or upper-bound agents. They should not be
described as fair-play RL agents.

## Naming Rules

The namespace and type names should make the unfairness impossible to miss.

Recommended Python import:

```python
import sts.omni as sts
```

Recommended type names:

- `OmniCombatEnv`
- `OmniRunEnv`
- `FullStateView`
- `FullSnapshot`
- `ExactCombatAction`
- `ExactRunAction`
- `ExactStepResult`
- `DebugTransition`
- `OracleSearch`

Avoid neutral names for omniscient types:

- `Env`
- `Observation`
- `Action`
- `StateView`
- `Snapshot`
- `LegalActions`
- `Info`

Use `Fair*` only for visibility-safe types. Use `Omni*`, `Full*`, `Exact*`,
`Debug*`, `Oracle*`, or `Branching*` for unfair types.

## Package Shape

Use PyO3 plus maturin for the first production path.

Rationale:

- MCTS needs fast in-process calls; a JSON subprocess protocol is too clumsy for
  hot rollouts.
- Python should not reimplement mechanics.
- The current Rust core already owns state, legal actions, transition logic,
  snapshots, and hashes.
- PyO3/maturin is the standard path for an importable Rust-backed Python
  package.

Initial layout:

```text
simulator/
  crates/
    sts_core/
    sts_verify/
    py_sts/              # cdylib PyO3 crate, or similar name
  python/
    sts/
      __init__.py
      omni.py            # friendly Python wrappers over PyO3 classes
      search.py          # MCTS/search helpers
      bridge.py          # CommunicationMod client adapter
      ui_api.py          # server-facing DTO helpers
```

The exact crate/package names can change, but the import boundary should remain
clear:

```python
import sts.omni
```

The PyO3 crate may depend directly on `sts_core` because this surface is
explicitly omniscient. A later fair package must bind a visibility facade
instead of binding raw core state.

## Core Python API

The first Python API should start with combat, then widen to run-level control
as the run-action surface stabilizes.

### Combat Environment

```python
import sts.omni as sts

env = sts.OmniCombatEnv.initial_fixture()
env = sts.OmniCombatEnv.from_snapshot_json(snapshot_json)

full_state = env.state_json()
snapshot = env.snapshot_json()
hash_hex = env.snapshot_hash()

actions = env.exact_legal_actions()
result = env.step(actions[0])

child = env.clone()
```

`OmniCombatEnv` owns a full `CombatState`.

Required methods:

```text
OmniCombatEnv.initial_fixture() -> OmniCombatEnv
OmniCombatEnv.from_state_json(json: str) -> OmniCombatEnv
OmniCombatEnv.from_snapshot_json(json: str) -> OmniCombatEnv
env.clone() -> OmniCombatEnv
env.state_json() -> str
env.snapshot_json() -> str
env.snapshot_hash() -> str
env.phase() -> str
env.exact_legal_actions() -> list[ExactCombatAction]
env.step(action: ExactCombatAction) -> ExactStepResult
```

Optional but useful early:

```text
env.state_dict() -> dict
env.card_by_id(card_id: int) -> dict
env.monster_by_id(monster_id: int) -> dict
env.pretty() -> str
```

### Combat Actions

Actions should mirror core `CombatAction` names and IDs in omniscient mode:

```python
sts.ExactCombatAction.end_turn()
sts.ExactCombatAction.play_card(card_id=17, target=3)
sts.ExactCombatAction.play_card(card_id=17, target=None)
```

This is intentionally different from fair mode, where actions are visible slots
such as `PlayHandSlot { hand_slot, target_slot }`.

The omniscient API may expose exact action legality. For example, `Havoc`
legality may depend on the top draw card. That is acceptable here because MCTS
is allowed to know the full state.

### Step Result

```python
@dataclass(frozen=True)
class ExactStepResult:
    state_json: str
    snapshot_json: str
    snapshot_hash: str
    phase: str
    exact_legal_actions: list[ExactCombatAction]
    transition: DebugTransition
    terminal: bool
    terminal_reason: str | None
```

`DebugTransition` should include:

```text
DebugTransition
  action
  previous_hash
  resulting_hash
  events
  rng_draws
  simulator_error
```

If event logs or RNG draw logs are not wired for a given path yet, return empty
lists and document the gap. Do not invent placeholder events that look
authoritative.

### Run Environment

Run-level support should be narrower at first because current run legal/apply
logic is split by screen and subsystem.

```python
run = sts.OmniRunEnv.from_snapshot_json(snapshot_json)
run = sts.OmniRunEnv.new_ironclad(seed="TEST", ascension=0)

run.state_json()
run.snapshot_json()
run.snapshot_hash()
run.phase()
run.current_decision()
run.exact_legal_actions()
run.step(action)
```

`OmniRunEnv` owns a full `RunState`.

If a run phase has no reliable exact legal-action adapter yet, return an
explicit `UnsupportedPhase` result. Do not silently guess, and do not patch in
observed real-game state as simulator truth.

### Run Actions

Run actions should initially mirror existing core action families:

- combat actions routed through `apply_combat_action_on_run`
- reward picks/skips
- potion use/discard
- map node choices
- event choices
- rest-site actions
- shop actions
- grid selection, confirm, and cancel
- screen navigation such as proceed, leave, and return where implemented

The Python action objects can be ergonomic wrappers, but the debug result should
record the resolved core action.

## Search API

The search API should live above `sts.omni`, not inside `sts_core`.

Initial combat search:

```python
from sts.search import CombatMctsConfig, search_combat

config = CombatMctsConfig(
    iterations=10_000,
    max_depth=40,
    rollout_policy="heuristic",
    objective="survive_then_damage",
)

recommendation = search_combat(env, config)
best_action = recommendation.best_action
```

Expected result shape:

```text
SearchRecommendation
  best_action
  principal_variation
  visits
  value
  win_probability
  expected_hp_delta
  terminal_rate
  diagnostics
```

The first useful objective does not need to solve the whole run. For trace
collection and live assistance, combat search can optimize:

1. avoid death
2. kill enemies
3. preserve player HP
4. reduce incoming damage this turn
5. prefer shorter winning lines

The objective should be configurable and visibly labeled. A search result is an
advisor output, not a simulator rule.

### Snapshot Discipline For Search

Search should branch through `clone()` or `snapshot_json()` / restore.

Rules:

- `exact_legal_actions()` must not mutate state.
- `state_json()` and `snapshot_hash()` must not mutate state.
- rollouts must operate on clones or restored snapshots.
- search diagnostics should include rollout limits and unsupported transitions.
- if a transition uses placeholder/source-partial mechanics, diagnostics should
  surface the fidelity category when available.

## UI Architecture

The UI should be a client of the Python API, not a separate simulator.

Recommended layers:

```text
Rust sts_core
  authoritative state and mechanics

PyO3 sts.omni
  branchable full-state simulator API

Python service
  session management, search jobs, bridge adapter, JSON DTOs

Web UI
  game board, debug panels, action controls, search controls

CommunicationMod bridge
  optional real-game mirror/control adapter
```

The UI can be implemented as a local web app. The server can be Python
initially because it needs to coordinate Python search, PyO3 simulator calls,
and CommunicationMod bridge state.

## UI Modes

The design should support five modes.

### Offline Simulator

The simulator is the only authority.

- UI reads `OmniRunEnv` or `OmniCombatEnv`.
- UI sends typed Python actions.
- Search can branch freely.
- No real game is required.
- Good for development, demos, and MCTS debugging.

### Trace Replay

The UI replays an existing CommunicationMod trace.

- `sts_verify` imports and normalizes trace states.
- UI shows observed states, commands, and simulator diffs.
- User can step forward/backward through trace records.
- Simulator seed-start replay can be shown beside observed real-game state.

### Live Bridge Mirror

The real game is the authority.

- CommunicationMod publishes observed state.
- UI displays bridge state and trace status.
- Simulator may run in parallel for comparison.
- UI does not send commands to the game.

### Live Bridge Control

Both simulator and real game receive actions.

- UI presents one action surface.
- Action is applied to the simulator.
- Adapter translates the same visible operation to a CommunicationMod command.
- The next observed real-game state is compared against predicted simulator
  state.
- Divergence is shown as a sync/parity issue, not silently repaired.

This mode is for assisted trace collection and live play. It must surface stale
bridge state, duplicate bridge clients, command acknowledgements, and trace
paths.

### Debug / Verifier

The UI exposes internal details.

- state hashes
- canonical diffs
- exact action lists
- event logs
- RNG logs and boundaries
- unsupported transitions
- observed-state restorations
- fidelity categories

This panel should be visually separate from the playable board. Debug-visible
state is not player-visible state.

## State Ownership And Sync

Offline simulator state is owned by `OmniRunEnv` or `OmniCombatEnv`.

Live bridge state is owned by the real game and represented by
CommunicationMod's observed JSON.

Verifier state is comparison data.

Do not blur these:

- observed CommunicationMod JSON is not full simulator truth.
- verifier normalization is not game mechanics.
- observed-state restoration is a verifier technique, not normal simulation.
- bridge commands are not the primary simulator action API.

When the UI imports or syncs observed real-game state into a simulator session,
that operation must be explicit and labeled:

```text
ImportObservedState
  source_trace
  action_step
  fields_imported
  fields_unknown
  fidelity_category = verifier_only
```

Silent sync would make later verification and debugging almost impossible.

## Unified UI Action Descriptor

Even though the first Python API is omniscient, the UI should use visible
operations where possible. That gives one action vocabulary for offline play,
bridge control, and later fair mode.

Initial UI descriptors:

```text
UiActionDescriptor
  PlayHandSlot { hand_slot, target_slot? }
  EndTurn
  UsePotionSlot { potion_slot, target_slot? }
  DiscardPotionSlot { potion_slot }
  ChooseVisibleOption { option_slot }
  ChooseMapNodeSlot { option_slot }
  ChooseRestOption { option_slot }
  ChooseShopSlot { option_slot }
  TakeRewardSlot { reward_slot }
  OpenCardReward
  OpenChest
  ToggleHandSlot { hand_slot }
  ToggleDiscardSlot { option_slot }
  ToggleExhaustSlot { option_slot }
  ToggleGridSlot { option_slot }
  ConfirmChoice
  CancelChoice
  SkipVisibleReward
  Proceed
  LeaveScreen
  ReturnToPreviousScreen
```

Resolution differs by backend:

- Offline omniscient backend resolves slots to exact core IDs and applies typed
  core actions.
- Live bridge backend translates slots/options to CommunicationMod command
  strings.
- Fair backend later keeps only visibility-safe descriptors and masks.

This descriptor layer should record both the visible descriptor and the resolved
exact action in debug mode.

## Decision Substates

The UI should render one primary decision substate at a time:

```text
DecisionSubstate
  NormalCombat
  PotionChoice
  ToolboxChoice
  CombatCardReward
  CardReward
  BossReward
  Map
  Rest
  ShopRoom
  ShopScreen
  Chest
  Event
  Grid
  ScreenNavigation
  HandSelect
  DrawSelect
  DiscardSelect
  ExhaustSelect
  Terminal
  Unsupported
```

If the simulator has multiple blocking substates and no source-backed priority
rule, the UI should show `Unsupported` instead of guessing. Debug mode can still
display the raw substates.

Potion use and potion discard can be ambient command families when the target UI
permits them.

## Bridge Adapter

The current CommunicationMod tooling is trace-collection oriented: a client
writes JSONL records, publishes current state/status files, waits for a
command, and sends string commands such as `PLAY`, `END`, `CHOOSE`, `POTION`,
`SKIP`, `PROCEED`, `LEAVE`, and `RETURN`.

That is useful bridge plumbing but should not become the offline simulator API.

The bridge adapter should own:

- reading current bridge state
- detecting stale state
- detecting duplicate bridge clients
- recording current trace path
- translating `UiActionDescriptor` to CommunicationMod strings
- waiting for command acknowledgement / next observed state
- surfacing bridge errors without mutating simulator state silently

Bridge DTO:

```text
BridgeStatus
  connected
  stale
  client_pid
  trace_path
  last_state_step
  pending_command
  last_command
  last_error
```

In live bridge control, each user action should produce:

```text
BridgeStepResult
  ui_action
  simulator_result
  command_sent
  observed_state
  parity_diff
  bridge_status
```

## UI Screens

The first UI should prioritize usefulness over visual parity with the game.

Core views:

- board: player, monsters, intents, hand, draw/discard/exhaust counts, relics,
  potions, current screen
- action panel: current decision descriptors and disabled reasons
- search panel: run MCTS, show recommendation, apply best action
- sync panel: offline/live/trace mode, bridge status, current trace path
- debug panel: exact IDs, full JSON, event log, hashes, diffs, fidelity labels

The debug panel should make hidden information visually distinct. A user looking
at an omniscient UI should not confuse debug-visible state with what a real
player can see.

## Interaction Robustness Requirements

The new UI must fix the failure modes from the current trace UI. In particular:

- clicking an action twice must not accidentally submit two game commands
- a stale action must not be accepted after the state has advanced
- invalid actions must not erase the action list
- unsupported actions must produce a visible error and leave the UI usable
- every visible game affordance must have a button or explicit unsupported
  explanation, including `LeaveScreen`, `Proceed`, `ReturnToPreviousScreen`,
  `ConfirmChoice`, `CancelChoice`, and reward/choice skips
- the UI must never silently fall into a state with no choices unless the
  simulator/game is terminal or the decision is explicitly marked unsupported
- bridge mode must show whether it is waiting for command acknowledgement,
  waiting for the next game state, stale, disconnected, or diverged

Use command lifecycle state for every user action:

```text
UiCommandLifecycle
  Ready
  Submitting { command_id, ui_action }
  WaitingForBridgeAck { command_id, sent_command }
  WaitingForNextState { command_id, expected_previous_state_id }
  Applied { command_id, resulting_state_id }
  Rejected { command_id, public_error }
  Diverged { command_id, parity_diff }
  Stale { last_state_id, bridge_status }
```

Each rendered action should carry the state identity it was derived from:

```text
UiActionDescriptor
  action_id
  source_state_id
  descriptor
  enabled
  disabled_reason?
```

The server must reject actions whose `source_state_id` does not match the
current session state. That prevents double-clicks and slow bridge updates from
applying old actions to new states.

In live bridge control, action submission should be single-flight by default:

1. user clicks an enabled action
2. UI disables all action buttons and shows the pending command
3. server records a unique `command_id`
4. simulator applies or rejects the action
5. bridge sends the corresponding CommunicationMod command, if requested
6. UI waits for acknowledgement / next observed state
7. action buttons are regenerated only from the new state

If the user clicks again while a command is pending, the UI should focus or
flash the pending command status instead of sending another command.

Invalid action handling should be boring:

```text
InvalidActionResult
  command_id
  source_state_id
  error
  state_unchanged = true
  actions = previous_or_regenerated_actions
```

Do not clear the action list on invalid actions. Do not infer terminal state
from an empty action list. Empty actions must be accompanied by one of:

- `terminal_reason`
- `unsupported_decision`
- `waiting_for_bridge`
- `stale_bridge`
- `internal_error`

The action generator should have coverage tests for every command family the
bridge can send:

- `PLAY`
- `END`
- `POTION`
- `CHOOSE`
- `CONFIRM`
- `CANCEL`
- `SKIP`
- `PROCEED`
- `LEAVE`
- `RETURN`

If the current screen has a visible command that is not implemented yet, render
it disabled with a specific `UnsupportedAction` reason rather than omitting it.

## Python Service API For The UI

The web UI should talk to a local service with stable JSON DTOs. This service is
not the hot MCTS loop; it is a session and presentation layer.

Suggested endpoints:

```text
POST /sessions
  { mode, seed, ascension, character, snapshot? }

GET /sessions/{id}/state
  -> UiState

GET /sessions/{id}/actions
  -> list[UiActionDescriptor]

POST /sessions/{id}/step
  { action: UiActionDescriptor, backend: "simulator" | "bridge" | "both" }
  -> UiStepResult

GET /sessions/{id}/pending-command
  -> UiCommandLifecycle

POST /sessions/{id}/search/combat
  { config }
  -> SearchJob

GET /sessions/{id}/search/{job_id}
  -> SearchRecommendation | SearchProgress

POST /sessions/{id}/snapshot
  -> FullSnapshot

POST /sessions/{id}/restore
  { snapshot }

GET /bridge/status
  -> BridgeStatus
```

`UiState` should be presentation-oriented and may include both public board
fields and debug-only fields when debug mode is enabled. Keep that distinction
explicit:

```text
UiState
  mode
  state_id
  decision_substate
  board
  visible_controls
  command_lifecycle
  debug_full_state?
  bridge_status?
  parity_status?
```

## Data Contracts

Use three different state contracts:

### FullSnapshot

Authoritative exact restore data.

- stable schema version
- simulator version
- full gameplay state
- full RNG state
- enough data to resume exactly

### FullStateView

Human/debug inspection data.

- JSON-friendly shape
- may include derived fields and helper labels
- not necessarily stable enough for long-term replay
- safe to show in debug UI

### UiState

Presentation DTO.

- derived from either simulator state or bridge observed state
- shaped for rendering and controls
- may include debug sections only when explicitly requested
- should not be used as exact restore input

Do not use one JSON blob for all three jobs.

## Error Model

Omniscient errors may be detailed.

Examples:

- `InvalidExactAction`
- `UnsupportedPhase`
- `SimulatorError`
- `SnapshotSchemaMismatch`
- `BridgeDisconnected`
- `BridgeStale`
- `BridgeCommandRejected`
- `ParityDiverged`

Detailed simulator errors are allowed in `sts.omni`. They are not allowed in
future fair mode unless mapped through a visibility-safe public error.

## Verification And Tests

Before calling the Python API usable, test:

- importing `sts.omni`
- creating an `OmniCombatEnv`
- listing exact legal actions
- stepping an action
- cloning and proving parent/child independence
- snapshot/restore round trip
- stable snapshot hash after restore
- `exact_legal_actions()` does not mutate state
- `state_json()` does not mutate state
- a tiny MCTS/search call returns an action without mutating the root

Before calling bridge control usable, test:

- bridge disconnected status
- stale bridge status
- duplicate client detection
- double-click protection with one command in flight
- stale `source_state_id` rejection
- invalid action preserves/regenerates available actions
- unsupported visible commands render disabled with reasons
- non-terminal empty action lists must report terminal, unsupported, waiting,
  stale, or internal-error state
- descriptor-to-command translation for `PLAY`, `END`, `CHOOSE`, `POTION`,
  `CONFIRM`, `CANCEL`, `SKIP`, `PROCEED`, `LEAVE`, and `RETURN`
- simulator-only step
- bridge-only command
- both-mode command with predicted-vs-observed diff

Before calling fair mode usable, use the tests in `fair_action_schema.md` and
`rl_visibility_matrix.md`. Do not infer fair safety from omniscient API tests.

## Implementation Plan

### Slice 1: Omniscient Combat Python Binding

- add PyO3/maturin crate
- expose `OmniCombatEnv`
- expose `ExactCombatAction`
- expose `ExactStepResult`
- support clone, state JSON, snapshot JSON, hash, exact legal actions, and step
- add Python smoke tests

Status: implemented as the `py_sts` Rust crate with a compiled `sts_omni`
extension and a source-level `sts.omni` wrapper.

Local verification on Windows currently needs the Python runtime directory on
`PATH` before running Rust tests for the PyO3 crate:

```powershell
$env:PATH = 'C:\Users\davton\AppData\Local\Python\pythoncore-3.14-64;' + $env:PATH
cargo test
```

The local wheel path is:

```powershell
py -3.14 -m maturin build -m crates/py_sts/Cargo.toml --release
py -3.14 -m pip install --force-reinstall target\wheels\py_sts-0.1.0-cp314-cp314-win_amd64.whl
$env:PYTHONPATH = "$PWD\python"
py -3.14 -m unittest discover -s python/tests
```

### Slice 2: Combat Search

- add Python `sts.search`
- implement simple deterministic one-ply or depth-limited search first
- keep the first MCTS/search implementation in Python
- move hot search loops into Rust only later, and only if benchmarks show the
  Python implementation is too slow
- expose recommendation and principal variation

Status: implemented as a deterministic Python depth search in `sts.search`.
The public entrypoint is:

```python
from sts.search import CombatSearchConfig, search_combat

recommendation = search_combat(env, CombatSearchConfig(max_depth=2))
```

The result exposes `best_action`, `principal_variation`, `visits`, `value`,
`win_probability`, `terminal_rate`, and diagnostics. It is a planning advisor,
not a simulator rule.

Verification:

```powershell
$env:PYTHONPATH = "$PWD\python"
py -3.14 -m unittest discover -s python/tests -v
```

### Slice 3: Local UI Service

- add session manager
- expose state/actions/step/snapshot/search endpoints
- render simulator-only combat first

Status: implemented as a dependency-free local Python service in
`sts.ui_service` plus static vanilla UI assets under `sts/ui_static`.

Implemented endpoints:

```text
POST /api/sessions
GET /api/sessions/{id}
GET /api/sessions/{id}/snapshot
POST /api/sessions/{id}/step
POST /api/sessions/{id}/search
```

The service uses snapshot hashes as `state_id`, requires `source_state_id` on
step requests, rejects stale actions without mutating state, and preserves
available actions after rejection.

### Slice 4: Run Environment

- expose `OmniRunEnv`
- support seed/start and snapshot restore
- add exact legal-action adapters by `RunPhase`
- return `UnsupportedPhase` for gaps

Status: implemented for deterministic `combat_fixture()` and `map_fixture()`
entrypoints, state/snapshot restore, clone, phase/current-decision reporting,
exact legal actions, and step dispatch.

The current exact action adapter covers core-backed combat, map, rest, event,
shop, and validation-backed reward actions. Seed-start construction and some
combat selection/potion substates are intentionally reported as explicit gaps
until their legal-action enumeration is first-class enough for UI/search use.

### Slice 5: Bridge Mirror

- replace the current trace UI with this UI; reuse only bridge plumbing that is
  still reliable
- read existing CommunicationMod status/current-state files as an adapter detail
- show bridge status and observed state
- replay traces in UI

Status: implemented as a read-only bridge mirror endpoint and UI panel. The
new Python service reads `tools/communication/session` files, reports
connected/stale/exited/pending-command state, and surfaces trace path, step,
available commands, and observed current-state JSON without sending commands.

### Slice 6: Bridge Control

- map UI descriptors to CommunicationMod commands
- send commands through bridge
- compare predicted simulator state with observed real-game state
- surface diffs and stale-state issues
- make this the future trace-collection path once simulator and bridge state
  synchronization is reliable enough

### Slice 7: Fair API

- only after the omniscient/debug workflow is useful
- build a separate facade using the fair docs
- do not wrap `sts.omni` objects as fair observations

## Open Questions

- Should the first import be `sts.omni` or `sts_omni`?
- Should `FullSnapshot` be JSON-only first, or should we add binary snapshots
  immediately after the smoke tests?
- Which combat fixture should be the first UI/search demo? Examples:
  - a tiny starter-deck Cultist fight, useful for checking the UI/search loop
    with simple Strike/Defend/Bash decisions
  - a known captured Act 1 trace combat, useful for comparing simulator advice
    against a real observed state
  - a hard late-Act combat from a trace, useful for testing whether MCTS advice
    actually helps trace collection
- Should live bridge control send to the simulator first, the real game first,
  or both as one coordinated operation? Examples:
  - simulator-first: apply the action locally, then send the command to the
    game only if the simulator accepts it; this protects the real run from
    obviously invalid UI actions
  - game-first: send the command to the real game, then advance/import/compare
    the simulator after CommunicationMod reports the result; this treats the
    real game as the source of truth
  - coordinated both-mode: send through one action pipeline, record both the
    predicted simulator result and observed game result, and surface a parity
    diff if they diverge
- Which fidelity categories should be elevated into first-class UI badges?
  Examples:
  - `source_backed`: decoded from target code or trace-backed; good default
    trust level
  - `captured_branch`: matches one observed trace branch but is not generalized;
    useful warning during search
  - `placeholder` or `legacy_fixed`: simulator scaffolding; search advice should
    be treated skeptically
  - `verifier_only`: trace repair/comparison machinery; should never look like
    normal game mechanics

## Summary

Build the first Python API as an explicitly omniscient `sts.omni` planning and
debug surface. It should bind Rust simulator state directly, expose exact
actions and snapshots, and make branchable search easy. Put the UI and MCTS on
top of that API.

Keep the fair RL docs as a separate future boundary. They are not the first
implementation contract, but they prevent us from accidentally presenting an
oracle as fair play. The API naming, package layout, tests, and UI labels should
all preserve that distinction.
