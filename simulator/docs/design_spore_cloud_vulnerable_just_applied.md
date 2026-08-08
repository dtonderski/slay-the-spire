# Spore Cloud Vulnerable justApplied

## Rule

Set `player.vulnerable_just_applied` on Spore Cloud **only when all hold**:

1. Vulnerable was newly applied (`had_no_vulnerable`)
2. `temp_thorns > 0` (Flame Barrier active)
3. `phase == MonsterTurn`

| Case | justApplied |
|------|-------------|
| FB kills Fungi mid-monster-turn (FIDL00227) | yes — skip same cleanup tick |
| Player-turn kill while FB still up (09774f3d) | no — upcoming cleanup ticks |
| Monster-turn death without temp thorns (15465, 450f84) | no |

Do not key off `temp_thorns` alone or `MonsterTurn` alone.
