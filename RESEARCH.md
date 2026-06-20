# Research Notes

This file records prior art and evidence gathered before implementation. It is intentionally separate from `DESIGN.md` so future coding sessions can revisit sources without bloating the architecture document.

## Key Prior Art

### gamerpuppy/sts_lightspeed

Repository: <https://github.com/gamerpuppy/sts_lightspeed>

Why it matters:

- Closest known prior art for this project.
- C++17 standalone Slay the Spire simulator and tree-search engine.
- README claims it is designed to be "100% RNG accurate".
- README reports speed of about 1M random playouts in 5s with 16 threads.
- README claims implementation progress includes all enemies, all relics, all Ironclad cards, all colorless cards, and everything outside combat/all acts.
- Supports loading from save files, with README caveat that loading into combat was currently the supported path.
- Exposes Python bindings through `pybind11`.

Architecture observations from source inspection:

- Has explicit `GameContext` and `BattleContext` split.
- Uses many named RNG streams: `aiRng`, `cardRandomRng`, `cardRng`, `eventRng`, `mathUtilRng`, `merchantRng`, `miscRng`, `monsterHpRng`, `monsterRng`, `neowRng`, `potionRng`, `relicRng`, `shuffleRng`, `treasureRng`.
- Implements Slay the Spire/libGDX-style xorshift RNG and Java `Random`/`Collections.shuffle` compatibility.
- Save loading restores RNG streams from seed counters such as `potion_seed_count`, `relic_seed_count`, `event_seed_count`, `monster_seed_count`, `merchant_seed_count`, `card_random_seed_count`, `card_seed_count`, and `treasure_seed_count`.
- Combat uses explicit `ActionQueue` and `CardQueue`, with `addToTop`/`addToBot` ordering. This confirms action queue fidelity is not optional for full parity.
- Code comments include uncertainty and game-specific quirks, for example time-based `mathUtilRng`, combat-victory queue clearing, and actions that consume RNG only to keep parity.

Design lessons:

- We should not pretend to be first. `sts_lightspeed` should be studied carefully before implementing RNG, save loading, map generation, reward generation, and action queues.
- We should not blindly port it. The Rust project needs stronger snapshot/replay tests, canonical diffs, and a smaller task discipline because the user wants safe vibe-coded implementation.
- The RNG stream list in `DESIGN.md` should be upgraded from speculative examples to a known reference list, while still requiring verification against the target game version.
- A future task should create `docs/prior_art/sts_lightspeed.md` or equivalent with exact behavior notes before implementing parity-sensitive systems.

### silentcoder99/sts_lightspeed

Repository: <https://github.com/silentcoder99/sts_lightspeed>

Why it matters:

- GitHub description says it is a fork of gamerpuppy's headless implementation plus MCTS agent with CommunicationMod integration.
- This may be useful for differential verification, especially if it contains practical bridges between real-game state and the simulator.

Design lessons:

- Before writing our own CommunicationMod verifier, inspect this fork for integration patterns.
- Do not assume the fork is more correct than upstream; use it to identify useful trace/control workflows.

### lhy-loveworld/rusted-spire

Repository: <https://github.com/lhy-loveworld/rusted-spire>

Why it matters:

- Rust headless Slay the Spire combat simulator for reinforcement learning.
- As of the inspected README/PLAN, it implements a scope similar to our proposed first combat milestone: Strike, Defend, Bash, Jaw Worm, combat loop, damage pipeline, named RNG streams, and planned PyO3 bindings.
- It explicitly says exact pixel-perfect RNG match with the Java game is deferred/out of scope for now.
- Its README says it intentionally removes the original game's animation-driven action queue and executes effects immediately.

Design lessons:

- Useful cautionary comparison: good for RL-MVP ergonomics, not sufficient as a fidelity model.
- Our design should differ by keeping action queue semantics from the beginning, even if milestone 1 uses only a tiny subset.
- The task plan should mention `rusted-spire` as an existence proof for the minimal Rust combat MVP, but should not copy its "no action queue" choice because this project prioritizes future parity.

