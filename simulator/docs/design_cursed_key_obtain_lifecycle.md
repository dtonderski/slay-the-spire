# Cursed Key chest-obtain lifecycle

## Source and checkpoints

The target `CursedKey.onChestOpen` path for a non-boss chest calls
`AbstractDungeon.returnRandomCurse()`, which samples `CardLibrary.getCurse()`
from `cardRng`, then queues `ShowCardAndObtainEffect`. The effect is not a
synchronous `masterDeck` mutation when the chest opens. In FIDL01385, the
ordinary `TreasureRoom` `CHOOSE 0` at step 154 exposes `COMBAT_REWARD` with the
16-card deck; the next reward input (gold at step 155) exposes the queued
`Shame` as card 17.

## Contract

The typed treasure transition consumes the card-RNG draw and records the curse
in the existing `RunState::pending_obtain_cards` queue. It leaves the master
deck unchanged while the opened chest owns the `Map` reward continuation.
Pending authority accepts only the Cursed Key + ordinary treasure reward shape
and a single normal curse; all other owners/shapes fail closed. The central
reward-input boundary settles pending obtains before applying the accepted
reward action, so the first input after the open resolves the visual effect
without an observation-derived deck or an unconditional flush.

The queue and settlement are simulator-owned and deterministic. Trace
observations only compare the resulting checkpoint and never select, hydrate,
or repair the pending card.
