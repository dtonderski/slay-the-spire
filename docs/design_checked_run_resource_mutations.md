# Checked Run Resource Mutations

## Problem

General run healing and gold helpers previously reproduced Java integer wrapping
and depended on a later state validator to notice the resulting negative or
contradictory state. That made direct callers infallible and allowed compound
effects such as Bloody Idol, floor-entry relics, shop generation, or combat
gold synchronization to mutate earlier fields before a later problem became
visible.

## Decision

Run healing and gold gain return `SimResult` and use checked arithmetic. Gold
gain is clone-and-commit because Bloody Idol healing is part of the same
semantic operation. Floor/rest entry relic effects, shop entry, and immediate
Neow rewards are also transactional at their public mutation boundaries.

All event, reward, potion, rest, map, verifier, and relic-acquisition callers
must propagate or explicitly classify the result. Invalid Neow reward kinds
return `IllegalAction` instead of panicking. Combat gold synchronization checks
both the observed transition delta and the retained combat total.

Java RNG seed derivation remains wrapping by design; it is not player-resource
arithmetic.

## Verification

Regression tests cover direct gold overflow, a late Bloody Idol healing failure,
two floor-entry gold relics where the second addition fails, immediate Neow max
HP overflow, and an invalid immediate-reward kind. Every failing transactional
case retains the exact pre-state.
