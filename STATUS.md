# Status

## What Exists

### Combat (`sts_core`)
- 25+ Ironclad cards (starter through Whirlwind), upgrades for most
- Full power/status system: strength, weak, vulnerable, dexterity, frail, ritual, metallicize, temp strength, monster weak
- Status cards: wound, dazed, burn, slimed; keywords: ethereal, exhaust, retain
- Exhaust hooks: Feel No Pain, Dark Embrace, Burning Pact
- X-cost Whirlwind (spend all energy, hit all enemies per energy)
- Internal action queue with damage, block, draw, exhaust events

### Monsters
- Fixed dummy, **Cultist**, **Jaw Worm**, **Gremlin Nob** (enrage on skills)

### Run / Meta
- Master deck, HP sync, **gold** (99 start)
- **Reward screen**: 3 fixed cards + 20 gold; skip / take card / take gold
- **Map**: 7-node fixed graph (combat/rest/shop/boss), floor tracking
- **Rest**: heal 30% max HP
- **Shop**: buy fixed Anger for 50 gold
- **Relics**: Vajra (+1 strength at combat start)

### Tests
- milestone1 golden replay, milestone6 monsters, milestone7 rewards, milestone8 map, milestone9 rest+shop, milestone10 relics
- **264 tests** passing: `cargo +stable-x86_64-pc-windows-gnu test` from `simulator/`

## What Is Not Implemented

- Remaining Ironclad cards (Havoc, Warcry, Searing Blow, etc.)
- Louses, Slimes, Lagavulin, Sentries, Act 1 bosses
- Card remove/upgrade at rest, reward RNG, map generation
- Most relics/potions, events, ascensions
- `sts_verify` crate, CommunicationMod parity, RL API, Python bindings

## Current Milestone

Milestones 5–10 in progress (cards, monsters, rewards, map, shop, relics).

## Next Task

More Act 1 monsters, card upgrade at rest site, ascension config (A0 baseline done implicitly).

## Last Updated

2026-06-18 (overnight session).
