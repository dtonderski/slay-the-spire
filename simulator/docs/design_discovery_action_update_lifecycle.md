# DiscoveryAction update lifecycle

## Decision

For the installed Slay the Spire 1.0 target with SuperFastMode configured for
600 FPS and a 100x delta multiplier, model a hand-played Discovery as:

1. The opening update generates the three visible, unique choices and opens
   the card-reward screen.
2. The reward screen pauses the action manager. On `CHOOSE`, the resumed
   `DiscoveryAction.update()` performs exactly one discarded three-card
   generation before retrieving the selected card.
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

## Historical notes superseded by this decision

The following notes preserve their original observations but their
multi-generation or cross-command Discovery model is superseded:

- [design_discovery_rng_settlement.md](design_discovery_rng_settlement.md)
- [design_discovery_dark_embrace_settle_draw.md](design_discovery_dark_embrace_settle_draw.md)
- [design_discovery_dark_embrace_stage2_before_magnetism.md](design_discovery_dark_embrace_stage2_before_magnetism.md)
- [design_discovery_dead_branch_letter_opener_generation.md](design_discovery_dead_branch_letter_opener_generation.md)
- [design_discovery_flush_on_combat_end.md](design_discovery_flush_on_combat_end.md)
- [design_discovery_flush_prior_residual.md](design_discovery_flush_prior_residual.md)

The separate [Discovery hex/dazed ordering note](design_discovery_hex_dazed_order.md)
remains applicable: bot follow-up actions may be parked behind the visible
reward screen, but that queueing is not a Discovery RNG residual.
