# Event Action Dispatch Split

## Problem

`apply_event_action` validates one typed choice and then handles every event in a
single match. The match is large enough that unrelated event mechanics share a
2,000-line edit surface. Its historical tail also treated choice zero on an
otherwise unhandled screen as a successful leave. That fallback is unsafe once
the family split is complete: a newly added or accidentally misrouted event can
silently become a plausible successful transition.

A family split must not turn an unknown stage into success, validate twice,
clone state more than once, or change when a partially mutated clone is
discarded after an error.

## Decision

The public dispatcher retains ownership of:

- `validate_event_action`;
- the single `RunState` clone;
- extraction of the validated `EventScreen` and choice index;
- exhaustive event-to-family routing;
- the final fail-closed dispatch invariant; and
- returning the completed clone.

Each family handler receives `&mut RunState`, `&EventScreen`, and the choice
index, and returns `SimResult<bool>`. `true` means one of that family's exact
event/stage arms ran. `false` is now an internal dispatch error: validated input
must never fall through to a generic leave transition. Event ownership is
encoded in an exhaustive `Event` match, so adding an event requires assigning a
family at compile time. Families must be disjoint.

The migration is mechanical and incremental. Each slice moves complete existing
match arms without changing their bodies, adds only module-depth path fixes,
and proves that reconstructing the original arm order yields the prior source.
Behavior changes require a separate source-backed slice and regression.

## Verification

Every family extraction runs focused core tests, strict workspace Clippy,
snapshot round trips, the full workspace suite, and two permanent-corpus
replays. Existing event tests remain the semantic regression surface; the
mechanical reconstruction audit proves the extraction itself did not rewrite
event mechanics.
