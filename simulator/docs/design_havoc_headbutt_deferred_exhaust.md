# Havoc Headbutt deferred exhaust

## Source behavior

`Havoc` force-plays the top card with `exhaustOnUseOnce`. When that card is
Headbutt and discard has more than one card, `PutOnDeckAction` opens a discard
grid and pauses. `UseCardAction` has not settled yet, so Headbutt is still in
`cardInUse` / limbo: CommunicationMod's discard still contains the cards that
were there when Headbutt played, and Dark Embrace has not drawn.

On `CHOOSE`, Headbutt puts the selected discard card on top of draw, then
force-exhausts. Dark Embrace, Feel No Pain, Charon's Ashes, and Dead Branch
resolve after that exhaust.

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
