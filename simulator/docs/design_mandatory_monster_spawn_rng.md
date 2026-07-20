# Mandatory monster spawn RNG

## Problem

Monster summon and split helpers accepted optional AI and HP RNG streams. When
the streams were absent, they still returned successful mutations while using
fixture HP, leaving fixture intents, or choosing hand-written slime intents.
Those states looked valid but did not preserve target RNG order or mechanics.

## Decision

- Require `monsterRng` for every spawned monster's opening intent.
- Require `monsterHpRng` when a summon rolls HP.
- Remove fallback summon HP and split-intent branches.
- Preserve the target constructor, HP, and opening-intent draw order.

The production combat state already owns both streams. Explicit deterministic
test streams cover fixture-driven tests.

## Verification

Existing source-backed Collector, Bronze Automaton, Reptomancer, and slime
split regressions pin draw counts, HP, move history, and intents. The strict
corpus, repeated permanent replay, and snapshot gates remain required.
