> **Historical note — superseded.** The combat-end flush below was designed
> for an abandoned cross-command Discovery RNG residual. It is retained as
> historical evidence; see
> [design_discovery_action_update_lifecycle.md](design_discovery_action_update_lifecycle.md)
> for the current source-backed lifecycle.

# Flush hand-played Discovery residual when combat ends

## Evidence

Hand-played Discovery arms a multi-stage SuperFastMode `generateCardChoices`
residual on `CombatState`. If the fight ends before stages complete, dropping
`run.combat` discards the pending burns. Live SuperFastMode still advances
`cardRandomRng` while the room tears down / reward opens.

FIDL00411 has two Discoveries in one act with a room transition between them;
Magnetism later wants Blind at +43 pool singles past Mind Blast. Flushing
unfinished residual at combat-end is the source-shaped place to finish those
burns before the next combat seeds `card_random` from the run counter.

## Model

When `apply_combat_action_on_run` observes `CombatPhase::Won`, before reward /
event transitions clear combat:

```rust
flush_pending_hand_discovery_card_reward_rng(&mut next_combat);
next.store_rng_counter(RunRngStream::CardRandom, &next_combat.rng.card_random_rng);
```

`flush_pending_hand_discovery_card_reward_rng` is `pub(crate)` for this call
site (also used before stacking a second hand-played Discovery).

## Status

- Permanent sample green (226/372/253/410/428).
- Does not alone clear FIDL00411 (+43 Blind) or FIDL00393@376 dual Magnetism.
- Complements pick-time dual-Discovery flush and DE stage-2-before-Magnetism.

## Non-goals

Do not skip flush when stacking Discovery mid-combat (still required so stage
6 arming does not drop prior residual).
