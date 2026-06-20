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

Expected result: `unexpected_diffs=0`, `seed_start.expected_failure=false`, verified labels through floor-3 combat completion and return-to-map steps, and `seed_start.first_boundary.path=$.actions[complete]`.

Current fidelity limit: VERIFY01 seed-start uses source-backed starter opening piles from `shuffleRng(seed + floor)`. CODEX04 seed-start verifies floor 1–3 combat with simulation-driven replay; captured floor-1/floor-2 reward screens, pickups, potion skip, and post-reward map returns are pinned against explicit expected states. Card reward RNG control flow now uses target-style `cardRng`, `cardRarityFactor`, and the full 72-card Ironclad reward pool; normal-combat gold now uses target-style `treasureRng.random(10, 20)`. Many pool entries are RNG-only until their card mechanics are implemented. Innate/extra-card opening piles still fall back to trace when seed shuffle does not match, post-END pile resync remains interim scaffolding, and relic pool popping/reward wiring plus broader reward RNG remains pending Milestone 24 work.

### Tests
- `cargo test` passing

## Current Captured Controller Trace

`verification/corpus/communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl` imports successfully with 42 states and 41 actions. Observed-state parity verifies floor 1–3 combat (Cultist, Small Slimes, 2 Louse), Dramatic Entrance, Battle Trance path cards, multiple `END` turns, and reward screens with `unexpected_diffs=0`. Unsupported commands are classified for Neow/map/seed-start gaps only.

## Next Task

Continue Milestone 24: generate elite/chest/boss relic reward screens from the persisted relic pool state and broaden reward RNG beyond normal-combat gold, then expand shop/rest/event parity as traces reach those rooms.

## Milestone 24 Notes

Normal reward potion drops are now source-backed for `potionRng.random(99)`, persisted `potionChance`, target rarity thresholds, and the full 33-potion Ironclad reward pool. Remaining M24 work is relic reward wiring, broader reward contexts, shop/rest/event RNG, and replacing any captured reward constants that are still pinned directly in the seed-start verifier.
Relic tier rolls are now source-backed for normal/chest-style and elite thresholds with persisted `relic_seed_count`; Ironclad common/uncommon/rare/shop/boss pool initialization uses target-order pools and Java `Collections.shuffle` seeded by five `relicRng.nextLong()` calls. Hidden relic pool state is serialized on `RunState`; pool popping follows target front/back removal, empty-pool fallbacks, Circlet/Red Circlet exhaustion, and source-backed spawn filters for floor/shop/bottled/starter-upgrade/campfire gates. Reward screens can now carry RNG-accurate `RelicKey` offers separately from implemented relic effects. Generated elite/chest/boss relic reward screens are still pending.

## Milestone 20 Notes

External seed conversion is source-backed from the target `SeedHelper.getLong(String)` bytecode in `desktop-1.0.jar`: uppercase, map `O` to `0`, parse in base 35 with alphabet `0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ`. Captured checks now pass for `VERIFY01`, `CODEX03`, and `CODEX04`, and seed-start CLI output includes `seed_start.numeric_seed`.

## Milestone 21 Notes

CODEX04 seed-start verification now covers the captured Neow colorless-card branch: `START IRONCLAD 0 CODEX04`, talk, choose the colorless-card reward option, verify `Deep Breath` / `Dramatic Entrance` / `Jack Of All Trades`, pick `Dramatic Entrance`, and leave to the first map-choice screen with the card in the deck. Broad Neow RNG is still classified as captured-branch only; exact general option generation remains future evidence work.

## Milestone 22 Notes

Milestone 22 is complete for the available captured evidence. Act 1 map, normal encounter selection, and monster spawn parity are source-backed for `VERIFY01`, `CODEX04`, and `CODEX03`. Full captured map topology/edges/room symbols match for all three seeds. Map-choice prefixes and chosen combat paths are pinned, including CODEX04 `[2, 3, 2]`, CODEX03 `[1, 0, 1]`, and VERIFY01 `[1, 2]` with captured nodes entering combat rooms. Normal encounter list generation covers weak/strong pools, first-strong exclusions, and no-repeat-last-two retries; room execution maps combat index to list entries via `normal_encounter_key_at_combat_index`. Target spawn state at combat entry covers Cultist, Jaw Worm, Small Slimes, and 2 Louse with floor-offset `monsterHpRng`, `miscRng` louse kind selection, and post-HP/bite Curl Up rolls from the decoded 3–7 range. Seed-start reports include `m22_encounter_report`; CODEX04 and CODEX03 each have three captured verified combat-entry rosters, while VERIFY01 has one captured verified entry plus two clearly separated source-backed predictions because that trace ends after the first combat reward. CODEX04 seed-start now reaches floor-3 combat completion; CODEX03 full seed-start replay remains outside the passing set because its Neow's Lament/reward branch is not implemented end to end.

## Milestone 23 Notes

Milestone 23 is complete for captured CODEX04/VERIFY01 scope. Observed-state and seed-start CODEX04 floor 1–3 combat parity pass with `unexpected_diffs=0`; END transitions are no longer draw/shuffle scope failures. Game-compatible pieces now in place: decoded Ironclad starter master-deck instance order and `shuffleRng(seed + floor)` opening piles (VERIFY01 pure; CODEX04 falls back to trace when innate/extra cards are present), top-of-pile draw semantics matching CommunicationMod bottom-first export, `StsRng` in-combat draws via `shuffle_rng`, deterministic slime/louse move cycles, and captured card mechanics for `Dramatic Entrance`, `Battle Trance`, and `Shrug It Off`. Post-END pile resync remains interim scaffolding until innate/extra-card master-deck ordering is fully decoded without trace fallback (M24 follow-up).
