# Status

## What Exists

### Combat
- **45 Ironclad cards** (Milestone 5 complete + Ascender's Bane + Dramatic Entrance)
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

### Verification (Milestone 12 + 19)
- CommunicationMod trace importer (`sts_verify`)
- Canonical observed-state normalizer for combat/run JSON
- `sts_verify` CLI: `trace`, `diff`, `parity`, `corpus`
- Observed-state sim-vs-real verifier for captured CommunicationMod traces
- Seed-start verifier mode parses `START IRONCLAD 0 VERIFY01` and verifies the captured trace through return to map with `seed_start.expected_failure=false`
- Manual corpus: milestone1, cultist bash step, known divergence list
- Regression corpus includes `trace-2026-06-18T16-50-50-232Z.jsonl` (CODEX04 controller trace)
- Nightly parity script: `scripts/nightly_parity.ps1`
- Observed-state verifier hygiene (Milestone 19):
  - unmapped combat/reward cards classify as unsupported instead of shifting indices
  - `PLAY n` no-target commands work for mapped no-target cards such as Dramatic Entrance
  - combat comparison uses the first living monster, not slot 0
  - unsupported monster-turn AI names monster groups (for example `AcidSlime_M`, `FuzzyLouseDefensive`)
  - reward `CHOOSE n` preserves observed choice indices when some reward options are unmapped
  - deck comparisons are partial when the observed deck contains unmapped cards
- Seed-start Neow coverage (Milestone 21):
  - `VERIFY01` verifies the captured Toy Ornithopter branch through return to map
  - `CODEX04` verifies talk, colorless reward choices, Dramatic Entrance pickup, leaving Neow, and the first captured map choice into a 54/54 HP Cultist
  - unchosen Neow branches remain explicitly classified

Run the VERIFY01 captured-trace verifier with:

```powershell
cd simulator
cargo run -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
```

Run the CODEX04 observed-state verifier with:

```powershell
cd simulator
cargo run -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
```

Expected result: `unexpected_diffs=0` with unsupported items named for seed-start gaps, unmapped cards, draw/shuffle scope, and unsupported monster groups.

Run the seed-start RNG harness with:

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
```

Expected result: `seed_start.expected_failure=false`, `seed_start.first_boundary.path=$.actions[complete]`, and `unexpected_diffs=0`.

Run the CODEX04 seed-start Neow harness with:

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
```

Expected result: `unexpected_diffs=0`, verified labels through `map first monster node`, and `seed_start.first_boundary.category=unsupported_combat_path` at the first CODEX04 combat action.

Current fidelity limit: VERIFY01 has captured-trace seed-start parity through return to map and source-backed first-three encounter spawn verification via `seed_start.m22_encounter_report`. CODEX04 seed-start parity covers the captured Neow colorless-card branch, first map choice into a 54/54 HP Cultist, and M22 encounter spawn verification for all three captured combat entries; executing CODEX04 combat remains Milestone 23 draw/shuffle/combat parity work.

### Tests
- `cargo test` passing

## Current Captured Controller Trace

`verification/corpus/communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl` imports successfully with 42 states and 41 actions. Observed-state parity verifies Cultist combat, Dramatic Entrance no-target plays, nonzero reward picks, and gold/card rewards with `unexpected_diffs=0`. Unsupported commands are classified for Neow/map/seed-start gaps, draw/shuffle scope, and slime/louse monster AI.

## Next Task

Begin Milestone 23: remove the early Act 1 draw/shuffle scope boundary and implement real move selection for Cultist, slimes, louses, Jaw Worm, and captured early-run cards so seed-start can replay combat through floor 3.

## Milestone 20 Notes

External seed conversion is source-backed from the target `SeedHelper.getLong(String)` bytecode in `desktop-1.0.jar`: uppercase, map `O` to `0`, parse in base 35 with alphabet `0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ`. Captured checks now pass for `VERIFY01`, `CODEX03`, and `CODEX04`, and seed-start CLI output includes `seed_start.numeric_seed`.

## Milestone 21 Notes

CODEX04 seed-start verification now covers the captured Neow colorless-card branch: `START IRONCLAD 0 CODEX04`, talk, choose the colorless-card reward option, verify `Deep Breath` / `Dramatic Entrance` / `Jack Of All Trades`, pick `Dramatic Entrance`, and leave to the first map-choice screen with the card in the deck. Broad Neow RNG is still classified as captured-branch only; exact general option generation remains future evidence work.

## Milestone 22 Notes

Milestone 22 is complete. Act 1 map, normal encounter selection, and monster spawn parity are source-backed for `VERIFY01`, `CODEX04`, and `CODEX03` through the first three combats. Full captured map topology/edges/room symbols match for all three seeds. Map-choice prefixes and chosen combat paths are pinned, including CODEX04 `[2, 3, 2]`, CODEX03 `[1, 0, 1]`, and VERIFY01 `[1, 2, x]` with all three nodes entering combat rooms. Normal encounter list generation covers weak/strong pools, first-strong exclusions, and no-repeat-last-two retries; room execution maps combat index to list entries via `normal_encounter_key_at_combat_index`. Target spawn state at combat entry covers Cultist, Jaw Worm, Small Slimes, and 2 Louse with floor-offset `monsterHpRng`, `miscRng` louse kind selection, and post-HP/bite Curl Up rolls from the decoded 3–7 range. Seed-start reports now include `m22_encounter_report` verifying first-three combat entry rosters for VERIFY01 (one captured trace entry plus two source-backed generated entries) and CODEX04/CODEX03 (three captured trace entries). CODEX04 seed-start still stops at the first unsupported combat command; that boundary is Milestone 23 work.

## Last Updated

2026-06-19 (completed Milestone 22: room execution, target combat-entry spawn parity, and seed-start M22 encounter reports for VERIFY01/CODEX04/CODEX03 first-three prefixes).
