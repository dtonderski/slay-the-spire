# Verification Strategy

## Principle

The simulator is not correct because its tests pass. It is correct only to the extent that tests prove specific mechanics and traces match the target game version.

Verification is staged:

1. Unit correctness for tiny local mechanics.
2. Golden tests for complete small transitions.
3. Deterministic replay from seed plus action trace.
4. Real-game state comparison through CommunicationMod-style exports.
5. Distribution checks for systems where exact hidden state is not yet observable.

Production live collection and permanent-corpus verification use only
seed-start replay. Simulator state is derived from the initial seed/config plus
accepted typed actions, never copied, repaired, or hydrated from a real-game
observation.

## Verification Outcome Contract

Default success is **clean through EOF**: every verifiable transition in the
file matches with first-boundary `category=none`. Incomplete / half-finished
runs pass when the recorded prefix is clean. Full clean runs also pass.

Corrupt or invalid traces are invalid input (or quarantine). Genuine simulator
divergences fail verification and remain evidence — they are not greenwashed
via expectation grades or silent truncation.

No passing outcome may be produced from empty diff lists alone. It also
requires zero unsupported transitions, no replay boundary, and complete
action-integrity evidence: one disposition per applicable action and no
duplicates.

The `parity` command streams clean-through-EOF by default: exit `0` for
`complete_pass`, `1` for invalid input, and `2` for a valid trace that fails.
Optional `--require-terminal` requires a full game-over run. Diagnostic
`--diagnostic-early-exit` stops at the first semantic boundary but records
`eof_validated=false`, so it can never pass or promote a trace. There is no
expectation manifest.

## Real-Game Comparison

