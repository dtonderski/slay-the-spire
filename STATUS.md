# Status

## What Exists

### Combat
- **45 Ironclad cards** (Milestone 5 complete + Ascender's Bane + Dramatic Entrance)
- Full Act 1 monster + boss roster
- Ascension modifiers A0-A20 (config, elites, damage, HP, Bane, deadly enemies, double boss)

### Run / Meta
- Reward screen with source-backed card/gold/potion/relic RNG; elite/chest/boss relic reward screens from persisted pools
- Shop: full target-style inventory (7 cards, 3 relics, 3 potions, remove service) via `merchantRng`/`cardRng`/`potionRng` and relic pools; legacy fixed Anger/Vajra/Fire fixture when `merchant_rng_seed == 0`
- Potions: Fire, Block, Fear, Gamble, Power, Attack, Skill, Colorless, Entropic Brew, Duplication, Distilled Chaos, Liquid Memories, Snecko Oil, Smoke Bomb, Elixir, plus deterministic Ancient, Blood, Heart of Iron, Cultist, Dexterity, Energy, Essence of Steel, Explosive, Liquid Bronze, Regen, Strength, Weak, Fruit Juice, Swift, Flex, Blessing of the Forge, and Speed effects; full 33-potion Ironclad reward pool for drops
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

Current fidelity limit: VERIFY01, CODEX04, and CODEX03 seed-start traces pass with `unexpected_diffs=0` through their declared completion boundaries (CODEX03 ends after floor-3 return-to-map; CODEX04 after floor-3 combat completion). Post-reward map returns are simulation-driven from captured map topology. Innate/extra-card opening piles still fall back to trace when seed shuffle does not match; post-END pile resync remains interim scaffolding. Act 1 boss reward remains outside the passing nightly set for VERIFY01/CODEX04/CODEX03.

Milestone 28 is complete on the TEST trace (`trace-2026-06-21T09-57-10-380Z.jsonl`). Shop inventory at entry (step 168) and shop purchase/purge through step 176 are source-backed: class-card prices use library rarity with target-style `(int)(base * factor)` truncation, colorless prices use `AbstractCard.getPrice` bases (50/75/150) with the 1.2 multiplier, `affordable_shop_picks` drives CommunicationMod `choice_list` and `CHOOSE` index mapping, and membership/sale pricing matches captured gold. Full seed-start parity reports `unexpected_diffs=0` through Act 1 boss relic return-to-map (`test_seed_start_full_act1_boss_relic_prefix`); nightly includes this trace.

Milestone 27 is complete for the same TEST trace through Act 1 boss relic pickup and pre–Act-2 map return. Coverage includes events, normal/elite combats, rest/treasure/shop rooms, potion/hand-select/reward flows, Guardian boss combat (observed-state sync), boss chest, and Cursed Key boss relic reward.

Milestone 29 is in progress. The TEST trace elite/boss slice has a passing guard test, `test_seed_start_m29_test_elite_boss_without_observed_sync`, with elite/boss observed-state restoration disabled. This slice covers Lagavulin sleep/Metallicize block, wake-on-HP-damage, player vulnerable, Regret end-turn damage, Demon Form/Thunderclap trace playability, Gremlin Nob coverage in the TEST route, Guardian mode-shift scaffolding, and Act 1 boss relic return through the M27/M28 verifier path. Important carve-out: the TEST Lagavulin fight uses Power Potion; the in-combat potion reward, temporary zero-cost card, and downstream potion-tainted combat state still sync from observed state and are not yet a full card/potion parity claim. M29 is not complete until a structurally complete Sentries seed-start trace is captured and verified. The overnight collector produced a structurally valid Sentries run, `trace-2026-06-23T02-56-19-245Z.run2.valid-prefix.jsonl`, which reaches floor 7 Sentries. `trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl` removes 49 no-progress card-reward skip/reopen pairs from that run. Seed-start verification now supports its captured transform-card Neow branch, Sever Soul, Uppercut, floor-2 lethal Bash sequencing where Vulnerable follows lethal damage, the captured The Ssssserpent event branch, the Sentries elite reward sequence, and the following Blue Slaver combat/reward prefix. On the cleaned trace it verifies all 225 actions with `unexpected_diffs=0`; the only remaining boundary is `missing_post_reward_boundary` because the trace ends on a reward screen before a final `PROCEED`.

### Tests
- `test_seed_start_m28_shop_entry_parity`, `test_seed_start_full_act1_boss_relic_prefix`, and `test_seed_start_m29_test_elite_boss_without_observed_sync` pass on `trace-2026-06-21T09-57-10-380Z.jsonl`
- Focused monster acceptance: `cargo test -p sts_core --test milestone6` passes
- Full-suite checks pass: `cargo test -p sts_core -- --test-threads=1` and `cargo test -p sts_verify --test corpus -- --test-threads=1`
- Nightly parity (`scripts/nightly_parity.ps1`) passes including TEST seed-start

## Current Captured Controller Trace

`verification/corpus/communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl` imports successfully with 42 states and 41 actions. Observed-state parity verifies floor 1–3 combat (Cultist, Small Slimes, 2 Louse), Dramatic Entrance, Battle Trance path cards, multiple `END` turns, and reward screens with `unexpected_diffs=0`. Unsupported commands are classified for Neow/map/seed-start gaps only.

`verification/corpus/communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl` (CODEX03) seed-start replay covers Neow's Lament, three combats (Jaw Worm, Cultist, 2 Louse), simulation-driven rewards/map returns, and ends after floor-3 return-to-map with `unexpected_diffs=0`.

## Next Task

Continue Milestone 31 by implementing the final passive potion behavior (`Fairy`). The current M31 slice covers deterministic potion effects and discovery choices for Ancient, Attack, Skill, Colorless, Blood, Heart of Iron, Cultist, Dexterity, Energy, Essence of Steel, Explosive, Liquid Bronze, Regen, Strength, Weak, Fruit Juice, Swift, Flex, Blessing of the Forge, Power, and Speed, plus Entropic Brew potion-belt refill, Duplication Potion's next-card replay flag, Distilled Chaos top-three draw-pile play, Liquid Memories discard-pile selection, Snecko Oil draw/cost randomization, Smoke Bomb non-boss escape, and Elixir multi-card exhaust selection.

The previous M29 cleaned single-run prefix can still be structurally checked with:

```powershell
node tools\communication\trace_tools.js validate verification\corpus\communication_mod\trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl
```

Current seed-start verifier result for that prefix: all 225 actions verify with `unexpected_diffs=0`; `seed_start.expected_failure=true` only because the cleaned trace ends on the final reward screen before a post-reward `PROCEED`.

Overnight collector hardening after the `M290001` run:

- `overnight_collector.js` rejects commands whose verb is not currently listed in `available_commands`.
- repeated identical commands on unchanged state fall back conservatively (`SKIP`, `PROCEED`, `LEAVE`, or `state`) and then exit instead of spamming forever.
- stale bridge/session files make the collector exit with a clear idle reason, letting the supervisor validate the partial trace instead of waiting forever.
- `overnight_supervisor.js` runs the collector in restart loops, validates the current trace after collector exit or stale-session startup, writes a `.valid-prefix.jsonl` salvage file when a trace is missing an action response, writes a `.best-run.jsonl` extracted keeper from valid traces, updates `tools/communication/session/harvest_report.json`, logs compact harvest-quality and best-run lines, and stops with a clear stale-session/bridge-exited reason when STS or CommunicationMod needs manual recovery.
- `overnight_collector.test.js` covers the known policy regressions: full potion belt reward, repeated card reward fallback, unavailable commands, living target selection, and state-signature changes. `overnight_supervisor.test.js` covers stale-session and trace-quality formatting without requiring a live STS process.
- `trace_tools.js validate` now reports starts, seeds, room path, encounters, deaths, terminal state, elite/boss room coverage, and a simple harvest score for harvested traces. `trace_tools.js report` adds per-run summaries and best-run selection for multi-run overnight captures, and `extract-best-run` materializes the highest-scoring run as a verifier-ready single-run trace.
- `harvest_status.js` is the non-mutating status check for the latest `harvest_report.json`; it validates referenced raw, valid-prefix, and best-run artifacts without creating or rewriting trace files.
- `overnight_preflight.js` checks for stale session files, pending `next_command.txt`, bridge-exited status, and sent-command/newer-than-summary mismatches before starting an overnight supervised run.
- `run_overnight_preflight.cmd` and `run_communication_checks.cmd` provide one-command Windows entry points for preflight and communication-tool regression checks.
- `run_overnight_guarded.cmd` is the safer overnight entry point: it runs preflight and starts the supervisor only when the bridge/session is fresh.
- The overnight collector map policy scores currently visible room choices deterministically, preferring elites, fights, chests, events, shops, then rests. It intentionally does not claim route lookahead until the bridge exposes enough stable map-node context for that.
- The overnight collector combat policy now has a small survival bias: when low HP faces heavy incoming damage, defensive cards outrank basic attacks. Transient choose-capable screens with no parsed choices now poll state instead of sending `CHOOSE 0`.
- `bridge_probe.js` is the active bridge liveness check for overnight setup. It writes one temporary `state` command, verifies whether CommunicationMod consumes it, and removes the probe command on failure so stale sessions do not poison the next launch.
- `trace_client.js`, `summary.json`, and `status.json` now include `client_pid`, which exposed duplicate bridge clients during live collection. Before overnight collection there should be exactly one active bridge client consuming commands.
- `overnight_collector.js` persists a pending `START` guard so it cannot send a second seed while the previous start transition is still awaiting an in-game confirmation.
- Live collection on 2026-06-23 produced `trace-2026-06-23T07-42-06-085Z.jsonl`. The raw trace validates at completed boundaries and, as of the latest snapshot, contains 3 starts (`M290005`..`M290007`), 378 completed actions, max floor 10, 3 elite-room entries, 2 deaths, shop/rest/chest/event coverage, and an active floor-7 elite fight. `trace-2026-06-23T07-42-06-085Z.best-run.jsonl` is valid and extracts `M290006`: 105 actions, max floor 10, 1 elite, terminal death.
- Live fixes from that run: `SHOP_ROOM` now sends `PROCEED` instead of reopening the shop after `LEAVE`, and `HAND_SELECT` now chooses then confirms required card selections. These prevent the observed shop reopen loop and Warcry hand-select polling stall.
- The same 2026-06-23 raw trace later produced a stronger best-run extraction, `trace-2026-06-23T07-42-06-085Z.best-run.jsonl`, selecting `M290008` / numeric seed `40560393133`. It is structurally valid with 193 actions, max floor 16, boss room coverage, and terminal `in_progress` inside Hexaghost combat. Milestone 30 now verifies the seed-start early-Act-1 slice through step 99 with `verified=99`, `unexpected_diffs=0`, and first boundary `$.actions[step=100].command` because the verifier intentionally stops after the treasure-to-map boundary. Coverage includes the captured transform-card Neow branch (`Sentinel`), floors 1-2 combats/rewards, Scrap Ooze success, The Ssssserpent, Sword Boomerang in the floor-5 combat, captured Looter escape-to-reward, rest, and treasure. Remaining M290008 support is explicitly captured-slice scoped: broad Neow RNG, Scrap Ooze success RNG, transformed-card opening pile generalization, Sword Boomerang random targeting, Looter escape AI, and later Act 1 rooms remain future work.

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-21T09-57-10-380Z.jsonl
```

Expected result: `unexpected_diffs=0`, `seed_start.expected_failure=false`, verified labels through shop purchase/purge and Act 1 boss relic return-to-map.

## Milestone 28 Notes

Milestone 28 is complete on `trace-2026-06-21T09-57-10-380Z.jsonl`. Shop inventory, purchase, purge, and affordable choice-list refresh are source-backed through step 176. Key model pieces: `shop_card_price_rarity` (library rarity for class cards), colorless `getPrice` bases with 1.2 multiplier, Java-style int truncation on class-card merchant rolls, and `affordable_shop_picks` for `CHOOSE` mapping. Corpus: `test_seed_start_m28_shop_entry_parity` (prefix through step 168) and `test_seed_start_full_act1_boss_relic_prefix` (full trace).

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
