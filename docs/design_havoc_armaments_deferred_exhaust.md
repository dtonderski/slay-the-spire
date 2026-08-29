# Havoc Armaments deferred exhaust

## Source behavior

`Havoc` / `Mayhem` / `Distilled Chaos` force-play the top card with
`exhaustOnUseOnce`. When that card is unupgraded Armaments and more than one
hand card can be upgraded, `ArmamentsAction` opens `HandCardSelectScreen` and
pauses. `UseCardAction` has not settled yet, so Armaments is still in
`cardInUse` / limbo: CommunicationMod's combat hand is only the upgrade
candidates, exhaust does not yet contain Armaments, and Charon's Ashes has
not fired.

On `CONFIRM`, `ArmamentsAction` returns the upgraded card (and any cards it
removed from the select projection) first. `UseCardAction` then force-exhausts
Armaments. Charon's Ashes, Feel No Pain, Dark Embrace, and Dead Branch resolve
after that rebuilt hand (FIDL01334: Dark Embrace draws Berserk after Rampage+ /
Armaments+ / Havoc+).

Witness: FIDL01254 steps 1070–1072. Maw stays at 246 HP while the select is
open; it drops to 243 only after CONFIRM, when Armaments first appears in
exhaust.

## Simulator contract

Force-played Armaments that opens a hand select defers its source `MoveCard`
until CONFIRM, the same family as Dual Wield / True Grit+ / Exhume. Ordinary
hand-played Armaments still settles through the delayed-source path.
Skipped retrieval still parks the selected card and still exhausts the
force-played source so relic/power hooks fire once.
