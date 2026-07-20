# Observation-independent proceed-to-map binding

## Problem

The seed-start verifier's shared proceed-to-map helper projected the observed
post-state and then, only when that observation reported `MAP`, cleared the
simulator's shop, reward, grid, and phase fields. Its transient boss branch also
patched treasure state before attempting the core transition. Those writes let
the observed result decide authoritative simulator state.

## Binding rule

The `PROCEED` command advances from simulator pre-state before the post-state is
examined:

- a non-event `Reward` applies core `SkipReward`, explicitly abandoning any
  remaining offers;
- `Treasure` applies core `Proceed`;
- a completed stage-2 Neow `Event` applies its core leave choice;
- `Idle` is an already-completed core transition with a remaining UI proceed;
- every other simulator phase fails closed.

The resulting simulator state must be `Idle` before it can be projected as a
map. Post-observation may identify a known transient frame and defer the stable
map comparison, but it cannot repair phase, reward, grid, or shop state.

The boss FTUE overlay is an explicit exception to command completion, not a
simulator transition: when the post-frame proves that overlay intercepted the
command, the verifier records the deferred UI step and leaves simulator state
unchanged for the subsequent dismissal command.

Boss combat-to-chest dispatch, final-victory dispatch, and other distinct
`PROCEED` families remain separate follow-up bindings; this helper only owns
transitions whose simulator destination is the map, plus the explicit FTUE
interception that prevents such a transition from occurring.
