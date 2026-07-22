# Event Action Dispatch Split

## Problem

`apply_event_action` validates one typed choice and then handles every event in a
single match.  The match is large enough that unrelated event mechanics share a
2,000-line edit surface, but its tail also defines important cross-event
behavior: choice zero leaves an otherwise unhandled screen, while every other
unhandled choice is illegal.

A family split must not turn an unknown stage into success, validate twice,
clone state more than once, or change when a partially mutated clone is
discarded after an error.

## Decision

The public dispatcher retains ownership of:

- `validate_event_action`;
- the single `RunState` clone;
- extraction of the validated `EventScreen` and choice index;
- the shared choice-zero Leave fallback;
- the final illegal-action error; and
- returning the completed clone.

Each family handler receives `&mut RunState`, `&EventScreen`, and the choice
index, and returns `SimResult<bool>`. `true` means one of that family's exact
event/stage arms ran. `false` means no arm matched and the dispatcher must
continue to its existing shared fallback. A handler must not interpret an
unmatched stage as success. Event ownership is derived from the canonical
`ACT1_EVENTS`, `ACT2_EVENTS`, `ACT3_EVENTS`, shrine, and special-event lists.
Families must be disjoint.

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
