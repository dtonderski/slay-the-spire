# Exhaust CONFIRM on the Act 3 boss skips combat rewards

## Source behavior

At Ascension 0, killing the Act 3 boss ends the run. The game exposes
`COMPLETE` (`VictoryRoom`) and does not open ordinary `CombatRewardScreen`.
`PROCEED` then enters the Spire Heart event.

PlayCard already routes that win through `enter_final_boss_victory`. Exhaust
select CONFIRM (True Grit under Havoc, Burning Pact, Recycle) used a separate
`settle_run_after_select_confirm` that always called
`enter_combat_reward_for_current_room`, which mapped every boss room to
`enter_boss_combat_reward_screen`.

FIDL01379: Havoc PlayTops True Grit+, exhausts Void. Charon's Ashes kills
Awakened One (5 HP). Real CONFIRM 1131 is `COMPLETE`; sim opened
`COMBAT_REWARD`.

## Decision

`enter_combat_reward_for_current_room` uses the same Act 3 boss rule as
PlayCard: `enter_final_boss_victory`, no reward RNG.

## Non-goals

- Do not skip rewards for Act 1/2 bosses or Act 3 non-boss rooms.
- Do not change the four-choice Spire Heart sequence after PROCEED.
