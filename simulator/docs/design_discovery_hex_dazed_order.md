# Discovery reward vs Hex Dazed order

## Scope

When the player is Hexed and plays Discovery, `HexPower.onUseCard` queues a
Dazed into the draw pile with `addToBot`. That insert must not resolve while the
Discovery card-reward screen is open.

## Evidence

FIDL00233 step 986: after `PLAY Discovery`, CommunicationMod shows `CARD_REWARD`
with draw pile still missing the Hex Dazed. Step 987 `CHOOSE` adds the chosen
card and the Dazed appears in the draw pile.

Opening the Discovery decision during queue *build* parked `SpendEnergy` /
source settlement behind the reward and also let Hex insert too early once the
decision existed without a pending-action bucket.

## Implementation

1. `discovery_queue` only enqueues `OpenDiscoveryCardReward`; it does not open
   the decision during queue construction.
2. `DiscoveryCardReward` carries `pending_actions`. While that decision is open,
   remaining bot follow-ups (Hex Dazed) are parked there.
3. Discovery CHOOSE flushes `pending_actions` after source close / Dead Branch.
