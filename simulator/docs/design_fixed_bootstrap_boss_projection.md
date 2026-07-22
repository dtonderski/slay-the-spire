# Fixed Bootstrap Boss Projection

## Problem

The seed-start verifier predicts the Act 1 boss from the seed and unlock state,
but it only inserted that prediction into the simulated bootstrap projection
when the post-observation happened to contain `act_boss`. Removing the observed
field therefore also removed the simulated field and silently reduced strict
coverage.

This did not mutate `RunState`, but the simulated projection schema still
depended on observed post-state. A strict comparison must not let observation
choose which simulator facts exist.

## Decision

Bootstrap observed and simulated projections are separate fixed-shape helpers.
The simulated projection always derives `act_boss` from the typed start command,
seed, and boss-unlock metadata. The observed projection always includes the
CommunicationMod `act_boss` field, using JSON `null` when it is absent so that
malformed or incomplete observations produce a normal strict difference.

Other seed-start phases retain their existing visibility contracts; this slice
only changes the bootstrap boss field.

## Verification

The existing forged-boss regression continues to prove that observation cannot
steer the prediction. A new regression removes `act_boss` from a permanent trace
and requires a bootstrap difference. Formatting, strict workspace Clippy,
workspace tests, snapshot round trip, and repeated permanent-corpus replay remain
commit gates.
