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

For the post-boss `NONE` frame in `TreasureRoomBoss`, the core transition and
next-act bookkeeping occur immediately, while a `PendingMapAssertion` retains
the originating `PROCEED`. Once that core transition proves the expected
destination, `NONE` classifies the post-state as a candidate transient frame;
the transient visibility contract then independently compares its screen name
and boss-room identity. Act, floor, map nodes, deck, relics, gold, and HP remain
deferred until the unchanged simulator destination is compared with a stable
`MAP` frame. Observation polls may extend the transient interval. A stable map
in either a poll post-state or the next command pre-state reconciles the
original action. Any other semantic command fails closed, and ending the trace
first reports an unresolved transient assertion.

The boss FTUE overlay is an explicit exception to command completion, not a
simulator transition: when the post-frame proves that overlay intercepted the
command, the verifier records the deferred UI step and leaves simulator state
unchanged for the subsequent dismissal command.

Boss combat-to-chest dispatch, final-victory dispatch, and other distinct
`PROCEED` families remain separate follow-up bindings; this helper only owns
transitions whose simulator destination is the map, plus the explicit FTUE
interception that prevents such a transition from occurring.

The regression fixture derives from the committed two-act live trace. It keeps
the act-three map prediction intact but inserts a target-shaped
`NONE/TreasureRoomBoss` frame and `STATE` poll before it. The transient-only
prefix must remain unresolved, while the stable version must reconcile the
original boss-chest `PROCEED` without diffs or unsupported transitions. A
forged transient screen name remains an unexpected diff even when the later map
matches.
