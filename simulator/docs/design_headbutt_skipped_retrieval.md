# Headbutt skipped retrieval

## Source behavior

`Headbutt` / `Headbutt+` queues `PutOnDeckAction` against discard. When discard
has more than one card, the action opens a GRID and pauses. `UseCardAction` has
not settled yet, so a Havoc-forced Headbutt is still in `cardInUse`.

On some CommunicationMod frames the later `PutOnDeckAction` update is skipped
after the GRID closes: the chosen discard card never moves onto draw. Headbutt
still force-exhausts, so Dark Embrace / Feel No Pain / Charon's Ashes / Dead
Branch resolve after the screen closes and can shuffle the leftover discard.

Witness: FIDL01306 steps 1065–1066. `PLAY 1` opens GRID over discard
`Thunderclap, Havoc` with an empty draw pile. `CHOOSE 1` closes the GRID,
exhausts Headbutt, and Dark Embrace draws Thunderclap after shuffling both
remaining discard cards. Havoc is not on top of draw.

## Simulator contract

The ordinary Headbutt confirm still puts the chosen discard card on draw and
then settles the source. Force-played Headbutt that opens a discard select
defers source exhaust until that close.

The skipped-retrieval candidate is eligible only for a typed Headbutt discard
select whose decision already holds `source_card`. Accept it only when the
ordinary confirm does not match the observed combat subset and the skipped
candidate does. Do not copy observed identities into simulation.

## Singleton auto-put exhaust alias

When discard has exactly one card, `PutOnDeckAction` auto-moves that card to
the top of draw without a GRID. CommunicationMod can also publish an exhaust
card with the same UUID on top of draw (FIDL01246 `Feed`, FIDL01834
`Reaper+`). The Java object stays in `exhaustPile`; combat end returns one
deck copy.

The verifier tries a post-PLAY Headbutt / Headbutt+ candidate that remints
each simulator exhaust card (top first) as `combat_only` onto draw. Accept
the first remint whose combat subset matches the observed post. The remint
id is derived from the exhaust instance, not from the observation.

The same singleton auto-put can republish the previous top of draw with the
same UUID after the discard card is added (FIDL01787 `Strike`). The candidate
then remints that simulator draw-pile top (top first) onto draw. The remint
id is derived from the live draw instance, not from the observation. The
extra copy is gameplay-affecting: the next draw takes it, and the original
card remains in the pile.

When those two listings later sit in hand together, Java `UseCardAction`
`removeCard` plus `resetCardBeforeMoving` drop every `hand.contains(c)` slot of
the same `AbstractCard` and discard the object once (FIDL01747 `Strike`). The
simulator evaporates the reminted sibling from hand on the real pile move when
`content_id` matches. Unrelated high-id combat-only statuses (Wounds) stay.
