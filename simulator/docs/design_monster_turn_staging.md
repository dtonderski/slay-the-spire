# Monster-turn staging split

## Purpose

`run_monster_turn` currently combines actor scheduling, special-intent
execution, generic intent effects, between-actor revival, next-intent
preparation, and round cleanup. Split these stages without changing combat
semantics, RNG consumption, error behavior, or mutation order.

## Invariants

- Snapshot actor IDs before execution. Spawned monsters do not act in the same
  monster turn, and removed actors remain harmless missing-ID skips.
- Preserve every special-intent early `continue`; those branches intentionally
  bypass the generic intent path and, where currently applicable, between-actor
  revival handling.
- Preserve generic intent ordering: snapshot observable inputs, execute the
  intent, restore deferred status piles, resolve pending damage/effects, apply
  surviving-player status cards, update monster-local state, prepare the next
  intent, apply Transient fading, then attempt Lizard Tail revival.
- Stop actor execution immediately if the player remains dead after revival.
  Round cleanup must not run on that path.
- After all scheduled actors finish, clean living monsters in vector order:
  vulnerable, weak, Malleable reset, end-of-turn powers (with the just-applied
  Ritual exception), Byrd flight reset, then temporary-strength restoration.
- Clean player vulnerable and intangible after monster cleanup, then apply the
  turn-transition block loss last.
- Keep all arithmetic checked and retain the current typed errors.

## First slice: round cleanup

Extract only the post-actor round cleanup into a named helper. Keep actor
execution inline until its special and generic paths have separately recorded
contracts. The helper accepts the actor IDs whose newly applied Ritual must not
tick during the same round.

## Verification

Review the moved body against the pre-extraction diff, run focused `sts_core`
tests, formatting, strict workspace Clippy, snapshot round trips, the full
workspace suite, and repeated permanent-corpus replay before commit.
