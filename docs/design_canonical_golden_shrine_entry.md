# Canonical Golden Shrine Entry

## Problem

Golden Shrine has two construction authorities. Normal event entry creates the
source-backed three-choice screen (`Pray`, `Desecrate`, `Leave`), while the
public generic `event_screen(Event::GoldenShrine)` routes through an early
milestone fixture exposing only `Pray`. Public compatibility wrappers can also
place that fixture into production `RunState`.

The same event identity can therefore begin with different legal actions based
only on which constructor a caller happened to use.

## Decision

- `event_screen(Event::GoldenShrine)` is the single context-free Golden Shrine
  constructor and exposes all three opening choices.
- Run-aware event construction delegates to that canonical constructor because
  Golden Shrine opening choices do not depend on run state.
- Remove the legacy/fixed Golden Shrine constructors, entry functions, and
  public re-exports.
- Preserve the old one-choice milestone scenario as a test-local fixture. It is
  explicit test data and cannot enter production through the core API.

Event resolution, deferred Regret acquisition, ascension gold, event RNG, and
saved trace behavior remain unchanged.

## Verification

Regression coverage must prove the generic and run-aware constructors expose
the same three choices, the test-local milestone fixture remains usable, and
workspace/corpus/snapshot gates remain deterministic.
