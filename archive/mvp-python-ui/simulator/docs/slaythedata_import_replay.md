# SlayTheData Import And Replay Contract

SlayTheData run histories are high-level run records. They are useful for
guided run-level decisions, but they are not strict transition traces and must
not be treated as proof that the simulator can reproduce every intermediate
state.

## Data Boundary

- `sts_core` remains the authoritative simulator state and transition engine.
- `sts_verify::slaythedata` owns typed import of raw SlayTheData rows and
  chunk-export rows.
- Python guided-collection code may keep using guided scripts for live bridge
  automation, but the Rust importer is the contract for simulator/verifier
  reconstruction.
- UI surfaces should consume an imported run or a derived guided script, not
  ad hoc SlayTheData JSON fields.

## What SlayTheData Can Drive

The importer may treat these fields as run-level evidence:

- run identity: character, ascension, seed, build version, play id
- route: `path_taken`, `path_per_floor`
- Neow: `neow_bonus`, `neow_cost`
- non-combat choices: card rewards, events, shops, campfires, boss relics
- floor potion budget: `potions_floor_usage`
- coarse final observations: final deck, relics, gold, floor reached, victory

These fields can initialize or guide simulator boundaries only when the
simulator can independently construct a legal state at that boundary.

## What SlayTheData Cannot Prove

Run histories do not contain exact combat action sequences, hand order,
draw/discard/exhaust order, action queue state, monster private AI history, or
RNG stream counters. Combat actions must therefore be delegated to the combat
agent or to strict CommunicationMod traces. Missing data must be reported as an
explicit import/reconstruction diagnostic instead of guessed.

Potion usage is recorded by floor, not by potion identity, target, or timing.
The combat agent may spend at most the imported floor budget, while the
simulator remains responsible for exact potion mechanics.

## Replay Modes

- `strict_trace`: CommunicationMod trace replay. This is parity evidence.
- `guided_slaythedata`: SlayTheData run-level choices plus simulator/combat
  agent actions. This may legally diverge from the source run and must be
  tagged as guided evidence.
- `checkpoint_reconstruction`: initialization from a SlayTheData-derived
  boundary. This is allowed only when unsupported fields and assumptions are
  surfaced in diagnostics.

## Diagnostics

Importer diagnostics are part of the contract. They should classify:

- unsupported character, ascension, build, or mode
- unsupported or unknown card/relic/potion/event names
- missing targets needed for a grid or repeated-choice screen
- ambiguous repeated same-floor decisions
- fields that are coarse by design, such as floor-only potion usage
- exact combat action absence

Diagnostics may block replay or mark it as lossy. They must never alter
production behavior based on a seed, run id, trace name, or observed outcome.

## No Seed-Specific Behavior

No production code may special-case a SlayTheData run id, seed, source file,
or captured field combination to make a run replay. Fixed run ids and seeds are
allowed only in tests, fixtures, manifests, and documentation.
