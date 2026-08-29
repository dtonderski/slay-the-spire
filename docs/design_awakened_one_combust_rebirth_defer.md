# Awakened One Combust first-kill REBIRTH timing

## External trace evidence (authoritative)

FIDL00368 / FIDL00395: Combust first-kills form-1 Awakened One during END with
low remaining HP. The **same** END publishes form-2 Dark Echo (`move_id=5`,
full HP) and the next hand. REBIRTH runs in that END's enemy phase.

FIDL00378: mid-player-turn first-kill; next END REBIRTHs (no extra defer flag).

## Open FIDL00391

After Combust END the trace shows half-dead (`move_id=3`) for a full player
turn before Dark Echo. A `defer_awakened_one_rebirth` flag was tried to hold
Stun through the death END; it matched 391's death END but **regressed**
the 368/395 witnesses (sim stayed dead/half-dead while real Dark Echo'd).

## Model (current)

Do **not** defer REBIRTH on Combust first-kill. Death END enemy phase runs
REBIRTH + Dark Echo like other first-kills that are pending at monster phase.

`defer_awakened_one_rebirth` remains on `MonsterState` for the Stun hold path
in `execute_state_oriented_special_intent` if a future source-backed caller
sets it; Combust no longer sets it.

## Residual

FIDL00391 fails earlier (death END half-dead vs same-END Dark Echo) until a
source-backed distinction explains 391 without breaking 368/395. Post-Echo AI
(8 vs 6) is a separate `monster_rng` stream issue.

## Post-Echo AI (update)

FIDL00391 first phase-2 AI roll after REBIRTH+Dark Echo is `roll=7` → Sludge
(6); real wants Tackle (8, roll≥50). Skipping the rebirth `monster_rng.random_int(99)`
burn advances 391 past step 1769 but breaks FIDL00368/395/378/269/441 with the
same ATTACK vs ATTACK_DEBUFF residual — dual-oracle on that burn. Death END
still shows half-dead for a full player turn on 391 only (1765–1766); 368/395
publish Dark Echo on the death END without a half-dead player turn.
