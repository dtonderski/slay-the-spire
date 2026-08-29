# Time Warp leftover STATE after a rejected PLAY

## Source behavior

`TimeWarpPower.onAfterUseCard` at 12 calls `callEndTurnEarlySequence` and
queues `EndTurnAction`. CommunicationMod can still publish `play` on that
frame (FIDL01645 CONFIRM 1771, FIDL01691 CONFIRM 1348). The next PLAY is
rejected (`ready_for_command` is false; only `end`/`state` remain).

Java's leftover `EndTurnAction` then publishes in the same order as leftover
END: discard/autoplay first, then `AbstractCreature.loseBlock` on living
monsters, then `takeTurn` and the next hand.

FIDL01645 STATE 1773 still has Time Eater block 20. STATE 1774 is
loseBlock-only (block 0, player HP/energy unchanged). STATE 1775 is the
full monster turn plus next draw.

The rejected command is PLAY, so `pending_rejected_combat_play` is not
leftover `EndTurn`. `leftover_end_state_publication_candidate` must also
accept a Time Warp-armed combat (`time_warp_end_turn`,
`time_warp_end_turn_pre_discard_settled`, or
`time_warp_end_powers_applied`) and try loseBlock before the full leftover
monster+draw projection.

After the full leftover settlement, Time Warp end flags are cleared: the
queued `EndTurnAction` finished, and the next END is an ordinary next-turn
click.

FIDL01691 reaches the loseBlock STATE, then first-divs on the SuperFastMode
next-turn mid-draw (Head Slam damage / Draw Reduction). That is a separate
monster-turn family.

## Non-goals

- Do not hydrate monster block from the observation.
- Do not treat leftover STATE 547 (FIDL01597) here.
- Do not change Courier restock or Discovery `generateCardChoices`.
