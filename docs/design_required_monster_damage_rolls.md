# Required Monster Damage Rolls

## Problem

Louse Bite and Darkling Nip damage are rolled when their encounter state is
created. The authoritative monster state stores that roll, but intent
preparation currently accepts a missing value and substitutes a representative
constant. A malformed import or fixture can therefore execute plausible but
incorrect attacks.

Combat-entry construction has a related ambiguity: a spawn awaiting its first
AI roll temporarily carries an ordinary intent inherited from the generic
monster constructor. If entry processing fails to replace it, that placeholder
looks executable.

## Decision

- `MonsterIntent::PendingAiRoll` explicitly represents the short-lived state
  between encounter-spawn construction and initial AI preparation.
- `CombatState::validate()` rejects every pending intent. Pending state is not
  valid at an authoritative legal/transition/snapshot boundary.
- Red Louse, Green Louse, and Darkling states require
  `rolled_attack_damage`. Missing data returns `SimError::InvalidState`.
- Louse and Darkling intent helpers accept a concrete damage value. They never
  choose a representative value for callers.
- Production encounter generation remains responsible for rolling and storing
  the value before initial AI preparation. Explicit fixtures must choose and
  store their deterministic damage value themselves.

No observation, wall-clock source, global RNG, or replacement monster profile
may repair missing state. Existing serialized concrete intents remain backward
compatible; a serialized pending intent is deliberately rejected when restored
as authoritative combat state.

## Verification

Regression tests must prove that missing rolled damage and unresolved entry AI
both fail validation, explicit deterministic Louse fixtures remain valid, and
production encounter entry still resolves concrete intents without changing
the expected RNG sequence.
