# Fire Breathing + Hand Drill

## Evidence

FIDL00367 steps 1395–1396 (Deca/Donu, Fire Breathing 6, Hand Drill):

- Player ends turn with a single ethereal Dazed; Deca’s DEFEND intent is
  Square of Protection (16 block on **all** living enemies); Donu attacks.
- Next hand draws include two Dazed and a Wound. Each status/curse draw queues
  `FireBreathingPower` → `DamageAllEnemiesAction` (amount 6).
- Three FB hits strip 16 block then deal 2 HP to each boss; the hit that clears
  remaining block applies Hand Drill’s 2 Vulnerable (`just_applied`).

FIDL01393 steps 481–482 provide the multi-hit card-order checkpoint:

- The selected Gremlin Wizard is at 21 HP with 6 block and no Vulnerable before
  normal Pummel (four hits of 2 damage).
- The first three hits consume exactly the 6 block. Hand Drill’s trigger is an
  `ApplyPowerAction` added to the action queue by `DamageAction`, so it remains
  behind the remaining Pummel hit; the fourth hit is therefore 2 damage, not
  `floor(2 * 1.5) = 3`.
- The target checkpoint is 19 HP, 0 block, and Vulnerable 2. This requires
  queuing the generic Vulnerable action rather than mutating the power while a
  damage action is still resolving.

## Implementation

`apply_fire_breathing_damage` already used block-aware unmodified damage. It now
also records `broke_block` (`block_before > 0 && blocked == block_before`) and,
with Hand Drill, applies Vulnerable 2 via
`apply_monster_vulnerable_with_relics`, setting `vulnerable_just_applied` so the
debuff survives the current round’s decay.

Player damage paths emit the same `ApplyVulnerable` internal action as a queued
follow-up. This preserves target-game `addToBot` ordering for all multi-hit
cards while retaining the existing direct Fire Breathing pulse behavior.

## Non-goals

- Do not treat Fire Breathing as HP_LOSS that ignores block.
- Do not seed-specifically force Deca/Donu ordering.
