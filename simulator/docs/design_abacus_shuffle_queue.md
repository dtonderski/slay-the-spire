# The Abacus shuffle is an addToBot GainBlockAction

## Source behavior

`TheAbacus.onShuffle` queues `GainBlockAction(6)` with `addToBot`. The discard
pile is already shuffled into the draw pile when that action is enqueued.
Any later player-selection action already on the queue — notably Warcry's
`PutOnDeckAction` / `HandCardSelectScreen` — therefore opens before Abacus
block is applied.

Sundial's counter update is synchronous in `onShuffle`. The energy grant is
an `addToBot GainEnergyAction`, so it uses the same post-select settlement
window as Abacus.

## Evidence

- FIDL01525 step 171: Warcry draws from an empty pile, shuffles, and opens
  HAND_SELECT with block still 0. CONFIRM at step 173 then grants the 6
  Abacus block.
- Existing overflow coverage still fails closed at the later GainBlock
  settlement, not at shuffle time.
- FIDL01624 step 1010: Warcry's 12th Sundial shuffle opens HAND_SELECT with
  energy still 3. CONFIRM at step 1012 then grants the 2 energy.

## Non-goals

- Do not invent a seed-specific Warcry branch.
