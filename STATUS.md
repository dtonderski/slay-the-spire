# Status

## What Exists

- Full combat engine: action queue, powers, status cards, card keywords, draw/shuffle, monster intents.
- **17+ Ironclad cards** including starter, simple attacks/skills, draw/energy, exhaust package.
- **Cultist** monster with ritual → attack pattern.
- **Run layer**: master deck, combat → reward screen, skip/take card reward.
- **Map layer**: fixed 7-node graph, node selection, floor/act tracking.
- Milestone integration tests: milestone1 (golden replay), milestone6 (Cultist), milestone7 (rewards), milestone8 (map).
- Golden replay hash: `9622d543ff250099`.
- **178 tests** passing (`cargo +stable-x86_64-pc-windows-gnu test` from `simulator/`).

## What Is Not Implemented

- Strength package cards (Inflame, Flex, Spot Weakness), complex cards
- Most Act 1 monsters (Jaw Worm, Louses, Slimes, elites, bosses)
- Gold/potion/relic rewards, card removal, upgrades
- Shops, rest sites, events, ascensions
- CommunicationMod parity, RL API, Python bindings

## Current Milestone

Milestone 5 (continuing): strength package + complex cards.

## Next Task

Milestone 5 batch 5: Inflame, Flex, Spot Weakness.

## Known Risks

- RNG parity unverified against real game.
- Use `stable-x86_64-pc-windows-gnu` toolchain.

## Last Updated

2026-06-18.
