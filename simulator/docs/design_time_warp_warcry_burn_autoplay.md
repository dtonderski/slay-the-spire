# Time Warp Warcry CONFIRM can autoplay a selected Burn before discard

## Witness

FIDL01274 step 1929 `CONFIRM` is Warcry+ as Time Eater's 12th card.
`CHOOSE 1` hid Burn. Real leftover: energy 3 (Lantern not yet), HP
8101→8099, Burn in discard, hand still `Anger / Immolate / Sever Soul+ /
Headbutt+`, Time Warp 0 / Strength 8, Time Eater block 0. Later STATE
polls discard, then Haste + start-of-turn draw.

Ordinary CONFIRM puts Burn on draw and settles the full early end-turn
(energy 4, monster block 20). Deferred-end-turn with PutOnDeck leaves
Burn on draw and HP 8101. Skipped retrieval parks Burn in hidden limbo
so autoplay never sees it.

## Source

`PutOnDeckAction` can complete before CONFIRM without `hand.moveToDeck`.
The selected Burn stays in hand. `TimeWarpPower.onAfterUseCard` at 12
resets the counter, queues +2 Strength, and `callEndTurnEarlySequence`
then `EndTurnButton.disable(true)` queues `EndTurnAction`.
`callEndOfTurnActions` autoplays that Burn (`LoseHP` 2, then discard)
before `DiscardAtEndOfTurnAction`. CommunicationMod can publish that
boundary.

Warcry still exhausts under Corruption. Dark Embrace's `addToBot`
`DrawCardAction` is queued behind the leftover EndTurn and does not
resolve on this frame (FIDL01274 leftover start-of-turn hand is five
cards from the shuffled 19, not six).

## Decision

Add a seed-start candidate for put-on-deck CONFIRM against Time Eater
when the selected card is an end-turn autoplay status/curse. Skip
PutOnDeck, settle the source without bot-queued Dark Embrace / Feel No
Pain, run `resolve_end_of_turn_playing_cards_for_time_warp_lag`, and
leave `time_warp_end_turn` set so leftover STATE / PLAY finish discard
and the monster turn. Do not change ordinary Warcry retrieval.

Leftover Ripple applies Vulnerable then Weak. WeakPower.atEndOfRound
after that takeTurn skips a just-applied first stack. Only decrement
leftover Weak when Weak was already present before takeTurn (FIDL01782
stacking). FIDL01274 leftover Strike+ then deals 6, not 9.
