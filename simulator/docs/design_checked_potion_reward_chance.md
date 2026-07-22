# Checked Potion Reward Chance

## Problem

Potion reward settlement updated the persistent `potion_chance` with unchecked
signed addition and subtraction. Invalid imported state could overflow after
the drop roll, either panicking or wrapping to a plausible chance while the RNG
counter advanced. The all-monsters-escaped reward path duplicated the unchecked
miss update.

## Decision

The target potion-reward helper becomes fallible and clone-and-commit for its
two authoritative inputs: `potionRng` and `potion_chance`. It checks the base
drop-chance sum and the selected hit/miss adjustment, performs every random
draw on a candidate RNG, and commits the RNG and chance only when settlement
succeeds. Overflow and underflow return `InvalidState` with neither input
changed.

All combat-reward entry paths propagate this result. The escaped-monster path
uses the same checked miss adjustment and stores its RNG counter only after the
chance update succeeds. Existing run action boundaries already operate on a
cloned candidate state, so a propagated error retains the exact authoritative
run, combat, reward, and RNG state.

Valid-path probability, potion selection, and RNG draw order remain unchanged.
This slice does not impose a speculative fixed range on `potion_chance`; it only
rejects arithmetic that the state representation cannot express.

## Verification

Focused regressions cover base-chance overflow, miss overflow, hit underflow,
unchanged helper RNG/chance inputs, and exact reward-entry rollback for the
escaped-monster branch. Existing reward RNG tests and permanent replay protect
valid hit/miss outcomes and draw order.
