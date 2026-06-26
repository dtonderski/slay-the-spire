# Task Breakdown

Rules for this file:

- Tasks are ordered.
- Implement one task per coding session.
- Each task must be small, testable, and update `STATUS.md`.
- Do not implement future mechanics early.
- If a task turns out too large, split it before coding.

## Milestone 0: Project Skeleton and Test Harness

### 0.0 Prior-Art Deep-Dive Notes

Scope:

- Create notes from `sts_lightspeed`, `rusted-spire`, CommunicationMod, and any relevant run-history tooling.
- Record exact files/functions inspected for RNG, action queue, save loading, and minimal Rust combat architecture.
- Decide which prior-art behavior needs tests before implementation.

Files likely touched:

- `RESEARCH.md`
- `STATUS.md`
- optional `docs/prior_art/sts_lightspeed.md`

Acceptance tests:

- No code tests required.
- Notes include at least RNG streams, save-file counters, action queue/card queue, and first milestone implications.

Do not implement:

- Rust workspace
- simulator code
- copied prior-art code
- parity claims

### 0.1 Create Rust Workspace

Scope:

- Add `simulator/Cargo.toml`.
- Add `simulator/crates/sts_core`.
- Add empty library with no simulator mechanics.
- Keep prior-art notes separate from source code.

Files likely touched:

- `simulator/Cargo.toml`
- `simulator/crates/sts_core/Cargo.toml`
- `simulator/crates/sts_core/src/lib.rs`
- `STATUS.md`

Acceptance tests:

- `cargo fmt` from `simulator/`
- `cargo clippy` from `simulator/`
- `cargo test` from `simulator/`

Do not implement:

- cards
- combat
- RNG
- RL API

### 0.2 Add Basic Types for IDs and Errors

Scope:

- Add typed IDs for cards, monsters, actions, and content.
- Add structured simulator error enum.

Files likely touched:

- `simulator/crates/sts_core/src/lib.rs`
- `simulator/crates/sts_core/src/ids.rs`
- `simulator/crates/sts_core/src/error.rs`
- `STATUS.md`

Acceptance tests:

- IDs serialize/deserialize if serde is added.
- Errors are printable and comparable enough for tests.

Do not implement:

- game state
- transitions
- content definitions

### 0.3 Add Snapshot Hash Placeholder

Scope:

- Add a minimal snapshot wrapper and deterministic hash helper over canonical serialized data.

Files likely touched:

- `simulator/crates/sts_core/src/snapshot.rs`
- `simulator/crates/sts_core/src/lib.rs`
- `STATUS.md`

Acceptance tests:

- same snapshot hashes identically
- field order does not drift
- snapshot round trip preserves hash

Do not implement:

- real game state fields beyond a placeholder schema/version

## Milestone 1: Ironclad Starter Cards vs One Simple Monster

This milestone is intentionally tiny. It is not full combat. It proves the loop for state, legal actions, transitions, tests, snapshots, and replay.

### 1.1 Define Minimal Combat State

Scope:

- Add player HP, block, energy.
- Add one monster with HP, block, and alive flag.
- Add hand, draw pile, discard pile, exhaust pile as card instance lists.
- Add phase: waiting for player, monster turn, won, lost.

Files likely touched:

- `simulator/crates/sts_core/src/combat/state.rs`
- `simulator/crates/sts_core/src/card.rs`
- `simulator/crates/sts_core/src/lib.rs`
- `STATUS.md`

Acceptance tests:

- initial fixture serializes
- snapshot round trip preserves state hash
- card instances cannot appear in two piles in the fixture

Do not implement:

- shuffling
- action queue
- powers
- relics
- rewards

### 1.2 Add Starter Card Definitions: Strike, Defend, Bash

Scope:

- Define static content for Strike_R, Defend_R, Bash.
- Include cost, target requirement, card type, base values.

Files likely touched:

- `simulator/crates/sts_core/src/content/cards.rs`
- `simulator/crates/sts_core/src/card.rs`
- `STATUS.md`

Acceptance tests:

- Strike costs 1, targets enemy, deals 6.
- Defend costs 1, no target, gives 5 block.
- Bash costs 2, targets enemy, deals 8 and applies 2 Vulnerable.

Do not implement:

- upgraded cards
- all Ironclad cards
- card rewards

### 1.3 Generate Legal Combat Actions

Scope:

- Legal `PlayCard` actions for playable cards in hand.
- Legal `EndTurn`.
- Reject targeted cards without targets and non-targeted cards with targets.
- Reject unaffordable cards.

Files likely touched:

- `simulator/crates/sts_core/src/action.rs`
- `simulator/crates/sts_core/src/combat/legal.rs`
- `STATUS.md`

Acceptance tests:

- Strike legal with living monster target.
- Defend legal without target.
- Bash illegal at 1 energy.
- dead monster cannot be targeted.
- legal action generation does not mutate state hash.

Do not implement:

- potions
- rewards
- map actions
- card-specific complex predicates

### 1.4 Apply Strike

Scope:

- Spend energy.
- Move Strike from hand to discard.
- Deal 6 unmodified damage to monster HP through block.
- End in waiting-for-player or won state.

Files likely touched:

- `simulator/crates/sts_core/src/combat/transition.rs`
- `simulator/crates/sts_core/src/combat/damage.rs`
- `STATUS.md`

Acceptance tests:

- monster HP decreases by 6.
- energy decreases by 1.
- card moves hand -> discard.
- invalid target returns error and preserves state.

Do not implement:

- strength
- vulnerable
- weak
- multi-hit

### 1.5 Apply Defend

Scope:

- Spend energy.
- Move Defend from hand to discard.
- Add 5 player block.

Files likely touched:

- `simulator/crates/sts_core/src/combat/transition.rs`
- `STATUS.md`

Acceptance tests:

- player block increases by 5.
- energy decreases by 1.
- card moves hand -> discard.

Do not implement:

- dexterity
- frail
- barricade

### 1.6 Apply Bash and Minimal Vulnerable

Scope:

- Spend energy.
- Move Bash from hand to discard.
- Deal 8 damage.
- Add Vulnerable amount 2 to monster state.

Files likely touched:

- `simulator/crates/sts_core/src/power.rs`
- `simulator/crates/sts_core/src/combat/transition.rs`
- `STATUS.md`

Acceptance tests:

- monster HP decreases by 8.
- monster has Vulnerable(2).
- Bash illegal with less than 2 energy.

Do not implement:

- Vulnerable damage multiplier
- Vulnerable turn decrement
- Artifact
- upgraded Bash

### 1.7 End Turn Against Fixed Simple Monster

Scope:

- Add a fixed monster that attacks for a constant amount.
- End turn discards remaining hand.
- Monster damage consumes block then HP.
- Player block clears at appropriate point for the simplified milestone.
- Draw next hand from a fixed draw pile without shuffle.

Files likely touched:

- `simulator/crates/sts_core/src/content/monsters.rs`
- `simulator/crates/sts_core/src/combat/turn.rs`
- `STATUS.md`

Acceptance tests:

- EndTurn is legal.
- remaining hand moves to discard.
- monster attack reduces block before HP.
- next hand is drawn deterministically.
- combat can reach lost state.

Do not implement:

- real Cultist
- monster intent RNG
- start/end-turn powers
- shuffle discard into draw pile

### 1.8 End-to-End Milestone 1 Golden Replay

Scope:

- Add one fixture with starter cards and fixed monster.
- Add action trace that wins combat.
- Add deterministic replay from initial state and from mid-combat snapshot.

Files likely touched:

- `simulator/crates/sts_core/tests/milestone1.rs`
- `verification/corpus/manual/milestone1.jsonl`
- `STATUS.md`

Acceptance tests:

- full replay final hash matches expected.
- replay from every decision snapshot matches final hash.
- no RNG draws are consumed.

Do not implement:

- rewards after victory
- Burning Blood
- real seed initialization

## Milestone 2: Minimal Combat Engine

### 2.1 Add Explicit Internal Action Queue

Scope:

- Add queued internal actions for play card, spend energy, deal damage, gain block, move card.
- Preserve milestone 1 behavior.
- Before coding, read the `sts_lightspeed` action/card queue notes created in task 0.0.

Acceptance tests:

- existing milestone 1 tests pass.
- event log records ordered internal actions.

Do not implement:

- generic event bus
- relic hooks
- full `sts_lightspeed` queue port

### 2.2 Add DamageInfo

Scope:

- Replace raw damage with structured damage info.
- Preserve Strike/Bash results.

Acceptance tests:

- block and HP math unchanged.
- damage event log includes source, target, amount.

Do not implement:

- modifiers
- thorns
- HP loss

### 2.3 Add Draw and Shuffle

Scope:

- Draw N cards.
- If draw pile is empty, shuffle discard into draw using simulator RNG.
- Add tests that distinguish simulator placeholder shuffle from exact game-compatible shuffle if exact parity is not implemented yet.

Acceptance tests:

- draw order deterministic.
- shuffle consumes logged RNG.
- legal actions and serialization consume no RNG.

Do not implement:

- broad exact game RNG parity beyond the local shuffle behavior under test
- special draw powers

### 2.4 Add Save-File/RNG Research Gate

Scope:

- Document which real save-file fields expose RNG counters.
- Compare those fields with `sts_lightspeed`'s save-file mapping.
- Decide whether save import moves earlier in the roadmap.

Files likely touched:

- `RESEARCH.md`
- `VERIFICATION.md`
- `STATUS.md`
- optional `docs/prior_art/save_rng.md`

Acceptance tests:

- No simulator tests required unless code already exists.
- `VERIFICATION.md` and `STATUS.md` updated with findings.

Do not implement:

- full save importer
- real-game decryption tooling
- broad RNG parity

### 2.5 Add Turn Structure

Scope:

- Start player turn.
- End player turn.
- Monster turn.
- Next intent placeholder.

Acceptance tests:

- block clear timing documented and tested.
- hand draw/discard matches expected simplified flow.

Do not implement:

- full action queue parity claims

## Milestone 3: Full Starter Deck and Core Combat Mechanics

### 3.1 Ironclad Starter Deck Constructor

Scope:

- 5 Strike_R, 4 Defend_R, 1 Bash.
- Ironclad base HP 80 for A0.

Acceptance tests:

- deck composition exactly matches expected.
- stable card instance IDs.

Do not implement:

- Ascender's Bane
- Neow bonuses

### 3.2 Burning Blood

Scope:

- Heal 6 at combat end, capped by max HP.

Acceptance tests:

- combat victory heals 6.
- no heal on loss.

Do not implement:

- Black Blood
- relic pool

### 3.3 Vulnerable Damage Multiplier

Scope:

- Player attacks against Vulnerable enemies deal increased damage.
- Decrement timing tested.

Acceptance tests:

- Bash then Strike deals expected damage after Vulnerable.
- Vulnerable decrements according to verified local rule.

Do not implement:

- Artifact
- all powers

### 3.4 Weak and Strength

Scope:

- Add attack modifiers needed by early monsters/cards.

Acceptance tests:

- Strength modifies attack damage.
- Weak modifies outgoing attack damage.
- rounding rules tested.

Do not implement:

- Dexterity
- Frail

## Milestone 4: Powers and Statuses

Tasks should add one power or status family at a time:

- Dexterity
- Frail
- Ritual
- Metallicize
- Wound
- Dazed
- Burn
- Slimed
- Ethereal
- Exhaust
- Retain

Each task requires:

- one behavior test
- one serialization test
- one interaction test if it touches existing mechanics

Do not implement multiple unrelated statuses in one task.

## Milestone 5: More Ironclad Cards

Add cards in dependency order:

1. Simple attacks: Anger, Cleave, Twin Strike.
2. Simple skills: Shrug It Off, True Grit with random exhaust later.
3. Draw/energy cards: Pommel Strike, Battle Trance, Seeing Red.
4. Exhaust package: Burning Pact, Feel No Pain, Dark Embrace.
5. Strength package: Inflame, Flex, Spot Weakness.
6. Complex cards: Whirlwind, Havoc, Warcry, Dual Wield, Searing Blow.

Each card task:

- card definition
- legal action tests
- normal play tests
- upgrade tests
- one interaction test if relevant

Do not implement reward generation just because a card exists.

## Milestone 6: Monsters

Add monsters by simplicity and verification value:

1. fixed dummy monster
2. real Cultist
3. Jaw Worm
4. Louses
5. Slimes
6. Gremlin Nob
7. Lagavulin
8. Sentries
9. Act 1 bosses

Each monster task:

- HP range
- move selection
- intent
- action execution
- ascension differences if current ascension milestone includes them

Do not implement all Act 1 monsters in one task.

## Milestone 7: Rewards and Deck Changes

Tasks:

- reward screen state
- skip reward
- add card reward to deck
- gold reward
- potion reward placeholder
- relic reward placeholder
- card reward generation with fixed pool
- card reward generation with rarity RNG
- card removal
- card upgrade

Do not implement map/shop/rest/event systems in reward tasks.

## Milestone 8: Map

Tasks:

- fixed map representation
- choose reachable node
- room transition
- act/floor counters
- generated map placeholder
- map generation parity experiments
- exact map generation after verified

Do not implement encounter generation until fixed map traversal works.

## Milestone 9: Shops, Rest Sites, Events

