# Copied-attack visibility contract

> **Historical and superseded.** Boundary schema v1 removed copied-attack
> visibility deferral and later-frame reconciliation. Active replay requires an
> immediate same-step authoritative completion.

## Problem

The seed-start verifier used one predicate to combine three different facts:
the pre-state and command implied a queued Double Tap copy, the observed
post-state exposed a decremented Double Tap counter, and observed monster state
differed from the fully resolved simulator state. That made an observed
difference itself part of deciding whether to defer verification.

## Evidence

The committed CODEX10 trace records Havoc+ playing Double Tap+, followed by
Uppercut+. At step 480, the post-frame is `WAITING_ON_USER`, Double Tap has
decremented from two charges to one, and Orb Walker has taken only the original
hit. The core transition includes the queued copy. Step 481 issues `END` against
the same incomplete state, so the retained trace correctly ends at
`unreconciled_copied_attack_frame` instead of claiming parity.

## Contract

- Whether a command queues a copied attack is derived only from authoritative
  pre-combat state, the typed combat action, and the played card definition.
- A fully matching post-projection is stable and is verified immediately.
- A non-matching post-frame is eligible for copied-attack deferral only when
  it is a command-ready `NONE` combat frame with no current action, has an
  authoritative player-power array, and exposes exactly the command-derived
  remaining Double Tap count.
- The partial frame is compared under the existing transient visibility
  projection, then creates a deferred assertion. It is never treated as a
  completed simulator outcome.
- The deferred assertion must reconcile against the full simulator projection
  before another semantic command. Missing, malformed, divergent, or
  unreconciled evidence remains a failure.

The observed frame classifies visibility only. It never changes the simulated
transition or its authoritative state.
