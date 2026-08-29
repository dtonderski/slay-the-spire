# Hand of Greed skips Awakened One first-form death

## Source behavior

`HandOfGreed` awards gold only when `AbstractMonster.lastDamageTaken > 0` and
the target is dead and not a minion. Awakened One's first form never calls
`die()`: `AwakenedOne.damage` sets `halfDead`, heals later via REBIRTH, and
keeps the fight open. That first-form zero is therefore not a fatal kill for
on-kill gold.

Darkling first-death already uses the same non-fatal half-dead exception.

## Evidence

- FIDL01480 step 1868: Hand of Greed drops form-1 Awakened One from 28 HP to
  half-dead. Observed gold stays 152; the simulator awarded 20.

## Non-goals

- Do not suppress gold on true form-2 death.
- Do not change Ritual Dagger growth unless a later source-backed witness
  requires the same half-dead exception.
