# Checked Player Debuffs

## Problem

Player Weak, Vulnerable, Frail, Hex, and Entangled applications and Strength
or Dexterity reductions currently use unchecked signed arithmetic after
Artifact and relic-immunity handling. Imported or otherwise divergent state at
an integer boundary can therefore panic in debug builds or wrap into plausible
state in optimized builds. Some monster-intent callers duplicate arithmetic
preflight checks, but other paths such as Spore Cloud, end-turn curses, and
queued player Vulnerable do not.

## Decision

Core player-debuff helpers return `SimResult<bool>`, where the boolean states
whether the debuff landed. Non-positive amounts are no-ops. Artifact consumes
one charge and returns `false`; Ginger and Turnip prevent their respective
debuffs without consuming Artifact. Additive applications use `checked_add`,
and Strength or Dexterity reductions use `checked_sub`, with a field-specific
`InvalidState` error. Confusion and Constricted use the same typed surface even
though their assignments cannot overflow.

Each helper proves the complete update on a copied `PlayerPowers` value before
assignment. Callers propagate failure through existing cloned combat and run
transitions. The duplicated monster-intent preflight helpers are removed so the
power boundary is the single arithmetic authority. Valid application order,
Artifact consumption, relic immunity, and vulnerable-just-applied behavior are
preserved.

This slice covers debuff application and reduction only. Positive player power
growth, temporary-power restoration, healing, and other combat counters remain
separate fail-closed work.

## Verification

Power-level tests cover overflow atomicity, Artifact, relic immunity, and
non-positive amounts. Authoritative transition regressions cover end-turn curse,
monster-intent, and nested Spore Cloud failures without mutating their input.
Formatting, strict workspace Clippy, full workspace tests, snapshot round trip,
and repeated permanent-corpus replay remain required before commit.
