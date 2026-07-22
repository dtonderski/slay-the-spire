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

## Second slice: state-oriented special intents

Extract the early-handled intents that do not add or remove monster entries:
half-dead Darkling countdown and revival, group healing and strength, Champ and
Collector buff variants, and Gremlin Leader encouragement. Return an explicit
handled/not-handled result so the caller preserves the existing early
`continue` and never falls through to generic intent execution. Preserve
Philosopher's Stone application, the Gremlin Leader RNG discard, checked move
counters, mutation order, and next-intent preparation exactly.

Keep Byrd's special attack, summons/splits, and ally-targeted block inline for
later slices because their vector mutation and target-selection behavior need
separate review.

## Third slice: spawning and targeted special intents

Move Byrd's grounded three-damage attack, all summon/split variants, and the
Bronze Orb, Deca, Centurion, and Shield Gremlin block variants behind the same
handled/not-handled boundary. Preserve Byrd's card-RNG intent execution and
double move recording; exact spawn helper and RNG argument order; the invalid
summoner-content error; summoner liveness checks; ally selection; checked move
counters; and next-intent preparation. These handled branches continue to
bypass generic effects and between-actor revival exactly as before.

## Verification

Review the moved body against the pre-extraction diff, run focused `sts_core`
tests, formatting, strict workspace Clippy, snapshot round trips, the full
workspace suite, and repeated permanent-corpus replay before commit.
