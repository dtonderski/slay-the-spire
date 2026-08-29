# Artifact blocks Battle Trance No Draw

## Source behavior

`NoDrawPower` is a `DEBUFF`. `BattleTrance.use()` queues `DrawCardAction` then
`ApplyPowerAction(NoDrawPower)`. Artifact consumes that apply, so No Draw is
not set.

FIDL01594: Panacea Artifact is spent on Battle Trance at step 961. Later Flex
stays temporary (`LoseStrengthPower`). Sim kept Artifact, so Flex became
permanent Strength and Anger hit Spheric Guardian for 8 instead of 6
(`block 23 != 21`).

## Non-goals

- Do not skip Battle Trance's draws; those actions are already on the queue.
- Do not change Head Slam `DrawReductionPower` Artifact handling.
