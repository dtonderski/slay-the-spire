# Observation-independent combat card rewards

## Problem

After a potion opened a combat card-reward screen, the seed-start verifier
removed `card_reward_ids` from both projections before comparison. It claimed
the visible offer was transient and relied on a later pick or skip to expose a
problem. A divergent unchosen card, or an entire divergent offer followed by a
coincidentally matching chosen result—could therefore pass.

Combat card-reward command binding also depended on the observed pre-screen,
and potion-result routing depended on the observed post-screen. An observation
could choose whether the verifier treated a core transition as a card reward,
ordinary combat continuation, or run reward.

## Binding and projection rule

The simulator's combat state owns potion, Discovery, and Toolbox card-reward
offers. Their presence binds `CHOOSE <index>` and `SKIP` to the corresponding
typed core actions. After potion use, the resulting core state selects exactly
one destination:

- an owned combat card reward projects `CARD_REWARD` with every visible offer;
- a run reward requires `RunPhase::Reward` and reward state;
- ordinary combat continuation requires combat phase and combat state;
- any other combination fails at `invalid_combat_potion_destination`.

Observed screen type and offer identities are comparison input only. No
card-reward identity or master-deck field is removed based on the observed
post-screen.

## Regression contract

A forged retained trace replaces the first visible Demon Form potion offer
with Strike while leaving the later chosen-result state untouched. Verification
must report `card_reward_ids` at the potion-open action. External trace replay
also covers potion-open and combat card-reward-pick sequences.
