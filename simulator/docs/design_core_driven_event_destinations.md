# Core-driven event destination verification

## Problem

After applying an event choice, the verifier selected its destination projector
from the observed post-screen. In particular, an observed `combat_state`
decided whether the action was treated as entering combat, while observed screen
types selected map, reward, grid, or event comparison shapes. A divergent core
destination could therefore be compared through the observation's expected
branch instead of failing at the transition.

## Dispatch rule

The result of `apply_event_action` owns destination selection:

- `Combat` with combat state -> combat projection;
- an active card grid -> grid projection;
- `Reward` -> reward projection;
- `Event` -> event projection;
- `Idle` with no event -> map-return projection;
- `Complete` is handled only by the typed Spire Heart terminal path;
- every other combination fails closed as an invalid event destination.

The observed post-state is projected using the simulator-selected destination
shape. Its screen type remains part of that projection, so a wrong observed or
simulated destination produces a visible diff rather than changing branches.
