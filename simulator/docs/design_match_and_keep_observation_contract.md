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

The current committed Match and Keep corpus replays without the old rewrite, so
there is no supported transient exception to preserve. If a future trace proves
that the target publishes a genuinely transient animation frame, that shape
must become a typed deferred assertion with a named visibility contract and a
required later reconciliation point. It must not restore observed-to-simulated
substitution or count the transient frame as complete parity.

## Regression

The regression derives from the committed session-19 trace. It removes one
otherwise visible slot from the initial twelve-card board. Verification must
report an `event choice` difference, keep the action out of the verified
disposition, and produce the same final simulator state as the unmodified
trace.
