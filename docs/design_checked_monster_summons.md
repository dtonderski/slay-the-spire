# Checked Monster Summons

## Problem

Monster summons and slime splits currently mutate monster groups and RNG streams
in place. They allocate instance IDs with unchecked `max + 1` arithmetic, return
silently when the declared summoner is missing, and rely on a dispatcher fallback
that treats every otherwise-unrecognized `SummonGremlins` intent as Gremlin
Leader Rally. A malformed imported state can therefore wrap an ID, consume only
part of the required RNG, or execute a plausible summon for the wrong monster.

## Decision

Each supported summon or split is a fallible transaction over the monster group
and every RNG stream it consumes. The helper stages cloned inputs, validates the
summoner/content/count and the complete generated monster-ID range, constructs
all children, and commits only after the operation succeeds. Any error leaves
the caller-owned monsters and RNG streams unchanged.

The combat-turn dispatcher matches supported summoners explicitly:

- Bronze Automaton creates Bronze Orbs;
- The Collector creates Torch Heads;
- large Acid Slime, large Spike Slime, and Slime Boss split;
- Reptomancer creates Daggers; and
- Gremlin Leader rallies gremlins.

Any other content carrying a summon intent is invalid authoritative state. A
missing, dead, or wrong-content summoner, a nonpositive requested count, an
exhausted monster-ID domain, or an impossible duplicate opening summon also
fails closed. Legitimate slot limits remain mechanics: Gremlin Leader and
Reptomancer may create fewer monsters than requested when fewer target slots are
available.

The unused representative Rally helper is not an authoritative target mechanic
and is removed rather than migrated.

## Verification

Regression tests cover checked ID exhaustion, malformed counts, duplicate
opening summons, missing/wrong summoners, unsupported dispatcher content, and
rollback of monster and RNG state for each representative failure. Existing
source-backed spawn order, HP, intent, and RNG-counter tests must remain
unchanged.