### utilForever/conquer-the-spire

Repository: <https://github.com/utilForever/conquer-the-spire>

Why it matters:

- C++ Slay the Spire simulator with reinforcement-learning ambitions.
- Older project, last pushed in 2020 during the inspected metadata.
- Useful mostly as historical RL/simulator prior art, not as a direct parity reference unless its internals prove otherwise.

Design lessons:

- Simulator/RL coupling can age poorly. Keep core simulator, verification, and RL wrappers separate.

### ForgottenArbiter/CommunicationMod

Repository: <https://github.com/ForgottenArbiter/CommunicationMod>

Why it matters:

- Slay the Spire mod that launches an external process and communicates over stdin/stdout.
- Sends JSON state whenever the game reaches a stable state.
- Accepts commands such as `play`, `end`, `key`, `click`, `wait`, and `state`.
- Requires ModTheSpire and BaseMod.

Design lessons:

- Primary real-game parity harness.
- Its state is observable game state, not necessarily full hidden state or exact RNG stream positions.
- The simulator snapshot schema should be designed to normalize CommunicationMod JSON while still retaining hidden simulator fields.

### ForgottenArbiter/spirecomm

Repository: <https://github.com/ForgottenArbiter/spirecomm>

Why it matters:

- Python package for interfacing with CommunicationMod plus a simple AI.
- Useful for protocol/client patterns.

Design lessons:

- Good source for action/state schema examples.
- Not a simulator architecture model.

### xaved88/bottled_ai

Repository: <https://github.com/xaved88/bottled_ai>

Why it matters:

- Actively developed Python bot for Slay the Spire using manually constructed strategies.
- README describes combat search over possible hand play orders with a custom simulation/evaluation.
- README states it does not access secret information such as future random rolls or draws.

Design lessons:

- Useful for separating "planner using visible state" from "simulator with hidden state".
- RL/planning APIs should support visible-observation mode, not just omniscient simulator state.

### elidupree/borg_the_spire

Repository: <https://github.com/elidupree/borg_the_spire>

Why it matters:

- Rust helper/AI built on CommunicationMod.
- Splits live communication from CPU-heavy analysis through a saved state file and browser UI.

Design lessons:

- Useful pattern for verifier tooling: capture real-game states to files, then run heavy diff/analysis separately.

### kronion/gym-sts

Repository: <https://github.com/kronion/gym-sts>

Why it matters:

- OpenAI Gym environment that runs the real game with ModTheSpire, BaseMod, CommunicationMod, and SuperFastMode.
- Can run the game headless in Docker.

Design lessons:

- Useful baseline for real-game-backed RL or verification.
- Too slow/heavy for many simulator rollouts, but valuable for trace generation and parity smoke tests.

### MaT1g3R/Slay-the-Spire-data

Repository: <https://github.com/MaT1g3R/Slay-the-Spire-data>

Why it matters:

- Run-history datasets for streamers, including an Ironclad sample.
- README says it can analyze local run-history folders and recommends Run History Plus.

Design lessons:

- Good for distribution checks, deck/path/reward outcome corpora, and high-level regression examples.
- Not enough for exact transition parity because run histories generally lack per-action hand/draw/discard/action-queue/RNG data.

### modargo/RunHistoryPlus

Repository: <https://github.com/modargo/RunHistoryPlus>

Why it matters:

- Slay the Spire mod for richer run histories.
- Used or recommended by run-history analysis tooling.

Design lessons:

- Investigate for coarse corpus generation.
- Do not treat it as a substitute for CommunicationMod traces.

## Datasets and Papers

### Analysis of Uncertainty in Procedural Maps in Slay the Spire

Paper: <https://arxiv.org/abs/2504.03918>

Why it matters:

- Uses a dataset of 20,000 Slay the Spire runs to analyze path uncertainty and outcomes.
- Confirms that run-history-scale data exists and can support distribution-level analysis.

Design lessons:

- Useful for high-level evaluation later.
- Does not solve transition-level simulator verification.

### Rule Synergy Analysis using LLMs

Paper: <https://arxiv.org/abs/2508.19484>

