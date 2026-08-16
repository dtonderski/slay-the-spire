# Time Warp Burning Pact CONFIRM can publish Metallicize before discard

## Source behavior

`TimeWarpPower.onAfterUseCard` at 12 calls `callEndTurnEarlySequence` and
queues +2 Strength. `MetallicizePower.atEndOfTurnPreEndTurnCards` then
`addToBot`s `GainBlockAction` before `DiscardAtEndOfTurnAction`.

CommunicationMod can snapshot that frame: Time Warp counter reset, Strength
applied, Metallicize block present, hand still held. FIDL01694 CONFIRM 1769
is that publication (`block 4`, energy still 0).

The ordinary confirm-without-end candidate has block 0. The Metallicize lag
candidate applies the pre-card end-turn block and marks
`time_warp_end_powers_applied` so the later forced END does not grant it twice.

## Non-goals

- Do not change Orichalcum or ordinary End Turn Metallicize order.
- Do not treat leftover STATE 547 (FIDL01597) here.
