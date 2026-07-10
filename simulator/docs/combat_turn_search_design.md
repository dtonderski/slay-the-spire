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
