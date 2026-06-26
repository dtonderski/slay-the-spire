# Status

## What Exists

### Combat
- **68 Ironclad cards plus deterministic colorless slices** (Milestone 5 complete + Ascender's Bane + Dramatic Entrance + base Flame Barrier + base Limit Break + base Offering + base Armaments + base Headbutt + base Reaper + base Second Wind + base Fiend Fire + base Corruption + base Juggernaut + base Barricade + base Berserk + base Rampage + base Brutality + base Combust + base Double Tap + base Rupture + base Blood for Blood + base Evolve + base Fire Breathing + base Feed + base Exhume + base Infernal Blade + deterministic colorless uncommon/rare sweep through Sadistic Nature)
- Latest slice: Milestone 32C evidence backfill added focused Strange Spoon non-exhaust/no-roll regressions for Swift Strike and Flash of Steel, and promoted only their narrow target-jar action-surface evidence from constant-pool inspection (`SwiftStrike` -> `DamageAction`, `FlashOfSteel` -> `DamageAction`/`DrawCardAction`). Earlier 32C work added Finesse/Good Instincts no-roll regressions and broadened Sadistic Nature's supported debuff hooks so Champion Belt Weak counts as a second player-applied debuff and Hand Drill Vulnerable triggers Sadistic when block breaks. The Blind/Trip/Dark Shackles follow-up audit added focused regressions for base Blind Sadistic Weak, base Trip + Champion Belt double Sadistic damage, and Dark Shackles Strange Spoon source-exhaust routing, but left those rows `unit_only` because exact target card action queues were not newly source-verified and monster Artifact is still unmodeled. Parallel 32C slices also promoted Panacea, Apotheosis, Master of Strategy, Hand Of Greed, Secret Weapon, and Secret Technique with focused source/Strange Spoon evidence while keeping caveats scoped: Apotheosis action ordering beyond the shared played-card exhaust hook remains local/source-audit incomplete, Secret Weapon/Technique search-screen order remains local, Hand Of Greed's gold-on-kill payout is not modeled because combat state does not carry run gold, Panache remains `unit_only` pending instruction-level timing evidence, monster Artifact is still not modeled for monster debuffs, generated-card/random-selection semantics remain local deterministic approximations unless separately source-backed, and there is no broad played-card CommunicationMod trace parity claim for these cards yet.
- Full Act 1 monster + boss roster
- Ascension modifiers A0-A20 (config, elites, damage, HP, Bane, deadly enemies, double boss)

### Run / Meta
- Reward screen with source-backed card/gold/potion/relic RNG; elite/chest/boss relic reward screens from persisted pools
- Shop: full target-style inventory (7 cards, 3 relics, 3 potions, remove service) via `merchantRng`/`cardRng`/`potionRng` and relic pools; legacy fixed Anger/Vajra/Fire fixture when `merchant_rng_seed == 0`
- Potions: full 33-potion Ironclad reward pool for drops, direct use/legality coverage for implemented active potions, discovery choices, Entropic Brew refill, Duplication replay, Distilled Chaos, Liquid Memories, Snecko Oil, Smoke Bomb, Elixir, and Fairy in a Bottle passive revive
- Events: Act 1 event/shrine pools with `generateEvent` shrine chance; map event rooms call `enter_event_screen`; Shining Light costs 20% max HP and upgrades up to two random upgradeable deck cards
- Rest: heal, smith, card removal (deterministic heal amount; no RNG)

### Relics / Potions
- Common simple relic: Strawberry pickup HP bonus
- Pickup/capacity relics: Blood Vial, Pear, Mango, Old Coin, Lee's Waffle, and Potion Belt
- Start-combat relics: Lantern, Bag of Preparation, Bag of Marbles, Bronze Scales, Thread and Needle, Red Skull
- Energy relic: Coffee Dripper energy per turn and rest restriction
- Start-combat relic: Anchor block
- On-card-play relic: Ink Bottle draw after 10 cards
- Damage/block relic: Ornamental Fan block every 3 attacks per turn
- Card-play counter relics: Nunchaku, Shuriken, Kunai, and Letter Opener
- Turn-timed combat relics: Happy Flower, Orichalcum, Horn Cleat, Captain's Wheel, Mercury Hourglass, and Stone Calendar
- Combat-victory healing relics: Black Blood and Meat on the Bone
- Room/rest healing relics: Meal Ticket, Regal Pillow, Dream Catcher, and Eternal Feather
- Damage mitigation relics: Torii and Tungsten Rod
- Shop/economy relics: Ceramic Fish, Membership Card, and Smiling Mask
- Boss-entry relic: Pantograph
- Debuff-immunity relics: Ginger and Turnip
- Boss energy relic: Mark of Pain
- Combat healing relic: Magic Flower
- Vulnerable synergy relics: Paper Phrog and Champion Belt
- Elite HP relic: Preserved Insect
- Curse synergy relics: Darkstone Periapt and Du-Vu Doll
- Boss energy/rest-restriction relic: Fusion Hammer
- Boss energy/potion-lockout relic: Sozu
- Boss energy/card-reward relic: Busted Crown
- Boss energy/card-limit relic: Velvet Choker
- Potion-use healing relic: Toy Ornithopter
- Card-add upgrade relics: Molten Egg, Toxic Egg, and Frozen Egg
- Small unblocked attack damage relic: The Boot
- Power-play healing relic: Bird-Faced Urn
- No-attack-turn energy relic: Art of War
- Card reward choice relic: Question Card
- Curse-prevention relic: Omamori
- Elite-combat strength relic: Sling of Courage
- Floor-entry gold relic: Maw Bank
- Rest-site energy relic: Ancient Tea Set
- Block-retention relic: Calipers
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
- Finesse/Good Instincts source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core good_instincts -- --test-threads=1`, `cargo test -p sts_core finesse -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Latest Swift Strike / Flash of Steel no-roll source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core swift_strike -- --test-threads=1`, `cargo test -p sts_core flash_of_steel -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Target jar evidence was limited to PowerShell zip/constant-pool inspection because `jar`, `java`, and `javap` were not on PATH in this environment.
- Latest Blind/Trip/Dark Shackles Sadistic hook audit checks: `cargo fmt`, `cargo test -p sts_core blind_triggers_sadistic -- --test-threads=1`, `cargo test -p sts_core trip_champion_belt_weak_triggers_sadistic -- --test-threads=1`, `cargo test -p sts_core dark_shackles_strange_spoon -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`.
- Latest Dark Shackles slice checks: `cargo fmt`, `cargo test -p sts_core dark_shackles -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, and `cargo clippy -p sts_core` (existing 8 warnings).
- Latest Deep Breath/Impatience source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core deep_breath -- --test-threads=1`, `cargo test -p sts_core impatience -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Coverage adds focused Strange Spoon no-roll regressions for both non-exhausting source cards and promotes matrix evidence only to source-class/action/count evidence while keeping exact Deep Breath shuffle/order and Havoc/top-draw caveats local.
- Latest Enlightenment source-evidence slice checks: `cargo fmt`, `cargo test -p sts_core enlightenment -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Adds focused Strange Spoon non-exhaust/no-roll coverage, confirms base turn-only versus upgraded combat-long hand cost reduction with unit tests plus target jar constant-pool evidence for `EnlightenmentAction` cost fields, and promotes only the narrow source/action/cost surface without a played-card trace-parity claim.
- Latest Forethought slice checks: `cargo fmt`, `cargo test -p sts_core forethought -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Latest Jack Of All Trades slice checks: `cargo fmt`, `cargo test -p sts_core jack_of_all_trades -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Latest Madness slice checks: `cargo fmt`, `cargo test -p sts_core madness -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Latest Mind Blast slice checks: `cargo fmt`, `cargo test -p sts_core mind_blast -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Latest Master of Strategy slice checks: `cargo fmt`, `cargo test -p sts_core master_of_strategy -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), `cargo test -p sts_core -- --test-threads=1`, and `cargo test -p sts_verify -- --test-threads=1`.
- Previous Trip slice checks: `cargo fmt`, `cargo test -p sts_core trip -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, and `cargo clippy -p sts_core` (existing 8 warnings).
- Previous Panacea slice checks: `cargo fmt`, `cargo test -p sts_core panacea -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, `cargo clippy -p sts_core` (existing 8 warnings), and `cargo test -p sts_verify -- --test-threads=1`.
- Previous Blind slice checks: `cargo fmt`, `cargo test -p sts_core blind -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, `cargo test -p sts_core -- --test-threads=1`, and `cargo clippy -p sts_core` (existing 8 warnings).
- Previous Flash of Steel slice checks: `cargo fmt`, `cargo test -p sts_core flash_of_steel -- --test-threads=1`, `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`, and `cargo test -p sts_core -- --test-threads=1`.
- Latest Bandage Up source-evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core bandage_up -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Adds focused Strange Spoon source-exhaust regression and promotes the matrix row only for source-backed heal/action/source-exhaust evidence while retaining the no trace-parity caveat.
- Latest Dramatic Entrance source-evidence slice checks: `cargo fmt` from `simulator/`, `cargo test -p sts_core dramatic_entrance -- --test-threads=1`, and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. Adds focused event-log coverage for the `DealDamageAll` source/action surface and keeps the existing Strange Spoon source-exhaust destination/counter regression; row promotion is limited to constant-pool/source-class action evidence plus local unit coverage, with no broad played-card CommunicationMod trace parity claim.
- Latest Panache timing audit checks: `cargo test -p sts_core panache -- --test-threads=1` and `cargo test -p sts_core --test m32a_matrix -- --test-threads=1`. The safe slice stays `unit_only`: a focused regression pins the simulator's current append-after-card-effects ordering, but source-level timing remains unpromoted until `PanachePower.onUseCard`/`UseCardAction` instruction ordering or played-card trace evidence is captured.
- Nightly parity (`scripts/nightly_parity.ps1`) passes including TEST seed-start

## Current Captured Controller Trace

`verification/corpus/communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl` imports successfully with 42 states and 41 actions. Observed-state parity verifies floor 1–3 combat (Cultist, Small Slimes, 2 Louse), Dramatic Entrance, Battle Trance path cards, multiple `END` turns, and reward screens with `unexpected_diffs=0`. Unsupported commands are classified for Neow/map/seed-start gaps only.

`verification/corpus/communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl` (CODEX03) seed-start replay covers Neow's Lament, three combats (Jaw Worm, Cultist, 2 Louse), simulation-driven rewards/map returns, and ends after floor-3 return-to-map with `unexpected_diffs=0`.

## Next Task

Milestone 32A is complete. The inventory is split across `simulator/docs/content_support_matrix.md`, `simulator/docs/m32a_cards_matrix.md`, `simulator/docs/m32a_relic_potion_matrix.md`, and `simulator/docs/m32a_run_world_matrix.md`; `simulator/crates/sts_core/tests/m32a_matrix.rs` now fails when known Ironclad A0 content or named run-world surfaces are missing from the matrices.

Current milestone: Milestone 32C, parity evidence backfill for inventory-completed content. Milestone 32B's deterministic card completion sweep is complete for the known Ironclad A0/card-pool rows in `simulator/docs/m32a_cards_matrix.md`; remaining `placeholder` card rows are mechanic-test fixtures or non-A0/special curse surfaces, not unimplemented Ironclad A0 card sweep work. Latest 32C slices: source-backed target bytecode evidence now aligns Metamorphosis and Chrysalis with `MakeTempCardInDrawPileAction` destination semantics, Secret Technique rejects play when the draw pile has no Skill, Transmutation generates from the full modeled colorless pool at temporary cost 0, Violence follows source-backed temporary Attack group selection/shuffle/discard fallback behavior, and upgraded colorless uncommon/rare forms now have content IDs, upgrade mappings, and local combat deltas for Blind, Discovery, Enlightenment, Forethought, Impatience, Trip, Apotheosis, Chrysalis, Hand Of Greed, Magnetism, Master of Strategy, Metamorphosis, Panache, Sadistic Nature, The Bomb, Thinking Ahead, Panic Button, and Purity; seed-start Sword Boomerang handling is narrowed so single-living-enemy combats are not blocked by the random-target unsupported gate, while multi-enemy Sword Boomerang remains explicitly unsupported until target RNG parity is source-backed; `trace-2026-06-25T00-44-15-558Z.clean-prefix.step548.jsonl` records clean shop/reward/deck evidence for Discovery, Secret Technique, Sword Boomerang, Forethought, Chrysalis, and Panache+ before the known manual/desync window.

Next task: select high-risk implemented surfaces for parallel 32C evidence work. Good independent lanes are targeted CommunicationMod trace capture/minimization for newly implemented colorless cards, source/bytecode checks for random-selection cards, and verifier regressions for any parity bug found during backfill.

Milestone 32 is complete. Completed relic slices now cover simple pickup/capacity relics, start-of-combat relics, first-attack damage relics, first-HP-loss draw relics, card-play counter relics, turn-timed combat relics, combat-victory healing relics, room/rest healing relics, damage mitigation relics, shop/economy relics, boss-entry relics, debuff-immunity relics, boss energy relics, boss conditional-energy relics, boss energy/enemy-strength relics, boss energy/gold-lockout relics, combat-healing multiplier relics, Vulnerable synergy relics, elite HP relics, elite-combat strength relics, floor-entry gold relics, rest-site energy relics, block-retention relics, reward-screen max-HP relics, X-cost relics, curse synergy relics, boss energy/rest-restriction relics, boss energy/potion-lockout relics, potion potency relics, boss energy/card-reward relics, boss energy/card-limit relics, boss energy/randomized-draw relics, hand-retention relics, information-only relics, rest removal relics, rest strength relics, rest dig relics, debuff-cleanse relics, Strike-card damage relics, potion reward guarantee relics, pickup upgrade relics, potion-use healing relics, card-add upgrade relics, small unblocked attack damage relics, power-play healing relics, no-attack-turn energy relics, shop start-turn strength relics, card reward choice relics, card reward count relics, curse-prevention relics, shuffle-trigger relics, monster-death relics, shuffle-counter relics, exhaust-damage relics, unplayable-card relics, one-shot revive relics, previous-turn card-count relics, block-break attack relics, Buffer relics, elite bonus-reward relics, chest bonus-reward relics, pickup removal/card-reward relics, bottled-card relics, card-copy relics, random-potion pickup relics, boss pickup bundle relics, random-card-on-exhaust relics, power-play cost relics, hand-empty draw relics, persistent turn-counter relics, chest-curse boss-energy relics, event-room replacement relics, exhaust-retention relics, map-jump relics, boss pickup multi-relic queue relics, starter-transform boss relics, off-character starter/fallback no-op relics, and starter/fallback no-op relics: Blood Vial, Pear, Mango, Old Coin, Lee's Waffle, Potion Belt, Lantern, Bag of Preparation, Bag of Marbles, Bronze Scales, Thread and Needle, Red Skull, Nunchaku, Art of War, Shuriken, Kunai, Letter Opener, Happy Flower, Orichalcum, Horn Cleat, Captain's Wheel, Mercury Hourglass, Stone Calendar, Black Blood, Meat on the Bone, Meal Ticket, Regal Pillow, Dream Catcher, Eternal Feather, Torii, Tungsten Rod, Ceramic Fish, Membership Card, Smiling Mask, Maw Bank, Ancient Tea Set, Calipers, Singing Bowl, Chemical X, Philosopher's Stone, Slaver's Collar, Snecko Eye, Ectoplasm, Runic Dome, Strike Dummy, Brimstone, Akabeko, Centennial Puzzle, Pen Nib, Self-Forming Clay, Clockwork Souvenir, Runic Cube, The Abacus, Gremlin Horn, Sundial, Charon's Ashes, Blue Candle, Medical Kit, Lizard Tail, Pocketwatch, Hand Drill, Burning Blood, Circlet, Red Circlet, White Beast Statue, Whetstone, War Paint, Pantograph, Ginger, Turnip, Mark of Pain, Magic Flower, Paper Phrog, Champion Belt, Preserved Insect, Sling of Courage, Darkstone Periapt, Du-Vu Doll, Fusion Hammer, Sozu, Sacred Bark, Busted Crown, Velvet Choker, Runic Pyramid, Frozen Eye, Peace Pipe, Girya, Orange Pellets, Toy Ornithopter, Molten Egg, Toxic Egg, Frozen Egg, The Boot, Bird-Faced Urn, Question Card, Prayer Wheel, Cracked Core, Frozen Core, Pure Water, Holy Water, Ring of the Snake, Ring of the Serpent, Omamori, Unceasing Top, Shovel, Fossilized Helix, Black Star, Matryoshka, Empty Cage, Bottled Flame, Bottled Lightning, Bottled Tornado, Dolly's Mirror, Orrery, Cauldron, Tiny House, Dead Branch, Mummified Hand, Strange Spoon, Wing Boots, Calling Bell, Pandora's Box, Astrolabe, Juzu Bracelet, Prismatic Shard, The Courier's source-backed shop discount, purge-cost, and card/relic/potion restock hooks, Incense Burner's persistent sixth-turn Intangible hook, Cursed Key's pickup energy plus non-boss chest curse hook, Tiny Chest's persistent fourth-`?` treasure replacement hook, Snecko Eye's pickup energy plus `cardRandomRng` opening/turn draw cost-randomization hooks, Strange Spoon's source-backed `cardRandomRng.randomBoolean()` played-card exhaust-to-discard hook, Wing Boots' three-charge same-next-floor map jump hook, Calling Bell's source-backed Curse of the Bell confirmation grid plus common/uncommon/rare screenless relic reward queue, and Pandora's Box's source-backed starter Strike/Defend removal plus `cardRandomRng` replacement confirmation grid, Astrolabe's source-backed three-card `miscRng` transform/auto-upgrade grid, Gambling Chip's start-of-combat multi-discard/redraw selection hook, Toolbox's start-of-combat colorless-card choice hook, Juzu Bracelet's source-backed `?` room monster-outcome conversion with persistent event-room chance counters, and Prismatic Shard's source-backed combat reward any-color card pool hook with the extra `cardRng.randomLong()` per pick. Acceptance evidence: all modeled relic keys promote without key-only placeholders, focused relic tests pass, relic counters round-trip through run/combat state, and the relic-heavy corpus traces pass seed-start verification.

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
