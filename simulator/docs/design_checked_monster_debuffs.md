# Checked Monster Debuffs

## Problem

The shared monster Weak, Vulnerable, and Strength-reduction helpers mutate
`i32` power fields with unchecked arithmetic. Malformed imported state can
therefore panic or wrap while executing cards, potions, relics, or queued
combat actions. Champion Belt compounds the risk by applying Vulnerable before
Weak, so a failure in the second operation must not leave only the first one
committed.

## Decision

Monster debuff helpers return `SimResult<bool>`. The boolean retains its current
meaning: `false` for a non-positive amount or an Artifact-blocked debuff, and
`true` only when the requested debuff lands. Artifact still consumes exactly
one charge before any destination arithmetic is considered.

Weak and Vulnerable use checked addition; Strength reduction uses checked
subtraction. An unrepresentable result returns `InvalidState` and leaves the
power state unchanged. Every card, potion, and relic caller propagates the
failure.

The relic-aware Vulnerable helper stages a copy of all monster powers, applies
Vulnerable and Champion Belt Weak to that copy, and commits only after both
succeed. The card-action path likewise stages this compound effect while
retaining the two application flags needed for Sadistic Nature.

Player debuffs use a separate shared boundary with substantially more callers
and are intentionally left for a follow-up slice.

## Verification

Unit regressions cover Weak and Vulnerable at `i32::MAX`, Strength at
`i32::MIN`, Artifact consumption ahead of a maxed destination, non-positive
amounts, and valid applications. A Champion Belt regression requires complete
rollback when its Weak addition overflows. Potion regressions require Fear and
Weak Potion failures to preserve the complete run, including the potion slot.
Existing workspace and permanent-corpus replay gates protect ordinary card,
potion, Artifact, relic, and Sadistic Nature behavior.
