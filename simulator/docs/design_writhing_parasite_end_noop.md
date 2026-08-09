# Writhing Mass Parasite / Implant END lag (FIDL00260)

## Observation

CommunicationMod can publish Writhing Mass's Implant across multiple frames:

1. The Implant `END` advances the real combat turn and resolves the monster's
   one-shot `has_siphoned` marker, while the published master deck can still
   omit Parasite for that boundary.
2. A later combat boundary publishes Parasite (and, when present, Ceramic Fish
   gold). That later `END` is still a real turn boundary, not a polling no-op.
3. FIDL01349 provides the same strict witness: step 992 has a 29-card deck and
   step 993 has 30 cards with trailing Parasite.

## Source-backed rules

1. Writhing Mass Mega Debuff queues the target's
   `AddCardToDeckAction(new Parasite())` after applying the combat effect. The
   simulator applies the combat transition immediately and records that typed
   card obtain in `RunState.pending_combat_obtain_cards`.
2. The pending combat obtain settles on the next combat-owned transition. Deck
   card-add relics (such as Ceramic Fish) run during that settlement, and the
   run/combat player views remain synchronized.
3. Never turn a real `END` into a no-op, bind pre-discard observed piles onto a
   post-`END` simulator, or use an observation to decide whether to mutate the
   authoritative deck. The observation is expected output only; queue state and
   source transitions determine settlement.

## Residual

The queue mechanism is generic; the current source-backed producer is Writhing
Mass's Mega Debuff / `AddCardToDeckAction`.
