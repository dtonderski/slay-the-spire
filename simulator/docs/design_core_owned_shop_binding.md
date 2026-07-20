# Core-owned shop command binding

## Problem

The seed-start verifier previously used observed `SHOP_ROOM` and `SHOP_SCREEN`
values to decide whether `CHOOSE` entered the merchant or purchased an item.
It then used the observed post-screen to label and project the result as a shop
screen, purge grid, or reward. A forged or stale observation could therefore
select a different transition family from the same simulator pre-state.

Shop-room exit also bypassed action validation by directly clearing shop,
reward, grid, and phase fields. Card purchases whose deck update arrived one
frame late removed `deck_ids` from comparison without registering an assertion
that had to settle.

## Core action boundary

Core shop state owns every binding:

- a closed merchant accepts only `CHOOSE 0` as `RunAction::EnterShop`;
- an open merchant binds `CHOOSE <index>` through the simulator-generated
  affordable shop choices;
- `LEAVE` applies validated `RunAction::LeaveShop`;
- a closed shop room applies validated `RunAction::Proceed` and reaches `Idle`.

`RunAction::Proceed` is now a legal shop action only while the merchant is
closed, inventory exists, and no grid is active. It removes the shop and moves
the run to `Idle`. The verifier no longer edits those fields itself.

## Destination projection

After the core action, one classifier derives the only valid destination from
`RunState`: room, merchant screen, grid, reward, or map. The observed post-screen
is projected independently against that destination; it cannot choose the
label, action, or phase. Inconsistent core destinations and unsupported
transient `NONE` frames fail closed.

## Deferred purchased-card visibility

CommunicationMod can publish spent gold and removed stock before a purchased
card appears in the deck. Non-deck shop fields are still compared immediately.
If the observed deck is exactly the simulator pre-deck, the purchase action is
left unverified with a `PendingDeckAssertion`. A later non-poll stable frame must
begin with the expected simulator deck or the action becomes an unexpected
diff; an unfinished assertion prevents a complete pass.

## Regression contract

Core tests require the closed shop room to expose and apply `Proceed`. Verifier
tests require choice-zero entry, reject other room indices, and exercise all
core-owned destinations. A forged purchase post-screen must remain a
`shop purchase` screen-type diff and cannot be reclassified as a purge grid.
The permanent corpus covers merchant entry, purchases, purge, reward return,
room exit, and deferred deck settlement.