Tasks:

- shop screen shape
- buy fixed card
- remove card
- buy fixed relic
- buy/use/discard potion
- rest heal
- smith upgrade
- first fixed event
- event availability tests
- event RNG tests

Do not implement every shop price rule at once.

## Milestone 10: Relics and Potions

Add by dependency:

- Burning Blood
- common simple relics
- energy relics
- start-combat relics
- on-card-play relics
- damage/block relics
- complex stateful relics
- simple potions
- targeted potions
- random-effect potions

Each task needs:

- legal use or hook trigger test
- state counter serialization test
- interaction test

## Milestone 11: Ascensions

Tasks:

- ascension config
- A1 elite count/rules where applicable
- A2 normal enemy damage/behavior deltas
- A7 normal enemy HP/move deltas
- A10 Ascender's Bane
- A17+ monster behavior changes
- A20 double boss

Only add an ascension when its dependent system exists.

## Milestone 12: Replay Parity

Tasks:

- CommunicationMod trace importer
- canonical observed-state normalizer
- real-vs-sim diff CLI
- first milestone 1 real-game trace, if possible
- first Cultist trace
- observed-state sim replay for supported combat/reward transitions
- explicit unsupported/unobservable classifications in verifier output
- first full Act 1 trace
- minimized divergence corpus
- nightly parity runner

Do not claim seeded-run parity until exact real-game traces pass from seed plus action trace.

## Milestone 13: Seed and RNG Parity Harness

This milestone is the boundary between observed-state replay and true seed-start parity. It should create failing, actionable tests before trying to make the simulator match the real game.

Tasks:

- parse and store the external seed string used by CommunicationMod `START`, such as `VERIFY01`
- document the real-game seed conversion algorithm and source evidence
- add simulator RNG stream names/counters for the streams needed by the captured trace
- add a seed-start verifier mode that starts from seed/config plus action trace, not from observed pre-state
- add the first expected-failing comparison against `trace-2026-06-18T06-04-49-264Z.jsonl`
- report the first divergence with stable paths and stream/counter context
- classify every RNG-consuming system not yet wired

Acceptance tests:

- the verifier can run in `observed-state` mode and `seed-start` mode
- `observed-state` mode passes its supported fields
- `seed-start` mode fails at the first known unsupported RNG boundary with a stable, documented reason
- legal action generation, serialization, hashing, and observation normalization consume no RNG

Do not implement:

- Neow rewards
- map generation
- encounter generation
- reward generation fixes
- broad game-version compatibility

## Milestone 14: Neow and Run Bootstrap Parity

Tasks:

- initialize Ironclad A0 from real seed/config
- model starter relic state separately from ordinary relic pool state
- add Neow screen/options for the captured trace
- implement the selected Neow branch used by `VERIFY01`
- add Toy Ornithopter or explicitly model it as an unsupported inert relic if it has no effect on the current trace segment
- verify the trace through leaving Neow without restoring from observed state

Acceptance tests:

- `START IRONCLAD 0 VERIFY01` creates the same character, ascension, HP, gold, starter deck, and initial screen class as the real trace
- Neow choices in the captured trace produce the same visible post-Neow state
- unsupported Neow branches are classified by name

Do not implement:

- all Neow rewards
- all boss swaps
- all relic effects
- map generation beyond the next milestone

## Milestone 15: Map and Encounter RNG Parity

Tasks:

- implement or port exact Act 1 map generation for the target game version
- verify the captured map choices for `VERIFY01`
- implement normal encounter selection for the first floor
- implement exact monster HP roll for the first encounter
- verify that the first floor is Cultist with the observed HP and initial intent

Acceptance tests:

- the simulator produces the same first map choices as the captured trace
- choosing the same map node enters the same room type
- the first encounter matches the captured monster roster, HP, block, powers, and visible intent
- divergence output identifies map RNG, encounter RNG, or monster HP RNG separately

Do not implement:

- full Act path parity
- elite/boss encounter rules beyond what the trace reaches
- reward RNG

## Milestone 16: Combat Draw, Shuffle, and Monster AI RNG Parity

Status: captured-path implementation complete for `trace-2026-06-18T06-04-49-264Z.jsonl`. Seed-start mode verifies the captured opening hand, all captured Cultist `PLAY` commands by visible card content/order, both captured `END` transitions, visible Cultist powers/intent, the first discard-to-draw shuffle order, and the lethal transition to `COMBAT_REWARD`. Broad game-compatible shuffle RNG and alternate monster AI paths remain out of scope; the next expected seed-start boundary is reward RNG at `$.actions[step=16].command`.

Tasks:

- implement real-game-compatible opening hand draw order from the seed-started run
- implement discard-to-draw shuffle behavior and RNG consumption
- verify hand/draw/discard ordering for the captured Cultist fight
- align Cultist A0 behavior with the target game version, including Ritual amount and move progression
- add simulator RNG logs for every draw/shuffle and monster AI RNG consumer

Acceptance tests:

- the captured first hand matches from seed-start mode
- every `PLAY` command in the Cultist segment maps to the same card instance/card content without observed-state restoration
- both captured `END` transitions match player HP/block/energy, monster powers, visible intent, and pile sizes/order where observable
- any remaining hidden state is tagged `unobservable` with a named indirect test plan

Do not implement:

- all monster AI RNG
- all card-specific RNG
- reward/shop RNG

## Milestone 17: Reward RNG and Post-Combat Parity

Status: captured-path implementation complete for `trace-2026-06-18T06-04-49-264Z.jsonl`. Seed-start mode verifies the captured 14-gold reward offer, gold pickup to 113, card reward choices `Twin Strike`, `Heavy Blade`, and `Intimidate`, and picking `Twin Strike` into the deck. Broad game-compatible reward gold/card/potion/relic RNG remains out of scope; the next expected seed-start boundary is post-reward map continuation at `$.actions[step=19].command`.

Tasks:

- implement combat reward gold amount RNG for the target version
- implement reward card pool generation for the captured run
- add missing reward card content needed by the captured trace, including Heavy Blade and Intimidate if the reward screen remains under exact comparison
- verify the captured gold offer and card reward choices from seed-start mode
- verify taking gold and picking Twin Strike without restoring reward state from observation

Acceptance tests:

- the reward screen after killing Cultist matches visible gold/card/potion/relic offers under the chosen comparison scope
- gold pickup changes gold by the captured amount
- card reward choices match order and content for the captured trace
- Twin Strike pickup mutates the deck exactly as observed
- unsupported reward fields are named and justified

Do not implement:

- shop generation
- all reward pools
- all relic rewards
- all potion rewards

## Milestone 18: End-to-End Seed-Start Trace Parity

Status: complete for `trace-2026-06-18T06-04-49-264Z.jsonl`. Seed-start mode verifies the captured trace from `START IRONCLAD 0 VERIFY01` through return to map, reports `seed_start.expected_failure=false`, and keeps broader RNG/general-seed limits named in the RNG boundary report.

Tasks:

- replay `trace-2026-06-18T06-04-49-264Z.jsonl` from seed plus actions through return to map
- fail on any unclassified real-vs-sim divergence
- keep observed-state replay as a diagnostic fallback, not as the main parity claim
- add the captured trace to the required regression corpus
- add minimized divergence traces for every bug found while reaching parity

Acceptance tests:

- seed-start verifier passes the captured trace through return to map
- observed-state verifier still passes supported transition checks
- CLI output distinguishes `verified`, `unsupported`, `unobservable`, and `unexpected_diff`
- nightly parity runner includes the captured trace
- `STATUS.md` documents exactly what seed-start parity covers

Do not implement:

- claims about full Act 1 or arbitrary seeds
- full-run outcome parity
- RL training integration

## Milestone 19: Trace Verifier Coverage and Corpus Hygiene

Goal: make observed-state verification trustworthy on newly captured controller traces by removing false positives and naming every unsupported surface precisely.

Tasks:

- add `trace-2026-06-18T16-50-50-232Z.jsonl` or a minimized version of it to the regression corpus
- support CommunicationMod `PLAY n` no-target commands for all mapped no-target cards
- add unmapped-card classification so unknown cards do not shift hand/deck indices and produce bogus diffs
- compare the correct live monster when earlier enemies are dead
- classify unsupported monster-turn AI by monster group instead of reporting it as an unexpected diff
- make reward/deck comparisons partial when the observed deck contains unmapped cards
- add regression tests for `Dramatic Entrance`, nonzero reward choices, and multi-monster observed-state comparison

Acceptance tests:

- `cargo test -p sts_verify`
- observed-state parity on the new controller trace has no false `unexpected_diff` caused by unmapped cards or dead-front monsters
- unsupported counts include stable reasons for every skipped command

Do not implement:

- real RNG parity
- new card mechanics beyond what is needed to avoid verifier misclassification
- broad monster AI parity

## Milestone 20: External Seed Conversion and RNG Stream Audit

Status: complete. `SeedHelper.getLong(String)` was recovered from the target `12-18-2022` `desktop-1.0.jar`; captured seed tests pass for `VERIFY01`, `CODEX03`, and `CODEX04`; seed-start reports now include `numeric_seed` and classify seed conversion as `source_backed`.

Goal: replace opaque seed handling with target-version-compatible seed initialization evidence.

Tasks:

- document Slay the Spire seed string to numeric seed conversion for version `12-18-2022`
- add tests for known seed strings captured through CommunicationMod
- map real save-file RNG counters to simulator stream names
- record which stream advances for Neow, map, encounter, monster HP, shuffle, rewards, relics, potions, shop, and events
- add CLI output that reports stream/counter deltas where observed or inferred

Acceptance tests:

- seed conversion tests pass for at least three captured seeds
- `seed-start` reports no `captured_opaque` status for seed conversion
- documentation cites exact source/code evidence

Do not implement:

- map generation
- reward RNG
- full save import

## Milestone 21: General Neow and Colorless Reward Parity

Status: captured-path implementation complete for `trace-2026-06-18T16-50-50-232Z.jsonl`. Seed-start mode verifies `CODEX04` from `START IRONCLAD 0 CODEX04` through the captured Neow colorless-card branch, including the `Deep Breath` / `Dramatic Entrance` / `Jack Of All Trades` reward screen, picking `Dramatic Entrance`, and leaving to the first map-choice screen. Broad real-game Neow option RNG remains classified as captured-branch only; executing map nodes and encounters begins in Milestone 22.

Goal: support the Neow branches seen in controller traces, including colorless-card rewards.

Tasks:

- implement Neow option generation from the real RNG streams
- implement colorless card reward generation for Neow
- add mapped content for colorless cards needed by captured traces, starting with `Dramatic Entrance`
- verify `CODEX04` through Neow card pick and leave from seed-start mode
- keep unsupported branch classification for boss swap and remove-card branches until implemented

Acceptance tests:

- seed-start parity reaches the first map choice for `CODEX04`
- observed-state parity can simulate `Dramatic Entrance` without classifying it as unknown
- unchosen Neow branches remain named, not silent

Do not implement:

- all colorless cards
- all boss relic swaps
- full Act 1 path parity

## Milestone 22: Act 1 Map, Encounter, and Monster HP RNG Parity for Arbitrary Captured Seeds

Status: complete for the available captured evidence. Captured-target coverage spans `trace-2026-06-18T16-50-50-232Z.jsonl`, `trace-2026-06-18T16-45-23-530Z.jsonl`, and `trace-2026-06-18T06-04-49-264Z.jsonl`. Full VERIFY01/CODEX04/CODEX03 map topology, map-choice prefixes, chosen combat paths, normal encounter list prefixes, and captured combat-entry spawn state (roster, HP, block, intent, powers) are source-backed. Room execution maps combat index to normal encounter list entries. Seed-start reports include `m22_encounter_report`; CODEX04 and CODEX03 have three captured verified combat-entry rosters, while VERIFY01 has one captured verified entry plus two source-backed predictions because the available VERIFY01 trace returns to map after floor 1. CODEX04 seed-start now replays through floor-3 combat completion under Milestone 23; CODEX03 full seed-start replay remains future work for its Neow's Lament/reward branch.

Goal: produce the same Act 1 map prefix and normal encounters as real STS for multiple captured seeds.

Tasks:

- implement target-version Act 1 map generation
- implement normal encounter selection, including first-three-fight rules
- implement monster group composition and HP rolls for Act 1 normal fights
- verify `VERIFY01` and `CODEX04` first three floors from seed-start mode
- add minimized divergence traces for map, encounter, and HP mismatches

Acceptance tests:

- first available map nodes match captured traces
- chosen nodes enter the same room type
- monster roster, HP, block, powers, and visible intent match for the first three fights on at least two seeds

Do not implement:

- elite/boss encounter parity beyond the reached trace path
- rewards
- shops/events/rest effects

## Milestone 23: Draw, Shuffle, Card, and Monster AI Parity for Early Act 1

Goal: remove the current “exact card draw/shuffle order after end turn is out-of-scope” boundary for early Act 1 traces.

Status: complete for captured CODEX04/VERIFY01 scope. Observed-state and seed-start CODEX04 floor 1–3 combat parity pass with `unexpected_diffs=0`; END transitions are no longer draw/shuffle scope failures. Starter-only opening piles are seed-derived via `shuffleRng(seed + floor)` and decoded Ironclad master-deck instance order; innate/extra-card opening piles fall back to trace when seed shuffle does not match. In-combat and end-turn draws use `StsRng` through `shuffle_rng`; draw piles use top-of-pile semantics matching CommunicationMod bottom-first export order. Post-END pile resync remains as interim scaffolding until innate/extra-card master-deck ordering is fully decoded without trace fallback.

