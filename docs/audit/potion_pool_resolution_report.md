# Potion Pool Audit Resolution Report

Date: 2026-07-01

Scope: resolution of the Ironclad potion pool audit in
`docs/audit/potion_pool_audit.md`.

## Result

All 33 Ironclad potion pool entries were checked against Slay the Spire desktop
`1.0` bytecode. The pool membership and order are correct after replacing the
local `Potion::Gamble` identity with `Potion::GamblersBrew`.

The audit issues found in the implementation were corrected:

- `Potion::GamblersBrew` now represents the target `GamblersBrew` potion, uses
  content id `203`, requires combat, does not directly consume potion RNG, and
  opens the Gambling Chip discard/draw selection.
- Blood Potion can now be used outside combat and heals run HP without requiring
  a combat state.
- Sacred Bark + Duplication Potion now creates two future duplication stacks
  instead of a single boolean one-shot.
- Sacred Bark + Liquid Memories now allows up to two discard-pile cards to be
  selected and returned to hand at zero cost.
- In-combat Entropic Brew now uses combat-filtered random potion generation so
  Fruit Juice is excluded.
- Entropic Brew now consumes random potion rolls according to potion capacity,
  matching the target loop behavior even when only some slots can receive a
  generated potion.

## Updated Files

- `simulator/crates/sts_core/src/potion/mod.rs`
- `simulator/crates/sts_core/src/run/potion.rs`
- `simulator/crates/sts_core/src/run/reward.rs`
- `simulator/crates/sts_core/src/run/mod.rs`
- `simulator/crates/sts_core/src/combat/state.rs`
- `simulator/crates/sts_core/src/combat/transition.rs`
- `simulator/crates/sts_core/src/combat/card_effects.rs`
- `simulator/crates/sts_core/src/lib.rs`
- `simulator/crates/sts_verify/src/sim_real.rs`
- `simulator/docs/m32a_relic_potion_matrix.md`
- `docs/audit/potion_pool_audit.md`

## Verification

Focused verification was run for the corrected behavior:

- `cargo test -p sts_core run::potion::tests`
- `cargo test -p sts_core duplication_potion_stacks_duplicate_multiple_future_cards`
- `cargo test -p sts_core duplication_potion_duplicates_panic_button_block_before_prevention`
- `cargo check -p sts_verify`

The full `cargo test -p sts_core` suite was also run during the fix pass and
still reported unrelated pre-existing failures outside the potion audit scope.
The duplication-related failure observed in that run was corrected and verified
with the focused regression above.

`cargo test -p sts_verify` compiles the changed verifier mapping but still has
two unrelated `m22` encounter-name parity failures:

- `m22::tests::codex03_lament_first_three_combat_spawns_match_target_generation`
- `m22::tests::codex04_first_three_combat_spawns_match_target_generation`
