# Demon Form ticks after the start-of-turn draw

## Source behavior

`DemonFormPower.atStartOfTurnPostDraw` queues `ApplyPowerAction` Strength
equal to its stack. That is not `RitualPower`, which ticks `atEndOfTurn` for
the player and `atEndOfRound` for monsters.

Playing Demon Form on turn N therefore first adds Strength at the start of
turn N+1, after the hand is drawn. A Time Warp forced end still runs the next
turn's post-draw hook.

## Evidence

FIDL01694: Demon Form on turn 6, Time Warp closes turn 7, Feed on turn 8.
Real Strength is 5 (Vajra 1 + two post-draw ticks) plus Flex 4. Mapping Demon
Form onto end-of-turn `ritual` skipped the Time Warp end tick and dealt Feed
as 17 instead of 19 (`359 != 361`).

## Non-goals

- Do not change monster or potion `RitualPower` end-of-turn timing.
