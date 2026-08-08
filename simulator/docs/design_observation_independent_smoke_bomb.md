# Observation-independent Smoke Bomb verification

> **Historical and superseded.** Boundary schema v1 removed Smoke Bomb UI
> staging and deferred visibility reconciliation. Active replay accepts only an
> immediate same-step authoritative completion and never carries later frames.

## Problem

The target game exposes Smoke Bomb escape through asynchronous UI frames. The
command first consumes the potion while combat remains visible, end-of-combat
healing can appear on a later combat frame, an empty combat-reward screen then
opens, and `PROCEED` can remain on that screen until the map settles.

The verifier previously represented those frames by overwriting the core
destination with copied pre-combat fields and later fabricating a `RewardScreen`
when observation reported `COMBAT_REWARD`. Repeated `PROCEED` commands were
counted as verified before the map appeared. That made observed UI timing an
authority over simulator state.

## Core and UI state

`RunAction::UsePotion` is the only authoritative transition. For Smoke Bomb it
must produce an idle run with no combat and no reward. The verifier stores that
core destination immediately and never replaces or patches it.

A separate `SmokeBombUiState` describes protocol visibility only:

- `Escaping` retains the pre-command combat snapshot and the originating action;
- `Reward` retains any reward-screen `PROCEED` actions awaiting a stable map.

The UI state cannot supply values to `RunState` and cannot choose a simulator
destination.

## Deferred visibility contract

While combat remains visible, the transient projector derives combat contents
from the pre-command simulator state and the consumed potion belt from the core
destination. Player HP is deliberately deferred because the target publishes
both pre-heal and post-heal combat frames during the escape timer. It is not
discarded: the first stable empty-reward projection compares HP, deck, relics,
gold, floor, and the empty reward contents against the core destination.

The Smoke Bomb action is reconciled only at that stable reward frame. A
`PROCEED` whose post-state is still the empty reward remains deferred, including
repeated accepted commands, and each is reconciled only after a stable map
projection matches. Ending verification before either reconciliation increments
`unresolved_transient_assertions` and therefore cannot be a complete pass.

## Regression contract

The focused state test requires the transient projector to leave both source
and destination untouched and the authoritative destination to remain idle with
no fabricated reward. The committed long live regression pins the real queued,
healed, empty-reward, repeated-`PROCEED`, and stable-map sequence; actions 808,
811, and 812 must all be verified with reconciled deferred assertions.
