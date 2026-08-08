# Corruption Havoc empty-draw PlayTop target (FIDL00428)

## Bug

`Havoc.use` always builds `PlayTopCardAction(getRandomMonster(...), exhaust)`.
Under Corruption the sim burned that roll at use-time but **dropped** the
monster id when the draw pile was empty (top unknown, `attach` false). After
shuffle, an Enemy top (Clothesline) was force-played with `target=None` and
`random_living_target=false` → `IllegalAction("Havoc top card requires a target")`.

## Fix

1. Self-exhausting Havoc always passes the use-time rolled monster into PlayTop.
2. `apply_play_top_draw_card` applies that id only when the revealed top is
   `TargetRequirement::Enemy`; non-Enemy tops clear the target.

## Witnesses

Permanent FIDL00428 + random-fidelity-7bc87642e5e3d5a8 / d1aea1bbd90e1dd6 /
d678f14850689274. Unit:
`corruption_havoc_empty_draw_force_plays_enemy_top_with_use_time_target_roll`.
