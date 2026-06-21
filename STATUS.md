# Status

## What Exists

### Combat
- **45 Ironclad cards** (Milestone 5 complete + Ascender's Bane + Dramatic Entrance)
- Full Act 1 monster + boss roster
- Ascension modifiers A0-A20 (config, elites, damage, HP, Bane, deadly enemies, double boss)

### Run / Meta
- Reward screen with source-backed card/gold/potion/relic RNG; elite/chest/boss relic reward screens from persisted pools
- Shop: full target-style inventory (7 cards, 3 relics, 3 potions, remove service) via `merchantRng`/`cardRng`/`potionRng` and relic pools; legacy fixed Anger/Vajra/Fire fixture when `merchant_rng_seed == 0`
- Potions: Fire, Block, Fear, Gamble; full 33-potion Ironclad reward pool for drops
- Events: Act 1 event/shrine pools with `generateEvent` shrine chance; map event rooms call `enter_event_screen`; Shining Light costs 20% max HP and upgrades up to two random upgradeable deck cards
- Rest: heal, smith, card removal (deterministic heal amount; no RNG)

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

Current fidelity limit: VERIFY01, CODEX04, and CODEX03 seed-start traces pass with `unexpected_diffs=0` through their declared completion boundaries (CODEX03 ends after floor-3 return-to-map; CODEX04 after floor-3 combat completion). Post-reward map returns are simulation-driven from captured map topology. Innate/extra-card opening piles still fall back to trace when seed shuffle does not match; post-END pile resync remains interim scaffolding. Captured shop/event/rest CommunicationMod traces and Act 1 boss reward remain outside the passing nightly set.

### Tests
- `cargo test` passing
- Nightly parity (`scripts/nightly_parity.ps1`) runs VERIFY01, CODEX04, and CODEX03 seed-start traces with `unexpected_diffs=0`

## Current Captured Controller Trace

`verification/corpus/communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl` imports successfully with 42 states and 41 actions. Observed-state parity verifies floor 1–3 combat (Cultist, Small Slimes, 2 Louse), Dramatic Entrance, Battle Trance path cards, multiple `END` turns, and reward screens with `unexpected_diffs=0`. Unsupported commands are classified for Neow/map/seed-start gaps only.

`verification/corpus/communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl` (CODEX03) seed-start replay covers Neow's Lament, three combats (Jaw Worm, Cultist, 2 Louse), simulation-driven rewards/map returns, and ends after floor-3 return-to-map with `unexpected_diffs=0`.

## Next Task

Milestone 27 is complete for the selected TEST trace. Seed-start replay verifies the full Act 1 path from `START` through boss relic reward (Cursed Key) and return-to-map before Act 2 with `unexpected_diffs=0`.

Verification command:

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-21T09-57-10-380Z.jsonl
```

Expected result: `seed_start.expected_failure=false`, `unexpected_diffs=0`, `seed_start.first_boundary.path=$.actions[complete]`.

## Milestone 27 Notes

Milestone 27 is complete for `trace-2026-06-21T09-57-10-380Z.jsonl` (seed `TEST` / numeric `1_218_623`). Seed-start verifies through Act 1 boss relic pickup and pre–Act-2 map return with `unexpected_diffs=0`. Coverage includes events (Scrap Ooze, Big Fish), normal/elite combats, rest/treasure/shop rooms, potion/hand-select/reward flows, Guardian boss combat (observed-state sync), boss chest, and Cursed Key boss relic reward. The trace is in nightly parity (`scripts/nightly_parity.ps1`) and `sts_verify/tests/corpus.rs`.

## Milestone 26 Notes

Milestone 26 is complete. The scratch `_tmp_test.rs` debugging artifact was removed, nightly parity passed, and the M25 seed-start regression gate is ready to use as the clean baseline for M27.

## Milestone 25 Notes

VERIFY01, CODEX04, and CODEX03 seed-start traces pass with `unexpected_diffs=0` through their declared completion boundaries. Nightly parity (`scripts/nightly_parity.ps1`) runs all three. Use `sts_verify minimize` to produce prefix traces under `verification/corpus/bugs/` when debugging new failures. Seed-start hidden-state assumptions are documented in `VERIFICATION.md` (shuffle fallback, pile resync, UUID fields, deferred card reward, combat-entry `cardRng` +3).

## Milestone 24 Notes

Milestone 24 is complete for captured reward RNG and source-backed shop/event generation. Normal-combat and elite/chest/boss relic rewards use target-style RNG over persisted pools without corrupting `relic_rng_counter` after pool initialization. Shop generation mirrors `sts_lightspeed` `Shop.cpp` (7 cards, 3 relics, 3 potions, sale slot, remove pricing) with `relic_key`-only shop relic ownership. Act 1 events use target pool lists with shrine roll; Golden Shrine, Cleric heal, and Shining Light (HP cost + random upgrades) have implemented outcomes. Seed-start VERIFY01/CODEX04 reward verification is simulation-driven; nightly parity includes both traces. Captured shop/event/rest CommunicationMod traces are not in the passing nightly set. Unmapped shop colorless cards are RNG placeholders until mapped. Post-reward map-return pins and CODEX03 remain Milestone 25.

## Milestone 20 Notes

External seed conversion is source-backed from the target `SeedHelper.getLong(String)` bytecode in `desktop-1.0.jar`: uppercase, map `O` to `0`, parse in base 35 with alphabet `0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ`. Captured checks now pass for `VERIFY01`, `CODEX03`, and `CODEX04`, and seed-start CLI output includes `seed_start.numeric_seed`.

## Milestone 21 Notes

CODEX04 seed-start verification now covers the captured Neow colorless-card branch: `START IRONCLAD 0 CODEX04`, talk, choose the colorless-card reward option, verify `Deep Breath` / `Dramatic Entrance` / `Jack Of All Trades`, pick `Dramatic Entrance`, and leave to the first map-choice screen with the card in the deck. Broad Neow RNG is still classified as captured-branch only; exact general option generation remains future evidence work.

## Milestone 22 Notes

Milestone 22 is complete for the available captured evidence. Act 1 map, normal encounter selection, and monster spawn parity are source-backed for `VERIFY01`, `CODEX04`, and `CODEX03`. Full captured map topology/edges/room symbols match for all three seeds. Map-choice prefixes and chosen combat paths are pinned, including CODEX04 `[2, 3, 2]`, CODEX03 `[1, 0, 1]`, and VERIFY01 `[1, 2]` with captured nodes entering combat rooms. Normal encounter list generation covers weak/strong pools, first-strong exclusions, and no-repeat-last-two retries; room execution maps combat index to list entries via `normal_encounter_key_at_combat_index`. Target spawn state at combat entry covers Cultist, Jaw Worm, Small Slimes, and 2 Louse with floor-offset `monsterHpRng`, `miscRng` louse kind selection, and post-HP/bite Curl Up rolls from the decoded 3–7 range. Seed-start reports include `m22_encounter_report`; CODEX04 and CODEX03 each have three captured verified combat-entry rosters, while VERIFY01 has one captured verified entry plus two clearly separated source-backed predictions because that trace ends after the first combat reward. CODEX04 seed-start now reaches floor-3 combat completion; CODEX03 seed-start replays Neow's Lament through floor-3 return-to-map with `unexpected_diffs=0`.

## Milestone 23 Notes

Milestone 23 is complete for captured CODEX04/VERIFY01 scope. Observed-state and seed-start CODEX04 floor 1–3 combat parity pass with `unexpected_diffs=0`; END transitions are no longer draw/shuffle scope failures. Game-compatible pieces now in place: decoded Ironclad starter master-deck instance order and `shuffleRng(seed + floor)` opening piles (VERIFY01 pure; CODEX04 falls back to trace when innate/extra cards are present), top-of-pile draw semantics matching CommunicationMod bottom-first export, `StsRng` in-combat draws via `shuffle_rng`, deterministic slime/louse move cycles, and captured card mechanics for `Dramatic Entrance`, `Battle Trance`, and `Shrug It Off`. Post-END pile resync remains interim scaffolding until innate/extra-card master-deck ordering is fully decoded without trace fallback (M24 follow-up).
