# Required Card Upgrades

## Problem

Transmutation+ and Astrolabe both require their generated cards to be upgraded,
but each used `upgrade_content_id(...).unwrap_or(base)`. Missing or unknown
upgrade metadata therefore produced a valid-looking unupgraded card instead of
reporting unsupported content.

## Decision

`required_upgrade_content_id` is the authority for mechanics that mandate an
upgrade. Unknown card content returns `UnknownContent`; known content without an
upgrade link returns `UnsupportedMechanic` for that content ID.

Transmutation constructs and validates its ordered upgraded candidate pool
before drawing from card RNG, then uses the same bounded index draw as before.
An empty or unrepresentably large pool returns `InvalidState` instead of
underflowing its RNG bound.
Astrolabe resolves every transformed card through the required-upgrade helper
before committing its local misc-RNG counter or changing the deck. Optional
upgrade contexts such as egg relics retain their existing semantics.

## Verification

Unit coverage distinguishes unknown content from a missing required upgrade.
Regressions require upgraded Transmutation+ and Astrolabe output, while
permanent trace replay pins their surrounding RNG behavior.
