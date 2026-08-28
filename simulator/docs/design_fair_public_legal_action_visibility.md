# Fair Public Legal-Action Visibility

Status: adopted for the combat-only `PlayerChoice` V1 boundary.
Last updated: 2026-07-23.

The simulator keeps one authoritative action system. `legal_run_decision_actions`
continues to enumerate exact internal legality for privileged callers, while
`player_choices` projects a fair-safe public subset of that list.

## Hidden-dependent card plays

Some card play predicates inspect draw-pile composition. In particular,
Secret Weapon requires an Attack in the draw pile and Secret Technique requires
a Skill. Historical PlayerChoice V1 omitted those commands because its design
had not yet committed to a public-knowledge representation of draw membership.

Current FairCombatObservation producers expose the draw pile as a canonical
public card multiset while hiding only its order. PlayerChoice V2 therefore
projects both cards directly from authoritative legality. Their predicates are
order-invariant functions of the same public multiset, so permuting hidden draw
order cannot change observation or choice bytes. The boundary still does not
synthesize legality or alter the authoritative action list.

## Combat phase gate

Potion actions are player decisions even when a combat selection overlay is
active, so they remain legal while `CombatPhase::WaitingForPlayer`. They are
rejected during `MonsterTurn`, `Won`, and `Lost` through the authoritative
potion validator. Enumeration and direct application therefore share the same
player-input phase boundary.
