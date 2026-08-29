# Unified Python Run Environment

Status: implementation contract.
Last updated: 2026-07-25.

## Decision

The public Python API has one state-owning environment, `RunEnv`, and one
player-action type, `Action`.

Fairness is a property of an observation projection, not a second simulator or
a second transition API. Omniscient search and fair policies receive different
state views but enumerate and apply the same player actions:

```text
RunEnv
  decision() -> Decision { observation, actions[] }
  observation() -> Observation
  legal_actions() -> tuple[Action, ...]
  step(Action) -> StepResult
  full_state() -> complete debugging dictionary
  snapshot() -> versioned restoration artifact
```

`RunEnv` owns the authoritative `RunState` and a monotonically increasing
decision revision. `Action` values returned by `legal_actions()` are bound to
that revision. Applying an action from an older revision fails before current
state is inspected. Cloning an environment preserves its revision, so an action
may be evaluated on an identical clone.

There is no public `step_fair`, `step_exact`, fair action, exact action, or
omniscient legal-action list. Internal `RunDecisionAction`, card IDs, monster
IDs, and source indices remain Rust implementation details. Replay and search
adapters may retain internal action types below the public Python boundary.

## State Views

- `observation()` returns the visibility-safe projection intended for policies
  and feature extraction. It is tagged by the active decision screen and
  covers combat, map, event, reward, treasure, rest, shop, card-grid, and
  complete screens.
- `full_state()` returns a deep JSON-derived Python dictionary containing the
  complete authoritative state. It is explicitly privileged and carries no
  persistence compatibility promise.
- `snapshot()` returns a versioned, validated serialized checkpoint plus its
  deterministic hash. `RunEnv.from_snapshot()` is the supported restoration
  path.

Two hidden-distinct environments may have equal observations while their full
states and snapshots differ. A snapshot is not an observation or model input.

## Screen Projection Contract

The observation discriminant follows the active decision screen rather than
only `RunPhase`, because card grids and reward flows can overlay an owning
event, rest site, shop, or treasure room. Every supported decision has a
concrete observation; unsupported or malformed state returns a stable public
error.

The same screen projection supplies public decision-local slots for every
non-combat action. Map node IDs, card IDs, relic identities retained for later
rewards, RNG state, and pre-rolled future outcomes remain private.

## Compatibility

The old native `OmniRunEnv`, `FairCombatEnv`, `ExactRunAction`, and
`PlayerChoiceRequest` bindings remain temporarily available in the private
native or compatibility implementation modules so existing verifier/search
code is not broken in the same change. They are not exported by the package
root. New notebooks and user-facing documentation use only `RunEnv`,
`Decision`, and `Action`.
