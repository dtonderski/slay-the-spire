# Leftover EndTurn after a rejected PLAY (Colosseum / Nilry)

## Decision

When CommunicationMod rejects a PLAY, drain any leftover `EndTurnAction`
the same way Time Warp already does: discard the published hand and mark
`time_warp_end_turn_pre_discard_settled` so a later STATE/END can finish
the monster turn and next draw.

This covers:

- Colosseum fight-two opening `END` (`opening_end_turn_pending`)
- Nilry's Codex SKIP (`resume_end_turn_after_nilrys_codex`)

FIDL01505 / FIDL01648 / FIDL01762 / FIDL01794 / FIDL01772 / FIDL01727
all send PLAY after that ready publication; the game errors and STATE
polls show `EndTurnAction` with the hand already discarded.

Nilry leftover also exhausts ethereal cards. Feel No Pain GainBlock can
publish one STATE later (or one tick at a time); the first leftover STATE
may still show 0 block.

## Source

`GameActionManager` keeps the END-queued `EndTurnAction` after SuperFastMode
flushes the opening draw (or after Codex closes). CommunicationMod reports
`ready_for_command` on the drawn hand, then rejects the next PLAY while
that action is still in the queue.

After leftover `takeTurn`, Java still runs `MonsterGroup.applyEndOfTurnPowers`.
`WeakPower.atEndOfRound` lives there (not in leftover hand discard). Stacking
onto existing Weak does not set `justApplied`, so leftover cleanup decrements
once. FIDL01782: Weak 1 + Writhing Mass ATTACK_DEBUFF 2 → Weak 2 at the next
ready frame; a skipped leftover Weak tick left Strike dealing 4 instead of 6.

A rejected PLAY after Nilry SKIP arms leftover EndTurn so later STATE polls
can finish monsters and the next-hand `DrawCardAction`. SuperFastMode may
publish after the first drawn card (FIDL01597 STATE 547 Clash, 548 the rest).
