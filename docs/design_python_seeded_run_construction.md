# Python Seeded Run Construction

## Problem

`OmniRunEnv.new_ironclad()` accepts an optional seed. Omitting it silently
returns `RunState::combat_fixture_with_ascension`, so a call that appears to
construct a normal run instead starts inside a deterministic test combat.
Callers cannot distinguish an accidental missing argument from an intentional
fixture request.

The constructor also returns `Ok` without validating the generated run, which
allows inputs such as an ascension above 20 to cross the Python boundary.

## Decision

- `OmniRunEnv.new_ironclad(seed, ascension=None)` requires a nonempty seed.
- It validates the resulting `RunState` and reports invalid construction as a
  Python `ValueError`.
- Seeded production construction starts from a dedicated canonical Ironclad
  base and never routes through `map_fixture()` or its milestone map.
- `combat_fixture()` and `map_fixture()` remain explicit, deterministic test
  entry points. They are never selected by normal run construction.

This is an API hardening change: callers that intentionally used
`new_ironclad(None, ...)` must call the named fixture constructor instead.

## Verification

Regression tests must prove explicit seeds remain deterministic, empty seeds
are rejected, and invalid ascension is rejected rather than returned as a run.
The workspace, corpus, deterministic replay, and snapshot gates must remain
green.
