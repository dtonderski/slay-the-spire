# Observation-independent proceed destination dispatch

## Problem

The verifier dispatched `PROCEED` by inspecting the post-observation:
`VictoryRoom` meant Spire Heart, `CHEST` meant boss chest, and everything else
was treated as a map return. A wrong simulator destination could therefore be
routed into the matching observed projector instead of producing a mismatch.
The final-boss branch also recorded success without comparing visible state.

## Dispatch rule

Destination is selected from simulator pre-state:

- an Act 3 boss victory applies core `Proceed`, discards the inaccessible
  ordinary boss reward offers, advances one floor, and enters the typed Spire
  Heart event;
- a non-final boss `Reward` in a boss room applies core `SkipReward` and must
  produce `Treasure`;
- all other map-bound cases use the separately validated proceed-to-map binder.

Observed post-state never chooses the branch. The Spire Heart projection
compares screen type, event identity, floor, gold, and player HP. The boss-chest
projection compares the core-produced treasure state. Any inconsistent core
phase fails closed at the command.

The permanent complete CODEX10 trace covers the final-boss route; the permanent
boss-prefix traces cover combat reward to boss chest.
