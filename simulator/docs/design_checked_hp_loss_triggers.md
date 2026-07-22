# Checked HP-Loss Triggers

## Problem

A positive HP-loss event can update several authoritative values in order:
every Blood for Blood instance gains one cost reduction, Centennial Puzzle and
Runic Cube may draw, Self-Forming Clay accumulates next-turn block, Red Skull
may synchronize Strength, and card-sourced HP loss finally applies Rupture.
Blood for Blood, Self-Forming Clay, and Rupture still use unchecked signed
addition. A late failure can also occur after earlier trigger mutations.

Combat-start Red Skull retains a second implementation separate from the
checked runtime synchronization boundary. Keeping both would leave error and
threshold semantics split while HP-loss hooks depend on the runtime helper.

## Decision

HP-loss and card-HP-loss hooks stage a cloned `CombatState` and commit only
after the complete ordered trigger sequence succeeds. Blood for Blood instance
reductions, Self-Forming Clay block, and Rupture Strength use `checked_add` and
return field-specific `InvalidState` errors. Blood for Blood updates remain one
per positive HP-loss event across hand, draw, discard, and exhaust piles.

The staged non-card hook runs Blood for Blood first, then relic triggers in
their existing order. The card hook performs that same sequence and applies
Rupture last. Zero or negative HP loss remains a no-op. A failure at any point
leaves all cards, counters, powers, piles, RNG, and relic state unchanged.

Combat-start Red Skull delegates to the same checked synchronization helper
with explicit relic presence, removing the duplicate arithmetic authority.

This slice does not change damage calculation, Buffer, Intangible, HP mutation,
or unrelated card/relic counters.

## Verification

Regressions cover Blood for Blood overflow across multiple piles, Self-Forming
Clay overflow after earlier triggers, Rupture overflow after relic triggers,
and combat-start Red Skull use of the shared checked boundary. Existing valid
card-loss, relic, draw, snapshot, and permanent replay behavior must remain
unchanged. Formatting, strict workspace Clippy, full workspace tests, snapshot
round trip, and repeated permanent-corpus replay remain required before commit.
