# Checked Red Skull Synchronization

## Problem

Red Skull adds or removes Strength whenever combat HP crosses half of max HP.
The synchronization helper currently uses unchecked signed arithmetic and the
threshold expression multiplies HP by two. Imported boundary values can
therefore panic or wrap, while callers have already mutated HP, max HP, or
healing state.

The same helper is reached from HP-loss relics, Feed max-HP gain, direct combat
healing, potion and relic healing, Regeneration, and post-combat Burning
Blood/Black Blood/Meat on the Bone. Those void-returning surfaces cannot carry
a typed failure back to the authoritative combat or run transition.

## Decision

Red Skull synchronization returns `SimResult`. It compares HP with
`max_hp / 2`, avoiding an overflowing intermediate while preserving the target
integer half-health boundary. Strength activation and removal use `checked_add`
and `checked_sub`; the active flag changes only after representable Strength is
proved.

Combat healing stages a cloned `CombatState`, applies healing, synchronizes Red
Skull, and commits only on success. Post-combat healing stages the whole
Burning Blood sequence so a later Meat on the Bone synchronization failure does
not retain an earlier heal. Potion-use, start-of-combat, power-card, turn,
Feed, run-entry, and event/test callers propagate or explicitly assert the
typed result through their existing transaction boundary.

This is prerequisite plumbing for checked HP-loss-trigger arithmetic. Blood for
Blood reductions, Rupture, Self-Forming Clay, and unrelated healing arithmetic
remain separate slices.

## Verification

Regressions cover activation overflow, removal underflow, atomic direct healing,
and authoritative combat-action rollback. Existing healing, Red Skull, potion,
post-combat, snapshot, and permanent replay tests must remain unchanged on
valid state. Formatting, strict workspace Clippy, full workspace tests,
snapshot round trip, and repeated permanent-corpus replay remain required
before commit.
