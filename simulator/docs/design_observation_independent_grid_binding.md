# Observation-independent grid command binding

## Problem

The seed-start verifier previously applied a grid `CHOOSE` command, inspected
the post-observation's screen type, and then confirmed the simulated grid when
the observed screen had closed. That made the observed result part of the
authoritative simulated transition and could turn a wrong predicted grid
lifetime into a passing replay.

## Binding rule

Grid command binding uses only the simulator pre-state and the typed command:

- `CHOOSE <index>` selects that simulator grid entry.
- Multi-select purposes keep the selected indices and required count in core
  state, but selection does not resolve the grid.
- `CONFIRM` validates the required selection count and resolves the modeled
  grid purpose.
- `CONFIRM` and `CANCEL` remain explicit commands.

The post-observation is projected and compared only after the simulated
transition is complete. It never chooses whether confirmation occurs.

## Destination rule

After the typed grid command executes, the verifier selects the next projector
from authoritative core state only. An open card grid remains a grid; otherwise
the core phase and matching shop, event, or reward ownership determine the
destination. Rest, treasure, and the command-facing `NONE` frame before
`PROCEED` are also explicit destinations. Inconsistent combinations fail at
an `invalid_grid_destination` boundary instead of falling through to a
plausible screen.

Observed screen type is comparison input only. It cannot select the shop,
event, rest, reward, treasure, grid, or proceed projector, and it cannot choose
the verifier's next semantic phase.

Event obtain/transform grids can publish their deck append one stable frame
late. Their action-frame projection retains that documented visibility delay,
but now creates a deferred deck assertion. The originating action is verified
only when a later stable observation contains the simulator-owned final deck;
a divergent or missing reconciliation remains a failure.

## Evidence and regression

The retained Transmogrifier regression exposes `confirm_up: true` after
`CHOOSE` and records a separate `CONFIRM` before returning to the event.
Focused binding coverage pins selection followed by explicit confirmation, and
the strict permanent and fidelity corpora protect the surrounding grid
families. A trace that omits the semantic confirmation cannot use its observed
resolved screen to manufacture that transition.

A forged-trace regression also changes a real shop-grid post-screen to `EVENT`.
The verifier must retain the core-owned `shop grid` projection and report the
screen mismatch; the forged observation cannot reroute comparison through the
event projector.
