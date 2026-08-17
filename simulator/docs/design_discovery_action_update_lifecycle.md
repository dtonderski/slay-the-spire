# DiscoveryAction update lifecycle

## Decision

For the installed Slay the Spire 1.0 target with SuperFastMode configured for
600 FPS and a 100x delta multiplier, model a hand-played Discovery as:

1. The opening update generates the three visible, unique choices and opens
   the card-reward screen.
2. The reward screen pauses the action manager. `CHOOSE` retrieves the
   visible selected card after burning discarded post-select
   `generateCardChoices` updates: two when another Discovery is still in
   hand, when Magnetism plays a Magnetism-generated Discovery (FIDL01787),
   or when Awakened One is present and 6+ cards remain after the source left
   (FIDL01561); otherwise one. A global no-burn SuperFastMode
   candidate regresses
   FIDL01630. See
   [design_discovery_first_combat_post_select_generations.md](design_discovery_first_combat_post_select_generations.md).
3. Retrieval adds the selected card with its temporary zero cost. Deferred
   `onUseCard` follow-ups then resolve before `UseCardAction` closes the source
   through the ordinary card-settlement/action queue, including exhaust hooks
   such as Feel No Pain, Dead Branch, and Strange Spoon.
4. The action is complete at that response boundary. There is no staged
   Discovery RNG settlement, deferred cross-command generation, combat-end
   flush, or source-card state to carry into a later `PLAY` or `END` command.

The simulator test [magnetism_disc_probe.rs](../crates/sts_core/tests/magnetism_disc_probe.rs)
exercises the production `PLAY` → `CHOOSE` path and pins the counter delta to
three draws at each boundary. It also checks that source exhaust reaches Feel
No Pain through the normal queue.

## Source evidence

The target jar is
`/mnt/d/SteamLibrary/steamapps/common/SlayTheSpire/desktop-1.0.jar`.
`com.megacrit.cardcrawl.actions.unique.DiscoveryAction.update()` calls
`generateCardChoices` before its opening-screen branch and before its
post-selection retrieval branch. The opening branch calls
`customCombatOpen` and returns after `tickDuration`; the resumed branch copies
the selected card, applies its temporary cost, queues the hand/discard effect,
clears `discoveryCard`, marks retrieval, and ticks duration. The installed
SuperFastMode configuration is
`/mnt/c/Users/davton/AppData/Local/ModTheSpire/SuperFastMode/SuperFastModeConfig.properties`
with `isDeltaMultiplied=true` and `deltaMultiplier=100.0`; the target display
configuration caps the game at 600 FPS. Together with
`ACTION_DUR_FAST = 0.25`, this yields one resumed update in the captured
environment.

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

Earlier notes modeled extra hidden `generateCardChoices` burns, Letter
Opener-specific generation counts, Dark Embrace settle draws, and
cross-command Discovery residuals flushed on a later `PLAY` or combat end.
Those hypotheses do not match the source update lifecycle above and must not
be reintroduced. Bot follow-ups may still wait behind the visible reward
screen; see [design_discovery_hex_dazed_order.md](design_discovery_hex_dazed_order.md).
That queueing is not a Discovery RNG residual.
