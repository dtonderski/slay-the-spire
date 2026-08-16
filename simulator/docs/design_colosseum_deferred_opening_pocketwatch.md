# Colosseum deferred opening must not double-apply start-of-turn relics

## Decision

When CommunicationMod publishes Colosseum fight two before the opening
`DrawCardAction` queue settles, the following `END` applies
`apply_start_of_combat_relics` and then starts the first player turn
**without** calling `apply_start_of_player_turn_relics` a second time.

`apply_start_of_combat_relics` already runs that hook (Happy Flower /
Pocketwatch turn counter / Art of War). A second call made
`player_turns_started == 2` with `cards_played_last_turn == 0`, so
Pocketwatch drew three extra cards on the first turn of fight two
(FIDL01563: real 5+Bag=7, sim 10).

Java `Pocketwatch.atBattleStart` sets `firstTurn = true`;
`atTurnStartPostDraw` skips the bonus draw while that flag is set.
Fight two is a new battle, so the first published hand is still first-turn.

## Rejected alternative

Changing the Pocketwatch predicate from `player_turns_started > 1` to
`> 0` would fire on ordinary combat turn 1. The `> 1` gate is the
compensation for the increment that already happens inside
`apply_start_of_combat_relics` on eager combat entry.