Why it matters:

- Uses Slay the Spire card synergy/rule interactions as a benchmark domain.
- Reports that models struggle with timing, state definition, and rule interactions.

Design lessons:

- Reinforces that action ordering and explicit state modeling are core risks.
- Future vibe-coded mechanics should require design notes and interaction tests.

## Practical Conclusions

1. `sts_lightspeed` is mandatory prior art for RNG, action queue, save loading, and content coverage.
2. A Rust rewrite is still justified if the goal is maintainable incremental correctness, strong tests, deterministic replay artifacts, and clean RL bindings.
3. Full game fidelity should be staged and evidence-driven. Existing projects make it plausible, not free.
4. CommunicationMod remains the real-game authority for observed-state parity.
5. Save files may expose RNG counters that CommunicationMod states do not. Save-file import should become an earlier verification tool than originally planned.
6. The simulator should support both omniscient state for replay/debug and visible state for agents that should not exploit hidden information.
7. Immediate-effect combat engines are attractive for RL speed but are the wrong default for long-term Slay the Spire parity.

## Save-File/RNG Research Gate

Task 2.4 finding:

- The existing 0.0 notes identify these real save-file RNG counter fields as parity-relevant: `potion_seed_count`, `relic_seed_count`, `event_seed_count`, `monster_seed_count`, `merchant_seed_count`, `card_random_seed_count`, `card_seed_count`, and `treasure_seed_count`.
- These fields matter because mid-run replay needs per-stream advancement counters, not just base seeds.
- The 0.0 `sts_lightspeed` notes say its save loading restores RNG streams from those same seed counters, matching the field list above.
- The public `gamerpuppy/sts_lightspeed` README describes the project as RNG-accurate, save-file loading capable, and able to search while knowing the game's RNG state. That supports treating its save/RNG handling as high-priority prior art, but it is not source-level proof of the exact field mapping.
- Decision: save import should move earlier than broad map/reward/shop RNG parity work, but after local snapshot/replay and RNG stream structure are stable.
- Gate before implementation: inspect exact `sts_lightspeed` save-loading source files and real decrypted save examples. Record source file/function names and confirm whether each listed counter maps to a named simulator RNG stream.

Current limitation:

- No full save importer, decryption tooling, or broad RNG parity claim exists in this repo.

## Milestone 20 Seed Conversion Audit

- Target binary inspected: `D:\SteamLibrary\steamapps\common\SlayTheSpire\desktop-1.0.jar`, game version shown by ModTheSpire as `12-18-2022`.
- Class inspected locally: `com/megacrit/cardcrawl/helpers/SeedHelper.class`.
- Reverse-engineering method: inspect the target game jar directly, extract the compiled helper class, disassemble the bytecode, and translate only the small `SeedHelper` methods needed for verification. The local JRE bundled with STS did not include `javap`, so the class was read from the jar and decoded with a small local class-file/bytecode inspection script instead of relying on guessed base conversion.
- `SeedHelper.getLong(String)` uppercases the seed string, maps `O` to `0`, then parses each character with alphabet `0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ`.
- The alphabet length is 35 because `O` is omitted. The numeric seed is accumulated as `value = value * 35 + alphabet_index(character)`.
- Captured seed checks:
  - `VERIFY01` -> `1957307888551`
  - `CODEX03` -> `22079335078`
  - `CODEX04` -> `22079335079`
- `SeedHelper.getString(long)` performs the inverse conversion using unsigned long text and the same alphabet.
- `SeedHelper.sterilizeString(String)` trims, uppercases, accepts only letters/digits via `([A-Z]*[0-9]*)*`, and maps `O` to `0`.

RNG stream/counter audit status:

