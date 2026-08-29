# Checked Card-Instance Allocation

## Problem

Run and combat state currently derive generated card IDs with unchecked
`max + 1` arithmetic. A state whose greatest card ID is the largest supported
positive signed-long value validates successfully, but its next generated card
is outside the external-state domain. Multi-card generators increment locally,
so they can also create a valid prefix before crossing the boundary.

## Decision

All authoritative card generation reserves its complete contiguous ID range
before mutating state or consuming RNG. The shared reservation helper validates
that the existing maximum and the final reserved ID remain in the positive
signed-long interoperability domain. `RunState` reserves after its deck maximum;
`CombatState` reserves after every authoritative combat location, including
open decisions and monster stasis.

Single-card generation reserves one ID. Multi-card rewards, selections, copies,
potions, and status generation reserve the full requested count before creating
the first card. Allocation failure is `InvalidState` and leaves authoritative
state and RNG unchanged.

## Verification

Regression tests cover exact-domain exhaustion, complete-range overflow, and
transactional rollback at representative run and combat action boundaries.
Existing generated-card ordering, RNG counters, snapshot round trips, and
permanent replay behavior remain unchanged.
