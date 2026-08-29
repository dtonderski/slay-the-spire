# Corruption onCardDraw zeros skill costForTurn

## Source behavior

`CorruptionPower.onCardDraw` runs after `ConfusionPower.onCardDraw` (Confusion
is applied at combat start; Corruption is added later). For a skill it calls
`setCostForTurn(-9)`, which clamps `costForTurn` to 0 and leaves sticky `cost`
at the Confusion value.

`MadnessAction` then gates on `costForTurn > 0`. A confused Havoc/Purity/Limit
Break drawn under Corruption is not a valid pick; `getRandomCard` retries onto
a remaining attack.

Play-time Corruption (skills are free) is not enough: Madness reads the
`costForTurn` field, not `hasEnoughEnergy`.

## Evidence

- FIDL01528: turn-6 Madness first-picked confused Purity (`temp_cost` 1) and
  consumed one `cardRandomRng`. Real Purity `costForTurn` was 0, so Java
  retried onto Strike+. The extra rolls shifted turn-7 Hemokinesis 3→2.
- FIDL01687: Madness left Perfected Strike+ at 2 because Limit Break+ still
  looked like a positive-cost skill.

## Non-goals

- Do not add a second persistent cost field in this change.
- Do not zero skills that were already in hand when Corruption was played
  (`onInitialApplication` is empty).
- Do not change X-cost skills (`setCostForTurn` no-ops when `costForTurn < 0`).
