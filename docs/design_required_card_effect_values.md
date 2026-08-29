# Required Card Effect Values

## Problem

Modeled card-effect builders read damage, block, and vulnerable values from the
canonical `CardDefinition`. Historically those paths used zero when a required
value was absent. A damaged or incomplete definition could therefore execute as
a plausible no-op and pass through replay instead of exposing invalid content.

## Decision

An effect builder that implements a card using one of these values must require
the field explicitly and return `SimError::InvalidState` when it is absent.
Optional metadata remains optional only in generic inspection paths that do not
execute the corresponding mechanic. Existing static definitions and action/RNG
ordering are unchanged.

The card queue is constructed before authoritative combat mutation is committed,
so a missing required value fails atomically at the combat action boundary.

## Verification

A focused regression proves that missing required effect metadata is rejected
instead of becoming zero. Core tests, strict workspace Clippy, snapshot tests,
workspace tests, and repeated permanent-corpus replay protect existing card
behavior and deterministic ordering.