The best current harness is [CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod). Its protocol sends JSON game state when the game is stable and accepts external commands. [spirecomm](https://github.com/ForgottenArbiter/spirecomm) demonstrates client-side use.

For the Phase 1 live trace UI, use the manual/quarantined checklist in
[`live_trace_real_game_smoke.md`](live_trace_real_game_smoke.md) when validating against a real ModTheSpire
process. Keep that smoke out of ordinary CI.

The verifier has two independent projection paths:

```text
validated trace pre-state + typed command
    -> core transition
    -> simulated projection

validated trace post-state
    -> observed projection

simulated projection <-> observed projection
    -> comparison and one action disposition
```

The simulated projection accepts simulator state only. The observed post-state
is expected output and cannot select content, repair RNG, reconstruct a screen,
or otherwise mutate authoritative simulation state. Every action must complete
immediately with its same-step authoritative state or error. `STATE` completes
only on `poll`; gameplay completes only on `interaction_ready`, `quiescent`, or
`terminal`. Quiescent and terminal boundaries cannot retain a queued end turn.
An `interaction_ready` boundary may retain one while a source-backed decision
pauses that turn—for example, Nilry's Codex opens its card reward inside
`END`; the command-execution fence must still advance before that state can
complete the command. Schema 6 also requires all published gameplay-affecting
effect counts to be zero. Intermediate, transient, delayed, or later-frame
completion is invalid input rather than deferred verification.

Full CommunicationMod payloads are external to Git. The active authoritative
corpus contains only the current collection epoch: fixed gameplay delta
(`collection.2`) plus boundary schema 6. A later schema starts a new explicitly
validated epoch; it is not accepted forward-compatibly. The 602 pre-collection.2
payloads and the failed schema-3/schema-4/schema-5 pilots are non-authoritative
evidence. Set
`STS_PERMANENT_CORPUS_DIR` and pass the desired active file explicitly:

```bash
cd simulator
export STS_PERMANENT_CORPUS_DIR=/path/to/permanent_traces
uv run -- cargo build --release -p sts_verify --bin sts_verify
target/release/sts_verify parity --require-terminal \
  "$STS_PERMANENT_CORPUS_DIR/<trace>.jsonl"
cargo run -q -p sts_verify -- replay --json \
  "$STS_PERMANENT_CORPUS_DIR/<trace>.jsonl" -o replay.json
cargo run -q -p sts_verify -- replay --json --at-step 3322 \
  "$STS_PERMANENT_CORPUS_DIR/<trace>.jsonl"
```

The replay artifact contains the same verification report, the final
authoritative `RunState` snapshot, and lightweight checkpoints with stable
snapshot hashes. `--at-step` retains the latest checkpoint at or before the
requested trace action and includes its full snapshot. A replay boundary still
returns the simulator state at the frontier, but exits `2`; it is not silently
treated as a successful complete replay. The exit codes are `0` for complete,
`1` for invalid input, and `2` for a valid trace that reaches a boundary.

Replay and parity share the same streaming transition engine. After the first
semantic difference, simulator execution stops but strict input parsing,
terminality, and disposition accounting continue through EOF. CommunicationMod
post-state observations are comparison evidence only, including when a trace is
replayed with `--at-step` or `--timeline`.

Generate corpus-wide status from the external directory:

```bash
cd simulator
uv run -- cargo run -q -p sts_verify --bin sts_verify -- status --markdown \
  "$STS_PERMANENT_CORPUS_DIR"
```

### Authoritative Hugging Face corpus

The private `dtonderski/sts-permanent-traces` Hugging Face dataset mirrors the
current reviewed 311-trace corpus, with each immutable payload stored as a
deterministic `<trace>.jsonl.gz`. The original 208 traces declare
`source_version="collection.2-schema6"`. The 103-trace FIDL01880–FIDL02008
cohort has a stale pre-schema-6 `source_version` string, so its immutable
metadata was not rewritten; its promotion record instead attests the installed
SuperFastMode `1.0.9-collection.2` JAR and exact collection artifact hashes.
Both cohorts use boundary schema 6 and passed structure, command-fence,
zero-effect, hand/card retrieval, terminal, repeatability, raw-diff, and hash
audits. Current verifier status is 281 complete passes and 30 explicit
unsupported frontiers, with zero trace errors and zero raw unexpected diffs.

Download the external corpus into the active directory:

```bash
export HF_TOKEN=<read-token>
export STS_PERMANENT_CORPUS_DIR="$PWD/simulator/verification/corpus/permanent_traces"
tools/hf_corpus.sh download dtonderski/sts-permanent-traces
```

`.cursor/start.sh` performs this download at boot when `HF_TOKEN` is available.
The 602 pre-collection.2 payloads remain only in the local
`legacy_pre_collection_2/permanent_traces/` archive. The repository
intentionally carries no generated inventory, outcome ledger, or status
snapshot; status is computed from immutable payloads and the verifier revision
being evaluated.

The external corpus is a **regression lock**, not a residual-rate proof. For the
combined Phase 3A fidelity confidence gate and its statistical limits, see
[`phase3a_statistical_fidelity_gate.md`](phase3a_statistical_fidelity_gate.md).

Validate new CommunicationMod captures before adding them to the external
corpus. Captured payloads remain immutable even when they expose a simulator
failure; do not trim, rewrite, or grade them into a pass.

### Divergence minimization

When a trace fails parity, build a compact prefix JSONL that reproduces the
first failure:

```bash
cd simulator
uv run -- cargo run -p sts_verify -- minimize \
  -o verification/corpus/bugs/my-bug.jsonl \
  "$STS_PERMANENT_CORPUS_DIR/<trace>.jsonl"
```

`minimize` runs parity, finds the first `unexpected_diff` or failure boundary,
and writes metadata plus all state/action lines through that step. Summary
fields go to stderr; the minimized trace goes to stdout or `-o`. Passing traces
exit 0 when there is no failure to minimize.

### Visibility contract

Every excluded field must have a narrow observation reason. Current comparison
exceptions are:

| State | Contract | Why |
|---|---|---|
| Card UUIDs and reward-screen internal IDs | Compare content identity, order, and visible offers; do not compare process-local identifiers | Simulator card IDs and target UUIDs use different identity domains |
| RNG counters not exported by CommunicationMod | Do not compare the counter directly; verify its effects in later visible offers, piles, encounters, and intents | The observation contains outcomes, not hidden stream positions |
| Runic Dome monster intent and move ID | Omit both from observed and simulated projections while the relic hides intent | The player cannot observe them |
| Missing target `move_id` | Omit the simulated `move_id` only for that monster and frame | Some CommunicationMod frames do not export it |
| Dead-monster Strength, Ritual, and Vulnerable | Omit only after that monster is dead | CommunicationMod exposes terminal powers inconsistently and they cannot affect future transitions |
| Intent and move ID after player or monster death | Omit only on the terminal frame | No later player decision can observe or act on them |

These exceptions affect comparison projections only. They never authorize
mutation of simulator state. Living-monster powers and authoritative-boundary
intent, HP, block, energy, pile order, rewards, relics, potions, deck, and
choices remain strict. A non-authoritative intermediate frame invalidates the
trace; it is never folded into or reconciled by a later observation.

Seed conversion status:

- External seed string captured: `VERIFY01`.
- Exact numeric seed conversion: implemented from the target `SeedHelper.getLong(String)` bytecode in the local `12-18-2022` desktop jar. Seeds are uppercased, `O` maps to `0`, and characters are parsed in base 35 using `0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ`.
- Current evidence in this repo: [`research.md`](../../docs/research.md) records the target jar/class inspected and captured checks for `VERIFY01`, `CODEX03`, and `CODEX04`.
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

Keep only compact, reviewable fixtures under `verification/corpus`:

- `manual/`: hand-authored tiny fixtures
- `bugs/`: minimized traces for fixed divergences when a compact file is useful

Full captures and coarse run logs are external data. Every parity bug fix keeps
its source capture immutable and adds focused regression coverage where useful.

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
- important verification status, risks, and follow-up work are documented in
  the commit message or a permanent project document

Before claiming a milestone complete:

- all milestone tasks complete
- at least one golden trace covers the milestone end to end
- the current fidelity limitations are documented
- real-game comparison is run if the milestone claims game parity
