# Card-in-use draw hand capacity

Slay the Spire's `UseCardAction` removes the played card from the visible hand
while card effects and `addToBot` follow-ups resolve. A draw queued by a
card-triggered relic therefore sees one fewer hand card, even though the
source card returns to its normal discard/exhaust destination only when
`UseCardAction` settles.

The simulator deliberately retains the authoritative source card in `hand`
until its final `MoveCard` so existing card queues can refer to it. Draw
operations that occur while `CombatState::card_in_use` identifies that source
must transiently remove it for hand-capacity and shuffle decisions, then
restore it at its original position. This is simulator-derived state only; no
trace observation participates in the operation.

The rule covers direct draw effects, including Runic Cube / Centennial Puzzle
HP-loss hooks, and does not alter the existing explicit
`DrawCardsWhilePlayedCardIsInLimbo` path: that path already removes the source
before invoking the shared draw implementation.