- Confirmed from prior notes / save fields: `potion_seed_count`, `relic_seed_count`, `event_seed_count`, `monster_seed_count`, `merchant_seed_count`, `card_random_seed_count`, `card_seed_count`, and `treasure_seed_count`.
- Current simulator/verifier stream labels: `potionRng`, `relicRng`, `neowRng`, `mapRng`, `monsterRng`, `monsterHpRng`, `shuffleRng`, `cardRewardRng`, and `rewardGoldRng`.
- Known mapping still requiring implementation evidence in future milestones:
  - monster encounter and monster HP use `monster_seed_count`
  - card draw/shuffle uses `card_random_seed_count`
  - card reward selection uses `card_seed_count`
  - relic rewards use `relic_seed_count`
  - potion rewards/use effects use `potion_seed_count`
  - event selection uses `event_seed_count`
  - shop/merchant generation uses `merchant_seed_count`
  - treasure/relic chest outcomes use `treasure_seed_count`

## Milestone 22 Map / Encounter Evidence

Target binary inspected: `D:\SteamLibrary\steamapps\common\SlayTheSpire\desktop-1.0.jar`, game version shown by ModTheSpire as `12-18-2022`.

Relevant target classes found in the jar:

- `com.megacrit.cardcrawl.dungeons.AbstractDungeon`
- `com.megacrit.cardcrawl.dungeons.Exordium`
- `com.megacrit.cardcrawl.map.MapGenerator`
- `com.megacrit.cardcrawl.map.MapRoomNode`
- `com.megacrit.cardcrawl.helpers.MonsterHelper`
- `com.megacrit.cardcrawl.monsters.MonsterInfo`
- Act 1 constructors including `Cultist`, `AcidSlime_M`, `SpikeSlime_S`, `LouseNormal`, and `LouseDefensive`

Bytecode metadata inspection, using the same local class-file parser approach as the seed audit, confirms these source-backed entry points:

- `Exordium` initializes `mapRng` as `new Random(Settings.seed + AbstractDungeon.actNum)`, so Act 1 map topology uses `seed + 1`.
- `AbstractDungeon.generateMap()` sets target map dimensions to 15 rows, 7 columns, and 6 paths, references `AbstractDungeon.mapRng`, calls `MapGenerator.generateDungeon(IIILcom/megacrit/cardcrawl/random/Random;)`, computes desired non-combat room counts, assigns fixed rows, then calls `RoomTypeAssigner.distributeRoomsAcrossMap(...)`.
- `Exordium.initializeLevelSpecificChances()` sets discretionary map room chances to shop `0.05`, rest `0.12`, treasure `0.0`, event `0.22`, and elite `0.08`.
- `AbstractDungeon.generateRoomTypes(roomList, count)` computes desired room counts with `Math.round(count * chance)` for shop, rest, treasure, elite, and event rooms. It adds shops, rests, elites, and events to the room list; treasure is counted but not appended by this target method, and monster rooms are not added until `RoomTypeAssigner.distributeRoomsAcrossMap(...)`.
- `MapGenerator.generateDungeon` creates the node grid, calls `createPaths`, then calls `filterRedundantEdgesFromRow` before room assignment.
- `MapGenerator.randRange(Random,min,max)` calls `rng.random(max - min) + min`, which is inclusive because target `Random.random(int)` is inclusive.
- `Exordium.generateMonsters()` calls `generateWeakEnemies`, `generateStrongEnemies`, and `generateElites`; its constants include Act 1 encounter keys such as `Cultist`, `2 Louse`, `Small Slimes`, `Large Slime`, `Lots of Slimes`, `Exordium Thugs`, `Exordium Wildlife`, and `3 Louse`.
- `Exordium` references `AbstractDungeon.monsterRng` during encounter list population / selection.
- `Exordium.generateWeakEnemies(3)` builds the weak pool as `Cultist`, `Jaw Worm`, `2 Louse`, and `Small Slimes`, each with weight `2.0`.
- `Exordium.generateStrongEnemies(12)` builds the strong pool as `Blue Slaver` 2.0, `Gremlin Gang` 1.0, `Looter` 2.0, `Large Slime` 2.0, `Lots of Slimes` 1.0, `Exordium Thugs` 1.5, `Exordium Wildlife` 1.5, `Red Slaver` 1.0, `3 Louse` 2.0, and `2 Fungi Beasts` 2.0. It normalizes weights, calls `populateFirstStrongEnemy(pool, generateExclusions())`, then appends 12 more entries with `populateMonsterList(pool, 12, false)`.
- `MonsterInfo.normalizeWeights` sorts by weight and divides by the total weight; for the weak pool this preserves the listed order and produces four `0.25` intervals.
- `MonsterInfo.roll` returns the first entry whose cumulative normalized weight is greater than the `monsterRng.random()` float.
- `AbstractDungeon.populateMonsterList(..., false)` rejects a rolled encounter if it matches the previous or two-back entry, then retries the same list slot.
- `AbstractDungeon.populateFirstStrongEnemy` repeatedly rolls from the normalized strong pool until the result is not in the exclusions list. `Exordium.generateExclusions()` excludes `Exordium Thugs` after `Looter`; `Red Slaver` and `Exordium Thugs` after `Blue Slaver`; `3 Louse` after `2 Louse`; and `Large Slime` plus `Lots of Slimes` after `Small Slimes`.
- `LouseNormal` and `LouseDefensive` constructors reference `AbstractDungeon.monsterHpRng.random(II)` for HP rolls.
- `Cultist`, `AcidSlime_M`, and `SpikeSlime_S` constructors expose target-version HP constants and `setHp` calls; exact numeric constants still need full bytecode instruction decoding before replacing captured HP fixtures.
- `AbstractDungeon.nextRoomTransition` reinitializes `monsterHpRng`, `aiRng`, `shuffleRng`, `cardRandomRng`, and `miscRng` as `new Random(Settings.seed + floorNum)` after incrementing `floorNum` for the entered room.
- Ironclad starter combat shuffle validation: the game shuffles a fixed starter instance order `[4, 9, 6, 5, 10, 3, 1, 2, 8, 7]` (CardId indices for the default 5/4/1 starter deck), not CommunicationMod deck export order or naive strike/defend grouping. `shuffleRng(seed + floor)` plus raw `Collections.shuffle` reproduces VERIFY01 opening hand/draw piles. Innate cards are excluded from the shuffle pool and prepended to the opening hand. CommunicationMod lists draw/discard piles bottom-first; the simulator draws from the pile top (last array entry). CODEX04 with Neow `Dramatic Entrance` still uses trace-pinned opening piles until extra-card master-deck ordering is decoded.

