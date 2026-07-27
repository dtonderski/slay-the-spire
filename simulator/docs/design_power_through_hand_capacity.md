# Power Through generated-card hand capacity

`PowerThrough.use` queues `MakeTempCardInHandAction(Wound, 2)` before its
`GainBlockAction`. The Java card-play path has already removed the played card
from the hand and placed it in limbo when that action runs. Therefore the
generated-card action counts the hand without `Power Through` for its capacity
calculation, then the played card returns to its normal discard/exhaust
destination after the card effects resolve.

The simulator keeps the played card in the authoritative hand until its final
pile-move action so existing card queues can resolve against one stable source
card. A dedicated internal action will reproduce only the source-backed hand
capacity boundary: temporarily exclude the played card while generating the
batch, preserve generated-card order, then restore the source for its queued
final move. No observed trace state participates in this transition.
