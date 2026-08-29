# Checked Combat Potion Stats

## Problem

Combat potions directly added to player block, persistent powers, temporary
Strength/Dexterity, and Duplication stacks with unchecked `i32` arithmetic.
Malformed imported combat state could therefore panic or wrap after the potion
had been removed from its slot. Blood Potion also multiplied max HP in `i32`
before dividing, so a representable percentage heal could overflow its
intermediate.

## Decision

All fixed player-stat potion bonuses use one checked helper that proves both
Sacred Bark multiplication and addition before assignment. Block, Ancient,
Heart of Iron, Cultist, Dexterity, Essence of Steel, Liquid Bronze, Regen,
Strength, Flex, Speed, and Duplication Potion propagate `InvalidState` when the
result is unrepresentable. Speed proves both its persistent and temporary
Dexterity results before assigning either.

Blood Potion calculates its percentage heal through an `i64` intermediate and
converts the final amount back to `i32`. With the target 20%/40% rates and a
validated positive `i32` max HP, the mathematical result is representable; no
valid-path result or healing cap changes.

`apply_potion_action` already validates the input and performs the complete use
on a cloned candidate run. Any propagated failure therefore preserves the
potion slot and the exact run/combat state. Monster Weak/Vulnerable potion
arithmetic belongs to the shared artifact/debuff boundary and remains a
separate follow-up rather than duplicating that mechanic here.

## Verification

A table-driven regression puts every affected destination at `i32::MAX`,
including both Speed destinations, and requires the exact typed error plus
unchanged input state. A valid Sacred Bark regression pins doubled values for
the multi-field Speed effect and representative block/power/stack effects.
Existing Energy and Fruit Juice overflow regressions remain unchanged.
