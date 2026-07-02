# CommunicationMod Combat Pile Order

This note defines the representation boundary between real-game
CommunicationMod observations and the authoritative simulator.

## Contracts

- `sts_core` stores combat piles in source-shaped order. Core mechanics must not
  branch on CommunicationMod, bridge, trace, or UI ordering.
- CommunicationMod observations are an external projection. Import code must
  convert observed arrays into source-shaped `CardPiles` before simulation.
- Verification must compare simulator state through a CommunicationMod-visible
  projection instead of comparing raw internal pile vectors to observed arrays.
- Bridge commands such as `PLAY n` use CommunicationMod-visible hand order.
  Command mapping must translate the visible slot to the source-shaped hand card
  id before building a `CombatAction`.
- Draw, discard, exhaust, and hand arrays must use named conversion helpers.
  Direct assignment from observed arrays to `CardPiles` is not allowed outside
  those helpers.

## Debugging Rule

Pile diffs should be classified by boundary:

- `import_mismatch`: observed arrays do not import to the intended internal
  source-shaped state.
- `projection_mismatch`: an internal source-shaped state projects to the wrong
  CommunicationMod-visible order.
- `mechanics_mismatch`: source-shaped state changed incorrectly after applying a
  simulator transition.

Do not fix pile-order traces with seed-specific branches or trace-specific
observed-state overrides. Fix the generic import, projection, command mapping,
or source mechanic that caused the mismatch.
