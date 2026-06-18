# Status

## What Exists

### Combat (`sts_core`)
- 25+ Ironclad cards (starter through Whirlwind), upgrades for most
- Full power/status system: strength, weak, vulnerable, dexterity, frail, ritual, metallicize, temp strength, monster weak, spikes (thorns)
- Status cards: wound, dazed, burn, slimed; keywords: ethereal, exhaust, retain
- Exhaust hooks: Feel No Pain, Dark Embrace, Burning Pact
- X-cost Whirlwind (spend all energy, hit all enemies per energy)
- Internal action queue with damage, block, draw, exhaust events

### Monsters
- Fixed dummy, **Cultist**, **Jaw Worm**, **Gremlin Nob** (enrage on skills)
- **Red Louse** (curl/bite), **Green Louse** (curl/bite + spikes 3)
- **Spike Slime** (lick weak / spit attack), **Acid Slime (S)** (attack / apply weak)
- **Lagavulin** (3-turn sleep, wake on hit, siphon −2 str/dex, 18-damage attacks)

### Run / Meta
- Master deck, HP sync, **gold** (99 start)
- **Reward screen**: 3 fixed cards + 20 gold; skip / take card / take gold
- **Map**: 7-node fixed graph (combat/rest/shop/boss), floor tracking
- **Rest**: heal 30% max HP; **smith** upgrades deck cards (e.g. Strike_R → Strike_R+)
- **Shop**: buy fixed Anger for 50 gold
- **Relics**: Vajra (+1 strength at combat start)

### Verification (`sts_verify`)
- Crate skeleton: trace JSONL types, `canonical_diff` stub, corpus path helpers
- Integration test loads `verification/corpus/manual/milestone1.jsonl` when present

### Tests
- milestone1 golden replay, milestone6 monsters, milestone7 rewards, milestone8 map, milestone9 rest+shop, milestone10 relics
- **301 tests** passing: `cargo +stable-x86_64-pc-windows-gnu test` from `simulator/`

## What Is Not Implemented

- Remaining Ironclad cards (Havoc, Warcry, Searing Blow, etc.)
- Sentries, Act 1 bosses
- Card remove at rest, reward RNG, map generation
- Most relics/potions, events, ascensions
- CommunicationMod parity, RL API, Python bindings

## Current Milestone

Milestone 6 monsters — Lagavulin done; next is Sentries.

## Next Task

Sentries (per TASKS.md Milestone 6).

## Last Updated

2026-06-18 (Lagavulin).
