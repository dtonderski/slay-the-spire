# Havoc PlayTop Malleable vs Letter Opener

## Evidence

FIDL00428 step 1282 (Writhing Mass Malleable 3, Letter Opener, Corruption Havoc → Bash):

- Bash deals 8 (46 → 38 HP), then Letter Opener’s third-skill blast deals 5
  (38 → 33 HP) with **no** Malleable block yet.
- Malleable’s `addToBot(GainBlock)` then grants 3 block (amount → 4). Final
  observation: HP 33, block 3.

If Malleable resolved inside the nested PlayTop before parent Letter Opener,
LO would chew the 3 block and leave HP 36 block 0 (previous sim failure).

## Implementation

While `play_top_force_exhaust_active` is set, `push_attack_block_follow_ups`
parks Curl Up / Malleable amounts on
`CombatState.deferred_play_top_monster_blocks`. After the nested PlayTop queue
finishes, those become `GainMonsterBlock` follow-ups returned to the parent
queue (behind Letter Opener / Hex bot actions from the outer skill).

## Non-goals

- Do not change ordinary (non-PlayTop) Malleable ordering vs Sadistic Nature.