Target RNG wrapper evidence:

- `com.megacrit.cardcrawl.random.Random` wraps `com.badlogic.gdx.math.RandomXS128`.
- `Random(Long)` constructs `RandomXS128(seed.longValue())` and starts `counter` at 0.
- `Random(Long, int)` constructs the same RNG, then advances by calling `random(999)` once per saved counter step.
- Every public draw inspected increments `counter` by 1.
- `random(int max)` calls `RandomXS128.nextInt(max + 1)`, so its integer max is inclusive.
- `random(int min, int max)` calls `min + RandomXS128.nextInt(max - min + 1)`, so both integer bounds are inclusive.
- `RandomXS128.setSeed(long)` maps seed 0 to `Long.MIN_VALUE`, applies `murmurHash3` to produce `seed0`, and applies `murmurHash3(seed0)` to produce `seed1`.
- `RandomXS128.nextLong()` uses xorshift128+ state transition: `s1 = seed0`, `s0 = seed1`, `seed0 = s0`, `s1 ^= s1 << 23`, `seed1 = s1 ^ s0 ^ (s1 >>> 17) ^ (s0 >>> 26)`, return `seed1 + s0`.
- The simulator's `StsRng` now encodes this wrapper separately from the older placeholder `SimulatorRng`.
- `AbstractMonster.setHp(min,max)` calls `AbstractDungeon.monsterHpRng.random(min,max)`, so monster HP ranges are inclusive.
- Decoded target-version ranges now represented in `sts_core`: Cultist A0 `48..54` / A7 `50..56`, Spike Slime (S) A0 `10..14` / A7 `11..15`, Acid Slime (M) A0 `28..32` / A7 `29..34`, red louse A0 `10..15` / A7 `11..16`, green louse A0 `11..17` / A7 `12..18`, and louse bite damage A0 `5..7` / A2+ `6..8`.
- Source-backed map topology validation: translated `MapGenerator` topology with Act 1 `seed + 1` map RNG produces `VERIFY01` first choices `x=1`, `x=2`, `CODEX04` first choices `x=0`, `x=2`, `x=4`, `x=5`, and CODEX04 chosen-path next choices `x=3` then `x=2`, `x=3`.
- Full captured topology/edge validation: comparing the first CommunicationMod map payloads for `VERIFY01` and `CODEX04` against `generate_exordium_map_topology(...)` now matches every visible node coordinate and outgoing child coordinate. The translated generator deduplicates edges by destination, matching the target map payload and avoiding duplicate children in projected maps.
- Source-backed fixed room row validation: `AbstractDungeon.generateMap()` assigns row 14 to `RestRoom`, row 0 to `MonsterRoom`, and row 8 to `TreasureRoom` before distributing the rest of the rooms. Endless `MimicInfestation` would replace row 8 with `MonsterRoomElite`, but the captured traces are normal non-endless runs.
- Source-backed discretionary room count validation: using the target `hasEdges && y != map.size() - 2` count passed to `generateRoomTypes` and Exordium chances, `VERIFY01` produces desired shop/rest/treasure/elite/event counts `3/6/0/4/12`; `CODEX04` produces `3/7/0/5/13`. After fixed rows 0, 8, and 14 are assigned, `RoomTypeAssigner.distributeRoomsAcrossMap` tops the list up with monster rooms to the remaining unassigned connected-node count, producing 23 shuffled monster-room entries for both captured seeds.
- Source-backed pre-shuffle room-list validation: the simulator now mirrors target order and timing before `Collections.shuffle`, giving `VERIFY01` 3 shops, 6 rests, 4 elites, 12 events, then 23 combats, and `CODEX04` 3 shops, 7 rests, 5 elites, 13 events, then 23 combats.
- Source-backed room-list shuffle validation: `RoomTypeAssigner.distributeRoomsAcrossMap` passes the underlying `mapRng.random` `RandomXS128` directly to `Collections.shuffle`, so the shuffle advances raw RNG state without incrementing the STS wrapper counter. CODEX04's shuffled room-list prefix now pins `Combat`, `Combat`, `Combat`, `Combat`, `Combat`, `Elite`, `Combat`, `Event`, `Combat`, `Combat`, `Combat`, `Event` while `map_rng_counter` remains 95.
- Captured room-placement validation: `RoomTypeAssigner` bytecode confirms left-to-right row iteration, parent/sibling duplicate-room checks, row bans for Rest/Elite on rows 0-4, and row bans for Rest on rows 13+. Translating those rules with target two-stage room-list construction reproduces the full VERIFY01 and CODEX04 captured maps: every visible node coordinate, room symbol, and outgoing child coordinate matches the first CommunicationMod map payloads.
- Chosen-node execution validation: the source-backed topology and captured room placement now project into the simulator's `FixedMap` / `MapRunState` API. For CODEX04, legal first choices are x=0/x=2/x=4/x=5; choosing row 0 x=2 reaches a combat node with next choice row 1 x=3; choosing row 1 x=3 reaches another combat node with next choices row 2 x=2 and row 2 x=3.
- Source-backed normal encounter list validation: zero-counter `monsterRng` produces `VERIFY01` weak prefix `Cultist`, `Jaw Worm`, `2 Louse`, and `CODEX04` weak prefix `Cultist`, `Small Slimes`, `2 Louse`, then continues into the decoded strong-list generation with first-strong exclusions and no-repeat-last-two retries.
- First Cultist HP validation: floor-1 `monsterHpRng` uses `seed + 1`, so the decoded Cultist A0 range rolls 49 for `VERIFY01` and 54 for `CODEX04`, matching both captured first encounters. Plain zero-counter `StsRng(CODEX04)` with no floor offset rolls 53, which is why the earlier captured value looked like a counter gap.
- Floor-2 Small Slimes HP validation: `MonsterHelper.spawnSmallSlimes()` branches on floor-2 `miscRng.randomBoolean()`. For `CODEX04`, the reached branch is `SpikeSlime_S` then `AcidSlime_M`; floor-2 `monsterHpRng = seed + 2` rolls 11 and 32 through the decoded ranges, matching the captured second encounter.
- Floor-3 louse HP validation: `MonsterHelper.getLouse()` branches on `miscRng.randomBoolean()` for each louse. For `CODEX04`, floor-3 `miscRng = seed + 3` chooses two defensive/green louses. Each louse constructor rolls HP and then bite damage from the same `monsterHpRng`; with that interleaving, floor-3 `monsterHpRng = seed + 3` produces observed HP 13 and 15.