Tasks:

- implement game-compatible draw-pile initialization and shuffling
- implement real move selection for Cultist, slimes, louses, Jaw Worm, and early Act 1 normal monsters
- add missing card mechanics for captured early-run cards, including `Dramatic Entrance`, `Battle Trance`, and `Shrug It Off` interactions
- verify all combat turns in `CODEX04` through floor 3 from seed-start mode
- make hidden RNG draws visible in simulator RNG logs

Acceptance tests:

- no `END` command in the early captured traces is unsupported due to draw/shuffle scope
- no supported combat transition in the early captured traces has unexpected HP, block, energy, intent, hand, or pile diffs
- unsupported combat commands are limited to genuinely unmapped card/content surfaces

Do not implement:

- all Ironclad/colorless cards
- elites/bosses unless reached by the selected corpus

Follow-up (M24+): remove post-END pile resync once innate/extra-card master-deck ordering is decoded; reward RNG simulation for CODEX04.

## Milestone 24: Reward, Potion, Relic, Shop, Rest, and Event RNG Parity

Status: complete for captured reward RNG and source-backed shop/event generation. VERIFY01/CODEX04 seed-start reward screens are simulation-driven from `cardRng`, `treasureRng`, `potionRng`, and persisted relic pools. Elite/chest/boss relic rewards pop from pools without regressing `relic_rng_counter` after initialization. Shop generation matches `sts_lightspeed` `Shop.cpp` (7 cards, 3 relics, 3 potions, sale slot, remove cost) with `relic_key`-only shop relic ownership. Act 1 event/shrine pools use target `generateEvent` selection with implemented outcomes for Golden Shrine, Cleric heal, and Shining Light (20% max HP loss plus up to two random deck upgrades via `miscRng`). Nightly parity runs VERIFY01 and CODEX04 with `unexpected_diffs=0`. Captured CommunicationMod shop/event/rest traces are not in the passing nightly set. Unmapped shop colorless cards use synthetic IDs for pool-index RNG only. Post-reward map-return pins in the seed-start verifier and CODEX03 Neow's Lament remain Milestone 25.

Goal: make post-combat and non-combat room outcomes seed-start compatible for captured Act 1 paths.

Tasks:

- implement game-compatible combat reward gold, card, potion, and relic RNG
- implement reward screen ordering and pickup semantics for multiple rewards
- implement shop inventory and price RNG
- implement rest/event RNG for captured events and outcomes
- verify post-combat reward screens and pickups in `CODEX04`
- expand nightly parity to run all captured seed-start traces that are expected to pass

Acceptance tests:

- reward offers and pickup mutations match captured traces without observed-state restoration
- potion/relic/shop/event RNG boundaries are either passing or explicitly expected-failing with first divergence paths
- nightly parity includes at least two distinct seed-start traces

Do not implement:

- full-game win-rate claims
- arbitrary-character parity
- RL training integration

## Milestone 25: Full Ironclad Act 1 Seed-Start Parity

Status: core complete for the three representative seed-start traces (VERIFY01, CODEX04, CODEX03). Nightly parity includes all three with `unexpected_diffs=0`. Divergence minimization CLI and seed-start hidden-state documentation are in `VERIFICATION.md`. Deferred: Act 1 boss reward when a captured trace reaches it.

Goal: for a selected set of Ironclad A0 seeds, replay Act 1 from seed plus controller actions without observed-state restoration.

Tasks:

- choose at least three representative controller traces with different Neow/path/reward shapes
- verify every action from `START` through Act 1 boss reward
- add divergence minimization tooling for new failing traces
- document remaining unobservable hidden-state assumptions
- make CI/nightly fail on regressions for passing seed-start traces

Acceptance tests:

- selected traces pass with `seed_start.expected_failure=false`
- `unexpected_diffs=0` for passing seed-start traces
- unsupported items are absent from the required passing scope or formally waived as unobservable with tests

Do not implement:

- Defect/Silent/Watcher parity
- Act 2/3 parity
- claims for arbitrary mods

## Milestone 26: Clean M25 Baseline

Status: complete. Scratch/debug artifacts were removed from the tracked baseline, nightly parity passed, and the M25 seed-start regression gate is ready for M27 trace expansion.

Goal: turn the current M25 state into a clean, committed regression baseline before adding more simulator surface.

Tasks:

- remove scratch files and unused debugging artifacts
- ensure nightly parity is the documented regression gate
- verify `VERIFY01`, `CODEX04`, and `CODEX03` seed-start traces still pass
- update `STATUS.md` with the clean baseline and next selected trace need
- commit the M25 baseline with a concise message

Acceptance tests:

- working tree contains no accidental scratch files
- `scripts/nightly_parity.ps1` passes
- no M26 changes add new simulator behavior

Do not implement:

- new card, relic, monster, room, or RNG mechanics
- new trace-specific verifier shortcuts

## Milestone 27: Full Act 1 Trace Through Boss Reward

Status: complete.

Goal: replay one captured Ironclad A0 Act 1 trace from `START` through Act 1 boss reward without observed-state restoration.

Tasks:

- **27.0 Trace selection and floor-1 prefix**: capture or select a trace that reaches Act 1 boss reward, add it to the CommunicationMod corpus, and verify `START` through floor-1 reward return-to-map in seed-start mode.
- **27.1 TEST floor-2 parity**: eliminate the earliest TEST trace divergence after floor-1 return-to-map; verify floor-2 map entry, combat, reward handling, and return-to-map.
- **27.2 TEST non-combat path parity**: verify the next shop/rest/chest/event segment reached by the TEST trace, implementing only selected-outcome mechanics reached by the trace.
- **27.3 TEST elite segment parity**: verify the first elite path segment reached by the TEST trace, including combat, rewards, and map return.
- **27.4 TEST boss reward parity**: verify boss combat completion, boss chest, boss relic reward, and stop before Act 2 room execution.
- add the trace to nightly parity once the full M27 acceptance tests pass.

Acceptance tests:

- selected trace reports `seed_start.expected_failure=false`
- selected trace reports `unexpected_diffs=0`
- first unsupported boundary, if any, is outside the declared Act 1 boss reward scope

Completed with `trace-2026-06-21T09-57-10-380Z.jsonl` (seed `TEST`): seed-start passes through boss relic Cursed Key and pre–Act-2 map return; nightly parity and `test_seed_start_full_act1_boss_relic_prefix` added.

Do not implement:

- Act 2 room execution
- arbitrary boss reward generalization beyond captured evidence

## Milestone 28: Act 1 Non-Combat Room Trace Coverage

Status: complete.

Goal: verify shop, rest, chest, and event room execution from captured seed-start traces.

Tasks:

- capture or select traces that enter shop, rest, chest, and at least two events
- verify room entry, choices, rewards, removals, upgrades, and map return
- replace any remaining room-specific observed-state restoration in the selected traces
- add explicit expected-failing boundaries for unselected event choices
- add passing room traces to nightly parity
- remove counter-search and observed-state reconstruction fallbacks from room verification
- make the TEST shop inventory derive from carried simulator RNG/pool state only
- fix shop purchase, purge, and post-buy choice-label parity on the TEST trace (steps 170–176)

Acceptance tests:

- at least one shop trace passes through purchase/removal or exit
- at least one rest trace passes through heal or smith
- at least one chest trace passes through reward pickup
- at least two event traces pass through selected outcomes
- all passing traces report `unexpected_diffs=0`
- no passing trace uses brute-force RNG counter search or observed shop-screen reconstruction

Progress:

- `trace-2026-06-21T09-57-10-380Z.jsonl`: full seed-start parity through Act 1 boss relic return-to-map with `unexpected_diffs=0` (`test_seed_start_full_act1_boss_relic_prefix`). Shop entry inventory from carried pool state (`test_seed_start_m28_shop_entry_parity`); purchase/purge through step 176 via `affordable_shop_picks`, library-rarity class pricing, colorless `getPrice` bases, and membership/sale gold. Nightly parity includes this trace.

Current blocker:

- none for M28 on the TEST trace; next milestones per roadmap (additional shop/rest/event captured traces).

Do not implement:

- all event choices
- all shop/relic/card mechanics unless reached by selected traces

## Milestone 29: Act 1 Elites and Bosses

Status: in progress. TEST-trace elite/boss slice is complete for the captured route: Lagavulin entry, sleep/Metallicize block, wake-on-HP-damage, player vulnerable, Regret end-turn damage, Demon Form/Thunderclap trace playability, Gremlin Nob coverage, Guardian mode-shift scaffolding, and Act 1 boss relic return are implemented. `test_seed_start_m29_test_elite_boss_without_observed_sync` passes with elite/boss observed-state restoration disabled, except for explicit UI/potion boundaries. A new overnight run prefix, `trace-2026-06-23T02-56-19-245Z.run2.valid-prefix.jsonl`, is structurally valid and reaches Sentries. `trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl` removes 49 no-progress card-reward skip/reopen pairs from that run. Seed-start verification supports its captured transform-card Neow branch, Sever Soul, Uppercut, lethal Bash sequencing where Vulnerable follows lethal damage, the captured The Ssssserpent event branch, the Sentries elite reward sequence, and the following Blue Slaver combat/reward prefix. On the cleaned trace it verifies all 225 actions with `unexpected_diffs=0`; the only remaining boundary is `missing_post_reward_boundary` because the trace ends on a reward screen before a final `PROCEED`. The corpus test and nightly parity now include this cleaned trace as an expected-failing boundary regression; it is not a passing M29 trace.

Goal: verify Act 1 elite and boss combats, including move RNG and special mechanics, from captured seed-start traces.

Tasks:

- capture or select traces covering Gremlin Nob, Lagavulin, and Sentries
- capture or select traces covering Slime Boss, Guardian, and Hexaghost over time
- implement missing elite and boss AI, move RNG, summons, split, sleep, mode, and special mechanics reached by the traces
- verify reward transitions after elite and boss combats
- add passing elite/boss traces to nightly parity

Completed slice:

- TEST Lagavulin prefix verifies without elite observed-state restoration through the pre-Power-Potion combat mechanics.
- Combined elite reward entry now consumes the hidden potion reward roll, keeping potion RNG aligned through the TEST shop.
- Observed-state replay no longer drops monster Vulnerable when the player is Weak; Weak and Vulnerable now compose through the simulator damage formula.
- In-combat Power Potion card reward, temporary zero-cost card play, and subsequent potion-tainted combat state still use observed sync and remain outside the elite/boss AI parity claim.
- Existing TEST full Act 1 trace still passes through boss relic reward via the M27/M28 verifier path.

Blocked capture:

- `trace-2026-06-21T03-24-47-580Z.jsonl` contains Jaw Worm, Cultist, 2 Louse, Acid Slime + Looter, Sentries, Lagavulin, and slimes, but has actions without matching state rows (including the final action), so `verify_seed_start_communication_mod_trace` rejects it with `MissingStateAfterAction`.
- `trace-2026-06-23T02-56-19-245Z.valid-prefix.jsonl` contains two attempts; the first dies on floor 1 and the second reaches Sentries. `trace-2026-06-23T02-56-19-245Z.run2.valid-prefix.jsonl` extracts the second run, containing Cultist, Small/M slime, Jaw Worm, Gremlins, Sentries, and Blue Slaver with one invalid trailing full-potion-belt reward action trimmed and documented by metadata. `trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl` collapses the old autopilot card-reward loop and validates with `tools/communication/trace_tools.js`. The former floor-2 lethal-Bash, step-143 event, step-173 Gremlin-fight hand/deck drift, card-reward offer, and M290001 map-entry subset boundaries are fixed. The cleaned trace reaches its end with `unexpected_diffs=0`, but still reports `missing_post_reward_boundary` because capture stops on the final reward screen. This expected-failing classification is pinned in `test_seed_start_m29_m290001_sentries_prefix_zero_diffs` and `scripts/nightly_parity.ps1`; M29 still needs a new final-`PROCEED` Sentries trace before completion.
- The overnight collector now guards unavailable command verbs, repeated identical commands on unchanged states, and stale bridge/session files. This is intended to prevent the card-reward and potion-reward loops seen during M290001 collection while still stopping rather than producing malformed traces if no conservative fallback is available.
- `overnight_supervisor.js` adds the intended overnight entry point: it restarts the collector, validates the active trace after each collector exit or stale-session startup, writes a `.valid-prefix.jsonl` salvage file for traces missing an action response, extracts a `.best-run.jsonl` keeper from valid traces, updates `session/harvest_report.json`, logs trace-quality coverage plus the best individual run from multi-run captures, and refuses to continue on stale bridge/session files instead of silently writing commands into a dead CommunicationMod session.
- `harvest_status.js` inspects the latest harvest report without mutating traces, so an overnight run can be checked quickly before deciding which generated artifacts to keep.
- `overnight_preflight.js` checks whether the bridge/session is fresh and safe before starting overnight supervision; it catches stale files, pending commands, exited bridge state, and sent-command/newer-than-summary mismatches.
- `run_overnight_preflight.cmd` and `run_communication_checks.cmd` provide one-command Windows entry points for preflight and communication-tool regression checks.
- `run_overnight_guarded.cmd` is the safer overnight entry point: it runs preflight and starts the supervisor only when the bridge/session is fresh.
- `overnight_collector.test.js`, `overnight_supervisor.test.js`, and `trace_tools.test.js` are the regression gates for autopilot command policy, supervisor trace reporting, and harvested-trace validation before the next live overnight attempt.
- 2026-06-23 live collection resumed after restarting STS and CommunicationMod. `trace-2026-06-23T07-42-06-085Z.jsonl` currently validates at completed boundaries and covers 3 starts (`M290005`..`M290007`), 378 completed actions, max floor 10, 3 elite-room entries, shop/rest/chest/event coverage, and 2 deaths; the active tail is a floor-7 elite fight. `trace-2026-06-23T07-42-06-085Z.best-run.jsonl` is valid and extracts `M290006` with 105 actions, max floor 10, 1 elite, and terminal death. Live blockers fixed during the run: duplicate bridge clients are now visible through `client_pid`; pending `START` state prevents repeated start commands; `SHOP_ROOM` proceeds instead of reopening the shop; `HAND_SELECT` chooses and confirms required card selections.
- After further collection, `trace-2026-06-23T07-42-06-085Z.best-run.jsonl` was regenerated from the same raw trace and now selects `M290008` (numeric seed `40560393133`): 193 structurally valid actions, max floor 16, and in-progress Hexaghost combat. This is the current boss-reaching evidence trace. It is not a passing M29 seed-start parity trace yet: seed-start stops at Neow option generation/choice handling for this branch, and observed-state parity still has unsupported/unmapped-card and monster intent/AI diffs.

