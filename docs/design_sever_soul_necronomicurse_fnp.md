# Sever Soul + Necronomicurse Feel No Pain

`SeverSoul.use()` queues `ExhaustAllNonAttackAction`, which snapshots the
hand once and `addToTop`s `ExhaustSpecificCardAction` for each non-Attack.
`Necronomicurse.triggerOnExhaust` is synchronous (`MakeTempCardInHandAction`)
and is not part of that snapshot.

FIDL01518 plays upgraded Sever Soul (cost 2) with Necronomicon. The first
use() exhausts Wound + the original curse (Feel No Pain 6) and leaves the
replacement. Necronomicon's second `use()` snapshots the replacement and
exhausts it (Feel No Pain 9). The newest curse stays in hand. A second
exhaust wave inside one `ExhaustAllNonAttackAction` would also eat the
copy's replacement (Feel No Pain 15).

`CardExhausted` is `push_front`, so the first play's `triggerOnExhaust`
inserts the replacement before the copied `ExhaustAllNonAttackCards`.
Feel No Pain itself stays `addToBot` (`GainBlockFromExhaust`) so Sharp Hide
still hits before that block.
