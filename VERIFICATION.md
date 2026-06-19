# Verification Strategy

## Principle

The simulator is not correct because its tests pass. It is correct only to the extent that tests prove specific mechanics and traces match the target game version.

Verification is staged:

1. Unit correctness for tiny local mechanics.
2. Golden tests for complete small transitions.
3. Deterministic replay from seed plus action trace.
4. Real-game state comparison through CommunicationMod-style exports.
5. Distribution checks for systems where exact hidden state is not yet observable.

## Real-Game Comparison

The best current harness is [CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod). Its protocol sends JSON game state when the game is stable and accepts external commands. [spirecomm](https://github.com/ForgottenArbiter/spirecomm) demonstrates client-side use.

Build a verifier that can:

- launch or connect to a real game through CommunicationMod
- send a scripted action
- capture the stable JSON state after each decision
- normalize the real-game state into the simulator's canonical snapshot schema
- apply the same action to the simulator
- diff canonical state after each step

For early work, manual captured JSON fixtures are enough. Automation comes later.

Current Milestone 12 observed-state replay command:

```powershell
cd simulator
cargo run -p sts_verify -- parity ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
```

This mode restores simulator state from each observed real pre-state, applies the matching CommunicationMod action, and compares a supported canonical post-state subset. It verifies the captured trace's supported combat/reward mechanics: Bash, Strike, Defend, end turn, Cultist attack/ritual behavior where currently modeled, Burning Blood heal, gold reward pickup, and Twin Strike pickup.

Observed-state mode does not verify seed-start RNG parity. Use seed-start mode for the captured `VERIFY01` trace. Broad game-compatible RNG remains bounded to the captured branches and must stay classified until later work implements it generally:

- `START IRONCLAD 0 VERIFY01` seed/bootstrap parity
- Neow option/reward RNG
- map generation and node RNG
- encounter selection and monster HP RNG
- exact reward gold/card RNG
- unmodeled reward cards such as Heavy Blade and Intimidate

Current seed-start harness command:

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T06-04-49-264Z.jsonl
```

This mode parses the real `START IRONCLAD 0 VERIFY01` command and verifies the captured Ironclad A0 trace through return to map without restoring from observed pre-state. It verifies the selected Neow path, first map choice, first Cultist encounter entry, captured Cultist combat through lethal Strike, captured reward offer, gold pickup, card reward choices, Twin Strike pickup, and post-reward `PROCEED`. For the captured trace, it reports `seed_start.expected_failure=false`, `seed_start.first_boundary.path=$.actions[complete]`, and `unexpected_diffs=0`.

The same seed-start mode also covers the captured `CODEX04` Neow opening through the first map-choice screen:

```powershell
cd simulator
cargo run -p sts_verify -- parity --mode seed-start ..\verification\corpus\communication_mod\trace-2026-06-18T16-50-50-232Z.jsonl
```

For `CODEX04`, it verifies talk, the captured colorless-card reward choices `Deep Breath`, `Dramatic Entrance`, and `Jack Of All Trades`, picking `Dramatic Entrance`, leaving Neow with that card in the deck, and taking the captured first map choice into a source-backed 54/54 HP Cultist encounter. It intentionally stops before executing the first CODEX04 combat command: the current boundary is `unsupported_combat_path`, with `unexpected_diffs=0`. The corpus tests separately pin CODEX04's first three observed map/encounter targets so exact map, encounter, and monster HP RNG work has executable targets even before seed-start can play through all intervening combat and rewards.

The seed-start report includes named RNG boundaries for the captured traces: seed conversion, Neow, map, encounter selection, monster HP, shuffle, card reward, reward gold, relic, and potion streams. Save-counter names are included where current research identifies likely real save fields. Captured branches are modeled narrowly: VERIFY01's Toy Ornithopter branch through the captured post-reward map return, and CODEX04's colorless-card Neow branch through the first captured map choice and 54-HP Cultist entry. Map RNG now has source-backed Act 1 topology choices from target `MapGenerator` with `mapRng = seed + actNum`, including CODEX04's first two selected nodes; fixed target rows 0 combat, 8 treasure, and 14 rest are source-backed from `AbstractDungeon.generateMap()`; desired room counts, two-stage room-list construction, raw `Collections.shuffle` room-list prefix, full CODEX04 captured room-symbol placement, and CODEX04 `FixedMap` traversal are source-backed from `Exordium.initializeLevelSpecificChances()` / `generateRoomTypes` / `RoomTypeAssigner.distributeRoomsAcrossMap` plus captured map symbols. Encounter selection now has source-backed Exordium normal-list generation for weak encounters, strong encounter weights, first-strong exclusions, and no-repeat-last-two retries. Monster HP now has a source-backed target `RandomXS128` wrapper, decoded ranges for the reached Act 1 monsters, floor-1 Cultist HP parity for `VERIFY01` and `CODEX04`, CODEX04 floor-2 Small Slimes HP parity, and CODEX04 floor-3 louse HP parity from `miscRng` plus `monsterHpRng = seed + floorNum`. Broad real-game Neow, elite encounter selection, alternate unreached branches, shuffle, and reward RNG remain later milestone work.

Seed conversion status:

- External seed string captured: `VERIFY01`.
- Exact numeric seed conversion: implemented from the target `SeedHelper.getLong(String)` bytecode in the local `12-18-2022` desktop jar. Seeds are uppercased, `O` maps to `0`, and characters are parsed in base 35 using `0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ`.
- Current evidence in this repo: `RESEARCH.md` records the target jar/class inspected and captured checks for `VERIFY01`, `CODEX03`, and `CODEX04`.
- The harness reports seed conversion as `source_backed`; broader RNG stream parity remains bounded by the later stream-specific milestones.

Also inspect [silentcoder99/sts_lightspeed](https://github.com/silentcoder99/sts_lightspeed), whose repository description says it integrates `sts_lightspeed` with CommunicationMod. If it contains reusable trace ideas, document them before building our own bridge.

## Differential Prior-Art Comparison

`sts_lightspeed` is not the real game, but it is too relevant to ignore.

Use it as a secondary oracle for:

- RNG stream names and counters
- seed string conversion
- save-file RNG restoration
- map generation
- reward generation
- shop generation
- monster move selection
- action/card queue ordering

Rules:

- A match with `sts_lightspeed` is supporting evidence, not proof.
- A mismatch with `sts_lightspeed` should trigger investigation, not automatic imitation.
- If `sts_lightspeed` and the real game disagree, the real game wins.
- Any behavior copied conceptually from prior art must be backed by our own tests and documented source notes.

Add a future tool that can run the same compact scenario through:

- this Rust simulator
- `sts_lightspeed`, where practical
- CommunicationMod/real game, where practical

Then produce a three-way canonical diff.

## RunLogger-Style Traces

RunLogger-style output, if available, should be treated as an action/outcome dataset, not as full truth.

Useful fields:

- seed
- character
- ascension
- floor path
- deck changes
- relics
- potions
- rewards offered and chosen
- combat outcomes
- HP/gold changes

Not enough for exact transition parity:

- hidden RNG stream positions
- full draw/discard/hand ordering at every action
- monster move history unless logged
- action queue internals
- transient per-card costs and combat-only state

Use run logs to build regression corpora and distribution checks after low-level mechanics are verified.

Run-history sources to investigate:

- RunHistoryPlus
- Slay the Spire's native run-history JSON
- `MaT1g3R/Slay-the-Spire-data`
- datasets referenced by map/path analysis papers

Expected use:

- deck/path/reward distribution checks
- high-level run outcome regression
- seeds and deck snapshots for reproducer hunting

Not expected use:

- exact hand/draw/discard parity
- action queue parity
- hidden RNG stream position parity

## Snapshot Format

Use JSON Lines for traces:

```json
{"type":"metadata","schema":1,"game_version":"TBD","sim_version":"TBD","source":"communication_mod"}
{"type":"state","step":0,"state_hash":"...","snapshot":{}}
{"type":"action","step":1,"action":{}}
{"type":"rng","step":1,"draws":[]}
{"type":"state","step":1,"state_hash":"...","snapshot":{}}
```

Snapshot kinds:

- `full`: simulator-only exact resume.
- `observed`: normalized from real-game exported state.
- `debug`: includes logs, RNG draws, and noncanonical fields.

Every snapshot must include:

- schema version
- target game version
- simulator version
- seed
- ascension
- character
- phase
- state hash

Save-file import should be treated as a verification feature, not just user convenience. `sts_lightspeed` shows that save files expose seed counters for several RNG streams. A future snapshot/import task should map these counters explicitly and test whether restoring from a real save can predict the next observed CommunicationMod state.

## Canonical State Diffs

Diffs should be stable and readable:

- deterministic object key ordering
- arrays preserved where order matters, such as hand, draw pile, monsters, rewards
- IDs normalized where real game UUIDs are not expected to match
- derived display fields ignored unless explicitly under test
- all gameplay-affecting fields included

Diff categories:

- `missing_field`
- `extra_field`
- `value_mismatch`
- `order_mismatch`
- `visibility_gap`
- `unverified_field`

For hidden state, do not silently ignore it. Mark it as `unobservable` and cover it with later indirect tests.

## Deterministic Replay

Replay contract:

- Given simulator version, content version, seed/config, and an action trace, replay produces identical final state hash.
- Restoring from a snapshot at any decision point and replaying the remaining actions produces the same final state hash.
- Legal action descriptors are identical before each replayed action.
- RNG draw logs are identical.

Replay tests:

- full replay from initial seed
- replay from every saved decision snapshot
- replay after JSON round trip
- replay after binary snapshot round trip, once binary exists

## Golden Tests

Golden tests are fixed fixtures with expected snapshots or diffs.

Initial golden corpus:

- Ironclad starter combat setup against fixed dummy monster.
- Strike reduces monster HP by 6 and consumes 1 energy.
- Defend adds 5 block and consumes 1 energy.
- Bash reduces monster HP by 8, applies 2 Vulnerable, and consumes 2 energy.
- End turn discards hand, clears block where appropriate, monster acts, and next hand is drawn.
- Combat victory enters reward or terminal-combat state.

Golden files must be small enough to review. If a snapshot is huge, test a canonical diff plus a separate hash.

## Unit Tests

Unit tests cover pure local rules:

- damage/block math
- vulnerable/weak/strength/dexterity once powers exist
- card cost and energy checks
- pile movement
- shuffle determinism
- monster move selection for one monster at a time
- reward generation helper rules
- map graph reachability
- serialization round trips

Unit tests should not require real-game fixtures.

## Property Tests

Use property tests for invariants, not for exact parity.

Candidate invariants:

- card instances are never duplicated across hand/draw/discard/exhaust/limbo unless explicitly copied.
- total HP stays within 0..max HP except during a transition before clamping if the game really does that.
- legal action generation is side-effect free.
- applying an invalid action never mutates state.
- snapshot round trip preserves state hash.
- no RNG draw occurs during legal action generation, serialization, hashing, or observation extraction.
- hand size/pile counts remain consistent after draw/discard/shuffle operations.

## Fuzz Tests

Fuzzing should use generated legal actions only at first.

Targets:

- random legal combat action sequences
- random snapshot/restore points
- random card order and draw pile setups
- random valid/invalid action payloads for parser robustness

Assertions:

- no panic
- no invalid state invariant
- deterministic replay after fuzz trace
- errors are structured for invalid external actions

Later, fuzz against real-game traces by mutating action sequences only where the current real-game state says the action is legal.

## Regression Corpus

Keep a `verification/corpus` directory once implementation begins:

- `manual/`: hand-authored tiny fixtures
- `communication_mod/`: captured real-game traces
- `run_history/`: coarse run logs
- `bugs/`: minimized traces for every fixed divergence

Every parity bug fix adds a minimized regression trace.

## Hidden and Unobservable State

CommunicationMod exposes a lot of state, but not necessarily every hidden pool, RNG stream position, internal counter, or action queue detail.

Handling strategy:

- represent hidden simulator fields explicitly
- tag fields with observability: `visible`, `exported`, `hidden`, `inferred`
- use controlled experiments to infer hidden state:
  - same seed, different action traces
  - compare next reward/shop/monster result
  - isolate one suspected RNG consumer
- prefer adding verification instrumentation to a local mod over guessing

Never delete hidden fields from snapshots just to make diffs pass. Use diff filters with named reasons.

## Prioritizing Parity Work

Priority order:

1. Mechanics in the current milestone.
2. Deterministic replay and snapshot restore.
3. Combat state that affects immediate legal actions.
4. Card/relic/power interactions used by Ironclad starter and common Act 1.
5. Rewards and deck mutation.
6. Map and encounter generation.
7. Shops, rest sites, and events.
8. Relics and potions by frequency and interaction risk.
9. Ascension modifiers.
10. Full seeded-run parity.

Do not chase rare interactions before the current milestone is proven.

## Save-File/RNG Gate

Task 2.4 decision:

- Save files are likely the earliest practical source for hidden RNG stream counters needed by mid-run replay.
- The RNG counter fields currently tracked from prior notes are `potion_seed_count`, `relic_seed_count`, `event_seed_count`, `monster_seed_count`, `merchant_seed_count`, `card_random_seed_count`, `card_seed_count`, and `treasure_seed_count`.
- `sts_lightspeed` should be used as a comparison target for save-file counter mapping, but not as final authority without source-file/function-level inspection and real-game save samples.
- Save import should move earlier than map/reward/shop parity work, after snapshot/replay and local RNG stream structure are stable.

Verification requirement before save import:

- For each RNG stream, document the real save field, the local simulator stream name, the draw counter interpretation, and at least one test fixture showing restore-then-draw behavior.

## Verification Gates

Before claiming a task complete:

- new or changed mechanics have tests
- deterministic replay test passes for affected fixtures
- snapshot round trip passes if state shape changed
- no new unreviewed RNG calls
- `cargo fmt`, `cargo clippy`, and `cargo test` pass from `simulator/` once code exists
- `STATUS.md` is updated

Before claiming a milestone complete:

- all milestone tasks complete
- at least one golden trace covers the milestone end to end
- the current fidelity limitations are documented
- real-game comparison is run if the milestone claims game parity
