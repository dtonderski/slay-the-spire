# Fair Python/RL API Design

## Purpose

This document proposes the design for a future Python API and reinforcement
learning interface for the Rust Slay the Spire simulator.

The immediate target is combat-only RL. The long-term target is full-run RL,
planning, replay, and evaluation. The design therefore starts with combat, but
names and boundaries should not trap us in a combat-only API.

The central requirement is fairness:

> The simulator may know hidden state. A fair RL agent must only receive
> information that a real player could infer from the UI.

This is stronger than "do not put the draw pile in the observation". Fairness
also applies to legal action masks, action descriptors, errors, `info` fields,
debug strings, logs, serialization, snapshots, and Python object behavior.

## Current Repo Context

The simulator currently has a clean separation between mechanics and
verification:

- `sts_core`: deterministic simulator state, content, actions, transition
  logic, RNG, snapshots, and run/combat systems.
- `sts_verify`: trace formats, normalization, real-game comparison, and
  CommunicationMod parity tooling.

The existing `DESIGN.md` already reserves future layers:

- `sts_rl`: optional later crate for environment wrappers and feature
  extraction.
- `py-sts`: later Python bindings using PyO3 or maturin.

That separation should be preserved. The simulator core should not know about
tensors, policies, reward shaping, Gymnasium, or Python ergonomics.

## Key Decision

Do not bind `sts_core` directly to Python for the fair RL API.

Instead, add a narrow Rust facade that privately owns authoritative simulator
state and returns only fair, derived data.

Recommended layers:

1. `sts_core`
   - Full authoritative simulator.
   - Owns `CombatState`, `RunState`, RNGs, snapshots, transition internals,
     hidden pools, counters, and all gameplay truth.

2. `sts_rl` or `sts_env`
   - Rust facade crate.
   - Owns hidden `CombatState` / `RunState` privately.
   - Exposes `Observation`, `ActionDescriptor`, `StepResult`, `EnvConfig`,
     and `VisibilityPolicy`.

3. `py-sts`
   - PyO3/maturin bindings over the facade.
   - Does not expose raw `sts_core` types in fair mode.

4. `sts_gym`
   - Thin Gymnasium-compatible Python wrapper.
   - Converts facade outputs into fixed-shape spaces, action masks, and
     discrete action indices.

5. Debug/omniscient module
   - Separate module, package, or feature flag.
   - Can expose full snapshots, full state hashes, RNG state, branchable
     restore, and replay/debug tools.
   - Must be visibly separate from the fair API.

## Why Not Bind `CombatState` Directly?

`CombatState` and `RunState` are the engine's truth. They are useful for tests,
verification, and exact replay, but they are not safe observations.

Direct binding would leak information such as:

- draw pile order
- RNG seeds, counters, and internal state
- monster move counters/history
- future reward/shop/event/relic pools
- generated choices before the player should see them
- full pending action/select internals
- hidden relic counters
- exact run-level stream state

Current core structs are also intentionally serde/debug friendly because that is
useful for verification. That makes them especially dangerous as Python-facing
objects. A Python user should not be one accidental `repr(env.state)` away from
seeing the future.

Tradeoff:

- Direct bindings are faster to implement and easier for debugging.
- A facade takes more design work, but gives us a meaningful fairness boundary.

Decision:

- Use direct/full state only in debug, verification, and omniscient tooling.
- Use a facade for fair RL.

## Security Invariant

The fair API should satisfy observational non-interference:

> If two authoritative simulator states differ only in hidden information, every
> fair public API output must be identical until that hidden information becomes
> visible through normal gameplay.

This must hold for:

- observations
- legal action descriptors
- action masks
- rewards
- `info`
- errors
- string/repr/debug output
- serialization
- logs
- pickle/deepcopy behavior
- batch environment worker state transfer

This is the core proof obligation. The API should be designed so violating it is
hard and easy to test.

## Fair API Shape

The Rust facade should expose a small environment surface:

```text
reset(config, seed) -> StepStart
observe() -> Observation
legal_actions() -> Vec<ActionDescriptor>
action_mask() -> Vec<bool>
step(action_index) -> StepResult
visible_replay_log() -> VisibleReplayLog
```

