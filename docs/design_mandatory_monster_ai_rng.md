# Mandatory monster AI reroll RNG

## Problem

Authoritative monster intent helpers accepted an optional `monsterRng`. Missing
streams selected fixed replacement rolls or skipped source RNG draws while still
returning plausible intents. Darkling encounter initialization used that path
even though the combat RNG stream was available.

## Decision

- Require `&mut StsRng` for Darkling, Nemesis, Looter, Mugger, Gremlin Leader,
  and Reptomancer authoritative intent selection.
- Remove fixed replacement rolls and skipped speech/branch draws.
- Pass the combat-owned `monsterRng` at both encounter entry and later turns.
- Keep representative fixture intent construction explicit and separate; it is
  not evidence of an authoritative AI roll.

## Verification

Source-backed unit regressions pin replacement ranges and draw counters. Map
entry, combat turns, strict corpus, repeated permanent replay, and snapshot
gates remain required before commit.
