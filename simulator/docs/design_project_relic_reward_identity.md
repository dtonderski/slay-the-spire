# Project Relic Reward Identity

## Problem

The reward command binder accepted the observed pre-state and compared its first
relic offer with the simulator offer before applying `CHOOSE`. A mismatch stopped
the core transition and was reported as an unsupported boundary. This mixed
observation comparison into typed command execution and could reclassify a
simulator-fidelity difference as unsupported coverage.

Reward projections compared only the presence and ordering of `RELIC` reward
types, not the visible relic identity, so removing the binder check without a
projection field would lose evidence.

## Decision

The reward binder accepts only authoritative `RunState` plus the command. It
binds the visible index against simulator-owned reward ordering and applies the
core action without reading observation JSON.

Observed and simulated combat-reward projections both contain a fixed,
ordered `relic_offer_ids` list. Observed values come from every CommunicationMod
relic reward entry; simulated values come from the primary, pending, and queued
`RewardScreen` relic offers in command order. Absent offers project as an empty
list, and unknown observed identities remain as raw strings. A mismatch is
therefore an ordinary strict diff and cannot steer or block the simulator
transition.

This slice preserves reward ordering, Calling Bell continuation behavior, and
all core reward semantics.

## Verification

Regressions require reward commands to use simulator-owned ordering and prove
that a forged relic identity produces a `relic_offer_ids` difference while the
core still takes the predicted relic. Formatting, strict workspace Clippy,
workspace tests, snapshot round trip, and repeated permanent-corpus replay remain
commit gates.
