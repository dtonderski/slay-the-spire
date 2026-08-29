# Pain + Centennial Puzzle before Battle Trance No Draw

`Pain.triggerOnOtherCardPlayed` `addToTop`s `LoseHPAction` after
`BattleTrance.use()` has already queued `DrawCardAction` and
`ApplyPowerAction(NoDraw)`. `CentennialPuzzle.wasHPLost` then `addToTop`s
`DrawCardAction(3)`. Puzzle draws run before Battle Trance's draws and before
No Draw.

FIDL01716: opening hand of 7 including Pain, play Battle Trance, hand fills
to 10 (`Iron Wave+` from draw). Leaving Pain at the end of the card queue
applies No Draw first and skips the Puzzle draw.
