# SlayTheData preparation and loadout sampling

These tools read the immutable local dataset and produce **synthetic training
specifications**, not real-game traces or reconstructed historical states. Root
generation uses an explicit fresh-state simulator constructor. This integration
does not rewrite captured traces, modify existing runs, or change the trainers.

## Current fitted artifact

Use `data/slaythedata/loadout-a0-v3/fit.json` from the repository root.
`v1` and `v2` are superseded exploratory artifacts, **not training inputs**:
v1 exposed that `is_prod` is false even for the inspected ordinary runs;
v2 exposed modded/unmapped content and retained raw IDs. The sampler rejects
v2's old identity namespace. The meanings of ordinary/special-run flags are not
inferred from `is_prod`; that field is recorded diagnostically only.

Cohort: Ironclad, A0, build `2020-07-30`, ending floors 1–56. Exclude
beta/daily/trial/endless/explicitly seeded records, empty or >100-card decks,
missing/nonintegral/out-of-range (1–1000) final max HP, duplicate relic
identities, simultaneous Burning Blood/Black Blood, and unmapped content.
A content rejection removes the **whole record**, never just the unknown card.
These restrictions are explicit training filters, not claims about all valid
Slay the Spire runs. In particular, catalog filtering excludes some legitimate
Prismatic Shard/off-color decks as well as unmistakably modded content.

Of 709,768 candidate source records, **614,044 fit / 68,499 held-out** records
remain. Losses are retained. `membership.jsonl` records row ID, play ID, band,
and split. SHA-256 of play ID determines a stable 90/10 split before fitting;
duplicate IDs cannot cross splits. First candidate in `(ending floor, row ID)`
order wins; counts of every exclusion are in `summary.json`. This historical
holdout is for distribution diagnostics, **not the proposed simulator-rollout
validation set** and not evidence of agent strength.

## Strategies

The generator independently draws components from each ending-floor band:

| Component | Strategy | Measured or assumed? |
|---|---|---|
| Deck size | Empirical histogram | Measured final-deck size |
| Card identities | Weighted with replacement, pooled per-copy counts | Measured composition; independence is intentional |
| Card upgrades | Empirical conditional on identity and band | Measured; includes repeat Searing Blow upgrades |
| Starter relic | Burning Blood, Black Blood, or neither | Measured; never both |
| Other relic count | Empirical unique-relic count histogram | Measured |
| Other relic identities | Frequency-weighted draws without replacement | Approximation; inclusion rates are not exactly preserved |
| Potion identities | Frequencies of logged acquisitions in cohort runs | Measured acquisition mix, **not inventory** |
| Potion occupancy | Uniform from 0 through capacity, random occupied slots | Explicit synthetic prior |
| Potion capacity | 3 at A0, +2 with Potion Belt | Existing simulator rule |
| Max HP | Histogram of last logged max-HP value | Measured, independently sampled from relics/deck |
| Current HP | Uniform integer from ceil(0.1 × max HP) through max HP | Explicit configurable synthetic prior |

Sozu does not erase existing potions: future acquisition restrictions must not
be mistaken for an empty-inventory rule. Discards, full inventories, and some
acquisition/usage details are absent, so do not derive an exact potion inventory
by subtracting logged uses. There is no fitted claim about current HP: terminal
HP is heavily affected by death, and its unfiltered histogram is unsuitable as
a combat-start distribution.

IDs are explicitly translated to the current simulator's public catalog;
`tools/loadout_identities.py` lists aliases and rejects unknown names without
fuzzy matching. Catalog SHA-256 is recorded. Card specs use a **base** content
key plus upgrade count, not an already-upgraded key plus another upgrade.
Identity recognition is not proof of complete mechanical support or historical
balance compatibility. The source build predates the simulator's 2022 target.

## What the data says

Selected fit-cohort means (not generated performance results):

| Ending floors | Fit records | Mean deck size | Mean max HP |
|---|---:|---:|---:|
| 1–5 | 25,723 | 11.02 | 81.15 |
| 16–17 | 98,436 | 15.91 | 83.03 |
| 33–34 | 92,485 | 24.41 | 85.23 |
| 50–53 | 102,265 | 29.74 | 91.22 |
| 54–56 | 26,105 | 28.95 | 94.96 |

Every band has at least 13,183 fit records and 1,558 held-out records. A
13,000-spec check (1,000 per band) passed bounds, uniqueness, starter-exclusion,
and capacity checks. Fit/held-out card-frequency total-variation distance is
about 0.006–0.020 across bands. This is distributional stability within the
historical cohort, not proof that independent decks win fights.