Captured CODEX04 executable targets now pinned in `sts_verify` corpus tests:

- First map screen after Neow: choices `x=0`, `x=2`, `x=4`, `x=5`.
- Floor 1 encounter: `Cultist` 54/54.
- Post-floor-1 map choices: `x=3`.
- Floor 2 encounter: `Spike Slime (S)` 11/11 and `Acid Slime (M)` 32/32.
- Post-floor-2 map choices: `x=2`, `x=3`.
- Floor 3 encounter: `Louse` 13/13 and `Louse` 15/15.

Next implementation evidence needed:

- Extend map parity from full VERIFY01/CODEX04 topology/edge/room-symbol parity, fixed rows, desired room counts, shuffled room-list order, and `FixedMap` traversal into later reachable node choices.
- Extend source-backed encounter selection from decoded normal-list generation to room execution, elite generation, alternate unreached group constructors, and the full first-three-fight seed-start path.
- Extend floor-offset `monsterHpRng` validation to alternate Small Slimes/louse branches and additional captured seeds.

## Milestone 24 Potion Reward Evidence

Source inspected: `%TEMP%\sts_lightspeed\src\game\GameContext.cpp`, `%TEMP%\sts_lightspeed\src\game\Game.cpp`, and `%TEMP%\sts_lightspeed\include\constants\Potions.h`.

