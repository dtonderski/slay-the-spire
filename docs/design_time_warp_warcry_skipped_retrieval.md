# Time Warp Warcry CONFIRM can skip retrieval and lag the forced end-turn

## Witness

FIDL01365 step 2392 `CONFIRM` is Warcry as Time Eater's 12th card
(Corruption exhausts Warcry). Real leftover: HP 9828, hand still
`Body Slam / Armaments / Cleave`, Clothesline absent from every pile,
Time Warp 0, Time Eater Strength 4. Later STATE polls discard, then
Head Slam (HP 9795) and the next hand.

Ordinary CONFIRM retrieves Clothesline onto draw and settles
`callEndTurnEarlySequence` in the same step (HP 9795, discarded hand).
Skipped-retrieval-with-settle still runs the monster turn. Retrieved
Warcry without settle leaves Clothesline on draw (`draw` 12 vs 11).

## Source

`PutOnDeckAction` can `tickDuration` complete before CONFIRM, so
`hand.moveToDeck` never runs. `TimeWarpPower.onAfterUseCard` at 12 still
resets the counter, applies +2 Strength, and queues `EndTurnAction`.
CommunicationMod can publish that frame before the queued end-turn
drains the hand.

## Decision

Add a seed-start candidate that combines skipped put-on-deck retrieval
with `settle_time_warp = false`. The selected card is parked in
`pending_hidden_hand_card_until_end_turn`. `time_warp_end_turn` stays
set so leftover STATE / rejected PLAY resume the queued end-turn.
Do not change ordinary Warcry retrieval or non-Time-Eater skipped
retrieval.
