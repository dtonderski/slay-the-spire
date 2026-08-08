# Pending-hidden settle vs ethereal-only hand (FIDL00278 / FIDL00257)

## Rule

`DiscardAction` leftover `HandCardSelectScreen.selectedCards` settle into discard
when END is clicked with a **non-empty visible hand**, even if ethereal exhaust
later empties that hand before bulk discard.

Empty-hand ENDs (hand already spent) still hold limbo cards outside every pile
so they do not contaminate the discard→draw shuffle (Burning Pact deferred).

## Bug

Settlement keyed off hand emptiness **after** ethereal exhaust. An only-Dazed
hand + Warcry put-on-deck skipped Inflame looked empty and kept Inflame out of
the shuffle bag (wrong draw order after refill).

## Fix

Capture `hand_nonempty_at_end_click` at the start of `end_player_turn`.
