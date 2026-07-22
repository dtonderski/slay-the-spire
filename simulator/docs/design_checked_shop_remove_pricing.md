# Checked Shop Remove Pricing

## Problem

Shop purge pricing converted the persistent `u32` removal count to `i32` with
`as`, then multiplied and added with unchecked signed arithmetic. A malformed
import could therefore generate a plausible negative or cheap purge price.
Confirming a purge also charged gold, broke Maw Bank, incremented the count,
and removed the card before deriving the next price.

## Decision

The supported count domain is the largest `u32` count for which
`75 + 25 * count` fits in `i32`. `RunState::validate` rejects larger counts.
Base-price derivation uses checked conversion, multiplication, and addition;
the public price helper returns `SimResult<i32>`. Smiling Mask still fixes the
visible price at 50, but does not make an unrepresentable authoritative count
valid. Courier and Membership Card retain their existing discount order.

Shop generation preflights the base purge price before allocating card IDs or
consuming any RNG. Purge confirmation proves both the next count and its next
derived price before charging gold, breaking Maw Bank, removing a card, or
closing the grid. Failure therefore leaves the exact run unchanged.

Private discount arithmetic widens its multiplication and rounding
intermediate to `i64`; valid nonnegative `i32` prices discounted by the static
fractions remain in `i32` without changing their rounded result.

## Verification

Focused regressions cover the maximum representable count, the first invalid
count, unchanged shop-generation RNG/state on invalid input, exact purge-grid
rollback at the maximum count, and the existing base/Courier/Membership/combined
and Smiling Mask prices. Permanent shop replay protects valid purge choice and
price sequencing.