The fair facade should not expose:

- `CombatState`
- `RunState`
- generic `Snapshot<T>`
- RNG objects
- RNG seeds/counters
- `clone`
- `restore`
- full JSON serialization
- full debug strings
- detailed hidden-state validation errors

For reproducibility, the fair API should prefer:

```text
seed + config + visible action log
```

over branchable mid-episode snapshots.

## Gymnasium Wrapper Shape

Gymnasium is the right compatibility target for Python RL libraries, but it
should be a wrapper over the facade, not the core API.

Initial wrapper:

```python
env = StsCombatEnv(config)
obs, info = env.reset(seed=seed)
obs, reward, terminated, truncated, info = env.step(action_index)
```

Recommended conventions:

- `action_space = gymnasium.spaces.Discrete(max_actions)`
- `observation_space = gymnasium.spaces.Dict(...)`
- `info["action_mask"]` contains valid visible action slots.
- `info["actions"]` contains visible descriptors for current action indices.
- `info["phase"]` contains coarse visible phase.
- `info` must not contain RNG, hidden hashes, full diffs, or exact hidden
  internals.

For someone new to RL, the mental model should be:

> observe fixed arrays, read the mask, choose a discrete action, step.

Avoid arbitrary nested JSON in the hot path. It is friendly to humans but
awkward for training code.

## Observation Representation

Use fixed-shape arrays with masks/padding in the Gym wrapper. The lower-level
facade can expose richer DTOs as long as they are visible-only.

Combat observation examples:

- player HP, max HP, block, energy
- visible player powers
- hand slots with visible card content/features
- hand slot mask
- draw pile count
- discard pile count
- exhaust pile visible contents/counts according to UI rules
- monster slots with HP, max HP, block, powers, alive flag
- monster intent when visible
- monster intent mask when hidden
- owned relics
- owned potions
- current visible prompt/phase

Do not use runtime `CardId` as a primary learning feature. Runtime card IDs are
instance identity, not card meaning. Observations should emphasize content IDs,
card definitions, cost, type, target mode, and relevant visible modifiers.

Action descriptors can carry runtime IDs internally if needed to route back into
`sts_core`, but those IDs should not become semantic features for the learner.

Tradeoff:

- Nested structured observations are easier to inspect.
- Fixed arrays are easier to train on and easier to declare as Gym spaces.

Decision:

- Facade DTOs may be structured.
- Gym wrapper should use fixed-shape arrays plus masks.

## Visibility Policy

Visibility must be centralized. Do not scatter visibility decisions across card,
relic, combat, run, and Python binding code.

Use hidden-by-default, visible-by-explicit-rule.

Suggested type:

```text
VisibilityPolicy
  observe_combat(state) -> CombatObservation
  observe_run(state) -> RunObservation
  visible_action_descriptors(state) -> Vec<ActionDescriptor>
  public_error(error, context) -> PublicError
  public_info(step_context) -> PublicInfo
  public_repr(object) -> String
```

The point is not the exact API. The point is that the same policy governs every
public output surface.

### Combat Visibility

Normally visible:

- player HP, max HP, block, energy
- visible player powers
- hand card contents and visible temporary costs
- discard pile count
- draw pile count
- exhaust pile contents if UI-visible
- monster HP, block, visible powers
- monster intent, unless hidden by an effect
- owned relics
- owned potions
- current player prompt

Normally hidden:

- draw pile order
- draw pile contents, except when visible by rule
- RNG state, seeds, counters, and logs
- monster move history/counters
- louse rolled damage before it is represented in visible intent
- generated future cards/rewards before shown
- internal action queue
- hidden relic counters
- exact hidden target-selection internals

Special cases:

- `FrozenEye` can reveal draw pile order.
- `RunicDome` hides monster intents.
- Other relic/card/effect visibility exceptions should be hooks in
  `VisibilityPolicy`, not ad hoc exceptions in Python.

### Run Visibility

Normally visible:

- current floor
- current HP/max HP/gold
- deck contents
- visible map nodes and edges
- current room/screen
- visible rewards once offered
- visible shop contents once the player can see them
- visible event choices

