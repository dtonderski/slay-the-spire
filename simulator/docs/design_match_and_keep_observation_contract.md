# Match and Keep observation contract

## Problem

Seed-start verification previously passed Match and Keep choice mismatches by
rewriting the observed choice array to the simulator projection. The rewrite
accepted missing slots, stale duplicate identities, and selected-card reveals
chosen from the observed frame. That made the comparison tautological and could
hide a divergent board while reporting the action as verified.

## Rule

The event command is bound from the pre-observation and applied to core state.
The post-observation and the core-owned event projection are then compared
without normalizing either side. Choice count, slot labels, revealed card
identities, deck effects, and event stage remain evidence.

Choice-list lag after a flip (or after a matched obtain settles) is modeled as
a typed deferred assertion: non-choice fields must already match the settled
core projection, observed choices must equal the pre-action choice list, and
the deferred action reconciles when a later frame matches the core projection
(or Leave after gameDone cleanup). Observed choice arrays are never rewritten
to the simulator projection.

Command binding for stage-2 card picks follows CommunicationMod's
`pickable.get(index)` semantics with two lag-aware rules:

1. Removal-stale pre (pre still lists a face-up card) → bind by visible index on
   the live sim board.
2. Resolution-stale pre (mid-pair / pre-name) → bind by the pre list's cardN
   label when it uniquely identifies a sim slot.

After the fifth attempt the target sets `gameDone` and a short `waitTimer`,
then transitions to CLEAN_UP/Leave. CommunicationMod only reports ready after
that wait, so the discrete post-state of the fifth attempt's second flip is
already Leave. The simulator mirrors that ready-state (not an intermediate
card-board hold).

When choice-list lag leaves the pre-observation on a card grid while core is
already on Leave, a CHOOSE index must not consume Leave → map; the verifier
treats that click as leave-ack lag until the next pre-list is Leave.

## Regression

The regression derives from the committed session-19 trace. It removes one
otherwise visible slot from the initial twelve-card board. Verification must
report an `event choice` difference, keep the action out of the verified
disposition, and produce the same final simulator state as the unmodified
trace.
