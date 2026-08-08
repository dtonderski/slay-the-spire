# Ceramic Fish reward gold lag (FIDL00426)

## Observation

After picking a combat card reward with **Ceramic Fish**, CommunicationMod can:

1. Publish the new deck card while **gold is still pre-Fish** (721 vs 730).
2. Emit a later **CHOOSE 0** that only reveals Fish gold (+9) while the **GOLD**
   reward line remains, before a subsequent CHOOSE actually takes the offer.

Taking gold on that intermediate CHOOSE jumps `+gold_offer` and desyncs
(FIDL00426: 730 vs 763).

## Rules

1. **Card reward pick comparison**: if simulated gold is exactly observed gold +
   `CERAMIC_FISH_GOLD`, lag the comparison gold to the observation (deck settled
   or deferred).
2. **Gold reward choose**: if applying `TakeGoldReward` diverges but the
   pre-choose reward subset already matches the observation (Fish gold present,
   GOLD still listed), treat as
   `gold reward (ceramic fish gold lag no-op)` and do not take gold.

## Non-goals

Does not change Fish gold amount, obtain timing in core, or potion-belt gold
rewrites.