Normally hidden:

- future unseen map information
- relic pools
- reward pools
- event lists
- shop inventory before it is visible
- boss/reward/card/potion/relic RNG stream state
- future room outcomes
- internal counters not exposed through UI

## Action Descriptors And Action Masks

Action descriptors are part of the observation boundary.

They can leak hidden state if generated from authoritative legality without
visibility filtering.

Example risk:

- Havoc can depend on the top draw card.
- If the action mask or target list changes based on the hidden top card, the
  agent learns something about the draw pile.

Therefore, fair action generation should use visible action shapes.

Safe descriptor examples:

```text
EndTurn
PlayHandSlot { hand_slot, target_slot? }
UsePotionSlot { potion_slot, target_slot? }
ChooseVisibleOption { option_index }
Confirm
Cancel
```

Unsafe descriptor examples:

```text
PlayCardByInternalId { card_id, target_id }
HavocTopCardIsAttackTargetMonster
ChooseRewardFromHiddenPool
DebugInternalAction(...)
```

When legality depends on hidden state, prefer one of these approaches:

1. Use visible-shape actions and resolve internally.
2. Reject invalid hidden-dependent attempts with a coarse public error.
3. In narrow cases, intentionally include all visible plausible targets so the
   mask does not reveal which hidden branch is true.

Tradeoff:

- Exact legal masks are convenient for learning and avoid invalid actions.
- Exact legal masks can become side channels.

Decision:

- Fair masks must be visibility-safe, not merely engine-legal.
- Debug/omniscient APIs may expose exact legality.

## Errors, Info, Repr, And Logs

Errors can leak hidden state.

Unsafe public errors:

```text
top draw card requires target
reward pool exhausted
RNG counter mismatch
unknown hidden card id
Havoc top card cannot have target
```

Safe public errors:

```text
InvalidAction
InvalidActionShape
ActionMasked
EpisodeDone
InternalError
```

Detailed reasons belong in debug/omniscient mode.

`info` should be small in the fair Gym wrapper. It can include:

- action mask
- visible action descriptors
- phase
- coarse terminal reason
- reward components if they are derived only from visible/current transition
  information

It should not include:

- full state hash
- RNG counters
- RNG logs
- hidden transition diffs
- full event log
- exact debug terminal internals
- hidden validation reasons

`__repr__`, `str`, Rust `Debug`, logs, panic messages, and serialization should
all be redacted in fair Python objects.

Tradeoff:

- Rich debug info dramatically improves development.
- Rich debug info in the fair env breaks the security model.

Decision:

- Keep fair output boring.
- Build good debug tools separately.

## Snapshots And Restore

The fair API should not expose mid-episode `snapshot()` / `restore()`.

Reason:

Even if the snapshot bytes are opaque, clone/restore creates a branch oracle. An
agent can restore, try actions, observe future draws/rewards, and infer hidden
state.

Use separate capability classes:

```text
ObservationSnapshot
  visible-only, safe to expose

ReplaySeedAndActions
  fair reproducibility path

FullSnapshot
  exact resume, debug/verification only

DebugSnapshot
  full state plus logs/RNG/diffs, debug/verification only
```

Tradeoff:

- Restore is extremely useful for planning, debugging, and tree search.
- Restore is not fair for model-free RL if exposed through the same API.

Decision:

- No branchable restore in fair env.
- Provide branchable restore only in explicit omniscient/debug APIs.

## Reward Design

Reward shaping should not live in `sts_core`.

The core/facade can expose visible transition signals. Python wrappers can turn
those into rewards.

Initial reward modes:

```text
terminal:
  win  -> +1
  loss -> -1
  otherwise 0

hp_delta:
  optional wrapper using visible HP changes

combat_progress:
  optional later wrapper using damage, block, or fight outcome
```

Avoid clever shaping at API birth. It can make early experiments harder to
interpret.

Tradeoff:

- Sparse terminal reward may learn slowly.
- Shaped reward can train faster but bakes in assumptions and can reward weird
  behavior.

Decision:

- Start with terminal reward.
- Add shaping as named wrappers, not simulator behavior.

