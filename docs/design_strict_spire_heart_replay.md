# Strict Spire Heart replay

## Problem

The seed-start verifier treated every action whose pre-observation reported
`VictoryRoom` as verified without binding the command, advancing core state, or
comparing the post-observation. A complete trace could therefore pass even if
the Heart choices or terminal transition diverged.

## State model

Core represents the terminal sequence explicitly:

- `RunPhase::Event` with `Event::SpireHeart` stages 0 through 3 exposes the
  `Continue`, `Attack`, `Continue`, and `Sleep` decisions;
- choosing `Sleep` enters `RunPhase::Complete` with no legal gameplay action;
- the CommunicationMod `PROCEED` after `GAME_OVER` is presentation-only. The
  verifier checks that the observed session leaves the game while core remains
  complete.

The verifier binds every Heart `CHOOSE` through `apply_event_action` and derives
the simulated event or game-over projection solely from the resulting core
state. No `VictoryRoom` observation selects or bypasses a transition.

## Regression evidence

The external complete CODEX10 trace contains the full four-choice Heart
sequence and terminal `PROCEED`. Its strict outcome must remain a complete pass
with one disposition per action.
