# Checked Run Acquisition Arithmetic

## Problem

Relic pickup and card-added relic effects used wrapping integer arithmetic for
max HP, current HP, energy, gold, and reward counts. Imported or divergent
state near an integer boundary could therefore acquire a relic or card, advance
RNG, and only later appear as an unrelated invalid state.

## Decision

Authoritative relic pickup and card insertion are transactional fallible
boundaries. Their complete acquisition effects use checked arithmetic and
return `InvalidState` before committing when a target integer cannot represent
the result. This includes relic max-HP and energy bonuses, Old Coin and Ceramic
Fish gold, Darkstone Periapt HP, and Tiny House/Orrery reward counts.

Card insertion now returns `SimResult` so shop, reward, event, and grid callers
cannot discard a card-added relic failure. The existing clone-and-commit run
transitions retain their pre-state, card inventory, relic pools, and RNG
counters on failure.

Seed derivation retains wrapping arithmetic because it models Java RNG seed
semantics rather than an authoritative player-state counter.

## Verification

Regression tests exercise HP, energy, gold, card-added relic, and late Tiny
House reward-count overflow. Each assertion checks the typed error and exact
pre-state rollback, including mutations and RNG work performed earlier in the
attempted pickup.
