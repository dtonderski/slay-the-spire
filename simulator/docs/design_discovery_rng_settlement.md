# Played Discovery RNG settlement

## Scope

Match the target `DiscoveryAction` card-RNG consumption after the player picks a
generated card. This is limited to a card-played, all-card Discovery reward; it
does not change potion or typed Discovery settlement.

## Evidence

- The target 12-18-2022 `DiscoveryAction.update()` generates three unique card
  choices at the start of every update, including updates after the reward
  selection, until the fast action finishes.
- SuperFastMode patches `tickDuration` but preserves the repeated update calls.
- Permanent oracle `random-fidelity-1a50b5ada2264b05` isolates the post-pick
  burn: Havoc autoplays Discovery (choices Fire Breathing / Hemokinesis /
  Battle Trance), player picks Hemokinesis, end turn, then Infernal Blade must
  generate Blood for Blood. With open settlement fixed at 1 visible + 4 hidden
  generations, the pick path must advance `cardRandomRng` by **eight** draws
  (two three-card generations + two settle draws). Four hidden generations
  (twelve draws) over-burns by four and selects Rampage instead.
- Earlier session-1204 True Grit calibration that pinned twelve draws is
  superseded by this cleaner Infernal Blade oracle; residual True Grit RNG may
  have included non-Discovery card-random uses.

## Verification

Pin the eight-draw counter delta in the focused Discovery choice test
(fixture counter 16 → 24), then replay
`random-fidelity-1a50b5ada2264b05` through Infernal Blade (`category=none`).
