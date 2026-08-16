# Colosseum leftover opening EndTurn does not expire Flex

## Source behavior

Colosseum fight two can publish its first ready hand from a leftover
`EndTurnAction` after `DrawCardAction` and before that action finishes.
`MayhemPower`-style wrappers aside, `GameActionManager.callEndOfTurnActions`
(including `LoseStrengthPower.atEndOfTurn`) has already run — or never
had this combat's Flex to expire.

The next PLAY is a real `UseCardAction`. Flex applies Strength +
LoseStrength. The leftover EndTurn then discards the rest of the hand,
runs monster `takeTurn`, and draws the next hand. It does **not** invoke
`LoseStrengthPower` again. Flex remains until the following explicit END.

`start_player_turn` still zeros `temp_strength` on ordinary leftover ends
(Nilry two-step, FIDL01597). Only the fight-two opening leftover sets
`preserve_temp_strength_on_next_start` so Flex applied on that ready frame
survives the following refill. Java only removes Flex from
`LoseStrengthPower.atEndOfTurn`, which that leftover action already ran.

## Evidence

FIDL01576: Colosseum Taskmaster + Nob. Opening END 587, PLAY Flex, leftover
end publishes a new five-card hand still showing Strength 2 / Strength Down.
Strike on Nob is 6+2=8 (83→75). A full second `end_player_turn` expired Flex
and dealt 6 (83→77).

## Non-goals

- Do not skip Flex expiry on an ordinary player END click.
- Do not change rejected-PLAY leftover discard (`opening_end_turn_pending`
  drain without playing the card).
