# State Validation Boundary

## Problem

`CombatState` and `RunState` are public, deserializable structs. Callers can
therefore construct states that normal simulator transitions would never
produce. Several transition paths previously interpreted absent RNG or
malformed state as a plausible deterministic result. A snapshot that parses is
consequently not necessarily safe to execute unless both its structure and its
cross-field invariants are enforced.

## Decision

Authoritative action boundaries and supported snapshot restoration validate
state before use. Validation is observationally pure and consumes no RNG.

The first implementation establishes `CombatState::validate()` and integrates
the existing unique-card-pile check. It rejects:

- unknown card or monster content;
- duplicate card or monster instance IDs;
- invalid HP, block, energy, ascension, timer, or pending-counter bounds;
- more than one active combat decision;
- decision state outside the player-input phase; and
- inconsistent combat-card-reward metadata.

Explicit fixtures remain supported, but they must provide deterministic RNG
streams instead of representing RNG as absent. Production execution must never
interpret `None` as seed zero, first-item selection, no shuffle, or no roll.

The four authoritative combat streams are structurally mandatory through a
single `CombatRngState`. It is flattened into `CombatState` so the existing
`shuffle_rng`, `monster_rng`, `monster_hp_rng`, and `card_random_rng` snapshot
field names remain stable. Missing or null streams fail deserialization before
validation or execution. Tests and explicit fixture builders may construct all
four streams from a deterministic seed; production transitions cannot enter a
no-RNG mode.

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

New snapshots use schema version 7. Schema-version-1 through version-3 snapshots
explicitly migrate their legacy card-reward and relic authorities, while
schema-version-1 through version-4 snapshots migrate missing legacy Combust
damage from the recorded stack count. Schema-version-1 through version-5
snapshots migrate the former independent combat-decision fields into one active
typed decision plus an ordered queue, preserving the old legal-action priority
when multiple overlays were recorded. Every successful restore is validated and
normalized to version 7. Schema-version-6 and older run snapshots derive an
explicit reward continuation once from an unambiguous retained event, shop,
rest, or treasure owner. Current snapshots must carry canonical Combust stack
and damage fields and the typed combat-decision representation; retired fields
fail closed rather than disappearing during deserialization. Combat no longer
repairs these authorities while executing mechanics.
Absent RNG state cannot be inferred from post-state or replaced with a seed
without changing mechanics. Invalid historical snapshots fail with a parse or
validation error and must be recreated from an authoritative seed/trace or an
earlier valid snapshot.

Raw JSON imports remain explicitly labeled debugging APIs and validate before
becoming executable environments. Versioned snapshots are the supported
restoration contract.

## Verification

Regression tests must prove that valid fixtures and snapshot round trips still
work, that the flattened RNG wire shape remains stable, and that missing or null
RNG, duplicate identities, unknown content, impossible bounds, and malformed
decision migrations fail before any transition executes.
