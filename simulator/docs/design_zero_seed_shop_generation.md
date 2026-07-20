# Zero-Seed Shop Generation

## Problem

Production shop entry treats `merchant_rng_seed == 0` as missing state and
installs the legacy one-card Anger/Vajra/Fire Potion milestone fixture. Zero is
a valid deterministic RNG seed, so this branch changes mechanics and hides the
normal card, relic, potion, pricing, sale-slot, and RNG-consumption behavior.

## Decision

Shop entry always calls the source-backed shop generator, including for seed
zero. Opening an already-created shop remains idempotent and consumes no
additional RNG. The legacy fixed screen moves into the milestone test that
still exercises it; production code neither exposes nor selects that fixture.

This slice does not change the stored run RNG schema. It removes zero as a
sentinel and preserves the existing merchant, card, potion, and relic stream
counter behavior for every seed.

## Verification

A regression must prove that a zero-seed run receives the complete generated
inventory, advances the normal RNG streams, and differs from the legacy fixed
screen. Existing seeded shop, corpus, and snapshot tests must remain
deterministic. Tests that specifically exercise the old milestone inventory
must install that fixture explicitly.
