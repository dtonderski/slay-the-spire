# Courier restock card prices

Shop **open** still truncates the 0.9–1.1 jitter to `i32`, then applies Courier
/ Membership with `MathUtils.round` (`round_discount`).

Courier **card restock** (`ShopScreen.purchaseCard`) keeps the jitter as a
float, multiplies Courier `0.8` (and Membership `0.5`) on that float, then
casts once to `int`. Truncating after jitter and again after `0.8` undercharges
when `base * jitter` is in `[n+0.5, n+1)`:

- FIDL01407 Searing Blow: combined float yields 62; double truncation yielded 61.
- FIDL01407 Rupture at 1716: both paths yield 61. A global `round_discount(4/5)`
  on the truncated jitter wrongly priced that slot at 62 and was rejected.

Colorless restock still applies the `1.2` markup on the float before those
discounts. Do not reuse opening-shop rounding for restock, and do not reuse
restock's single cast for opening-shop discounts.
