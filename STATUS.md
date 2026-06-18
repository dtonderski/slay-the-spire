# Status

## What Exists

- Rust simulator workspace (`simulator/sts_core`) with combat engine, action queue, snapshots, RNG placeholder.
- Ironclad starter deck, Burning Blood, strength/weak/vulnerable/dexterity/frail combat math.
- Milestone 4 complete: ritual, metallicize, status cards (wound/dazed/burn/slimed), card keywords (ethereal/exhaust/retain).
- Milestone 5 in progress:
  - Simple attacks: Anger, Cleave, Twin Strike (+ upgrades)
  - Simple skills: Shrug It Off, True Grit
- Milestone 6 started: Cultist monster (50 HP, ritual then attack pattern).
- `TargetRequirement::AllEnemies`, `InternalAction::DealDamageAll`, `InternalAction::DrawCards`.
- Monster definitions with per-monster intent/move selection.
- Milestone 1 golden replay hash: `077e7df619d1e8c5`.
- **129 tests** passing with `cargo +stable-x86_64-pc-windows-gnu test` from `simulator/`.

## What Is Not Implemented

- Remaining Ironclad cards (draw/energy, exhaust package, strength package, complex cards)
- Most Act 1 monsters beyond Cultist
- Rewards, map, shops, rest sites, events
- Relics (beyond Burning Blood), potions, ascensions
- Save import, CommunicationMod parity, RL API, Python bindings

## Current Milestone

Milestone 5: More Ironclad Cards (batch 3+: draw/energy cards next).

## Next Task

Milestone 5 batch 3: Pommel Strike, Battle Trance, Seeing Red.

## Known Risks

- Exact RNG parity requires real-game traces.
- Use `stable-x86_64-pc-windows-gnu` toolchain (MSVC link broken).

## Last Updated

2026-06-18.
