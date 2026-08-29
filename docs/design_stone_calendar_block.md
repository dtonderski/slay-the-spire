# Stone Calendar lethal prediction respects block (FIDL00415)

## Observation

Turn-7 Stone Calendar deals **52 unmodified** damage (block first). With Spheric
Guardian at **20 HP + 60 block**, Calendar leaves **20 HP + 8 block** and combat
continues. Simulator predicted finish via `monster.hp <= 52` only, early-exited
end-turn (skipped self-loss, applied Burning Blood, opened rewards) while the
trace stayed in combat (FIDL00415).

## Rule

`stone_calendar_would_finish` requires every living monster’s **HP + block**
(non-negative) to be `<= STONE_CALENDAR_DAMAGE`, matching
`deal_unmodified_damage_to_monster` block-then-HP application.

## Non-goals

Does not change Calendar damage amount, turn index, or ordering relative to
Combust/Constricted when the hit is actually lethal through block.
