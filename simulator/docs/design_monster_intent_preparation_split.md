# Monster intent-preparation split

## Purpose

`prepare_next_intents_for_ids` combines scheduling filters, direct intent
cycles, the common AI roll, and content-specific rolled intent tables. Split
those responsibilities without changing RNG consumption, monster iteration,
history recording, or fail-closed behavior.

## Invariants

- Compute group-derived inputs once before iterating and preserve monster
  vector order.
- Apply `only_ids`, half-dead Darkling, initial-lock, and completed-split gates
  before ordinary content dispatch in their current order.
- Direct intent families must not consume the common `random_int(99)` roll.
  Preserve any branch-specific discarded or decision RNG draws exactly.
- Rolled intent families consume the common roll before content dispatch, even
  when a particular target helper ignores its value. Preserve additional RNG
  calls inside target helpers and their order.
- Assign one next intent and call `record_target_move` exactly once on every
  handled path, except gates that intentionally retain the existing intent.
- Unknown, approximate, or malformed content retains the current typed error;
  do not introduce fallback intents.

## First slice: direct intent families

Extract the pre-roll direct paths for small Acid Slime, small Spike Slime,
Torch Head, Transient, Looter, Mugger, direct-cycle Gremlins, Gremlin Wizard,
and Slime Boss. Return an explicit handled/not-handled result. Preserve the
small Spike Slime discarded roll, Looter/Mugger source RNG, Gremlin Tsundere's
living-monster projection, fallible Transient damage, and move recording.

## Verification

Review the moved bodies against the pre-extraction diff, then run formatting,
focused `sts_core` tests, strict workspace Clippy, snapshot round trips, the
full workspace suite, and repeated permanent-corpus replay before commit.
