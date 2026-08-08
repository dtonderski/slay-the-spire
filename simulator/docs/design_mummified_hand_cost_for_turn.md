# Mummified Hand candidate filter

## Model

STS `MummifiedHand` selects among hand cards with **costForTurn > 0** after the
played Power's cost-changing effects. The simulator previously required
`definition.cost > 0 && cost_for_turn > 0`, which excluded X-cost cards that had
a positive turn cost.

```rust
(cost_for_turn > 0 && !corruption_zeroed)
```

Corruption still zeroes Skills for eligibility when Corruption is active.

## Status

Source-aligned filter. Does not by itself clear FIDL00387 Discovery pre-open
debt. Permanent corpus remains green.
