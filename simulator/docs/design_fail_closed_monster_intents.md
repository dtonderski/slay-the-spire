# Fail-Closed Monster Intent Boundary

## Problem

Generic monster intent preparation currently turns two distinct correctness
failures into plausible gameplay:

- an unregistered monster identity is interpreted as `FIXED_SIMPLE_MONSTER`;
- a known monster without a source-backed intent implementation is assigned an
  approximate public-backlog cycle.

Both outcomes can survive as ordinary combat state. A caller therefore cannot
distinguish modeled mechanics from substituted behavior.

## Decision

Generic intent preparation returns `SimResult<MonsterIntent>`.

- An unregistered `ContentId` returns `SimError::UnknownContent`.
- A known identity that reaches the approximate public-backlog branch returns
  `SimError::UnsupportedMechanic` carrying that `ContentId`.
- Source-backed, monster-specific intent paths remain unchanged.

Combat entry and end-turn processing propagate these errors to the existing
authoritative action boundary. They do not commit a partially advanced state.
Tests may still construct explicit deterministic monsters, but they cannot use
the production transition path to obtain a representative intent for
unsupported content.

This slice does not rewrite monster AI or decide that every historically
"backlog" identity is unsupported. A monster that is handled by a concrete
source-backed branch remains executable; the error applies only when execution
would otherwise reach the approximate generic cycle.

The source-backed complex-intent helper contains only supported identities and
returns `None` for every other identity. Approximate implementations for the
explicitly unsupported monsters are not retained behind an early error check,
and the helper has no default `Stun` fallback that could become reachable after
future dispatch changes.

## Verification

Regression tests must prove that unknown identities return `UnknownContent`,
known approximate identities return `UnsupportedMechanic`, supported monsters
retain their intents and RNG counters, and failed entry/end-turn transitions do
not expose a plausible successor state.
