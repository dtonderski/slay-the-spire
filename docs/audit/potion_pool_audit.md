# Ironclad Potion Pool Audit

Date: 2026-07-01

Scope: compare the local Ironclad potion definitions against Slay the Spire
desktop `1.0` bytecode from
`D:\SteamLibrary\steamapps\common\SlayTheSpire\desktop-1.0.jar`.

Evidence used:

- Local pool and metadata: `simulator/crates/sts_core/src/potion/mod.rs`
- Local potion use logic: `simulator/crates/sts_core/src/run/potion.rs`
- Local random potion helper: `simulator/crates/sts_core/src/run/reward.rs`
- Jar bytecode inspected with `javap`: `PotionHelper`, `GamblersBrew`,
  `BloodPotion`, `DuplicationPotion`, `DuplicationPower`, `LiquidMemories`,
  `BetterDiscardPileToHandAction`, `EntropicBrew`,
  `AbstractDungeon.returnRandomPotion`, and `AbstractPotion.getPotency`

## Summary

`IRONCLAD_POTION_POOL` has the correct target size, membership, and order for
Ironclad potion generation. The original audit found that the local
`Potion::Gamble` variant occupied the target game's `GamblersBrew` slot.

Follow-up implementation renamed that entry to `Potion::GamblersBrew` and fixed
the behavior differences listed below. This document preserves the original
audit findings and records their resolution status.

## Pool Order

Target `PotionHelper.getPotions(IRONCLAD, false)` starts with the three
Ironclad-specific potions:

1. `BloodPotion`
2. `ElixirPotion`
3. `HeartOfIron`

It then appends the shared potion sequence:

1. `Block Potion`
2. `Dexterity Potion`
3. `Energy Potion`
4. `Explosive Potion`
5. `Fire Potion`
6. `Strength Potion`
7. `Swift Potion`
8. `Weak Potion`
9. `FearPotion`
10. `AttackPotion`
11. `SkillPotion`
12. `PowerPotion`
13. `ColorlessPotion`
14. `SteroidPotion`
15. `SpeedPotion`
16. `BlessingOfTheForge`
17. `Regen Potion`
18. `Ancient Potion`
19. `LiquidBronze`
20. `GamblersBrew`
21. `EssenceOfSteel`
22. `DuplicationPotion`
23. `DistilledChaos`
24. `LiquidMemories`
25. `CultistPotion`
26. `Fruit Juice`
27. `SneckoOil`
28. `FairyPotion`
29. `SmokeBomb`
30. `EntropicBrew`

Local `IRONCLAD_POTION_POOL` now matches this order with
`Potion::GamblersBrew` in the target `GamblersBrew` slot.

## Resolution Status

- `Potion::Gamble` was replaced by `Potion::GamblersBrew` for the target
  `GamblersBrew` slot, with a serde alias retained for old saved local data.
- Gambler's Brew now requires combat and opens the discard/draw selection instead
  of applying local gold RNG.
- Blood Potion now supports out-of-combat healing.
- Sacred Bark + Duplication Potion now creates two future duplication stacks.
- Sacred Bark + Liquid Memories now allows up to two discard-pile cards to be
  selected and returned at zero cost.
- In-combat Entropic Brew now filters Fruit Juice and consumes target random
  potion rolls based on potion capacity.

## Differences Found

### `Potion::Gamble` is not Gambler's Brew

Local files:

- `simulator/crates/sts_core/src/potion/mod.rs`
- `simulator/crates/sts_core/src/run/potion.rs`

The target pool entry is `GamblersBrew`. The jar implementation queues
`GamblingChipAction(player, true)`, discarding any number of cards from hand and
drawing the same number.

Local `Potion::Gamble` instead rolls potion RNG and either grants 50 gold or
loses 50 gold. This is not a target Slay the Spire potion behavior.

Impact: random potion generation can produce the correct slot by order, but
using the potion has the wrong mechanics and wrong semantic identity.

### Blood Potion out-of-combat use fails locally

Local files:

- `simulator/crates/sts_core/src/potion/mod.rs`
- `simulator/crates/sts_core/src/run/potion.rs`

`Potion::Blood` is not listed in local `requires_combat()`, so validation allows
out-of-combat use. The use implementation then unconditionally expects an active
combat state.

The jar `BloodPotion.use()` has both branches: in combat it queues `HealAction`;
outside combat it directly heals the player.

Impact: out-of-combat Blood Potion use should heal run HP, but local execution
will fail instead.

### Sacred Bark + Duplication Potion duplicates too little

Local files:

- `simulator/crates/sts_core/src/run/potion.rs`
- `simulator/crates/sts_core/src/combat/state.rs`

The target `AbstractPotion.getPotency()` doubles potency when the player has
Sacred Bark. `DuplicationPotion.use()` applies `DuplicationPower(potency)`, and
that power can cover two future cards with Sacred Bark.

Local code stores Duplication Potion as a boolean
`duplication_potion_pending`, so Sacred Bark still duplicates only one card.

Impact: Sacred Bark + Duplication Potion is under-modeled.

### Sacred Bark + Liquid Memories returns too few cards

Local files:

- `simulator/crates/sts_core/src/run/potion.rs`
- `simulator/crates/sts_core/src/combat/transition.rs`

The target `LiquidMemories.use()` queues
`BetterDiscardPileToHandAction(potency, 0)`. With Sacred Bark, potency is 2.

Local code opens a discard select with a single selected index and confirms one
card back to hand at zero cost.

Impact: Sacred Bark + Liquid Memories should support returning up to two cards,
but local code returns only one.

### In-combat Entropic Brew can roll Fruit Juice locally

Local files:

- `simulator/crates/sts_core/src/run/potion.rs`
- `simulator/crates/sts_core/src/run/reward.rs`

The target `EntropicBrew.use()` calls `AbstractDungeon.returnRandomPotion(true)`
in combat. The boolean path rerolls `Fruit Juice`, preventing it from appearing
from in-combat Entropic Brew.

Local Entropic Brew calls `target_random_potion()`, which can return
`Potion::FruitJuice`.

Impact: local in-combat Entropic Brew can generate a potion the target game
explicitly filters out.

### In-combat Entropic Brew consumes fewer RNG rolls in full-belt cases

Local files:

- `simulator/crates/sts_core/src/run/potion.rs`

The jar in-combat branch loops over `player.potionSlots` and queues an
`ObtainPotionAction` each time. Local code loops while `open_potion_slots() > 0`
after consuming Entropic Brew.

Impact: when using Entropic Brew from a belt with few open slots, local code can
consume fewer random potion rolls than the target. This matters for potion RNG
counter parity.

## Non-Differences Confirmed

- Pool length is 33 for Ironclad.
- The Ironclad-specific prefix and shared potion order match the target.
- `FairyPotion.canUse()` is false in the jar, consistent with local passive-use
  rejection.
- Simple numeric Sacred Bark doubling is broadly represented by local
  `potion_multiplier()` for direct payload potions.
