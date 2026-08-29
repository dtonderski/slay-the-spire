# Fallible Card Upgrades

## Problem

The shared card-upgrade helper returned `Option<CardInstance>`. `None` represented
a card that is not upgradeable, but the API had no way to represent an upgrade
that is legal in the game and exceeds the simulator's storage domain. Searing
Blow uses a `u8` per-instance upgrade counter, so attempting to upgrade
Searing Blow+255 performed unchecked addition and could panic or wrap to zero.

Searing Blow damage also substituted static damage and coerced Searing Blow+
with a missing count to +1. Imported, malformed metadata could therefore become
a plausible card instead of failing validation.

## Decision

`upgrade_card_instance` returns `SimResult<Option<CardInstance>>`. `Ok(None)` is
reserved for genuinely non-upgradeable content. A representational failure is a
typed `InvalidState` error. Game-rule upgradeability remains a boolean, so
Searing Blow+255 is still exposed as an upgradeable card; applying +256 fails
explicitly instead of disappearing from legal actions.

Every production mutation caller propagates the error, including Armaments,
Apotheosis, Blessing of the Forge, Warped Tongs, rest smithing, grids, events,
random relic upgrades, and reward upgrade rolls. Bulk paths compute selected
upgrades before committing card or RNG-counter mutations.

Searing Blow metadata is exact: the base card must have count zero, the upgraded
card must have a positive count, and other cards must have no Searing Blow count.
Damage construction requires static metadata and checked arithmetic; it no longer
repairs a missing upgraded count.

Profile-backed Note For Yourself cards use the same fallible repeated-upgrade
helper during run validation and event construction, so an impossible saved
upgrade count is rejected at the import boundary rather than truncated.

## Verification

Regressions cover malformed combat metadata, impossible profile-card upgrades,
representable max-count damage, the typed +256 failure, and a public rest-smith
action that remains legal but returns the error without mutating its input.
Existing repeated-upgrade and damage sequence tests remain required. Formatting,
strict workspace Clippy, workspace tests, snapshot round trip, and repeated
permanent-corpus replay remain commit gates.
