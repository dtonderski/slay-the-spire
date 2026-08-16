# Courier restock 4/5 rounding (rejected)

Restock currently uses `(price as f32 * 0.8) as i32`. Switching that to
`round_discount(4, 5)` made FIDL01452 and FIDL01735 complete-pass but
regressed FIDL01249 (step 971 `565 != 564`) and FIDL01479 (step 626
`17 != 16`). FIDL01407 also failed earlier (step 1716 `1127 != 1126`).

The ±1 shop-gold quartet is not one rounding rule. Do not ship a global
restock-round change.
