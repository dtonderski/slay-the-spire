# SlayTheData run-level supervision and map audits

Implemented **conditional card/boss-relic supervision**, including a shared
choice head that initializes the actual [A0 run actor](../run_training/README.md#supervised-choice-head).
The original standalone ranker remains a separate baseline; its checkpoint is
not interchangeable with the shared head. This is **head-only pretraining**, not
historical full-state reconstruction or supervision of the full-context backbone
and critic. The input is immutable run-history data, never simulator state.
The [short HTML overview](../../docs/literature_review/slaythedata_pretraining.html)
is for reading; this README owns the implementation workflow and limitations.

## What works / what does not

| Surface | Implemented | Remaining limitation |
|---|---|---|
| Card rewards | Observed picked/rejected identities, upgrades, alias mapping, candidate equivalence classes; supervised ranking | Conditional on taking a card. Skip/Bowl eligibility is not reconstructed, so those entries stay in the audit, not the training labels. |
| Boss relics | Observed picked/rejected slates; supervised ranking with ascension and prior boss picks | Missing picks are rejected, not inferred skips. No floor alignment or contemporaneous deck is fabricated. |
| Elite/other relics | Acquisition records retained; earlier acquisitions can inform history | These are pickups, not boss-style mutually exclusive slates. Logs do not establish all offered-but-declined relics or optional skip decisions. No acquisition is converted into a fake choice. |
| Public context | Strictly earlier-floor logged card picks/skips, event edits, smith targets, purchases/purges and relic gains/losses | This is **partial history**, not a reconstructed deck/inventory. Same-floor edits are excluded. Parallel log sources are not deduplicated into asserted owned items. |
| Routing | Native seed-only maps; complete-path compatibility and multiplicity; wrong-seed controls; hypothetical labels for unique paths | A0 only, normal 15-row acts 1–3, historical build `2020-07-30` tested. Unknown historical map equality, ambiguous coordinates, Wing Boots and A20 remain outside the supported label contract. |
| Standalone baseline | Small CPU ranker, slate/history ablation, uniform/frequency baselines, train-only vocabulary | Its `slaythedata_choice_bc_v1` checkpoints are still **not** accepted by `rl/train.py`. |
| Online transfer | `train_run_prior` trains the shared `ItemPrior`; `--encoder choice` loads it into the actual macro actor and supports normal online updates/checkpoints | Only ascension, card floor and item identities are shared inputs. Full-context backbone/critic stay fresh. Conditional labels do not supervise taking versus skipping. The bounded A0 comparison did not establish stronger play. |

Exact deck reconstruction remains substantial work: identity-sensitive Neow
bonuses, Astrolabe/Pandora/random upgrades, omitted event edits and cross-array
same-floor ordering need historical source-backed handling. Final decks and
relic lists are never used to fill earlier inputs. Missing HP/inventory/counters
are not filled with zero. The current partial-history experiment does not wait
for that work, but also does not claim it is solved.

## Run

From the repository root, build the independent map exporter:

```bash
cargo build -p sts_core --example export_research_maps
uv sync --project rl --reinstall-package sts-sim
```

The exporter consumes only `{seed, act, ascension}` JSONL and wraps the existing
`sts_core::map::generate_target_map_topology`. It accepts signed numeric Java
longs, not base-35 display seeds. It rejects nonzero ascension (the generator has
no ascension parameter), unsupported acts, overflow and extra fields. It does
not accept an observed route, alter generation, or depend on RL.

From `rl/` (replace `DATABASE`; all output directories must be **new**):

```bash
# Stdlib-only preprocessing; database is opened mode=ro + query_only.
uv run --no-project python -m pretraining.choices \
  --database DATABASE --output-dir ../tmp/pretraining/choices \
  --runs-per-ascension 1000 --ascensions 0 20 --seed 20261015

# One seed, five fixed epochs: a baseline, not a tuned performance claim.
uv run python -m pretraining.train_choices \
  --dataset ../tmp/pretraining/choices/examples.jsonl \
  --output-dir ../tmp/pretraining/slate --context slate --epochs 5 --seed 17
uv run python -m pretraining.train_choices \
  --dataset ../tmp/pretraining/choices/examples.jsonl \
  --output-dir ../tmp/pretraining/history --context history --epochs 5 --seed 17

# Actual run-policy item head (different architecture/checkpoint from the above).
uv run python -m pretraining.train_run_prior \
  --dataset ../tmp/pretraining/choices/examples.jsonl \
  --output-dir ../tmp/pretraining/run-prior --epochs 5 --seed 17
# Load and compare using the canonical run-training guide linked above.

# Run separately for --act 1, 2, and 3, with distinct output directories.
uv run --no-project python -m pretraining.map_audit \
  --database DATABASE --map-binary ../target/debug/examples/export_research_maps \
  --output-dir ../tmp/pretraining/maps-act1 --runs 200 --act 1 \
  --seed 20261016 --permutations 9999
```

Local investigated database: `/home/davton/dev/slay-the-spire/data/slaythedata/runs.sqlite`.
The database/raw archives need not be copied into a worktree. Model weights,
raw-data-derived examples, and per-run audit artifacts belong in ignored local
storage, not Git. No captured trace is changed by these tools.

### Extraction contract

- Sample a seeded uniform permutation of indexed cohort row IDs, then apply
  ordinary-run filters until the requested run count. Reproduction requires the
  same indexed database: a different import/row-ID order can change the sample
  despite the same seed, so retain the pinned membership. Losses remain eligible;
  `is_prod` is diagnostic, not a guessed vanilla filter. Missing required flags
  fail closed. This is a run-record sample, not a player sample.
- Normalize identities with the existing explicit catalog/aliases; preserve
  upgrades. Unknown/ambiguous past content rejects affected examples. For
  example, the current mapper recognizes raw `Flex` as both a card and a public
  potion key; these purchases are conservatively rejected until the historical
  shop-log namespace is established. Unknown future card identities do not
  enter an earlier prefix.
- Candidates are sorted distinct identity/upgrade classes, not invented UI
  indices. The label is an identity, not “the first candidate.” Multiple
  indistinguishable offers collapse into one class. Singletons have no ranking
  signal and are rejected.
- `context` is the only policy input, alongside `kind` and candidates. All
  provenance, run group, final outcome, seed, label and debug metadata remain
  outside the feature builder. Card context omits current-floor edits because
  separate arrays do not establish their ordering. Boss context does **not**
  assume ordinal-to-floor alignment.
- Split by SHA-256(play ID) before training. Existing modulo-10 historical
  holdout remains `test`; modulo-100 buckets 1–9 form validation (9%), others
  train (81%). Duplicate play IDs cannot cross splits. This does not establish
  player-disjointness or remove different-ID semantic duplicates/shared seeds.
- `membership.jsonl` pins source row/file/record identity and normalized raw JSON
  hash. `audit.jsonl` retains rejections/special choices/acquisitions. A missing
  `summary.json` means extraction did not complete. The trainer refuses missing
  or incomplete summaries and verifies the example-file hash before loading.
  Summary also pins the catalog hash.

The standalone `train_choices` model embeds candidate identities and a whitelisted
bag of logged-history features, and trains candidate-set cross-entropy. Unknown validation identities
use a neutral shared embedding; vocabulary is training-only. The history model
uses log-scaled edit counts, **not** net card ownership. Training is fixed-epoch,
CPU, one thread, explicit sampling/model seeds. `--evaluate-test` is opt-in;
leave it off while developing. No “best test checkpoint” selection exists.

### Map audit and hypothetical route examples

Selection is frozen **before** any map generation or matching. Each selected
run has a distinct seed and play ID within its act's cohort. Ordinary A0 runs
must have a complete 15-room segment ending at the requested `BOSS` delimiter.
Whole runs with Wing Boots in final inventory/acquisition logs are conservatively
excluded even if acquired later; this is an explicit selection bias. Malformed,
beta and incomplete cases remain counted, not replaced after a map mismatch.

Use `path_taken`, which records map symbols. Do **not** substitute
`path_per_floor`, which can contain a resolved fight/shop for an original `?`.
`?` is matched literally, never as a wildcard. A dynamic program counts all
coordinate paths with the recorded symbols. Nothing selects a path or repairs
map state to force agreement. The generator receives only seed/act/A0.

Outputs include frozen selection, independently generated maps, matching-path
counts and retrospective coordinate ambiguity. `hypothetical_routes.jsonl`
contains branch choices only for a **unique complete coordinate path**, under
an explicit unproven-map-equality assumption. Their context is just the
hypothesized public map and current coordinate; no seed, remaining route or
future choices enter context. Labels/current coordinates are retrospectively
inferred, not observed. The BC trainer **rejects this separate schema**. This
is a useful research export, not approved route supervision or real-game parity.

## Measured pilot

Aggregate results and reproducibility hashes: [`results.json`](results.json),
[`game_map_results.json`](game_map_results.json), and
[`run_prior_results.json`](run_prior_results.json). These are retained
measurements, not a benchmark promise.

### Shared-head transfer into the real run actor

`train_run_prior` uses the same completed, hash-checked extraction and play-ID
splits, but supervises the actual `run_training.item_prior.ItemPrior` module.
Only explicit ascension, card floor and offered identities enter it. Boss floor
has a false presence bit both offline and online; deck, inventory and logged
history are not asserted or zero-filled. Vocabulary is fitted on training slates
only. Epoch count is fixed, test evaluation opt-in, and no simulator state is
constructed or reconstructed for supervised training.

Five epochs, width 32, seed 17: **51.6% validation agreement, 0.987 NLL**, versus
33.1% uniform and 39.8% pick-frequency agreement. All 1,590 validation slates
were covered by the 229-item training vocabulary. The test split stayed untouched.
This is a different architecture from the older baselines below, not a controlled
claim that a particular feature improved performance.

The actual `train.py --task run --encoder choice` comparison used natural A0,
Act-3 binary reward, the same frozen sampled combat policy, identical initial
backbone/critic weights, and identical environment/policy seed setups. Each arm
had four updates × 32 training runs and 64 held-out seeds evaluated before/after.
Scratch used the same architecture/vocabulary but no supervised weights.

| Final validation, 64 paired seeds | Scratch | Pretrained head |
|---|---:|---:|
| Act-3 clears | 0 / 64 | 0 / 64 |
| Mean reached floor (descriptive) | 13.34 | 13.67 |
| Errors / cutoffs | 0 / 0 | 0 / 0 |

**No stronger-play result.** The exploratory paired mean-floor difference was
+0.328, with seed-bootstrap 95% interval [-0.266, 0.953]. Neither arm received a
positive binary reward in training either; these four updates primarily exercise
entropy regularization and integration, not successful-run credit assignment.
A longer, positive-signal training comparison remains necessary.

Across both arms: 512 completed trajectories (including repeated evaluations,
**not** 512 independent validation seeds), 116,929 accepted actions, 1,050 card
picks and six boss-relic picks. All eight online optimizer updates completed;
no batches were discarded. Checkpoint inspection verified identical initial
non-head weights, exact loading of supervised head weights, and subsequent
online head updates. A separate 4+1-update exact-resume check reproduced five
uninterrupted updates bit-for-bit in model, optimizer and RNG states; all 96
final-step training/evaluation journals also matched after decompression.
Budgets match online episodes, **not total compute**: offline fitting/validation
added about 0.8 s on one CPU thread, excluding startup.
Timing was not a controlled throughput experiment. No A20/Heart claim.

The numeric card-ID prerequisite was also fixed without renumbering native rows
or padding old combat weights; see the [numeric transport contract](../../simulator/docs/python_api.md#numeric-combat-batches).

### Original game-bytecode differential check (2022 A0)

A stronger test than route compatibility calls **the original game JAR's
`AbstractDungeon.generateMap`**, including `MapGenerator` and `RoomTypeAssigner`,
then compares the complete exported playable graph against Rust. There is no
copied Java map algorithm, observed-route input or post-generation correction.

- **21,600 / 21,600 maps equal:** 7,000 fresh distinct seeds × all three acts,
  plus the 600 seed/act cases from the historical route audit.
- Compared all starts, **1,296,922 connected nodes and their room symbols**, and
  **1,603,806 directed edges**, including unvisited branches and boss exits.
- No fresh seed had a mismatch in any act. Treating each fresh seed (not each
  act) as one trial, the 99.9% one-sided upper mismatch-rate bound is
  **0.098634%**, assuming representative independent trials. This bounds the
  fraction of seeds disagreeing with the tested oracle, not the probability
  that every aspect of the simulator is correct.
- A separate 50-seed / 150-map check also passed with a different cosmetic RNG
  seed. No gameplay RNG stream is supplied by the graphics scaffolding.

**Authority and scope:** PC `[V2.3.4] (12-18-2022)`, original JAR SHA-256
`cfad868ac8d65a88e71a0bf096fb09f78811e553effe0787c5309a655e081673`.
Normal A0 acts 1–3; signed seeds with non-overflowing act offsets. Boss identity,
burning-elite markers, A1+, daily/endless variants and other game versions are
not covered. This is **original-bytecode differential evidence**, not an
immutable real-game transition trace or an exhaustive proof over every seed.
The older `2020-07-30` build still has route evidence only.

`GameMapOracle.java` supplies headless graphics, a real fallback font, an inert
UI texture and in-memory empty preferences so the game's UI static initializers
can run without OpenGL. Display-config output is sandboxed in the new output
directory. No game methods/classes are replaced or instrumented. An unconstructed
dungeon receiver is allocated only to invoke the original
`initializeLevelSpecificChances` method, audited to assign static starting
probabilities. Act seed offsets come from the `Exordium`, `TheCity` and
`TheBeyond` constructors immediately before `generateMap`; the JAR's own RNG,
room-count rounding, shuffling and placement routines execute unchanged.
The harness rejects other version strings pending a source audit.

Reproduce from `rl/`, with your own legally supplied game JAR (not distributed
here), the native exporter, and a Java runtime containing `jdk.compiler` (tested
on Linux/OpenJDK 25):

```bash
uv run --no-project python -m pretraining.game_map_audit \
  --jar /path/to/desktop-1.0.jar \
  --map-binary ../target/debug/examples/export_research_maps \
  --output-dir ../tmp/pretraining/game-map-proof --runs 7000 --seed 20261017
```

This tests 21,000 fresh maps without SlayTheData. To reproduce the retained
21,600-case measurement, also pass `--route-audit-dir` followed by the three
previous map-audit output directories. Requests are frozen before either
implementation runs; complete outputs, mismatches, source/JAR/binary hashes and
logs are retained. Any graph mismatch makes the CLI exit nonzero. Use
`--runs 50 --cosmetic-seed -911` in another new directory for the cosmetic control.

### Seed-based maps

Build `2020-07-30`, A0, 200 independently selected records **per act**:

| Act | Recorded route fits own-seed map | Unique coordinate path | Fits wrong-seed map | Hypothetical branch labels |
|---|---:|---:|---:|---:|
| 1 | 200 / 200 | 83 | 4 / 39,800 | 455 |
| 2 | 200 / 200 | 92 | 8 / 39,800 | 536 |
| 3 | 200 / 200 | 81 | 9 / 39,800 | 503 |

This is strong evidence of **seed/route association and compatibility across
these builds**, not exact full-map equality. This audit happened to select 600
distinct run groups across three different act-conditioned cohorts; retain the
per-act bounds rather than claiming 600 trials of universal map equality. Many symbol paths still identify
multiple coordinates even on the correct candidate map.

With 9,999 random seed-route permutations, no score reached the observed 200
compatible pairs in any act: Monte Carlo association p = 0.0001, using the
`(exceedances + 1)/(permutations + 1)` estimator. The null is exchangeable
seed-route pairing, **not “the algorithm is incorrect.”** Off-diagonal pairs
share routes/maps and are not independent Bernoulli trials.

For zero failures in 200 independent representative trials of this observable
test, the one-sided exact 99.9% upper failure-rate bound is:

```text
1 - 0.001^(1/200) = 3.395%
```

It takes at least 6,905 zero-failure trials to bound that test's failure rate
below 0.1% at 99.9% confidence. **Even that cannot establish full-map equality:**
an incorrect unvisited node or edge can leave the recorded route compatible.
A regression test demonstrates this non-identifiability. Full topology/room
snapshots from historical-game captures or an independently audited historical
JAR are still needed to close the **2020-versus-2022 version gap**. A20 generation needs separate source-backed
implementation/validation; this PR does not patch gameplay based on outcomes.

### Choice extraction and training

2,000 ordinary-flag records (1,000 each A0/A20), 2,194 candidates visited:

- 21,156 card-choice records → **14,943** conditional card-ranking examples.
- 1,389 boss entries → **1,362** boss-relic-ranking examples.
- 5,224 card skip/Bowl entries retained in audit, not given invented eligibility.
- 7,620 relic acquisitions retained as non-choice records.
- 13,186 train / 1,590 validation / 1,529 test decisions. Test not evaluated.
- Rejections include 230 singleton card slates, 27 missing boss picks, 698
  examples affected by ambiguous purchases, 10 affected by unaligned
  purchase/purge arrays, and unsupported identities. See aggregate JSON.

Fixed five epochs, width 32, seed 17; validation on 1,590 decisions / 165 runs:

| Model | Mean decision NLL ↓ | Tie-aware top-1 agreement ↑ |
|---|---:|---:|
| Uniform | 1.106 | 33.1% |
| Training pick frequencies | 1.180 | 39.8% |
| Learned slate-only | **1.012** | **50.3%** |
| Partial logged history | 1.101 | 45.6% |

History did **not** improve the overall pilot. Boss-relic history NLL was 1.003
versus 1.022 slate-only, but there were only 137 validation relic decisions and
one optimization seed: not a reliable strategic-strength claim. Do not treat
this as evidence that reconstruction or more data will necessarily pay off.

## Validation

```bash
# From rl/
uv run --no-sync python -m unittest discover -s tests -q
uv run --no-sync ty check pretraining run_training tests/test_pretraining.py tests/test_run_item_prior.py
# From repository root
cargo test -p sts_core --example export_research_maps
```

Tests cover causal prefix exclusion, identity/class mapping, malformed records,
missing special-action legality, split boundaries, read-only repeatability,
permutation-equivariant logits, finite gradients, shared offline/live identities,
whole-slate OOV fallback, preservation of take/skip probability mass, actual actor
checkpoint transfer, seed parsing, literal event symbols, path ambiguity,
hypothetical-label refusal, and statistical bounds.
Workspace Rust checks and reviewed corpus replay passed for this PR; no
simulator mechanics were changed. See the PR for the complete check list.

Further reconstruction, map validation and integration experiments are tracked
in the shared [`TODO.md`](../../TODO.md), not a second subsystem backlog.
