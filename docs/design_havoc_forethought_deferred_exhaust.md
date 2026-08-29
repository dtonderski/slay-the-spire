# Havoc Forethought deferred exhaust

## Source behavior

`Havoc` / `Mayhem` / `Distilled Chaos` force-play the top card with
`exhaustOnUseOnce`. When that card is base Forethought (or Forethought+) and
another hand card exists, `PutOnDeckAction` opens `HandCardSelectScreen` and
pauses. `UseCardAction` has not settled, so Forethought is still in
`cardInUse` / limbo: CommunicationMod's combat piles do not contain it.

On `CONFIRM`, the selected card moves to the bottom of the draw pile and
`UseCardAction` force-exhausts Forethought.

Witnesses: FIDL01437 steps 951–953 (Havoc → Forethought, choose Bloodletting);
FIDL01593 steps 930–932 (Havoc → Forethought, choose Havoc). Exhausting the
source when the select opened dropped the instance and CONFIRM failed with
`unknown card: card:36` / `card:73`.

## Simulator contract

Force-played Forethought / Warcry / Thinking Ahead that opens a hand select
defers its source `MoveCard` until CONFIRM, the same family as force-played
Armaments / Dual Wield. Ordinary hand-played cards still settle through the
delayed-source path. On CONFIRM, `play_top_force_exhaust_active` force-exhausts
Forethought after the selected card is placed; Warcry / Thinking Ahead already
exhaust via their delayed-source destination.

## Non-goals

- Do not change skipped PutOnDeck retrieval.
- Do not change Burning Pact's existing early force-exhaust at screen open.
