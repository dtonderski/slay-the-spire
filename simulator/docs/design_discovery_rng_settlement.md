> **Historical note — superseded.** The multi-generation and cross-command
> Discovery settlement model below is retained as evidence of an abandoned
> hypothesis. See [design_discovery_action_update_lifecycle.md](design_discovery_action_update_lifecycle.md)
> for the current source-backed lifecycle.

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

## Hand-played staged settlement

The active FIDL00226 source sequence provides a second lifecycle oracle. After
hand-played Discovery's seven immediate post-pick generations, the source
emits Magnetism cards in this order: Dramatic Entrance, Transmutation, Blind,
Dark Shackles, Enlightenment, and Mayhem. The intervening `cardRandomRng`
stream requires the selected action's remaining invisible updates to settle
across END boundaries: 26 unique-choice generations plus one draw, then 11
generations plus two draws, followed by one, two, and one final settlement
draws. These are modeled as pending action lifecycle stages, not seed- or
observation-specific rebinding. The same trace then plays Transmutation and
source-generates Deep Breath, covering the next colorless RNG call.

## Verification

Pin the eight-draw counter delta in the focused Discovery choice test
(fixture counter 16 → 24), replay `random-fidelity-1a50b5ada2264b05` through
Infernal Blade (`category=none`), and replay the FIDL00226 active witness
through EOF.
