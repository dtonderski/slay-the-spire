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

## Evidence and regression

The retained Transmogrifier regression exposes `confirm_up: true` after
`CHOOSE` and records a separate `CONFIRM` before returning to the event.
Focused binding coverage pins selection followed by explicit confirmation, and
the strict permanent and fidelity corpora protect the surrounding grid
families. A trace that omits the semantic confirmation cannot use its observed
resolved screen to manufacture that transition.
