# Status

## What Exists

### Combat
- **44 Ironclad cards** (Milestone 5 complete + Ascender's Bane)
- Full Act 1 monster + boss roster
- Ascension modifiers A0–A20 (config, elites, damage, HP, Bane, deadly enemies, double boss)

### Run / Meta
- Reward screen with rarity-weighted RNG card choices; potion/relic take actions
- Shop: buy Anger, Vajra relic, Fire Potion (fixed prices)
- Potions: Fire, Block, Fear, Gamble
- Events: fixed Golden Shrine event with availability and placeholder RNG checks
- Rest: heal, smith, card removal; fixed + generated map placeholder with A1+ elite nodes

### Relics / Potions
- Common simple relic: Strawberry pickup HP bonus
- Energy relic: Coffee Dripper energy per turn and rest restriction
- Start-combat relic: Anchor block
- On-card-play relic: Ink Bottle draw after 10 cards
- Damage/block relic: Ornamental Fan block every 3 attacks per turn
- Stateful relic: Ice Cream preserves energy between turns
- Random-effect potion: Gamble Potion (+50/−50 gold via RNG)

### Verification (Milestone 12)
- CommunicationMod trace importer (`sts_verify`)
- Canonical observed-state normalizer for combat/run JSON
- `sts_verify` CLI: `trace`, `diff`, `parity`, `corpus`
- Manual corpus: milestone1, cultist bash step, known divergence list
- Nightly parity script: `scripts/nightly_parity.ps1`

### Tests
- **454 tests** passing

## Next Task

Milestone 12 complete for starter scope. Future work: full Act 1 trace replay parity and sim-side trace replay driver.

## Last Updated

2026-06-18 (Milestones 10–12).
