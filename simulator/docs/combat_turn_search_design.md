# Combat Turn Search Design

## Goal

Make the live combat planner reliably preserve run continuation so guided runs
reach later acts. Eliminate avoidable-damage choices without removing valuable
Feed, Hand of Greed, thief, or potion trade-offs.

## Decisions

1. Terminal combat states are represented by an explicit outcome: win/loss,
   ending HP and max HP, gold, and remaining potion identities. Survival is a
   hard priority. Terminal outcomes use dominance before provisional resource
   utility.
2. Nonterminal block, incoming damage, energy, and monster HP remain search
   guidance only. They are never reported as final combat outcomes.
3. Beam pruning preserves at least one branch for each distinct first action
   before remaining width is filled by score. This prevents one action family
   from crowding every defensive alternative out of the beam.
4. Search advances through complete player turns: an outer depth is consumed
   at End Turn or combat termination, while an inner bounded beam enumerates
   card and potion sequences within the turn.
5. An avoidable-damage audit compares completed current-turn plans. A plan may
   take more immediate HP loss only when it buys a terminal win, persistent
   max-HP/gold value, potion conservation, or a materially stronger future
   state. The audit is conservative and does not compare states with different
   hidden future state as if they were equivalent.

## Safety and limits

- Inner turn enumeration has its own ply bound to prevent zero-cost loops.
- Live search has two explicit limits: a deterministic transition budget and an
  optional wall-clock failsafe. Benchmarks and regressions compare policies at a
  fixed transition budget; the wall-clock limit is only an operational guard
  for the live collector and is recorded whenever it truncates a search.
- A canonical serialized `RunState` fingerprint may deduplicate transpositions
  within a beam layer. Equal states include RNG streams, counters, pile order,
  relics, and action-queue state. The cache never substitutes observed live
  state and never survives across different simulator roots. This experimental
  cache is disabled by default: the initial sealed-root benchmark removed many
  transitions but serialization overhead increased latency on four of five
  late-act roots and one root lost ending HP. It must not replace the validated
  plan-suffix cache without a broader quality gate.
- The unplayed suffix of a validated plan remains the only cross-turn warm
  start. It is replayed from the new simulator root before use; an illegal
  suffix is discarded.
- Exact simulator transitions remain authoritative; the planner does not
  hydrate or repair state from observations.
- Promotion requires regression gates plus grouped combat-start benchmarks.
  Mean HP alone is insufficient; losses and nonterminal searches dominate.
- Potionless-versus-potion counterfactual search is a follow-up after the turn
  search is stable.

## Initial implementation

The first slice implements explicit terminal outcomes, terminal max-HP/gold/
potion value, complete-turn beam layers, first-action diversity, and the rule
that an incomplete within-turn horizon cannot replace a completed incumbent.
The avoidable-damage audit and separate potionless counterfactual search remain
follow-ups; they require broader late-act and potion-root coverage before live
promotion.
