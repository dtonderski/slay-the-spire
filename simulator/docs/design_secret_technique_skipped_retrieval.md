# Secret Technique / Secret Weapon skipped retrieval

Vanilla `SkillFromDeckToHandAction` / `AttackFromDeckToHandAction` open a
filtered draw-pile grid. After `CHOOSE`, the resumed update retrieves the
selected card into hand and `UseCardAction` settles the source.

If that retrieval update completes before the selected card is moved — the
same SuperFastMode / CommunicationMod skipped-retrieval family as Exhume and
Discovery — CommunicationMod publishes a closed `NONE` combat screen whose
draw pile still contains the chosen card. The source still settles (Secret
Technique exhausts; Secret Weapon discards). Relic `onExhaust` (Dead Branch)
is addToBot before power `onExhaust` (Dark Embrace draw), matching
`AbstractPlayer.onExhaust` (FIDL01373 CHOOSE 1: Feel No Pain generated, then
Anger and Strike drawn).

Witness: FIDL01255 step 217. `CHOOSE 0` on Shrug It Off / Burning Pact /
Shrug It Off leaves the first Shrug It Off in the draw pile, exhausts Secret
Technique, and never adds a fifth hand card.

The ordinary `apply_draw_select_choice` path remains authoritative. Skipped
retrieval is a separate core transition that closes the draw select, settles
the source, and leaves the selected card in the draw pile. The verifier
rebuilds that candidate only from the pre-`CHOOSE` simulator state and may
replace the ordinary transition only when the complete observed combat subset
matches the candidate, the ordinary retrieve-to-hand result does not, and the
post-state is a quiescent combat `NONE` screen.
