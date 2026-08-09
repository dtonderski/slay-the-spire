# Cross-combat skipped put-on-deck retrieval

## Scope

A force-played Warcry/Thinking Ahead/Forethought selection can close its
selection screen with the selected card outside every serialized combat pile.
The existing verifier candidate parks that typed card until the next `END`.

A separate source-backed edge occurs when a non-`END` final blow closes the
combat before that `END`: the combat-local pending card would otherwise be
lost when reward entry drops the old `CombatState`. The target can publish the
same card in the next combat's discard on its first applicable `END`.

## Rule

Keep the selected `CardInstance` as verifier-local typed residual state only
after the existing full-projection-validated skipped put-on-deck candidate is
accepted. Do not derive it from observations. Preserve it across a
non-`END` combat transition that removes the combat. On a later `END` with an
active combat, construct a candidate by appending that already-typed card to
the discard pile; accept it only when the stable post-action frame and the
complete existing combat projection match. Clear the residual after an
`END`, whether the normal or candidate projection matches.

Do not carry a card promoted by the same `END` that wins through end-turn
powers; that card is settled by the ordinary combat cleanup. Do not alter
core combat rules, hydrate state from the trace, weaken comparisons, or
introduce trace/seed-specific logic.

## Witness

FIDL01259 has a mid-turn final blow after a skipped Warcry retrieval. The
selected Wound is absent through the next combat until its first `END`, where
the target publishes it in discard. The residual is generic to the typed
skipped put-on-deck lifecycle, not to that trace or card identity.
