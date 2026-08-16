# Darkling COUNT wins after deferred Cube / Fire Breathing

## Source behavior

`Darkling.damage` on first lethal hit sets `halfDead`, `setMove(COUNT, UNKNOWN)`,
and `addToBottom(SetMoveAction(COUNT))`. `takeTurn` has already queued
`RollMoveAction`. `getMove` while `halfDead` would choose REINCARNATE (byte 5,
BUFF/STUN). The later `SetMoveAction` restores COUNT (byte 4, UNKNOWN).

Runic Cube defers `DrawCardAction` until after a multi-hit CHOMP. Fire
Breathing from those Wound draws therefore kills the Darkling *after* the
early death snapshot inside `execute_generic_monster_intent`. The COUNT
restore must use the post-pending-effects half-dead flag (FIDL01313).

## Non-goals

- Do not skip COUNT→REINCARNATE on a Darkling that began the monster phase
  already half-dead (middle Darkling at END 802).