## First Implementation Scope

Start with a restricted combat-only environment:

- Ironclad
- fixed combat fixtures
- Strike, Defend, Bash
- one or a few simple monsters
- no potions
- no rewards
- no map
- no shop
- no choice-opening cards at first

Then add:

1. Multiple monsters.
2. More normal cards.
3. Choice actions: hand select, discard select, exhaust select.
4. Relics with visible effects.
5. Potions.
6. Combat rewards.
7. Full run phases.

Do not permanently design around only combat. Use names that leave room for
`RunEnv`, even if the first mode is combat-only.

Tradeoff:

- A tiny first environment is not representative of real Slay the Spire.
- It lets us validate the observation/action/fairness architecture before the
  full game makes every mistake expensive.

Decision:

- Small combat-first slice.
- Run-shaped architecture from day one.

## Python Binding Choice

Use PyO3 plus maturin after the Rust facade stabilizes.

Rationale:

- Python RL tooling expects importable native packages.
- PyO3 is the standard Rust-to-Python binding path.
- maturin handles local development builds and wheel packaging.
- Python should not reimplement game mechanics.

Tradeoff:

- PyO3 bindings introduce packaging complexity.
- JSON/subprocess APIs would be easier to prototype but slower and less
  ergonomic for RL.

Decision:

- Build the Rust facade first.
- Bind the facade with PyO3/maturin.
- Avoid binding raw `sts_core` state.

## Test Strategy

Fairness needs tests before Python bindings become habit.

### Observational Equivalence Tests

Construct pairs of authoritative states with identical visible state but
different hidden state:

- different draw pile order
- different RNG seeds/counters
- different relic pool
- different reward pool
- different event list
- different shop future
- different monster move history
- different generated-but-not-visible pending choices

Assert identical fair outputs:

- `Observation`
- `ActionDescriptor`
- action mask
- `StepResult.info`
- public errors
- public repr/string output
- fair serialization

### Visibility Exception Tests

Specific tests:

- draw pile order is hidden by default
- `FrozenEye` reveals draw pile order
- monster intent is visible by default
- `RunicDome` hides monster intent
- Havoc-style action masks do not reveal hidden top-deck target mode
- RNG fields never appear in fair observation or `info`

### Python Export Denylist Tests

The fair Python module should not expose:

- `CombatState`
- `RunState`
- `Snapshot`
- `FullSnapshot`
- RNG types
- `clone`
- `restore`
- full `to_json`
- pickle/deepcopy hidden state

### Redaction Tests

Golden tests for:

- repr
- str
- exceptions
- logs
- serialized observation
- public info dict

### Fuzz Hidden Fields

Fuzz hidden fields while holding visible fields fixed. Public fair outputs must
remain equal until the hidden state becomes visible through normal gameplay.

## Documentation To Add Before Implementation

Before writing bindings, add:

1. Field-by-field visibility table.
2. Action descriptor schema.
3. Fair/debug/omniscient API boundary document.
4. Reward wrapper notes.
5. Gym observation/action space schema.
6. Test checklist for new cards, relics, potions, and run screens.

## Open Questions

These do not block the architecture:

- Exact first observation schema and max padding sizes.
- Whether the first Gym wrapper should use pure dict spaces or immediately
  expose tensor-friendly arrays.
- How much of CommunicationMod's observed shape should influence the facade.
- How to model belief-state APIs later.
- Whether debug/omniscient APIs should live in a separate package, separate
  feature flag, or both.
- How strict to be about timing side channels. For local RL this is probably
  lower risk than action-mask/info/repr leaks.

## Summary

The fair Python/RL API should be a facade, not raw simulator bindings.

The simulator core keeps full truth. The fair facade derives visible-only
observations, visibility-safe action descriptors, coarse public errors, and
minimal public info. Gymnasium sits on top as a beginner-friendly wrapper with
fixed arrays, masks, and discrete actions.

The main design tradeoff is speed of implementation versus strength of the
fairness boundary. Binding core state directly would be faster, but would almost
certainly leak hidden information. A facade costs more upfront, but makes
cheating structurally unavailable through the fair Python API.

