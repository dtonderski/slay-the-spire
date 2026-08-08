# Fire Breathing + Hand Drill

## Evidence

FIDL00367 steps 1395–1396 (Deca/Donu, Fire Breathing 6, Hand Drill):

- Player ends turn with a single ethereal Dazed; Deca’s DEFEND intent is
  Square of Protection (16 block on **all** living enemies); Donu attacks.
- Next hand draws include two Dazed and a Wound. Each status/curse draw queues
  `FireBreathingPower` → `DamageAllEnemiesAction` (amount 6).
- Three FB hits strip 16 block then deal 2 HP to each boss; the hit that clears
  remaining block applies Hand Drill’s 2 Vulnerable (`just_applied`).

## Implementation

`apply_fire_breathing_damage` already used block-aware unmodified damage. It now
also records `broke_block` (`block_before > 0 && blocked == block_before`) and,
with Hand Drill, applies Vulnerable 2 via
`apply_monster_vulnerable_with_relics`, setting `vulnerable_just_applied` so the
debuff survives the current round’s decay.

## Non-goals

- Do not treat Fire Breathing as HP_LOSS that ignores block.
- Do not seed-specifically force Deca/Donu ordering.