Acceptance tests:

- each Act 1 elite has at least one passing seed-start trace
- at least one Act 1 boss has a passing seed-start trace through boss reward
- monster HP, block, powers, intents, piles, rewards, and player state match with `unexpected_diffs=0`

Do not implement:

- Act 2/3 elites or bosses
- arbitrary Ascension-specific boss variants beyond selected trace scope

## Milestone 30: Harvested Hexaghost Seed-Start Slice

Status: complete for the declared early-Act-1 slice.

Goal: verify the harvested Hexaghost best-run trace from seed start through Neow and the early Act 1 route segment without unexpected diffs.

Completed:

- added captured M290008 Neow transform branch support (`transform a card`, `obtain 100 gold`, `lose all gold max hp +16`, boss swap) with `Sentinel` as the transformed card
- added identity/playability coverage needed by this slice for `Sentinel`, `Bloodletting`, `Sword Boomerang`, and `Hemokinesis`
- verified M290008 through Neow, floors 1-2 combats and rewards, Scrap Ooze, The Ssssserpent, the next combat/rest/treasure segment, and the step-99 treasure-to-map boundary
- added `test_seed_start_m30_m290008_hexaghost_early_act1_slice`

Acceptance tests:

- `cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-23T07-42-06-085Z.best-run.jsonl` reports `verified=99`, `unexpected_diffs=0`, and first boundary `$.actions[step=100].command`
- `cargo test -p sts_verify test_seed_start_m30_m290008_hexaghost_early_act1_slice`

Still future work:

- general Neow option generation and rewards remain Milestone 33
- transformed-card opening pile fallback remains Milestone 34
- Scrap Ooze success RNG, later event selection, Sword Boomerang random targeting, Looter escape AI, and later Act 1 rooms are captured-slice support here, not broad parity claims

## Milestone 31: Potion Completion Pass

Status: complete.

Goal: implement all potion use effects and legality rules reachable by Ironclad runs.

Tasks:

- inventory all Ironclad-available potions against implemented potion effects
- implement targeting, combat/non-combat legality, belt behavior, discard, and use effects
- implement RNG-affecting potion behavior
- verify potion-heavy captured traces where available
- add unit tests for each potion effect

Completed slice:

- implemented deterministic Ironclad potion effects and discovery choices for Ancient, Attack, Skill, Colorless, Blood, Heart of Iron, Cultist, Dexterity, Energy, Essence of Steel, Explosive, Liquid Bronze, Regen, Strength, Weak, Fruit Juice, Swift, Flex, Blessing of the Forge, Power, and Speed
- implemented Entropic Brew potion-belt refill using `potionRng`
- implemented Duplication Potion's one-shot next-card replay flag
- implemented Distilled Chaos top-three draw-pile play with `cardRandomRng` targeting
- implemented Liquid Memories discard-pile selection returning the chosen card to hand at zero cost
- implemented Snecko Oil draw-five and playable-hand cost randomization with `cardRandomRng`
- implemented Smoke Bomb non-boss escape with no reward
- implemented Elixir multi-card hand exhaust selection
- implemented Fairy in a Bottle passive lethal-damage revive and direct-use rejection
- preserved existing Fire, Block, Fear, Gamble, Power, discard, target validation, and potion-reward/shop belt behavior
- added unit tests for each new deterministic potion effect

Remaining:

- no known Ironclad reward-pool potion remains unimplemented in `sts_core`
- potion-heavy seed-start trace verification once a suitable captured trace reaches these effects

Acceptance tests:

- every potion in the Ironclad reward pool can be used or rejected with target-compatible legality
- potion use mutates player, monsters, deck, hand, piles, or RNG counters according to target behavior
- potion-heavy captured traces do not produce unexpected diffs inside implemented scope

Do not implement:

- modded potions
- character-specific potion effects for other characters except shared behavior

## Milestone 32: Relic Completion Pass

Status: complete.

Goal: implement all Ironclad-available relic hooks needed for full-run simulation.

Tasks:

- inventory relic pool entries against implemented pickup, combat, turn, reward, shop, and event hooks
- implement missing common, uncommon, rare, shop, event, and boss relic effects reachable by Ironclad
- model relic counters and persistence fields
- verify relic-heavy captured traces where available
- add unit tests for each relic hook family

Completed slice:

