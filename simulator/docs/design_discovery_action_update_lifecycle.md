# DiscoveryAction update lifecycle

## Decision

The installed collection fork now supplies every gameplay action with a fixed
1/60-second tick (`1.0.9-collection.2`). The older 600 FPS / 100x observations
below came from the legacy pre-collection.2 cohort and cannot define target
pulse counts.

Model a hand-played Discovery lifecycle as:

1. The opening update generates the three visible, unique choices and opens
   the card-reward screen.
2. The reward screen pauses the action manager. After `CHOOSE`, resumed
   `DiscoveryAction.update()` calls may generate discarded choices while the
   action's fast duration counts down. Their count is a property of the fixed
   action-update lifecycle, never the encounter, remaining hand, source card,
   or trace identity.
3. Retrieval adds the selected card with its temporary zero cost. Deferred
   `onUseCard` follow-ups then resolve before `UseCardAction` closes the source
   through the ordinary card-settlement/action queue, including exhaust hooks
   such as Feel No Pain, Dead Branch, and Strange Spoon.
4. The action is complete at that response boundary. There is no staged
   Discovery RNG settlement on later `PLAY` or `END` commands.

Production replay currently burns one discarded post-select generation for an
ordinary retrieved Discovery so unsupported update-count behavior remains a
visible RNG divergence. Do not restore the legacy encounter/hand-shape table.
Derive any replacement count from the fixed-duration source lifecycle and clean
collection.2 evidence or direct instrumentation.

The legacy potion-specific picked/skipped pulse tables and delayed
`PendingPotionCardRewardSettlement` were removed. Attack, Skill, Power, and
Colorless Potion offers consume card RNG when their visible choices are
generated; selecting or skipping an already-open offer does not apply
trace-fitted hidden generations on later turns.

## Source evidence

The target jar is
`/mnt/d/SteamLibrary/steamapps/common/SlayTheSpire/desktop-1.0.jar`.
`com.megacrit.cardcrawl.actions.unique.DiscoveryAction.update()` calls
`generateCardChoices` before its opening-screen branch and before its
post-selection retrieval branch. The opening branch calls
`customCombatOpen` and returns after `tickDuration`; the resumed branch copies
the selected card, applies its temporary cost, queues the hand/discard effect,
clears `discoveryCard`, marks retrieval, and ticks duration. The installed SuperFastMode configuration still accelerates rendering, but
collection fork `.2` patches `AbstractGameAction.tickDuration` to subtract a
fixed 1/60 second. Display FPS and the visual delta multiplier therefore no
longer define the gameplay update count.

CommunicationMod exposes the open reward while the action is pending and
does not expose the post-`CHOOSE` state as ready until the action/card queues
are empty. Therefore a simulator model that leaves Discovery pending across
ordinary later commands is source-incompatible.

`AbstractPlayer.useCard()` appends `UseCardAction` after `card.use()` and its
`onUseCard` follow-ups. Consequently, after CHOOSE the target order is the
discarded Discovery generation, selected-card retrieval, deferred follow-ups,
then source settlement. For an exhausting source, Dead Branch consumes its
card RNG from `moveToExhaustPile` only after those earlier actions.

## Rejected alternatives

Earlier notes modeled encounter-, hand-, and source-specific
`generateCardChoices` counts, Letter Opener-specific counts, Dark Embrace
settle draws, and cross-command Discovery residuals flushed on a later `PLAY`
or combat end. Those hypotheses came from legacy collection timing and must
not be reintroduced. Bot follow-ups may still wait behind the visible reward
screen; see [design_discovery_hex_dazed_order.md](design_discovery_hex_dazed_order.md).
That queueing is not a Discovery RNG residual.
