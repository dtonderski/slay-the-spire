# State Validation Boundary

## Problem

`CombatState` and `RunState` are public, deserializable structs. Callers can
therefore construct states that normal simulator transitions would never
produce. Today several transition paths interpret absent RNG or malformed state
as a plausible deterministic result. A snapshot that parses is consequently
not necessarily safe to execute.

## Decision

Authoritative action boundaries and supported snapshot restoration validate
state before use. Validation is observationally pure and consumes no RNG.

The first implementation establishes `CombatState::validate()` and integrates
the existing unique-card-pile check. It rejects:

- missing shuffle, monster, monster-HP, or card-random RNG streams;
- unknown card or monster content;
- duplicate card or monster instance IDs;
- invalid HP, block, energy, ascension, timer, or pending-counter bounds;
- more than one active combat decision;
- decision state outside the player-input phase; and
- inconsistent combat-card-reward metadata.

Explicit fixtures remain supported, but they must provide deterministic RNG
streams instead of representing RNG as absent. Production execution must never
interpret `None` as seed zero, first-item selection, no shuffle, or no roll.

`RunState::validate()` applies the same fail-closed boundary to run-level state.
It rejects impossible player, act, floor, potion-slot, reward, grid, shop, and
combat ownership state, validates all imported card content, and requires an
embedded combat to satisfy the combat invariants with matching ascension.

Known-but-unmodeled card identities may remain visible on reward, shop, or
grid choice surfaces. They are not executable content: selecting one into the
deck still fails validation with `UnknownContent`. This preserves honest choice
projection without turning unsupported card behavior into a modeled success.

Run screens are intentionally allowed to coexist with their owning phase.
Events, rewards, shops, and card grids use retained underlays during legitimate
subflows. Card-grid entries are also projections or candidates rather than a
second authoritative copy of every deck card, so identity uniqueness is checked
within the deck and within combat-owned locations instead of rejecting valid
deck/grid aliases. This exception is part of the screen contract, not silent
normalization.

## Snapshot compatibility

New snapshots use schema version 2. Schema-version-1 snapshots may be restored
only if the deserialized state already satisfies the version-2 invariants. This
is the only honest automatic migration: absent RNG state cannot be inferred
from post-state or replaced with a seed without changing mechanics. Invalid
version-1 snapshots fail with a validation error and must be recreated from an
authoritative seed/trace or an earlier valid snapshot.

Raw JSON imports are retained temporarily for debugging compatibility, but
they validate before becoming executable environments. A later API cleanup can
rename or remove them after external Python consumers are audited.

## Verification

Regression tests must prove that valid fixtures and snapshot round trips still
work, and that missing RNG, duplicate identities, unknown content, impossible
bounds, and conflicting decisions fail before any transition executes.
