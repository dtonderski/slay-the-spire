# Status

## What Exists

- Research and design documentation (`README.md`, `RESEARCH.md`, `DESIGN.md`, `VERIFICATION.md`, `TASKS.md`, `AGENT_RULES.md`, `STATUS.md`).
- Rust simulator workspace under `simulator/` with `sts_core` crate.
- Typed IDs, errors, snapshots, deterministic RNG placeholder, combat state, legal actions, internal action queue.
- Ironclad starter cards (Strike_R, Defend_R, Bash) with damage, block, vulnerable, end-turn flow.
- Starter deck constructor (5/4/1), Burning Blood, strength/weak/vulnerable combat math.
- Milestone 4 powers/statuses: dexterity, frail, ritual, metallicize, wound, dazed, burn, slimed, ethereal, exhaust, retain.
- Card keywords (`ethereal`, `exhaust`, `retain`, `unplayable`) and status card definitions.
- End-of-turn hand resolution (burn damage, ethereal exhaust, retain).
- End-of-turn power hooks (ritual, metallicize, weak/frail decrement, monster ritual/strength).
- Generic attack/skill play queues for card definitions with standard damage/block values.
- Milestone 1 golden replay final hash: `7640a497d20aa951`.
- 86 tests passing (`cargo +stable-x86_64-pc-windows-gnu test` from `simulator/`).

## What Is Not Implemented

- Most Ironclad cards beyond starter set
- Real Act 1 monsters (Cultist, Jaw Worm, etc.) beyond fixed dummy
- Rewards, map, shops, rest sites, events
- Relics (beyond Burning Blood innate), potions
- Ascensions, save import, CommunicationMod parity
- RL API, Python bindings, `sts_verify` crate

## Current Milestone

Milestone 5: More Ironclad Cards.

## Next Task

Milestone 5 batch 1: Simple attacks (Anger, Cleave, Twin Strike).

## Known Risks

- Exact Slay the Spire RNG parity requires controlled comparison against the real game.
- `sts_lightspeed` is prior art, not authority.
- CommunicationMod may not expose every hidden field.
- Use `stable-x86_64-pc-windows-gnu` for local verification (MSVC link broken).

## Last Updated

2026-06-18.
