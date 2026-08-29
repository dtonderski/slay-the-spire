# Observation-independent Neow potion rewards

## Problem

The seed-start verifier selected one of two simulated Neow three-potion flows
from the observed post-command screen. An observed `EVENT` frame caused the
verifier to pre-populate all three potions and declare the reward complete;
an observed `COMBAT_REWARD` frame entered a verifier-only pick state. The core
also granted every potion immediately, so neither path represented the target
reward lifecycle authoritatively.

## Target evidence

The installed `12-18-2022` target JAR's `NeowReward.activate` bytecode adds
three random potions with `AbstractRoom.addPotionToRewards`, opens
`CombatRewardScreen`, and removes only the automatically constructed card
reward. Potion choices remain on that reward screen until picked or skipped,
and the empty screen requires `PROCEED`.

## Decision

- Core Neow selection opens a `RewardScreen` containing three ordered potion
  offers with `RewardContinuation::Neow`; it does not grant them eagerly.
- The hidden card-reward RNG consumption remains part of opening the screen.
- Each verifier `CHOOSE 0` is bound to core `TakePotionReward { index: 0 }`.
- Verifier `PROCEED` must succeed through the core reward action and projects
  the resulting core map state.
- Owned potion identities and remaining offered potion identities are compared
  throughout this flow.
- The observed post-screen can no longer select an alternate simulated flow.

## Verification

Core regression coverage follows option selection through all three potion
picks and `PROCEED`. Verifier regression coverage checks generated offer
identity after every pick and proves that an observed direct `EVENT` result is
reported as a difference instead of changing replay state.