- expanded modeled relic keys for simple pickup/capacity/start-combat relics: Blood Vial, Pear, Mango, Old Coin, Lee's Waffle, and Potion Belt
- implemented immediate pickup effects for Pear, Mango, Old Coin, Lee's Waffle, and Potion Belt capacity
- implemented Blood Vial start-of-combat healing
- routed Potion Belt capacity through reward pickup, shop potion purchase, and Entropic Brew refill
- added focused unit tests for key mapping, pickup effects, start-combat heal, and capacity integration
- expanded modeled start-of-combat relics: Lantern, Bag of Preparation, Bag of Marbles, Bronze Scales, Thread and Needle, and Red Skull
- implemented their combat entry hooks and added focused unit tests for energy, draw, vulnerable, thorns, plated armor, and conditional strength
- expanded modeled card-play relics: Nunchaku, Shuriken, Kunai, and Letter Opener
- implemented card-play counters/effects and added focused unit tests for energy, strength, dexterity, all-enemy damage, turn resets, and counter serialization
- expanded modeled turn-timed combat relics: Happy Flower, Orichalcum, Horn Cleat, Captain's Wheel, Mercury Hourglass, and Stone Calendar
- implemented start/end player turn hooks and added focused unit tests for energy, conditional block, turn-specific block, all-enemy damage, first-turn initialization, and counter serialization
- expanded modeled combat-victory healing relics: Black Blood and Meat on the Bone
- implemented upgraded/conditional victory healing and added focused unit tests for win/loss timing, caps, and half-HP checks
- expanded modeled room/rest healing relics: Meal Ticket, Regal Pillow, Dream Catcher, and Eternal Feather
- implemented shop-entry healing, rest bonus healing, modeled Dream Catcher card reward, and focused rest/shop/event regression tests
- expanded modeled damage mitigation relics: Torii and Tungsten Rod
- implemented unblocked attack damage and generic HP-loss mitigation across monster attacks, spikes, Burn, and Regret with focused tests
- expanded modeled shop/economy relics: Ceramic Fish, Membership Card, and Smiling Mask
- implemented Ceramic Fish card-add gold across reward/shop/event additions, modeled Membership Card key promotion, and Smiling Mask removal pricing with focused reward/shop/state tests
- expanded modeled boss-entry relics: Pantograph
- implemented boss-combat start healing from current map room state with focused boss/non-boss tests
- expanded modeled debuff-immunity relics: Ginger and Turnip
- implemented player Weak/Frail prevention helpers and wired Ginger into monster weak intents with focused relic/monster tests
- expanded modeled boss energy relics: Mark of Pain
- implemented Mark of Pain pickup energy and two-Wound deck mutation with focused state/relic tests
- expanded modeled combat healing relics: Magic Flower
- implemented source-backed combat-only `MathUtils.round(heal * 1.5)` healing for Blood Vial, Burning/Black Blood, Meat on the Bone, Pantograph, Blood Potion, and Regen with focused tests
- expanded modeled Vulnerable synergy relics: Paper Phrog and Champion Belt
- implemented Paper Phrog's 75% Vulnerable damage bonus and Champion Belt weak-on-player-applied-Vulnerable hooks for cards and Bag of Marbles with focused tests
- expanded modeled elite HP relics: Preserved Insect
- implemented elite-room monster HP reduction at combat initialization with focused elite/non-elite tests
- expanded modeled curse synergy relics: Darkstone Periapt and Du-Vu Doll
- implemented explicit modeled-curse classification, max-HP-on-curse-add, and strength-per-curse combat-start hooks with focused tests
- expanded modeled boss energy/rest-restriction relics: Fusion Hammer
- implemented Fusion Hammer pickup energy and rest-site smith disabling with focused state/rest tests
- expanded modeled boss energy/potion-lockout relics: Sozu
- implemented Sozu pickup energy, generated reward potion suppression, reward/shop potion acquisition rejection, and Entropic Brew no-fill behavior with focused tests
- expanded modeled potion potency relics: Sacred Bark
- implemented Sacred Bark key promotion and doubled modeled potion potency for direct combat/non-combat potion effects plus Fairy revive healing, with focused potion/reward/relic tests
- expanded modeled boss energy/card-reward relics: Busted Crown
- implemented Busted Crown pickup energy and one-choice card rewards with focused state/reward RNG tests
- expanded modeled boss energy/card-limit relics: Velvet Choker
- implemented Velvet Choker pickup energy, per-turn card play counter tracking, and six-card legal-action limit with focused state/relic/legal tests
- expanded modeled hand-retention relics: Runic Pyramid
- implemented Runic Pyramid key promotion and end-turn non-Ethereal hand retention after Burn/Regret/Ethereal cleanup, with focused hand/relic tests
- expanded modeled information-only relics: Frozen Eye
- implemented Frozen Eye key promotion and explicit no-op semantics because ordered draw-pile state is already visible to simulator callers, with focused relic tests
- expanded modeled rest removal relics: Peace Pipe
- implemented Peace Pipe key promotion, content-id round trips, and rest-site card removal gating so removal is no longer available without Peace Pipe, with focused rest/milestone tests
- expanded modeled debuff-cleanse relics: Orange Pellets
- implemented Orange Pellets key promotion, persistent card-type flags, Attack/Skill/Power trigger reset, and modeled player debuff cleanup with focused relic tests
- expanded modeled rest strength relics: Girya
- implemented Girya key promotion, content-id round trips, campfire Lift action with three-lift cap, persisted lift count, and combat-start Strength from stored lifts with focused rest/state tests
- expanded modeled potion-use healing relics: Toy Ornithopter
- implemented Toy Ornithopter promotion from relic keys and potion-use healing for combat/non-combat potion use, including discard no-op and Magic Flower combat-healing coverage
- expanded modeled card-add upgrade relics: Molten Egg, Toxic Egg, and Frozen Egg
- implemented shared card-add upgrade handling for reward choices and deck insertion paths with focused reward/state tests
- expanded modeled small unblocked attack damage relics: The Boot
- implemented The Boot key promotion and attack-only unblocked damage floor after block, with focused relic/damage tests
- expanded modeled power-play healing relics: Bird-Faced Urn
- implemented Bird-Faced Urn key promotion and Power-card healing through the combat healing path, with focused relic tests
- expanded modeled no-attack-turn energy relics: Art of War
- implemented Art of War key promotion and previous-turn attack tracking with focused counter/energy tests
- expanded modeled card reward choice relics: Question Card
- implemented Question Card key promotion and +1 card reward choice, including Busted Crown stacking coverage
- expanded modeled curse-prevention relics: Omamori
- implemented Omamori key promotion and two-charge curse prevention before deck insertion/card-added relic hooks, with focused state tests
- expanded modeled elite-combat strength relics: Sling of Courage
- implemented Sling of Courage key promotion and elite-only combat-start strength, with focused run-state tests
- expanded modeled shuffle-trigger relics: The Abacus
- implemented The Abacus key promotion and block gain on discard-to-draw shuffle paths with focused draw tests
- expanded modeled monster-death relics: Gremlin Horn
- implemented Gremlin Horn key promotion and per-monster death energy/draw hooks for card and potion damage paths, with focused combat tests
- expanded modeled shuffle-counter relics: Sundial
- implemented Sundial key promotion, combat counter serialization, and every-third-shuffle energy gain through shared shuffle hooks, with focused draw tests
- expanded modeled exhaust-damage relics: Charon's Ashes
- implemented Charon's Ashes key promotion and all-enemy unmodified damage on card exhaust, including monster-death relic follow-ups, with focused combat tests
- expanded modeled unplayable-card relics: Blue Candle and Medical Kit
- implemented Blue Candle curse play/exhaust/HP-loss and Medical Kit status play/exhaust through shared card play and exhaust hooks, with focused legal/combat tests
- expanded modeled one-shot revive relics: Lizard Tail
- implemented Lizard Tail key promotion, run-state used flag persistence, and lethal-combat revive timing before Fairy fallback, with focused reward/combat tests
- expanded modeled previous-turn card-count relics: Pocketwatch
- implemented Pocketwatch key promotion, persisted previous-turn card-play counting, and start-of-turn draw-three timing, with focused relic counter/draw tests
- expanded modeled block-break attack relics: Hand Drill
- implemented Hand Drill key promotion and attack-only block-break Vulnerable through the shared damage transition path, with focused combat tests
- expanded modeled starter/fallback no-op relics: Burning Blood, Circlet, and Red Circlet
- implemented key promotion and content-id round trips for Burning Blood, Circlet, and Red Circlet; Burning Blood's Ironclad victory heal remains modeled by the existing implicit Ironclad combat-victory path
- expanded modeled floor-entry gold relics: Maw Bank
- implemented Maw Bank key promotion, +12 gold floor entry, and shop-spend break behavior, with map/shop/grid tests
- expanded modeled rest-site energy relics: Ancient Tea Set
- implemented Ancient Tea Set key promotion, rest-site arming, next-combat +2 energy, and one-shot consume helper, with map/run-state tests
- expanded modeled block-retention relics: Calipers
- implemented Calipers key promotion and end-of-turn block retention, with focused combat-turn tests
- expanded modeled reward-screen max-HP relics: Singing Bowl
- implemented Singing Bowl key promotion and explicit open-card-reward max-HP action, with focused reward/state/relic tests
- expanded modeled X-cost relics: Chemical X
- implemented Chemical X key promotion and +2 Whirlwind X value, including zero-energy Chemical X play, with focused legal/combat tests
- expanded modeled boss energy/enemy-strength relics: Philosopher's Stone
- implemented Philosopher's Stone key promotion, pickup energy, and combat-start monster Strength, with focused run-state tests
- expanded modeled boss conditional-energy relics: Slaver's Collar
- implemented Slaver's Collar key promotion and elite/boss-only combat energy, with focused run-state tests
- expanded modeled boss energy/gold-lockout relics: Ectoplasm
- implemented Ectoplasm key promotion, pickup energy, and positive gold-gain prevention across reward, event, relic pickup, card-add, floor-entry, and potion gain paths, with focused tests
- expanded modeled boss energy relics: Runic Dome
- implemented Runic Dome key promotion and pickup energy with focused run-state tests
- expanded modeled Strike-card damage relics: Strike Dummy
- implemented Strike Dummy key promotion and +3 damage for Strike/Strike+ card transitions, with focused combat tests
- expanded modeled shop start-turn strength relics: Brimstone
- implemented Brimstone key promotion and start-of-player-turn Strength for the player and living monsters, with focused relic tests
- expanded modeled potion reward guarantee relics: White Beast Statue
- implemented White Beast Statue key promotion and guaranteed normal-combat potion rewards when potion gain is allowed, without mutating normal potion chance, with focused reward tests
- expanded modeled pickup upgrade relics: Whetstone and War Paint
- implemented Whetstone key promotion and source-backed `miscRng.randomLong()` Java shuffle upgrade of two random Attack cards on pickup, with focused state tests
- implemented War Paint key promotion and source-backed `miscRng.randomLong()` Java shuffle upgrade of two random Skill cards on pickup, with focused state tests
- expanded modeled first-attack damage relics: Akabeko
- implemented Akabeko key promotion, combat-wide first-Attack tracking, and source-backed +8 Vigor-style damage for the first Attack card, with focused combat tests
- expanded modeled first-HP-loss draw relics: Centennial Puzzle
- implemented Centennial Puzzle key promotion and first combat HP-loss draw-three hook for monster attacks, spikes, Burn, and Regret, with focused relic/combat tests
- expanded modeled attack-counter relics: Pen Nib
- implemented Pen Nib key promotion and tenth-Attack damage doubling with a persisted attack counter, including focused single-hit and multi-hit combat tests
- expanded modeled HP-loss response relics: Self-Forming Clay
- implemented Self-Forming Clay key promotion and HP-loss block gain for monster attacks, spikes, Burn, and Regret through the shared HP-loss relic hook, with focused relic/combat tests
- expanded modeled start-of-combat artifact relics: Clockwork Souvenir
- implemented Clockwork Souvenir key promotion and source-backed start-of-combat Artifact gain, with focused relic tests
- expanded modeled HP-loss draw relics: Runic Cube
- implemented Runic Cube key promotion and source-backed draw-one on each HP-loss event through the shared HP-loss relic hook, with focused relic tests
- expanded modeled hand-empty draw relics: Unceasing Top
- implemented Unceasing Top key promotion, content-id round trips, and draw-one timing after the played card leaves an empty hand, with focused combat tests for normal draw, retained hand cards, draw-lock prevention, and power-card removal
- expanded modeled rest relics: Shovel
- implemented Shovel key promotion, content-id round trips, campfire Dig legality, and relic-only reward-screen generation through the existing relic RNG/pool path with focused rest/state tests
- expanded modeled Buffer relics: Fossilized Helix
- implemented Fossilized Helix key promotion, content-id round trips, combat-start Buffer, and one-stack HP-loss prevention for monster attacks, spikes, Burn, and direct combat HP-loss actions with focused combat tests
- expanded modeled elite bonus-reward relics: Black Star
- implemented Black Star key promotion, content-id round trips, elite reward-screen second relic queuing, and sequential relic collection through the existing reward action with focused reward tests
- expanded modeled chest bonus-reward relics: Matryoshka
- implemented Matryoshka key promotion, content-id round trips, persisted two-chest counter, and chest reward-screen second relic queuing with focused reward tests
- expanded modeled pickup removal relics: Empty Cage
- implemented Empty Cage key promotion, content-id round trips, pickup-triggered two-card removal grid, and sequential card removal confirmation with focused grid/reward tests
- expanded modeled bottled-card relics: Bottled Flame, Bottled Lightning, and Bottled Tornado
- implemented bottled-card key promotion, content-id round trips, pickup-triggered filtered card grids, persistent deck-card bottle flags, and opening-hand placement with focused grid/reward/combat-setup tests
- expanded modeled card-copy relics: Dolly's Mirror
- implemented Dolly's Mirror key promotion, content-id round trips, pickup-triggered deck selection grid, and selected-card duplication with a fresh card instance id and focused grid/reward tests
- expanded modeled card reward count relics: Prayer Wheel
- implemented Prayer Wheel key promotion, content-id round trips, normal-combat two-card-reward pending counts, and sequential card reward opening/taking with focused reward tests
- expanded starter/fallback no-op relics for off-character imports: Cracked Core, Frozen Core, Pure Water, Holy Water, Ring of the Snake, and Ring of the Serpent
- implemented off-character starter/replacement key promotion and content-id round trips as no-op relics within the Ironclad simulator scope
- expanded modeled random-potion pickup relics: Cauldron
- implemented Cauldron key promotion, content-id round trips, and pickup-triggered random potion filling up to available capacity while respecting Sozu and existing filled slots
- expanded modeled boss pickup bundle relics: Tiny House
- implemented Tiny House key promotion, content-id round trips, max-HP/gold/heal pickup bundle, one random deck upgrade via `miscRng`, and reward-screen card reward queuing
- expanded modeled random-card-on-exhaust relics: Dead Branch
- implemented Dead Branch key promotion, content-id round trips, `cardRandomRng` random card generation from the non-healing Ironclad combat pool, hand-full overflow to discard, and focused run-level exhaust tests
- expanded modeled power-play cost relics: Mummified Hand
- implemented Mummified Hand key promotion, content-id round trips, `cardRandomRng` positive-cost hand-card discounting after Power play, and turn-only temp-cost reset coverage
- expanded final modeled relic families: The Courier, Incense Burner, Cursed Key, Tiny Chest, Snecko Eye, Strange Spoon, Wing Boots, Calling Bell, Pandora's Box, Astrolabe, Gambling Chip, Toolbox, Juzu Bracelet, and Prismatic Shard
- implemented source-backed shop restock/discount hooks, persistent turn counters, chest/event/map replacement hooks, exhaust-retention RNG, same-floor map jumps, boss pickup bundle/transform grids, start-of-combat choice grids, `?` room Juzu handling, and Prismatic Shard any-color combat rewards
- updated seed-start verifier relic synchronization so observed modeled relics promote into active modeled relic state instead of legacy key-only placeholders

Remaining:

- none for the Milestone 32 Ironclad A0 relic completion scope; broader cross-run/generalization coverage continues in later milestones

Acceptance tests:

- every Ironclad-available relic can be gained without placeholder semantics
- implemented relics apply pickup, start-combat, card-play, damage, turn, reward, and room hooks at target timing
- relic counters round-trip through run/combat state

Do not implement:

- modded relics
- non-Ironclad-only interactions unless shared relic behavior requires them

## Milestone 32A: Content Inventory and Coverage Matrix

Status: complete.

Goal: stop discovering core Ironclad surfaces only when a captured trace happens to hit them. Build a complete Ironclad-relevant content inventory and compare it against simulator implementation, tests, and parity evidence before continuing broad-run generalization.

Principle:

- Wiki and public reference data are acceptable for inventory and first-pass numeric facts.
- Target bytecode/source notes, CommunicationMod traces, and controlled experiments remain the authority for RNG, ordering, hidden counters, and timing-sensitive behavior.
- Every unsupported surface should be visible in a matrix instead of being found accidentally during a later trace.

Tasks:

- [x] inventory all Ironclad-relevant cards, colorless cards reachable by Ironclad, potions, relics, monsters, bosses, events, rooms, rewards, shops, rest actions, keys, and ascension deltas
- [x] create or update a support matrix with columns for content ID, category, implementation status, unit-test status, seed-start trace status, source/bytecode evidence, wiki/reference evidence, and known caveats
- [x] classify each surface as `implemented`, `placeholder`, `inventory_only`, `unsupported`, `waived`, or `not_in_scope`
- [x] identify deterministic surfaces that can be safely implemented from inventory data without waiting for a trace
- [x] identify parity-sensitive surfaces that require source-backed or trace-backed evidence before claims
- [x] add a coverage-report command or test fixture that fails when known Ironclad A0 content is missing from the matrix
- [x] document the difference between content completeness, unit correctness, and seed-start parity

Completed artifacts:

- `simulator/docs/content_support_matrix.md` defines the matrix vocabulary and evidence rules.
- `simulator/docs/m32a_cards_matrix.md`, `simulator/docs/m32a_relic_potion_matrix.md`, and `simulator/docs/m32a_run_world_matrix.md` split the inventory by domain.
- `simulator/crates/sts_core/tests/m32a_matrix.rs` fails when known Ironclad A0 cards, generated card-pool IDs, relics, potions, monsters, encounters, events, room kinds, or named run-world surfaces are missing from the matrix.

Acceptance tests:

- the matrix has an entry for every known Ironclad A0 core-game surface in the declared scope
- every existing simulator content ID maps to a matrix entry
- every matrix entry has an explicit status and evidence level
- unsupported and waived surfaces have named reasons
- current nightly parity continues to pass

Do not implement:

- broad behavior changes during the inventory task
- parity claims from wiki-only evidence
- non-Ironclad character completion
- modded content

## Milestone 32B: Deterministic Content Completion Sweep

Status: in progress.

Goal: use the Milestone 32A matrix to implement missing deterministic Ironclad A0 content proactively, instead of waiting for traces to encounter it.

Tasks:

