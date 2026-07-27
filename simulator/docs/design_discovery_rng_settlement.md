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
- CommunicationMod session-1204 uniquely requires two more `cardRandomRng`
  draws than the previous ten-draw model: the earlier True Grit and Bronze Orb
  Stasis outcomes remain unchanged only at a +2 offset, and the later True Grit
  then selects the observed card.
- Four hidden three-card generations consume twelve draws for this captured
  no-duplicate sequence. No separate synthetic "settle" draw is needed.

## Verification

Pin the twelve-draw counter delta in the focused Discovery choice test, then
replay session-1204 through the later True Grit transition before resuming the
live run.
