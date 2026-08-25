# The Bomb lethal end-turn + Constricted (FIDL00403)

## Observation

FIDL00403 end-turn kills Spire Growth / Serpent via The Bomb's final tick during
powers-before-hand. Real reward HP is **3877** from pre-end **3876 block 5** with
**Constricted 10** and Burning Blood:

- Constricted THORNS consumes block first: block 5 → 0, HP 3876 → 3871
- Bomb already killed; hand/Regret skipped (FIDL00244)
- Burning Blood +6 → **3877**

Simulator previously early-finished on bomb lethality and skipped Constricted,
yielding 3876 + 6 = **3882**.

## Ordering

| Path | Constricted |
|------|-------------|
| Bomb-lethal pre-hand victory (FIDL00244 Regret skip) | **Apply** before Burning Blood (FIDL00403) |
| Combust/ethereal kill after hand (FIDL00443) | **Skip** (combat already over) |
| Normal end-turn (FIDL00415 Metallicize/Decay, FIDL00061 Burn/Rupture) | After Burn/Decay autoplay, before ethereal exhaust |

## Implementation

In `end_player_turn`, when `had_bomb_timer` and all monsters are dead after
`apply_end_of_player_turn_powers_before_hand`, call
`apply_end_of_turn_constricted` then `finish_combat_if_over` (Burning Blood).

Do not move Constricted before Burn/Decay autoplay on the ordinary path.
