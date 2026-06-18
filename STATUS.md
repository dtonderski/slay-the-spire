# Status

## What Exists

### Combat
- **44 Ironclad cards** (Milestone 5 complete + Ascender's Bane)
- Full Act 1 monster + boss roster
- Ascension modifiers A0-A20 (config, elites, damage, HP, Bane, deadly enemies, double boss)

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
- Random-effect potion: Gamble Potion (+50/-50 gold via RNG)

### Verification (Milestone 12)
- CommunicationMod trace importer (`sts_verify`)
- Canonical observed-state normalizer for combat/run JSON
- `sts_verify` CLI: `trace`, `diff`, `parity`, `corpus`
- Observed-state sim-vs-real verifier for the captured CommunicationMod trace.
- Seed-start verifier mode parses `START IRONCLAD 0 VERIFY01`, verifies captured bootstrap/Neow/map/first encounter/Cultist combat/reward pickup, and reports the first expected post-reward map boundary.
- Manual corpus: milestone1, cultist bash step, known divergence list
- Nightly parity script: `scripts/nightly_parity.ps1`
- Planned post-12 milestones now separate observed-state replay from true seed/RNG parity.

Run the captured-trace verifier with:

```powershell
cd simulator
cargo run -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
```

Current fidelity limit: this is observed-state transition replay. It does not prove that simulator RNG can produce the same run from `VERIFY01`.

Run the seed-start RNG harness with:

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
```

Expected result: `seed_start.expected_failure=true` at `$.actions[step=19].command`. This is intentional until reward-to-map continuation is covered by Milestone 18.

### Tests
- **454 tests** passing

## Next Task

Start Milestone 18 end-to-end seed-start trace parity. Seed-start full-trace RNG parity is not currently claimed until `PROCEED` back to map passes.

## Last Updated

2026-06-18 (Milestone 17 captured reward seed-start path added).
