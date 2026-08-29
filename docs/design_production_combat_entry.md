# Production combat entry without fixtures

## Problem

Normal, elite, boss, and event combat entry began with
`CombatState::initial_fixture()`. Callers replaced the fixture monster, piles,
player fields, and RNG streams in stages, but production correctness still
depended on remembering every overwrite. The Act 1 boss path also selected A0
fixture monsters and only changed the combat-level ascension afterward.

## Decision

- Keep `initial_fixture` and named encounter fixtures for tests only.
- Add a crate-owned combat-entry constructor that requires the real player,
  monsters, piles, relics, ascension, and all four combat RNG streams.
- Reject an empty monster list at construction.
- Build normal, elite, boss, and event combats from explicit monster vectors.
- Construct every boss through its registered definition and requested
  ascension instead of using A0 fixture state.
- Validate the complete combat after initial AI rolls and run-level relic
  initialization, before publishing it into `RunState`.

## Verification

Constructor tests prove that caller-provided state is retained and empty
encounters fail closed. A boss-entry regression proves ascension-aware monster
construction. Existing map, event, strict corpus, deterministic replay, and
snapshot gates remain required before commit.
