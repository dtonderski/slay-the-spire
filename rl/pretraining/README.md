# SlayTheData run-level pretraining pilot

Implemented **offline conditional item ranking**, not a complete run actor or
an RL warm-start. The existing [A0 run actor](../run_training/README.md) uses a
different feature/model contract; this pilot does not replace or warm-start it.
The input is immutable run-history data, never simulator state.
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
| Learning | Small CPU PyTorch ranker, slate-only/history ablation, uniform/frequency baselines, train-only vocabulary, validation metrics | Checkpoints are `slaythedata_choice_bc_v1`, **not** accepted by `rl/train.py`. No online policy adapter, RL fine-tuning, run-strength or Heart-success result. |

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

The model embeds candidate identities and a whitelisted bag of logged-history
features, and trains candidate-set cross-entropy. Unknown validation identities
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

Aggregate results and reproducibility hashes: [`results.json`](results.json).
These are retained measurements, not a benchmark promise.

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
snapshots from historical-game captures, plus broader source/version coverage,
are needed for an equality claim. A20 generation needs separate source-backed
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
uv run --no-sync ty check pretraining tests/test_pretraining.py
# From repository root
cargo test -p sts_core --example export_research_maps
```

Tests cover causal prefix exclusion, identity/class mapping, malformed records,
missing special-action legality, split boundaries, read-only repeatability,
permutation-equivariant logits, finite gradients, seed parsing, literal event
symbols, path ambiguity, hypothetical-label refusal, and statistical bounds.
Workspace Rust checks and reviewed corpus replay passed for this PR; no
simulator mechanics were changed. See the PR for the complete check list.

Further reconstruction, map validation and integration experiments are tracked
in the shared [`TODO.md`](../../TODO.md), not a second subsystem backlog.