- implement missing deterministic potion, card, relic, event, room, reward, rest, and monster/boss behaviors that do not depend on unresolved RNG stream order or hidden target timing
  - [x] Implement base Iron Wave (`IRON_WAVE`, ContentId(100)) as a deterministic common attack: 1 energy, targeted enemy, 5 damage, 5 block, no trace-parity claim.
  - [x] Implement base Body Slam (`BODY_SLAM`, ContentId(101)) as a deterministic common attack: 1 energy, targeted enemy, damage equal to current player block, no trace-parity claim.
  - [x] Implement base Clash (`CLASH`, ContentId(102)) as a deterministic common attack: 0 energy, targeted enemy, 14 damage, playable only when the hand contains attacks only, no trace-parity claim.
  - [x] Implement base Wild Strike (`WILD_STRIKE`, ContentId(107)) as a deterministic common attack: 1 energy, targeted enemy, 12 damage, adds a Wound to the draw pile with local deterministic pile order, no trace-parity claim.
  - [x] Implement base Heavy Blade (`HEAVY_BLADE`, ContentId(108)) as a deterministic common attack: 2 energy, targeted enemy, 14 damage with Strength applied 3 times, no trace-parity claim.
  - [x] Implement base Perfected Strike (`PERFECTED_STRIKE`, ContentId(109)) as a deterministic common attack: 2 energy, targeted enemy, 6 damage plus 2 per Strike-named card in current combat piles, no trace-parity claim.
  - [x] Implement base Power Through (`POWER_THROUGH`, ContentId(111)) as a deterministic uncommon skill: 1 energy, no target, 15 block, adds two Wounds to hand with local deterministic card IDs/order, no trace-parity claim.
  - [x] Implement base Reckless Charge (`RECKLESS_CHARGE`, ContentId(113)) as a deterministic uncommon attack: 0 energy, targeted enemy, 7 damage, adds one Dazed to the draw pile with local deterministic card IDs/order, no trace-parity claim.
  - [x] Implement base Pummel (`PUMMEL`, ContentId(118)) as a deterministic uncommon attack: 1 energy, targeted enemy, 2 damage 4 times, exhausts, no trace-parity claim.
  - [x] Implement base Hemokinesis (`HEMOKINESIS`, ContentId(114)) as a deterministic uncommon attack: 1 energy, targeted enemy, lose 2 HP, 15 damage, no trace-parity claim.
  - [x] Implement base Blood for Blood (`BLOOD_FOR_BLOOD`, ContentId(116)) as a deterministic uncommon attack: 4 energy reduced by 1 per positive player HP-loss event this combat, targeted enemy, 18 damage, no trace-parity claim.
  - [x] Implement base Sentinel (`SENTINEL`, ContentId(127)) as a deterministic uncommon skill: 1 energy, no target, 5 block, gain 2 energy when exhausted, no trace-parity claim.
  - [x] Implement base Doubt (`Doubt`, ContentId(63)) as a deterministic curse/status: unplayable, end-turn 1 Weak per copy in hand after the normal player Weak tick, no trace-parity claim.
  - [x] Implement base Wound (`Wound`, ContentId(4)) as a deterministic inert status: unplayable, non-ethereal, normal end-turn discard or Runic Pyramid retention, Medical Kit status play/exhaust path through existing hooks, no trace-parity claim.
  - [x] Implement base Regret (`Regret`, ContentId(62)) as a deterministic curse/status: unplayable, non-ethereal, loses HP equal to current hand size once per Regret copy at end turn, normal discard or Runic Pyramid retention, Blue Candle curse play/exhaust path through existing hooks, no trace-parity claim.
  - [x] Implement base Dazed (`Dazed`, ContentId(5)) as a deterministic status: unplayable, ethereal, exhausts at end-turn before discard/Runic Pyramid retention, no trace-parity claim.
  - [x] Implement base Demon Form (`Demon Form`, ContentId(138)) as a deterministic rare power: 3 energy, no target, removed from hand on play, grants player Ritual 2, gains Strength through the existing local end-of-player-turn ritual lifecycle, no trace-parity claim.
  - [x] Implement base Entrench (`ENTRENCH`, ContentId(126)) as a deterministic uncommon skill: 2 energy, no target, doubles current player block without Dexterity/Frail modifiers, no trace-parity claim.
  - [x] Implement base Bloodletting (`BLOODLETTING`, ContentId(130)) as a deterministic uncommon skill: 0 energy, no target, lose 3 HP, gain 2 energy, no trace-parity claim.
  - [x] Implement base Carnage (`CARNAGE`, ContentId(131)) as a deterministic uncommon attack: 2 energy, targeted enemy, 20 damage, ethereal, no trace-parity claim.
  - [x] Implement base Ghostly Armor (`GHOSTLY_ARMOR`, ContentId(134)) as a deterministic uncommon skill: 1 energy, no target, 10 block, ethereal, no trace-parity claim.
  - [x] Implement base Bludgeon (`BLUDGEON`, ContentId(139)) as a deterministic rare attack: 3 energy, targeted enemy, 32 damage, no trace-parity claim.
  - [x] Implement base Impervious (`IMPERVIOUS`, ContentId(146)) as a deterministic rare skill: 2 energy, no target, 30 block, exhausts, no trace-parity claim.
  - [x] Implement base Limit Break (`LIMIT_BREAK`, ContentId(141)) as a deterministic rare skill: 1 energy, no target, doubles current signed player Strength, exhausts, no trace-parity claim.
  - [x] Implement base Offering (`OFFERING`, ContentId(151)) as a deterministic rare skill: 0 energy, no target, lose 6 HP, gain 2 energy, draw 3 cards, exhausts, no trace-parity claim.
  - [x] Implement base Armaments (`ARMAMENTS`, ContentId(105)) as a deterministic common skill: 1 energy, no target, 5 block, upgrades one non-source hand card with a local upgrade mapping through explicit-purpose hand-select state, no trace-parity claim.
  - [x] Implement base Headbutt (`HEADBUTT`, ContentId(106)) as a deterministic common attack: 1 energy, targeted enemy, 9 damage, optionally puts one selected discard-pile card on top of draw pile through explicit-purpose discard-select state, no trace-parity claim.
  - [x] Implement base Clothesline (`CLOTHESLINE`, ContentId(104)) as a deterministic common attack: 2 energy, targeted enemy, 12 damage, 2 weak, no trace-parity claim.
  - [x] Implement base Intimidate (`INTIMIDATE`, ContentId(115)) as a deterministic uncommon skill: 0 energy, 1 weak to all living enemies, exhaust, no trace-parity claim.
  - [x] Implement base Rage (`RAGE`, ContentId(125)) as a deterministic uncommon skill: 0 energy, no target, this-turn attack plays gain 3 block via turn-scoped local state, no trace-parity claim.
  - [x] Implement base Rampage (`RAMPAGE`, ContentId(121)) as a deterministic uncommon attack: 1 energy, targeted enemy, 8 damage, increases only the played card instance's future Rampage damage by 5 for the current combat, no trace-parity claim.
  - [x] Implement base Combust (`COMBUST`, ContentId(123)) as a deterministic uncommon power: 1 energy, no target, grants serialized end-turn HP-loss/all-enemy damage power state, removes the played power card through the existing power-card path, no trace-parity claim.
  - [x] Implement base Barricade (`BARRICADE`, ContentId(143)) as a deterministic rare power: 3 energy, no target, grants player block-retention power state, removes the played power card through the existing power-card path, and preserves block at turn-transition cleanup, no trace-parity claim.
  - [x] Implement base Evolve (`EVOLVE`, ContentId(136)) as a deterministic uncommon power: 1 energy, no target, grants serialized status-draw power state, removes the played power card through the existing power-card path, draws 1 extra card per stack when a Status card is drawn, no trace-parity claim.
  - [x] Implement base Berserk (`BERSERK`, ContentId(145)) as a deterministic rare power: 0 energy, no target, applies 2 player Vulnerable, grants serialized start-turn energy power state, and removes the played power card through the existing power-card path, no trace-parity claim.
  - [x] Implement base Brutality (`BRUTALITY`, ContentId(148)) as a deterministic rare power: 0 energy, no target, grants serialized start-turn HP-loss/draw power state, removes the played power card through the existing power-card path, no trace-parity claim.
  - [x] Implement base Double Tap (`DOUBLE_TAP`, ContentId(137)) as a deterministic rare skill: 1 energy, no target, next played Attack card replays effect actions once per stack without extra energy or extra card movement, no trace-parity claim.
  - [x] Implement base Second Wind (`SECOND_WIND`, ContentId(128)) as a deterministic uncommon skill: 1 energy, no target, exhaust all non-Attack hand cards except the source, gain 5 block per exhausted non-source card, move source to discard, no trace-parity claim.
  - [x] Implement base Fiend Fire (`FIEND_FIRE`, ContentId(144)) as a deterministic rare attack: 2 energy, targeted enemy, exhaust every other current hand card, deal 7 damage once per exhausted other hand card, exhaust source through the played-card exhaust path, no trace-parity claim.
- add focused unit tests for every newly completed surface
  - [x] Add Iron Wave definition, legality, transition, discard, and event-log tests.
  - [x] Add Body Slam definition, legality, transition, discard, current-block damage, zero-block, Akabeko interaction, and event-log tests.
  - [x] Add Clash definition, legality/playability predicate, transition, discard, damage, Akabeko interaction, and event-log tests.
  - [x] Add Wild Strike definition, legality, transition, discard, damage, Wound-to-draw-pile, local pile-order, Strength, Akabeko interaction, and event-log tests.
  - [x] Add Heavy Blade definition, legality, transition, discard, base damage, Strength multiplier, temp-Strength multiplier, Akabeko interaction, and event-log tests.
  - [x] Add Perfected Strike definition, legality, transition, current-combat-pile Strike count, upgraded Strike count, non-Strike exclusion, Strength, Akabeko, discard, and event-log tests.
  - [x] Add Power Through definition, legality, transition, block, Dexterity/Frail interaction, Wound-to-hand deterministic ID/order, Duplication Potion, Gremlin Nob enrage, discard, and event-log tests.
  - [x] Add Reckless Charge definition, legality, transition, zero-cost play, Dazed-to-draw-pile deterministic ID/order, Strength, Akabeko, discard, Havoc top-draw effect, and event-log tests.
  - [x] Add Pummel definition, legality, transition, four-hit damage, exhaust movement, Strength, Vulnerable, Akabeko, Pen Nib, Strange Spoon, Havoc top-draw effect, and event-log tests.
  - [x] Add Hemokinesis definition, legality, transition, HP loss, damage, discard movement, Strength, Akabeko, Pen Nib, Tungsten Rod, Buffer, Duplication Potion, and event-log tests.
  - [x] Add Blood for Blood definition, legality, target rejection, unreduced affordability, HP-loss-triggered cost reduction, dynamic effective-cost spending, Buffer prevention, serialization, damage/discard, and event-log tests.
  - [x] Add Sentinel definition, legality, normal-play block/discard, Dexterity/Frail, no normal-play exhaust energy, exhaust-trigger energy, non-Sentinel exhaust control, Feel No Pain interaction, exhaust-select, and event-log tests.
  - [x] Add Doubt definition/classification/unplayable, end-turn Weak, existing Weak lifecycle, Artifact, Ginger, multiple-copy, discard, and Runic Pyramid tests.
  - [x] Add Wound definition/unplayable/non-ethereal, no-play-action, serialization, end-turn discard, Runic Pyramid retention, Medical Kit, and existing generation-path tests.
  - [x] Add Regret definition/classification/unplayable/non-ethereal, no-play-action, serialization, end-turn hand-size HP loss, multiple-copy, mitigation, discard, Runic Pyramid, Blue Candle, and event acquisition tests.
  - [x] Add Dazed definition/unplayable/ethereal, end-turn exhaust, Runic Pyramid exhaust, and Wound non-ethereal control tests.
  - [x] Add Demon Form definition/rarity, legality, transition, effective-cost, event-log, Ritual gain, repeated end-turn Strength lifecycle, serialization, Bird-Faced Urn, and Unceasing Top tests.
  - [x] Add Entrench definition/rarity, legality, transition, zero/nonzero block, effective-cost, event-log, Dexterity/Frail exclusion, and Duplication Potion tests.
  - [x] Add Bloodletting definition, legality, transition, HP loss, energy gain, discard movement, event-log order, Tungsten Rod, and Buffer tests.
  - [x] Add Carnage definition, legality, transition, discard movement, Strength, Akabeko, Pen Nib, played discard, unplayed ethereal cleanup, Runic Pyramid, and event-log tests.
  - [x] Add Ghostly Armor definition, legality, transition, block, Dexterity/Frail interaction, played discard movement, unplayed ethereal end-turn behavior, and event-log tests.
  - [x] Add Bludgeon definition, legality, transition, discard, damage, Strength, Akabeko interaction, and event-log tests.
  - [x] Add Impervious definition, legality, transition, block, Dexterity/Frail interaction, exhaust-pile movement, and event-log tests.
  - [x] Add Limit Break definition, legality, transition, positive/zero/negative Strength, temporary Strength exclusion, effective-cost, exhaust-pile movement, and event-log tests.
  - [x] Add Offering definition, legality, transition, HP loss, energy gain, draw, exhaust-pile movement, Tungsten Rod, Buffer, Centennial Puzzle, Runic Cube, and event-log tests.
  - [x] Add Armaments definition, legality, target rejection, unaffordable, block, hand-select purpose, Warcry regression, hand-select filtering, selected-card upgrade, stale selection rejection, no-upgradeable fallback, and discard tests.
  - [x] Add Headbutt definition, legality, target rejection, unaffordable, empty-discard, lethal-selection skip, discard-select purpose serialization, selected discard-card-to-draw-top, Liquid Memories regression, and discard movement tests.
  - [x] Add Clothesline definition, legality, transition, discard, and event-log tests.
  - [x] Add Intimidate definition, legality, all-living-enemy, dead-enemy skip, exhaust, and event-log tests.
  - [x] Add Rage definition, legality/target rejection, transition, turn-scoped stacking, attack-only trigger, Havoc/top-draw attack trigger, turn expiry, serialization, and event-log tests.
  - [x] Add Rampage definition/rarity, legality/missing-target/energy, transition, effective-cost spending, per-card-instance growth, serialization, Strength/Vulnerable, Akabeko/Pen Nib, and event-log tests.
  - [x] Add Combust definition/rarity, legality/target rejection, transition, effective-cost spending, power removal, serialized power state, end-turn HP loss/all-enemy damage, stacking, lethal reward transition, Bird-Faced Urn, Unceasing Top, and event-log tests.
  - [x] Add Barricade definition/rarity, legality/target rejection, transition, effective-cost spending, power removal, serialized power state, turn-transition block retention, Calipers precedence, Bird-Faced Urn, Unceasing Top, and event-log tests.
  - [x] Add Evolve definition/rarity, legality/target rejection, transition, effective-cost spending, power removal, serialized power state, status-card extra draw, stacked draw, chained status draw, start-turn refill, Bird-Faced Urn, Unceasing Top, and event-log tests.
  - [x] Add Berserk definition/rarity, legality/target rejection, transition, self-Vulnerable, Artifact, effective-cost spending, power removal, serialized power state, start-turn energy, Ice Cream, Bird-Faced Urn, Unceasing Top, and event-log tests.
  - [x] Add Brutality definition/rarity, legality/target rejection, transition, effective-cost spending, power removal, serialized power state, start-turn HP loss/draw, stacking, Bird-Faced Urn, Unceasing Top, and event-log tests.
  - [x] Add Double Tap definition/rarity, legality/target rejection, transition, effective-cost spending, serialized pending replay state, Gremlin Nob skill hook, next-Attack replay consumption, stacking, non-Attack non-consumption, and event-log tests.
  - [x] Add Second Wind definition, legality, no-target rejection, unaffordable, non-Attack exhaust filtering, aggregate block, source discard, Sentinel/Feel No Pain/Dark Embrace hook interaction, and event-log tests.
  - [x] Add Fiend Fire definition, legality, target rejection, unaffordable, hit-count, zero-other-card, source exclusion, other-card exhaust, Sentinel/Feel No Pain/Dark Embrace hook interaction, Strange Spoon source-only behavior, and event-log tests.
