> **Historical note — superseded.** The staged residual and Dark Embrace
> ordering below describe an abandoned Discovery lifecycle. See
> [design_discovery_action_update_lifecycle.md](design_discovery_action_update_lifecycle.md)
> for the current source-backed lifecycle.

# Discovery residual stage 2 before Magnetism with Dark Embrace (FIDL00393)

## Evidence

Hand-played Discovery under SuperFastMode arms a multi-stage residual
`generateCardChoices` burn. Stage 1 (first EndTurn after pick path) arms stage 2
with `end_turns_remaining = 2` (one wait EndTurn, then burn).

With Dark Embrace, FIDL00393's **second** Magnetism generation (END 373, after
Flash of Steel at 372 from DE+1 pick settle) needs the stage-2 residual **before**
that EndTurn's start-of-turn Magnetism roll. Leaving stage 2 after EndTurn yields
Secret Weapon; collapsing the wait and burning stage 2 first yields Magnetism
card (real).

Permanent FIDL00226 has Magnetism without Dark Embrace — gate on `dark_embrace > 0`.

## Model

1. **Pre-EndTurn (run layer):** when `pending_hand_discovery_card_reward_stage == 2`
   and `dark_embrace > 0`, set remaining to 1 and call
   `settle_pending_hand_discovery_card_reward_rng` before applying EndTurn. Then
   set stage-3 remaining to 2 so the same EndTurn's post-settle only decrements
   (does not also burn stage 3).

2. **Stage-2 settle draws:** when `dark_embrace > 0`, burn
   `HAND_PLAYED_DISCOVERY_SECOND_DEFERRED_SETTLE_DRAWS + 4` (base 2 → 6). Without
   DE keep base 2.

Together these place Magnetism at offset 0 for END 373 on FIDL00393.

## Status

- FIDL00393 advances past END 373 (Magnetism card) and PLAY Magnetism / Slimed to
  **END 376**: dual Magnetism hand `Impatience`/`Transmutation` (real) vs
  `Panache`/`Enlightenment` (sim). Dual Impatience+Transmutation is not within
  +80 singles at that counter — residual stream debt after 373 (stage 3+ or
  Slimed+DE draw ordering), not fixed by stage-3 pre-burn alone (overshoots).
- Permanent FIDL00226 / FIDL00372 green with this branch.

## Non-goals

- Do not pre-burn stage 2 without the DE gate (breaks 226).
- Do not change global `SECOND_DEFERRED_SETTLE_DRAWS` (breaks 226 stage-2 amount).
- Do not force stage ≥3 before EndTurn without a dual-oracle (overshoots 376).

## END 376 residual (dual Magnetism)

After Magnetism card at 373, real plays it (374 → amount 2) then Slimed (375).
Sim correctly reaches amount 2 before END 376. Dual Magnetism generates
`Panache`/`Enlightenment`; real wants `Impatience`/`Transmutation`. Probe: that
dual sits at **+82** pool singles from the pre-generation counter — far beyond
remaining Discovery residual stages (3–5 draws). Residual is earlier/other
`card_random` debt between 373 and 376 (Slimed+DE draw/shuffle path or
stage-machine remainder), not a missing second Magnetism stack.
