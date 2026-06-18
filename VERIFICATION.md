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

It does not verify seed-start RNG parity. The report must classify these as unsupported until later milestones implement them:

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

This mode parses the real `START IRONCLAD 0 VERIFY01` command, verifies the captured Ironclad A0 bootstrap, selected Neow path, first map choice, first Cultist encounter entry, and captured Cultist combat through lethal Strike without restoring from observed pre-state. It reports an expected failure at `$.actions[step=16].command`, the first reward command. It stops there because exact reward gold, card reward, potion, and reward-screen RNG are not implemented yet.

The seed-start report includes named RNG boundaries for the captured trace: seed conversion, Neow, map, encounter selection, monster HP, shuffle, card reward, reward gold, relic, and potion streams. Save-counter names are included where current research identifies likely real save fields. Captured branches are modeled narrowly: talk, choose random common relic, obtain Toy Ornithopter as an inert relic for this trace, leave Neow, choose the first monster map node, enter a 49-HP Cultist encounter, verify the captured opening hand, verify both captured `END` transitions including the first discard-to-draw shuffle order, and verify the captured kill.

Seed conversion status:

- External seed string captured: `VERIFY01`.
- Exact numeric seed conversion: unimplemented; the harness carries `VERIFY01` as an opaque captured seed for the current branch.
- Current evidence in this repo: `RESEARCH.md` and `DESIGN.md` identify `sts_lightspeed` and target-version source inspection as required evidence before implementing conversion.
- The harness must keep reporting exact seed conversion as an RNG boundary until source-level evidence and tests are added.

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
