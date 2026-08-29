# Havoc Headbutt deferred exhaust

## Source behavior

`Havoc` force-plays the top card with `exhaustOnUseOnce`. When that card is
Headbutt and discard has more than one card, `PutOnDeckAction` opens a discard
grid and pauses. `UseCardAction` has not settled yet, so Headbutt is still in
`cardInUse` / limbo: CommunicationMod's discard still contains the cards that
were there when Headbutt played, and Dark Embrace has not drawn.

On `CHOOSE`, Headbutt puts the selected discard card on top of draw, then
force-exhausts. Relic `onExhaust` (Dead Branch into hand) runs before power
`onExhaust` (Dark Embrace draw), matching Purity / Secret Technique. Feel No
Pain and Charon's Ashes also resolve on that exhaust. UseCardAction is paused
while the discard grid is open, so `CardExhausted` never fires for this source.

Witness: FIDL01306 step 1065. Havoc force-plays Headbutt while discard is
`Thunderclap, Havoc`. The GRID frame still shows that discard and an empty
draw pile. After `CHOOSE 1` Headbutt appears in exhaust, Dark Embrace draws
Havoc, and Thunderclap is on top of draw.

## Simulator contract

Force-played Headbutt that opens a discard select defers its source `MoveCard`
until the select closes, the same family as Dual Wield / True Grit+ / Exhume /
Armaments. The force-exhaust marker is parked on the discard-select so a later
hand play cannot inherit `exhaustOnUseOnce`. Ordinary hand-played Headbutt still
discards on confirm. Singleton or empty discard auto-complete still
force-exhausts immediately when there is no player select.