Normal reward potion drops are driven by `GameContext::addPotionRewards`: start from `chance = 40 + potionChance`, force `chance = 0` once the reward screen already has four rewards, roll `potionRng.random(99)`, add 10 to `potionChance` on miss, and subtract 10 on hit. A hit calls `returnRandomPotion(potionRng, cc)`.

`returnRandomPotion` rolls rarity with `potionRng.random(0, 99)`: `<65` common, `<90` uncommon, otherwise rare. It then repeatedly calls `getRandomPotion`, which rolls an index with `potionRng.random(PotionPool::poolSize - 1)`, until the selected potion has the requested rarity. The Ironclad pool has 33 entries in target order, beginning `BloodPotion`, `ElixirPotion`, `HeartOfIron`, `Block Potion`, `Dexterity Potion`, `Energy Potion`, `Explosive Potion`, `Fire Potion`.

## Milestone 24 Relic Reward Evidence

Source inspected: `%TEMP%\sts_lightspeed\src\game\Game.cpp`, `%TEMP%\sts_lightspeed\src\game\GameContext.cpp`, and `%TEMP%\sts_lightspeed\include\constants\RelicPools.h`.

`returnRandomRelicTier(relicRng, act)` rolls `relicRng.random(0, 99)`. Acts 1-3 use 50% common, 33% uncommon, and 17% rare; Act 4 uses 0% common and 100% uncommon. `returnRandomRelicTierElite(relicRng)` rolls `relicRng.random(99)` with `<50` common, `>82` rare, otherwise uncommon.

Normal monster `createCombatReward()` does not add any relic. Elite rewards call `returnRandomRelic(returnRandomRelicTierElite(relicRng))`. Relic pool selection itself is more involved: `initRelics()` fills class-specific common/uncommon/rare/shop/boss pools and shuffles each with Java `Collections.shuffle` using `java::Random(relicRng.nextLong())`; `returnRandomRelic` then pops from the front for normal rewards, with fallbacks for empty pools and spawn filters.
