# End-turn Feel No Pain Juggernaut ignores No Block

## Source behavior

`FeelNoPainPower.onExhaust` `addToBot`s `GainBlockAction`. That action calls
`AbstractPlayer.gainBlock`, which notifies `JuggernautPower.onGainedBlock`.
Panic Button's `NoBlockPower` suppresses block from cards, not from this
power callback. The existing mid-turn exhaust path already grants FNP block
through No Block (`feel_no_pain_exhaust_block_ignores_no_block_power`).

FIDL01702 END 1138: Panic Button No Block is still up. Ethereal Ghostly Armor
exhausts. FNP grants 4 block. Juggernaut deals 5 to Maw (224→219). The next
player turn then `loseBlock`s.

## Decision

Deferred end-turn Juggernaut from FNP must not require `no_block_turns == 0`.
Block was already gained on the FNP path that ignores No Block.

## Non-goals

- Do not let card `GainBlockAction` ignore No Block.
- Do not change Metallicize / Plated Armor vs No Block.
