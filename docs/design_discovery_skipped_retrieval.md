# Discovery skipped retrieval

## Source behavior

Vanilla `DiscoveryAction.update()` generates a three-card offer, opens the
combat card-reward screen, and pauses. After `CHOOSE`, the resumed update
generates one discarded three-card set and then retrieves the selected card
(`ShowCardAndAddToHandAction` with a temporary cost). `UseCardAction` still
settles the Discovery source through the ordinary exhaust/discard queue.

If `DiscoveryAction` completes on its opening update (`ACTION_DUR_FAST` plus
SuperFastMode / CommunicationMod load), the card-reward screen stays visible
but no resumed update runs after `CHOOSE`. The selected card is never created
and the discarded post-select `generateCardChoices` never happens.
CommunicationMod then exposes a closed `NONE` combat screen whose hand, draw,
discard, exhaust, and limbo piles contain no copy of the chosen card.
`UseCardAction` still settles the Discovery source after the screen closes.

External trace `FIDL01246` is the first permanent witness: `CHOOSE 0` on
Dark Embrace / Flex / Offering leaves hand `Iron Wave, Defend_R`, exhausts
Discovery, and never publishes a third Dark Embrace.

## Simulator contract

The ordinary `apply_combat_card_reward_choice` path remains authoritative and
still appends the generated card. Skipped retrieval is a separate core
transition that:

1. requires an open `DiscoveryCardReward`
2. does not burn a discarded post-select generation
3. does not allocate or insert the selected card
4. still drains deferred `card.use()` follow-ups and closes the Discovery
   source, including exhaust hooks

The verifier rebuilds that candidate only from the pre-`CHOOSE` simulator
state. It may replace the ordinary transition only when the complete observed
combat subset matches the candidate, the ordinary add-to-hand result does not,
and the post-state is a quiescent combat `NONE` screen. No observed identities
or seed-specific hydration are used.

The uncreated card is not parked in `pending_hidden_hand_card_until_end_turn`.
Unlike a selection-screen card that later flushes into discard, a skipped
Discovery retrieval never materializes the generated instance.
