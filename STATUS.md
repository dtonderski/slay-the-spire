# Status

## What Exists

- **Combat engine**: 20+ Ironclad cards, powers/statuses, exhaust hooks, multi-hit/all-enemy attacks, temp strength, monster weak.
- **Monsters**: Fixed dummy, Cultist (ritual/attack), Jaw Worm (chomp/thrash/bellow cycle).
- **Run layer**: master deck, gold (99 start), reward screen (3 cards + 20 gold), skip/take rewards.
- **Map layer**: fixed 7-node graph, floor/act tracking, rest room → heal 30% max HP.
- Integration tests: milestone1–9.
- Golden replay hash: see `tests/milestone1.rs`.
- **231 tests** passing with `cargo +stable-x86_64-pc-windows-gnu test`.

## What Is Not Implemented

- Complex cards (Whirlwind, Havoc, etc.), remaining Act 1 monsters, elites/bosses
- Shops, events, relics (beyond Burning Blood), potions, ascensions
- Card upgrade/remove at rest, reward RNG, map generation
- `sts_verify`, CommunicationMod parity, RL API

## Current Milestone

Milestone 5 (complex cards) / Milestone 6 (more monsters).

## Next Task

Milestone 5 batch 6: complex cards (Whirlwind first) or Milestone 6 Louses.

## Last Updated

2026-06-18.
