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

Status: complete. Captured-target coverage spans `trace-2026-06-18T16-50-50-232Z.jsonl`, `trace-2026-06-18T16-45-23-530Z.jsonl`, and `trace-2026-06-18T06-04-49-264Z.jsonl`. Full VERIFY01/CODEX04/CODEX03 map topology, map-choice prefixes, chosen combat paths, normal encounter list prefixes, and first-three combat-entry spawn state (roster, HP, block, intent, powers) are source-backed. Room execution maps combat index to normal encounter list entries. Seed-start reports include `m22_encounter_report` with three verified first-combat entries for VERIFY01/CODEX04/CODEX03. CODEX04 seed-start still stops at the first unsupported combat command; that is Milestone 23 scope.

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

Status: in progress. Observed-state CODEX04 floor 1–3 combat parity passes with `unexpected_diffs=0`; END transitions are verified. Seed-start CODEX04 now verifies floors 1–3 combat via simulation (opening piles pinned from trace entry, post-END resync, reward/map steps observed-only). Pure `shuffleRng` opening-hand parity remains open.

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

## Milestone 24: Reward, Potion, Relic, Shop, Rest, and Event RNG Parity

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