- add serialization and counter round-trip tests for newly stateful surfaces
  - [x] No new stateful surface in the Iron Wave slice.
  - [x] No new stateful surface in the Body Slam slice.
  - [x] No new stateful surface in the Clash slice.
  - [x] No new stateful surface in the Wild Strike slice.
  - [x] No new stateful surface in the Heavy Blade slice.
  - [x] No new stateful surface in the Pummel slice.
  - [x] No new stateful surface in the Hemokinesis slice.
  - [x] No new stateful surface in the Sentinel slice.
  - [x] No new stateful surface in the Doubt slice.
  - [x] No new stateful surface in the Wound slice.
  - [x] No new stateful surface in the Regret slice.
  - [x] No new stateful surface in the Dazed slice.
  - [x] Demon Form uses existing player Ritual power state; serialization coverage was added for the promoted behavior.
  - [x] No new stateful surface in the Entrench slice; block doubling uses existing player block state.
  - [x] No new stateful surface in the Bloodletting slice.
  - [x] No new stateful surface in the Carnage slice.
  - [x] No new stateful surface in the Ghostly Armor slice.
  - [x] No new stateful surface in the Bludgeon slice.
  - [x] No new stateful surface in the Impervious slice.
  - [x] No new stateful surface in the Limit Break slice; signed Strength uses existing player power state.
  - [x] No new stateful surface in the Offering slice; HP loss, energy, draw, and exhaust use existing combat state/hooks.
  - [x] Armaments uses explicit-purpose hand-select state and mutates existing hand card content IDs for combat-local upgrades.
  - [x] Headbutt uses explicit-purpose discard-select state; Liquid Memories retains its separate discard-select purpose.
  - [x] Rage uses turn-scoped player combat state and includes player-state serialization coverage.
  - [x] Rampage adds serialized card-instance state for combat-local damage growth.
  - [x] Combust adds serialized player power state for end-turn HP loss and all-enemy damage.
  - [x] Blood for Blood adds serialized card-instance state for combat-local cost reduction.
  - [x] Barricade adds serialized player power state for block retention.
  - [x] Evolve adds serialized player power state for status-card extra draw.
  - [x] Berserk adds serialized player power state for start-turn energy gain.
  - [x] Brutality adds serialized player power state for start-turn HP loss and draw.
  - [x] Double Tap adds serialized combat-local pending next-Attack replay state.
  - [x] Fire Breathing adds serialized player power state for Status/Curse draw-trigger damage.
- add interaction tests when a surface touches existing card, relic, power, potion, pile, reward, or room hooks
  - [x] Iron Wave covers damage/block/pile movement through the existing card queue and transition hooks.
  - [x] Body Slam covers current-block-derived damage, attack damage modifiers, and pile movement through the existing card queue and transition hooks.
  - [x] Clash covers card-type-sensitive legal action generation plus damage/pile movement through the existing card queue and transition hooks.
  - [x] Wild Strike covers generated status-card insertion, attack damage modifiers, and pile movement through the existing card queue and transition hooks.
  - [x] Heavy Blade covers multi-Strength-scaling attack damage and pile movement through the existing card queue and transition hooks.
  - [x] Perfected Strike covers combat-pile-derived attack damage, attack damage modifiers, and pile movement through the existing card queue and transition hooks.
  - [x] Power Through covers generated status-card insertion, block modifiers, duplication, skill-play hooks, and pile movement through the existing card queue and transition hooks.
  - [x] Reckless Charge covers generated status-card insertion, attack damage modifiers, zero-cost play, Havoc top-draw effects, and pile movement through the existing card queue and transition hooks.
  - [x] Pummel covers multi-hit attack damage, attack damage modifiers, exhaust movement, Strange Spoon exhaust retention, Havoc top-draw effects, and pile movement through the existing card queue and transition hooks.
  - [x] Hemokinesis covers HP-loss mitigation/prevention, generic attack damage modifiers, local Duplication Potion behavior, and pile movement through existing card queue and transition hooks.
  - [x] Blood for Blood covers HP-loss hooks across combat piles, dynamic effective-cost spending, generic attack damage modifiers, and pile movement through existing card queue and transition hooks.
  - [x] Sentinel covers generic skill block, block modifiers, normal discard movement, and card-aware exhaust hooks through existing card queue and transition hooks.
  - [x] Doubt covers end-turn hand curse behavior, player Weak lifecycle, Artifact/Ginger prevention, multiple-copy stacking, discard movement, and Runic Pyramid retention through existing hand/turn hooks.
  - [x] Wound covers inert unplayable status behavior, normal discard, Runic Pyramid retention, Medical Kit status play/exhaust, and existing Wild Strike/Power Through/Mark of Pain generation hooks.
  - [x] Regret covers end-turn HP loss, HP-loss mitigation/hooks, multiple-copy triggering, normal discard, Runic Pyramid retention, Blue Candle curse play/exhaust, and existing Big Fish acquisition hooks.
  - [x] Dazed covers unplayed ethereal status cleanup, exhaust movement, Runic Pyramid interaction, and Wound non-ethereal contrast through existing hand cleanup hooks.
  - [x] Demon Form covers power-card removal, effective cost, power-play relic hooks, hand-empty draw hooks, Ritual power persistence, and repeated end-turn Strength gain through existing card queue and turn-power hooks.
  - [x] Entrench covers dynamic block doubling, effective cost, skill-play hooks, Duplication Potion sequential doubling, and pile movement through existing card queue and transition hooks.
  - [x] Bloodletting covers HP-loss mitigation/prevention, energy gain, skill-play hooks, and pile movement through the existing card queue and transition hooks.
  - [x] Carnage covers generic attack damage modifiers, played discard movement, and unplayed ethereal cleanup through existing card queue and hand cleanup hooks.
  - [x] Ghostly Armor covers generic skill block, block modifiers, played discard movement, and unplayed ethereal end-turn exhaust behavior through existing card queue and hand cleanup hooks.
  - [x] Bludgeon covers high-cost targeted attack damage and pile movement through the existing card queue and transition hooks.
  - [x] Impervious covers high-block skill behavior, block modifiers, and exhaust movement through the existing card queue and transition hooks.
  - [x] Limit Break covers signed Strength doubling, temporary Strength exclusion, effective-cost spending, and exhaust movement through existing card queue and transition hooks.
  - [x] Offering covers HP-loss mitigation/prevention, HP-loss draw relic hooks, energy gain, draw, Havoc top-draw effect, and exhaust movement through existing card queue and transition hooks.
  - [x] Armaments covers block modifiers, hand-select purpose serialization, hand-select filtering, selected hand-card content upgrade, and no-upgradeable fallback through existing card queue and selection hooks.
  - [x] Headbutt covers targeted attack damage, explicit discard-select purpose serialization, selected discard-to-draw-pile movement, lethal-selection skip, and played-card discard movement through existing card queue and selection hooks.
  - [x] Rage covers turn-scoped state serialization, Skill play hooks, later Attack play hooks, Havoc top-draw attacks, block modifiers through the local block calculation path, discard movement, and turn reset.
  - [x] Rampage covers per-card-instance growth, effective-cost spending, attack damage modifiers, damage relic queue modifiers, and discard movement through existing card queue and transition hooks.
  - [x] Combust covers effective-cost spending, power-card removal, power-play relic hooks, hand-empty draw hooks, and end-turn HP-loss/all-enemy damage through existing card queue, HP-loss, damage, and turn hooks.
  - [x] Barricade covers effective-cost spending, power-card removal, power-play relic hooks, hand-empty draw hooks, turn-transition block retention, and Calipers precedence through existing card queue and turn cleanup hooks.
  - [x] Evolve covers effective-cost spending, power-card removal, power-play relic hooks, hand-empty draw hooks, stacked/chained status-card extra draw hooks, and start-turn refill through existing card queue, draw, and turn hooks.
  - [x] Berserk covers self-debuff Artifact handling, effective-cost spending, power-card removal, power-play relic hooks, hand-empty draw hooks, and start-turn energy with Ice Cream through existing card queue and turn hooks.
  - [x] Brutality covers effective-cost spending, power-card removal, power-play relic hooks, hand-empty draw hooks, and start-turn HP-loss/draw stacks through existing card queue, HP-loss, draw, and turn hooks.
  - [x] Double Tap covers effective-cost spending, Skill-play hooks, discard movement, serialized replay state, and next-Attack effect replay through existing card queue and transition hooks.
  - [x] Fire Breathing covers effective-cost spending, power-card removal, power-play relic hooks, hand-empty draw hooks, serialized damage stacks, and Status/Curse draw-trigger all-enemy damage through the shared draw hook.
  - [x] Feed covers base rare attack definition, target legality, attack damage modifiers, fatal Feed-scoped current/max-HP gain, exhaust movement, and monster-death relic hooks through existing damage and card queue hooks.
- update the support matrix status and caveats as each surface moves from `inventory_only` or `placeholder` to `implemented`
  - [x] Move Iron Wave from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Body Slam from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Clash from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Wild Strike from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Heavy Blade from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Perfected Strike from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Power Through from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Reckless Charge from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Pummel from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Hemokinesis from `placeholder` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Blood for Blood from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Sentinel from `placeholder` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Doubt from `placeholder` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Wound from `placeholder` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Regret from `placeholder` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Dazed from `placeholder` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Demon Form from `placeholder` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Entrench from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Bloodletting from `unsupported` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Carnage from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Ghostly Armor from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Bludgeon from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Impervious from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Limit Break from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Offering from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Armaments from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Headbutt from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Clothesline from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Intimidate from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Rage from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Rampage from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Combust from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Barricade from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Evolve from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Berserk from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Brutality from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Double Tap from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Rupture from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Fire Breathing from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Feed from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Second Wind from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Move Fiend Fire from `inventory_only` to `implemented` in `simulator/docs/m32a_cards_matrix.md`.
  - [x] Complete the remaining deterministic colorless-card sweep entries in `simulator/docs/m32a_cards_matrix.md`: Discovery, Enlightenment, Forethought, Jack Of All Trades, Madness, Mind Blast, Master of Strategy, The Bomb, Thinking Ahead, Transmutation, Metamorphosis, Secret Technique, Sword Boomerang, Violence, Mayhem, Panache, Secret Weapon, Panic Button, Purity, Apotheosis, Chrysalis, Hand Of Greed, Magnetism, and Sadistic Nature.
