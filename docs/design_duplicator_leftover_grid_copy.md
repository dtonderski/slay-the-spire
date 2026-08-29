# Duplicator leftover GridCardSelectScreen copy

`GridCardSelectScreen` (Headbutt discard select) does not call
`HandCardSelectScreen.prep()`. A SuperFastMode skipped-retrieval Headbutt can
leave the chosen card in `selectedCards` after combat. The verifier records that
leftover as `pending_headbutt_alias`.

`Duplicator.update()` copies `gridSelectScreen.selectedCards[0]` with
`makeStatEquivalentCopy` and `ShowCardAndObtainEffect`. `CardHelper.obtain` is
authoritative at construction, so the copy is in `masterDeck` on the first
Duplicator frame, while the dialog can still show Pray/Leave (`pickCard` was
never set by Pray). The leftover list is then cleared.

FIDL01325 is the unique corpus witness: Mayhem PlayTop Headbutt, skipped
retrieval of Cleave (`CHOOSE 6`), then Duplicator enter grows the deck by an
unbottled Cleave copy. Leave without Pray keeps that copy.

Do not hydrate the copy from the observed deck. Consume only the leftover
Headbutt alias already accepted on the skipped-retrieval path.
