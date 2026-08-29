# Checked Run Progression and Max HP

## Problem

The last non-seed `wrapping_add` operations under `sts_core::run` advanced
terminal floors, granted event or potion max HP, and granted Energy Potion
energy. Divergent imported state could therefore wrap to plausible negative
values after consuming a potion or changing screens, leaving detection to a
later validator.

## Decision

`RunState` owns checked helpers for one-floor advancement and ordinary max-HP
gain. The max-HP helper proves both max and current HP results before assigning
either field and rejects negative gain requests. Boss-chest and Spire Heart
entry propagate checked floor advancement before changing screen ownership.

Big Fish, Forgotten Altar, Singing Bowl, immediate Neow rewards, and Fruit
Juice use the shared max-HP boundary. Fruit Juice separately checks its mirrored
combat HP values. Energy Potion uses checked combat energy addition. All of
these actions already operate on cloned candidate state, so failure cannot
consume the potion or expose a partial transition.

The three remaining `wrapping_add` operations under `sts_core::run` are Java
RNG seed derivations and remain intentional.

## Verification

Regression tests cover both fields of atomic max-HP gain, negative input,
terminal floor overflow, event action propagation, Energy Potion overflow,
run-side Fruit Juice overflow, and a late mirrored-combat Fruit Juice failure.
