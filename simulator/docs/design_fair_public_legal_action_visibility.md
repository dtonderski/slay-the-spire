# Fair Public Legal-Action Visibility

Status: adopted for the combat-only `PlayerChoice` V1 boundary.
Last updated: 2026-07-23.

The simulator keeps one authoritative action system. `legal_run_decision_actions`
continues to enumerate exact internal legality for privileged callers, while
`player_choices` projects a fair-safe public subset of that list.

## Hidden-dependent card plays

Some card play predicates inspect draw-pile composition. In particular,
Secret Weapon requires an Attack in the draw pile and Secret Technique requires
a Skill. The pure V1 public boundary does not carry a public-knowledge model for
unrevealed draw-pile composition. Emitting those play commands would therefore
leak the predicate through the presence or absence of a visible hand-slot
choice.

V1 omits those two card-play commands from `PlayerChoice`. It does not synthesize
replacement legality and does not change the authoritative internal action list.
The fair environment can expose them later once its atomic observation/history
contract represents the relevant known draw-pile information.

## Combat phase gate

Potion actions are player decisions even when a combat selection overlay is
active, so they remain legal while `CombatPhase::WaitingForPlayer`. They are
rejected during `MonsterTurn`, `Won`, and `Lost` through the authoritative
potion validator. Enumeration and direct application therefore share the same
player-input phase boundary.
