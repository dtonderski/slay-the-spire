# Runic Cube + Burn END leftover shuffle (FIDL01762)

FIDL01762 step 1390 `END` first-divs after a matching 1389 frame
(Wound + two Burns in hand, one Defend on top of draw, 34-card discard,
Combust, Feel No Pain, Runic Cube, one living Orb Walker).

## Observed leftover

Real after END: hand 6, draw 27, discard **5**
(`Burn, Bash, Strike_R, Defend_R, Wound`).

Sim: hand 6, draw 26, discard **6**. Same 38-card multiset. The extra
discarded card and the EmptyDeckShuffle permutation are both wrong.

Comparison is `real != sim`.

## Queue

`RunicCube.wasHPLost` queues `DrawCardAction` with `addToTop` per
HP-loss packet. Burn/Decay/Regret are `CardQueueItem`s from
`callEndOfTurnActions` and resolve before `AbstractRoom.endTurn`
queues Combust `LoseHPAction` + `DamageAllEnemiesAction` and
`DiscardAtEndOfTurnAction`.

On FIDL01762 step 1390 the lone Defend is drawn by the first Burn.
The second Burn shuffles the 34-card discard plus Burn 1 (already
settled by `UseCardAction`). Combust then draws one card into the
remaining hand, and that card is discarded with Wound / Defend /
the second Burn's Cube card. Leftover discard is 5.

A single Burn also resolves before Combust when that Cube empties a
one-card draw pile (FIDL01641 leftover `Strike, Anger, Bloodletting,
Parasite, Strike` — the played Burn is shuffled). Exception: Dark
Embrace plus an ethereal on top of draw keeps Combust first so the
ethereal is pulled before Burn shuffles (FIDL01665 leftover-empty
discard + Dazed exhaust).
`callEndOfTurnActions` queues only the autoplays that were in hand at
END click. A Burn Combust Cube then draws is discarded without playing
(FIDL01762 step 1393: real HP 9381 = Combust 1 + queued Burn 2 + laser
22, not a second Burn).

Global Combust `DeferDraws` regresses FIDL01762 step 1157 (Chosen/Byrd,
no Burns). Combust Cube stays immediate; Evolve/Fire Breathing stay
queued behind discard (FIDL01335 / FIDL01565).

Time Eater Head Slam `ApplyPowerAction(DrawReductionPower)` is a
DEBUFF. Artifact consumes it (FIDL01762 step 1846), so the following
start-of-turn `DrawCardAction(gameHandSize)` is still five cards plus
the Runic Cube draw from Head Slam's unblocked hit.
