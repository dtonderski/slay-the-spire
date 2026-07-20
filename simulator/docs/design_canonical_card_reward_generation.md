# Canonical Card Reward Generation

## Problem

The core reward API exposes four alternative three-card constructors alongside
the target-backed generator:

- a fixed Anger/Cleave/Shrug It Off milestone reward;
- a compatibility wrapper for that fixed reward;
- a simulator-only `SimulatorRng` rarity/card picker;
- a compatibility wrapper for the simulator-only picker.

It also exposes an unused alias for the target-backed generator. Production
reward entry already uses `target_card_reward_choices` and the persistent
target-style `StsRng` stream. The only alternate-path consumer is an old
milestone determinism test using the simulator-only compatibility wrapper.
Keeping the alternatives public permits new production code to select a
plausible but divergent reward authority.

## Decision

- `target_card_reward_choices` is the sole public Ironclad combat-card reward
  constructor.
- Remove fixed and simulator-only placeholder reward constructors and their
  facade re-exports.
- Remove the unused source-backed alias; the canonical function itself states
  the target contract.
- Preserve target rarity-factor updates, duplicate rerolls, upgrades, card IDs,
  and persistent RNG counters unchanged.

The milestone determinism test moves to the canonical target generator. No
fixture consumes the fixed choices, so no test-local copy is needed. Future
tests that require fixed choices should construct explicit `CardInstance`
values locally.

## Verification

Repository search must show no alternate reward constructor, and existing
deterministic reward, workspace, corpus, repeated replay, and snapshot tests
must remain green.
