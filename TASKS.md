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
- first full Act 1 trace
- minimized divergence corpus
- nightly parity runner

Do not claim seeded-run parity until exact real-game traces pass from seed plus action trace.
