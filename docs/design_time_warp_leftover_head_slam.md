# Time Warp leftover Head Slam is one takeTurn

## Source behavior

After Time Warp's 12th card, leftover `EndTurnAction` runs one monster
`takeTurn`, then the next `DrawCardAction`. FIDL01691 STATE 1351 is
loseBlock. STATE 1352 is SuperFastMode after Head Slam, empty-deck
shuffle, and the first two draws (`Wound`, `Bash`). STATE 1353 finishes
the Draw Reduction hand (`Burning Pact`, `Wild Strike`).

`TimeEater.takeTurn` for Head Slam is one `DamageAction` plus
`ApplyPowerAction(DrawReductionPower)`. Odd Mushroom makes the 26+4 hit
37. The next intent is Reverberate; it must not execute on this leftover.

## Bug

Leftover `settle_leftover_end_turn_monster_and_draw` left
`time_warp_end_turn` set. `start_player_turn` then called
`settle_time_warp_end_turn_if_ready`, which ran a second `end_player_turn`.
The second takeTurn was Reverberate (3x11). Combined HP 6059→5989, no
Draw Reduction (`!time_warp_end_turn` skipped the DEBUFF), five-card hand.

## Decision

Clear Time Warp end flags at the start of leftover monster+draw so Head
Slam applies Draw Reduction and the next start-of-turn does not force
another monster turn.

SuperFastMode can still publish after `EmptyDeckShuffleAction` and before
the remaining draws. The leftover STATE candidate peels completed
start-of-turn draws back onto the draw-pile top and continues that
`DrawCardAction` on the next leftover STATE.

## Non-goals

- Do not change ordinary (non-leftover) Head Slam Artifact handling.
- Do not hydrate HP or hand from the observation.