- leave explicit expected-failing parity notes for surfaces whose behavior is implemented locally but not yet proven against real-game traces
  - [x] Iron Wave matrix caveat names the missing real-game trace parity evidence.
  - [x] Body Slam matrix caveat names the missing real-game trace parity evidence.
  - [x] Clash matrix caveat names the missing real-game trace parity evidence.
  - [x] Wild Strike matrix caveat names the missing real-game trace parity and exact Wound draw-pile shuffle-order evidence.
  - [x] Heavy Blade matrix caveat names the missing real-game trace parity evidence.
  - [x] Perfected Strike matrix caveat names the missing master-deck count and real-game trace parity evidence.
  - [x] Power Through matrix caveat names the missing real-game trace parity and exact full-hand/generated-Wound evidence.
  - [x] Reckless Charge matrix caveat names the missing real-game trace parity and exact Dazed draw-pile shuffle-order evidence.
  - [x] Pummel matrix caveat names the missing real-game trace parity evidence.
  - [x] Hemokinesis matrix caveat names the missing played-card real-game trace parity, decoded action-order evidence, and Duplication Potion source timing evidence.
  - [x] Blood for Blood matrix caveat names the missing played-card real-game trace parity, source-backed exact HP-loss trigger semantics/action-order evidence, and upgraded behavior.
  - [x] Sentinel matrix caveat names the missing played-card real-game trace parity, source-backed exhaust-order evidence, and upgraded Sentinel behavior.
  - [x] Doubt matrix caveat names the missing real-game trace parity and source-backed ordering/duplicate-trigger evidence.
  - [x] Wound matrix caveat names the missing real-game trace parity and source-backed exact Wound timing evidence.
  - [x] Regret matrix caveat names the missing standalone real-game trace parity and source-backed exact end-turn timing evidence.
  - [x] Dazed matrix caveat names the missing real-game trace parity and source-backed exact ethereal timing evidence.
  - [x] Demon Form matrix caveat names the missing played-card real-game trace parity and source-backed exact power lifecycle/timing evidence.
  - [x] Entrench matrix caveat names the missing played-card real-game trace parity and source-backed exact action-order evidence.
  - [x] Bloodletting matrix caveat names the missing played-card real-game trace parity and source-backed action-order evidence.
  - [x] Carnage matrix caveat names the missing played-card real-game trace parity and exact ethereal timing evidence.
  - [x] Ghostly Armor matrix caveat names the missing real-game trace parity and exact ethereal timing evidence.
  - [x] Bludgeon matrix caveat names the missing real-game trace parity evidence.
  - [x] Impervious matrix caveat names the missing real-game trace parity evidence.
  - [x] Limit Break matrix caveat names the missing played-card real-game trace parity, source-backed exact action-order evidence, upgraded behavior, and local signed Strength semantics.
  - [x] Offering matrix caveat names the missing played-card real-game trace parity, source-backed exact action-order evidence, and upgraded behavior.
  - [x] Armaments matrix caveat names the missing played-card real-game trace parity, source-backed exact action-order evidence, upgraded behavior, local upgrade-mapping limit, and Havoc top-draw selection behavior.
  - [x] Headbutt matrix caveat names the missing played-card real-game trace parity, source-backed exact action-order evidence, upgraded behavior, local draw-pile top ordering, and local lethal-selection behavior.
  - [x] Clothesline matrix caveat names the missing real-game trace parity evidence.
  - [x] Intimidate matrix caveat names the missing real-game trace parity evidence.
  - [x] Rage matrix caveat names the missing real-game trace parity, source-backed exact trigger ordering/modifier evidence, and upgraded behavior.
  - [x] Rampage matrix caveat names the missing played-card real-game trace parity, source-backed exact card-UUID/copy semantics, and upgraded behavior.
  - [x] Combust matrix caveat names the missing played-card real-game trace parity, source-backed exact end-turn timing/source evidence, stacking semantics, and upgraded behavior.
  - [x] Barricade matrix caveat names the missing played-card real-game trace parity, source-backed exact power lifecycle/timing evidence, and upgraded behavior.
  - [x] Evolve matrix caveat names the missing played-card real-game trace parity, source-backed exact draw-trigger timing/source evidence, and upgraded behavior.
  - [x] Berserk matrix caveat names the missing played-card real-game trace parity, source-backed exact power lifecycle/timing evidence, and upgraded behavior.
  - [x] Brutality matrix caveat names the missing played-card real-game trace parity, source-backed exact start-turn timing/source evidence, and upgraded behavior.
  - [x] Double Tap matrix caveat names the missing played-card real-game trace parity, source-backed exact replay timing/source evidence, upgraded behavior, and Duplication Potion combination evidence.
  - [x] Rupture matrix caveat names the missing played-card real-game trace parity, source-backed exact source/action-order evidence, and upgraded behavior.
  - [x] Feed matrix caveat names the missing played-card real-game trace parity, upgraded behavior, local current-HP convention, and unmodeled minion/half-dead fatal exclusions.

Acceptance tests:

- every matrix entry marked deterministic-and-in-scope is either implemented with focused tests or has a documented blocker
- no new implementation depends on observed-state restoration to pass unit tests
- legal action generation, serialization, hashing, and observation extraction remain RNG-free
- `cargo fmt`, `cargo clippy`, and `cargo test` pass from `simulator/`

Do not implement:

- RNG-sensitive behavior from wiki-only evidence
- action-queue timing guesses for complex interactions
- broad Act 2/3/4 seed-start parity
- ascension expansion beyond the currently declared A0 sweep

## Milestone 32C: Parity Evidence Backfill for Inventory-Completed Content

Status: planned.

Goal: turn locally implemented inventory-complete content into real-game-backed parity claims where possible.

Tasks:

- prioritize matrix entries that are implemented but lack source-backed or trace-backed evidence
- capture targeted CommunicationMod traces or controlled experiments for unverified bosses, events, potions, relic hooks, card interactions, and monster moves
- decode target bytecode/source behavior for RNG-sensitive systems where trace observation alone is insufficient
- add minimized regression fixtures for every parity bug found during backfill
- update support matrix evidence levels from `wiki/reference` or `unit_only` to `trace_backed`, `source_backed`, or `source_and_trace_backed`
- require every newly passing trace to avoid observed-state restoration inside its declared scope

Acceptance tests:

- high-risk matrix entries have either trace/source evidence or an explicit non-blocking caveat
- each newly proven surface has at least one regression test or captured trace reference
- verifier output names remaining unverified surfaces rather than treating them as unknown
- nightly parity includes any new passing targeted traces

Do not implement:

- new content that was not identified by Milestone 32A unless it is required to reproduce a parity bug
- arbitrary win-rate claims
- non-Ironclad or modded parity

## Milestone 33: Neow Generalization

Status: in progress. Initial seam extraction is complete: captured seed-start Neow branch tables now live behind a first-class `sts_core::run::neow` facade, with focused tests preserving VERIFY01/CODEX03/CODEX04/TEST/M290001/M290008 behavior. Source-backed Ironclad A0 option generation is now implemented behind that facade: target jar bytecode pins `NeowEvent.rng = Random(Settings.seed)`, the slot order, slot reward/drawback pools, and the five option-screen RNG draws. Normal `THREE_CARDS` generation is source-backed as common/uncommon only with target `0.33f` uncommon rolls on `NeowEvent.rng`; forced-rare card reward generation for `ONE_RANDOM_RARE_CARD` and `THREE_RARE_CARDS` is source-backed on `NeowEvent.rng` with explicit counter tests. Colorless reward generation for `RANDOM_COLORLESS` and `RANDOM_COLORLESS_2` is source-backed as a split-stream helper: rarity burns on `NeowEvent.rng`, identity draws on `cardRng`, and duplicate retries consume only `cardRng`; CODEX04/TEST seed-start colorless reward choices now use this generator. Neow's three-potion reward is source-backed as three direct full-pool `potionRng` draws, not reward-rarity potion logic. Boss-swap application now removes Burning Blood before popping the initialized boss relic pool, preserving relic-pool initialization as the only relic RNG consumption. Remaining reward application stays captured-branch scoped until the decoded potion, curse, transform, normal-card, colorless, and relic helper paths are wired into seed-start verification.

Goal: replace captured-branch Neow handling with source-backed option generation and reward application for Ironclad A0.

Tasks:

- decode target Neow option generation and reward selection
- implement all Ironclad A0 Neow options, costs, and rewards
- replace captured-branch-only verifier handling
- add seed-start tests for several seeds with different option sets
- document any remaining unobservable Neow assumptions

Acceptance tests:

- selected Neow traces pass without captured-branch special cases
- unchosen Neow branches are generated from source-backed logic
- Neow RNG boundaries no longer report captured-branch-only status for Ironclad A0

Do not implement:

- all ascension-specific Neow variants unless needed for A0 parity
- boss-swap downstream relic interactions beyond Ironclad A0 scope

## Milestone 34: Shuffle and Deck Generalization

Status: planned.

Goal: remove trace fallback for opening piles and post-END pile resync.

Tasks:

- decode remaining master-deck ordering for innate, extra-card, transformed, removed, upgraded, and obtained cards
- derive opening piles from seed and deck state for all selected traces
- remove post-END pile resync scaffolding
- add regression tests for starter-only, innate-card, colorless-card, and modified-deck openings
- update RNG boundary docs once fallback is removed

Acceptance tests:

- seed-start combat setup never restores hand/draw/discard/exhaust piles from trace for selected Ironclad traces
- no post-END pile resync is required for passing traces
- shuffle RNG counters and pile orders match captured CommunicationMod states

Do not implement:

- non-Ironclad deck-order special cases
- modded card pools

## Milestone 35: Full Act 1 Corpus

Status: planned.

Goal: prove Act 1 robustness across a diverse Ironclad A0 corpus.

Tasks:

- collect 5-10 full Act 1 Ironclad A0 traces with varied Neow choices, paths, shops, events, elites, bosses, relics, cards, and potions
- add all passing traces to nightly parity
- minimize and store failing prefixes for unresolved divergences
- document formally waived unobservable assumptions
- ensure no selected trace uses observed-state restoration

Acceptance tests:

- selected Act 1 traces pass through Act 1 boss reward with `seed_start.expected_failure=false`
- all passing traces report `unexpected_diffs=0`
- nightly parity fails on any regression in the Act 1 corpus

Do not implement:

- Act 2 progression
- arbitrary seed win-rate claims

## Milestone 36: Act 2 Support

Status: planned.

Goal: extend source-backed seed-start parity through Act 2 boss reward for at least one Ironclad A0 trace.

Tasks:

- implement Act 2 map generation, room pools, events, encounters, elites, bosses, rewards, shops, rests, and chests as reached by selected traces
- capture or select at least one trace through Act 2 boss reward
- add Act 2 monster AI and special mechanics reached by the trace
- add divergence minimization for Act 2-specific failures
- add passing Act 2 trace to nightly parity

Acceptance tests:

- selected trace passes from `START` through Act 2 boss reward
- Act 2 map, encounter, reward, and room RNG boundaries are source-backed or explicitly scoped
- `unexpected_diffs=0` for the selected passing Act 2 trace

Do not implement:

- Act 3 progression
- all Act 2 branches not reached by selected traces

## Milestone 37: Act 3 Support

Status: planned.

Goal: extend source-backed seed-start parity through Act 3 boss reward or heart-key transition for at least one Ironclad A0 trace.

Tasks:

- implement Act 3 map generation, room pools, events, encounters, elites, bosses, rewards, shops, rests, and chests as reached by selected traces
- capture or select at least one trace through Act 3 boss reward
- add Act 3 monster AI and special mechanics reached by the trace
- verify boss reward and end-of-act transition
- add passing Act 3 trace to nightly parity

Acceptance tests:

- selected trace passes from `START` through Act 3 boss reward or declared heart-key transition
- Act 3 RNG boundaries are source-backed or explicitly scoped
- `unexpected_diffs=0` for the selected passing Act 3 trace

Do not implement:

- Act 4 unless selected trace reaches it
- non-Ironclad characters

## Milestone 38: Act 4 Support

Status: planned.

Goal: simulate Ironclad Act 4 through end-of-run for captured heart runs.

Tasks:

- implement key acquisition effects and constraints
- implement Act 4 map/room sequence, final shop/rest handling, Shield and Spear, and Heart
- implement Beat of Death, Invincible, artifact, buff, debuff, and multi-enemy special mechanics reached by selected traces
- capture or select at least one Heart run
- add passing Act 4 trace to nightly parity

Acceptance tests:

- selected trace passes from `START` through run completion
- Shield/Spear and Heart state transitions match captured states
- end-of-run resolution is represented in verifier output

Do not implement:

- non-heart ending variants beyond selected trace evidence
- score/stat screens unless needed for trace parity

## Milestone 39: Broad Ironclad Regression Corpus

Status: planned.

Goal: make complete Ironclad A0 seed-start parity robust across a broad corpus.

Tasks:

- collect a corpus of complete Ironclad A0 traces with diverse seeds, routes, bosses, relics, shops, events, cards, potions, and endings
- run the corpus in nightly parity without observed-state restoration
- require minimized failing prefixes for every new unexpected diff
- track coverage by card, relic, potion, monster, event, and room type
- document remaining unsupported surfaces as explicit non-goals

Acceptance tests:

- all selected full-run traces pass with `seed_start.expected_failure=false`
- all selected full-run traces report `unexpected_diffs=0`
- corpus coverage reports include major Ironclad cards, relics, potions, monsters, events, and bosses

Do not implement:

- win-rate claims
- arbitrary mod support
- non-Ironclad parity

## Milestone 40: Full Ironclad Simulator Readiness

Status: planned.

Goal: declare the scoped full Ironclad simulator ready for unmodded A0 trace replay.

Tasks:

- audit all Ironclad card, potion, relic, monster, event, room, map, reward, shop, rest, and RNG surfaces
- remove or formally waive all captured-branch and trace-fallback scaffolding in the supported scope
- publish a clear support matrix for implemented, waived, and unsupported surfaces
- run the full regression corpus and nightly parity
- document how to capture new traces and triage divergences

Acceptance tests:

- support matrix has no unknown Ironclad A0 core-game surfaces inside declared scope
- full regression corpus passes with `unexpected_diffs=0`
- verifier documentation explains remaining assumptions and how to reproduce parity checks

Do not implement:

- ascension expansion beyond A0
- Defect/Silent/Watcher parity
- modded-game support
