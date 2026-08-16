# Dolly's Mirror shop obtain

## Source behavior

Buying Dolly's Mirror opens a one-card `GridCardSelectScreen`. Selecting a
card constructs `ShowCardAndObtainEffect` for a copy of that card.
`CardHelper.obtain` is authoritative at construction time, so the copy enters
the master deck on the grid select.

## Rejected deferral

FIDL01357, FIDL01566, and FIDL01798 publish the pre-copy deck on the first
post-grid `SHOP_SCREEN` and only show the copy after the next purchase.
FIDL01244, FIDL01267, and FIDL01617 publish the copy immediately on that same
grid-select return. A deferred shop obtain therefore matches some collector
frames and regresses others. Leave the obtain immediate until a source-backed
generic discriminator exists.

## Non-goals

- Do not invent a seed-specific shop branch.
- Do not use observed deck order to choose when the copy commits.