## Usage

From `rl/`:

```python
import random
from pathlib import Path
from loadout_sampling import LoadoutSampler

sampler = LoadoutSampler.load(Path('../data/slaythedata/loadout-a0-v3/fit.json'))
spec = sampler.sample(random.Random(123), floor=50)
# spec.deck, spec.relics, spec.potions, spec.hp, spec.max_hp
```

The floor is supplied by the separate encounter sampler. Loadout band lookup
also accepts noncombat floors because source ending-floor cohorts can contain
them; this does not permit those floors in the encounter generator. There is
no fallback/extrapolation for empty bands. The sampler refuses held-out models.

Refit from repository root (choose a **new** output directory):

```bash
uv run --no-project python rl/tools/fit_loadout_distributions.py \
  --database data/slaythedata/runs.sqlite \
  --output-dir data/slaythedata/loadout-a0-new
uv run --no-project python rl/tools/check_loadout_distributions.py \
  data/slaythedata/loadout-a0-new
```

The JSON is sufficient for sampling; sampling never queries the 366-GiB database.
`check_loadout_distributions.py` writes `diagnostics.json`. The full database is
only opened read-only during fitting.

## On-demand combat roots

From `rl/`, after rebuilding the binding (`uv sync --project rl
--reinstall-package sts-sim` from the repository root):

```python
import random
from pathlib import Path
from loadout_sampling import LoadoutSampler
from synthetic_roots import sample_root

sampler = LoadoutSampler.load(Path('../data/slaythedata/loadout-a0-v3/fit.json'))
root = sample_root(random.Random(123), sampler, floor=55)
decision = root.state.decision()  # Normal fair policy API; Heart combat is initialized.
# Omit floor to sample uniformly across the 44 combat-capable A0 floors.
# State.from_synthetic_spec(root.spec_json) reconstructs the identical initial root.
```

`rl/synthetic_roots.py` combines the samplers. `State.from_synthetic_spec` installs
owned items and HP before the normal combat-entry pipeline, including opening
shuffle, monster AI, relic effects, and opening selections. No preceding fights
are rolled out. These are combat-only roots, not maps from which to continue a
full run. Feed policies the fair decision, never construction seeds/spec metadata.

Explicit defaults: persistent combat counters zero, unused Lizard Tail, expired
Neow's Lament, fresh-state run bookkeeping (zero Wing Boots charges and zero
used Omamori charges), Ritual Dagger growth zero, gold
99, and each bottle targets the first eligible deck card. JSON inputs can set
the six persistent combat counters, Lizard Tail use, gold, and Ritual Dagger
bonus. These priors are **not inferred historical state**.

Missing bottle targets, Prismatic Shard (unsupported by the fair API), and
upgrade counts above the simulator's 255 limit reject an entire loadout. The
historical fit contains a rare Searing Blow +256 entry; its data is preserved,
not clamped. `sample_root` retries at most 64 times and returns the rejection
reasons; it keeps the chosen encounter and combat seed fixed. All other errors
propagate. This conditioning slightly changes the original independent marginals.

Initialization smoke: 10,000 roots, all 44 floors covered, all exposed decisions;
72 loadouts rejected (41 missing power-bottle targets, 31 Prismatic Shards).
Separate unit tests initialize every encounter and advance turns. This is not
real-game parity or proof of all subsequent transitions.

A broader random-action stress check found a simulator transition failure after
1,187 successful 20-action-or-terminal probes: duplicated Power Through can raise
`UnknownCard` (public `choice is invalid`). The synthetic spec and accepted action
prefix are retained in
`data/slaythedata/loadout-a0-v3/synthetic-root-transition-failure.json`.
This is **not a real-game trace** and is not silently skipped or repaired. The
required decompiled source was unavailable locally, so no speculative gameplay
fix was made. Do not launch unattended broad-loadout training without resolving
this transition failure.

## Interpretation limits

Final-loadout marginals are **not exact precombat states**: same-floor rewards
and upgrades may have happened after a fight, and early ending-floor cohorts
are overwhelmingly losses/abandonments. Independent sampling intentionally
breaks synergy and count correlations. All of this is appropriate to label as
synthetic broad training, not natural-run validation.

The constructor does not reapply acquisition effects of Pandora's Box,
Whetstone, fruit relics, etc. to a sampled final deck/HP: these are already-owned
items, and historical measurements already reflect acquisition history. Tests
check that ordinary start-of-combat effects apply once while acquisition effects
do not. Broader simulator fidelity remains bounded by trace coverage, not these
synthetic tests.
