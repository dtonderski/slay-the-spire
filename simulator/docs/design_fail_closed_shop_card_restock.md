# Fail-closed shop card restocking

## Problem

Courier restocking classified a purchased non-colorless card with
`shop_card_type`. When classification failed, production silently substituted
`CardType::Attack` and generated a plausible replacement from the wrong pool.
Known cards that do not belong in a shop, such as curses or statuses imported
into a shop slot, could therefore pass run validation and mutate the run before
the divergence became visible.

## Decision

- A shop card slot must contain either a supported colorless shop card or a card
  classified by the Ironclad shop pool.
- Known content outside those pools fails with `UnsupportedMechanic`.
- Courier restocking returns `SimResult` and propagates the same typed error
  instead of substituting `CardType::Attack`.
- Restocked card prices use the same rounded Courier/Membership discounts as
  shop open (`round_discount`), not `(price as f32 * 0.8) as i32` (FIDL01407
  Searing Blow 62 vs 61).
- Unknown card definitions remain `UnknownContent` through the existing run
  validation boundary.

## Verification

A regression test imports a known non-shop curse into a Courier shop and proves
the purchase is rejected before a restock can fabricate an Attack replacement.
The complete workspace, strict corpus, deterministic replay, and snapshot gates
remain required before commit.
