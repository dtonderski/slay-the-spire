# Havoc Hex Dazed: post-remove, pre-forced-card-draw

## Evidence (FIDL00381 / FIDL00410)

Havoc under Hex force-plays a drawing attack (Pommel+). CommunicationMod final
draw order matches Hex `addToRandomSpot` against the pile **after** PlayTop
removes the forced card and **before** that card's draws resolve.

- Post-draw insert (size after Pommel draws) desyncs FIDL00381 at step 302.
- Mid-PlayTop insert (size after remove, before draws) yields `complete_pass`
  on FIDL00410 and advances FIDL00381 past the Havoc/Pommel boundary.

## STS ordering

`Havoc.use` `addToBot(PlayTopCardAction(getRandomMonster(...)))`, then
`HexPower.onUseCard` `addToBot(MakeTempCardInDrawPileAction)`.
`PlayTopCardAction` removes the top card then `addToTop`s the forced play.
The bot Hex action can therefore observe the post-remove draw pile while the
forced card's draw effects have not yet run.

## Implementation

In `process_internal_queue`, when executing `PlayTopDrawCard`, drain trailing
Hex Dazed `AddGeneratedCardToDrawPileRandomSpot*` actions and apply them after
`apply_play_top_draw_card` removes the top card but before the nested forced-
card queue runs (`apply_play_top_with_mid_hex`).

## Non-goals

- Do not move ordinary (non-PlayTop) Hex inserts relative to card.use draws.
- Do not change Armaments/Burning Pact/Discovery deferred-Hex parking.
