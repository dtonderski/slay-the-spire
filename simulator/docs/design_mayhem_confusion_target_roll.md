# Mayhem target roll before start-of-turn draw

## Source behavior

`MayhemPower.atStartOfTurn` (not post-draw) queues one anonymous action per
stack *before* `GameActionManager` queues `DrawCardAction`. That wrapper's
`update()` calls `MonsterGroup.getRandomMonster(null, true, cardRandomRng)`
and then `addToBot(new PlayTopCardAction(monster, false))`.

`getRandomMonster` always consumes `Random.random(0, size-1)`, including a
single living monster (`random(0, 0)`). Confusion's `onCardDraw` rolls
therefore start after those Mayhem target rolls.

PlayTop still *executes* after the base hand draw and after Brutality's
post-draw extra draw (`addToBot` places it behind actions already queued).

## Evidence

FIDL01474 step 588: Snecko Confusion + Mayhem, play Battle Trance+ from a
freshly drawn hand. Real cost 0 (energy stays 4). Simulator rolled the
Mayhem target after the five hand draws, so Battle Trance+ received the
next `random(3)` (cost 1, energy 4 != 3). Shifting the target roll before
the refill restores costs `3,3,1,1,0`.

FIDL00381 remains: Mayhem still plays the post-Brutality top card.

## Non-goals

- Do not move PlayTop execution before the base five-card refill.
- Do not skip the single-monster `random(0, 0)` roll.
