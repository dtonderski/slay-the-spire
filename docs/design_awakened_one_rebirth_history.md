# Awakened One REBIRTH keeps move history (FIDL00391)

## Observation

After Combust first-kill deferral:

- Death END: half-dead, `move_id=3` (REBIRTH/Stun).
- Rebirth END: `hp=300`, `move_id=5` (Dark Echo), **`last_move_id=3`**.

So history still contains the REBIRTH stun byte when Dark Echo is set. Clearing
history on awaken made `last_move_id` 5-only and did not match CommMod.

## Model

`awaken_one_after_first_death` no longer clears `move_history`. It appends Dark
Echo (5) after existing form-one + REBIRTH entries via `record_target_move`.

Phase-two `last_two_moves(6|8)` remains false until those moves actually play,
so the first post-Echo AI branch is unchanged by retained history.

## Residual

FIDL00391 still fails first post-Echo AI (`move_id` 8 Tackle real vs 6 Sludge
sim) — `monster_rng` value still desynced earlier in the fight. FIDL00378 remains
complete_pass with history retention.
