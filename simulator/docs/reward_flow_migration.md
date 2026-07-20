# Reward Card Flow Migration

`RewardScreen` currently represents one card-reward state machine with three
independent fields: `card_reward_active`, `card_reward_pending`, and
`pending_card_reward_count`.  Their combinations can contradict one another,
and accessor methods currently repair those contradictions by inventing a
count.  Authoritative state must instead have one representation.

Schema version 3 stores `card_reward_flow` as an explicit tagged enum:

- `none`: no card reward screen remains;
- `pending { remaining }`: one or more screens remain unopened;
- `active { remaining }`: a screen is open, and `remaining` includes it.

The positive `remaining` value is represented by `NonZeroU8`, making zero-count
pending or active states unrepresentable.  Opening a reward preserves the
count.  Taking or skipping the active reward consumes exactly one and produces
either another pending reward or `none`.

Version 1 and 2 run snapshots are migrated before deserialization.  The legacy
count remains authoritative when nonzero; otherwise a true legacy pending or
active flag migrates with a count of one.  Active takes precedence over pending
because it describes the currently visible screen.  The three legacy fields are
then removed.  Current raw/debug state import does not run this migration:
legacy or contradictory input must not be silently repaired at an authoritative
boundary.

Combat snapshots have no reward state.  They accept the validated historical
versions and the current version without a value migration.
