# Havoc Dual Wield still increments Time Warp

## Source behavior

`DualWield` has no custom `canUse`. `PlayTopCardAction` autoplay therefore
constructs a real `UseCardAction`. `DualWieldAction` immediately finishes when
the hand has no Attack/Power (`cannotDuplicate.size() == hand.size()`), so no
select opens.

`TimeWarpPower.onAfterUseCard` still increments. FIDL01668 Havoc+ PlayTop Dual
Wield is 6→8, then Defend/Impervious/Strike reach 11 so the next Strike is the
12th card.

## Non-goals

- Do not open a Dual Wield select when no Attack/Power remains.
- Do not increment Time Warp on Clash/unplayable PlayTop (`dontTriggerOnUseCard`).
