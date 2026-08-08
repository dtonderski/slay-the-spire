# Clear combat decisions when combat ends (FIDL00243)

## Observation

After a lethal play, CommunicationMod still emits END. That refill path runs
`start_player_turn`, where **Mayhem** can force-play Armaments (or similar) into
a hand-select while every monster is already dead. The resulting state had
`phase = Won` with `decision = Some(...)`, which fails validation:
`combat decision is active outside the player phase`.

## Rule

Whenever combat phase becomes **Won** or **Lost**, clear `decision` and
`queued_decisions`:

- `finish_combat_if_over`
- `apply_combat_action` terminal win/loss
- `start_player_turn` post-Mayhem all-dead / player-dead exits

## Test

`mayhem_after_combat_won_does_not_leave_stale_decision`
