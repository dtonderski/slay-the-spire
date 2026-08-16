# MadnessAction samples the remaining hand then retries

## Source behavior

`useCard` calls `hand.removeCard` before `MadnessAction` runs.
`findAndModifyCard` then uses `hand.getRandomCard(cardRandomRng)` —
`random(remaining.size() - 1)` — and recurses when the pick fails the
`costForTurn > 0` (if any card still has a positive turn cost) or printed
`cost > 0` gate. A successful pick writes both `cost` and `costForTurn` to 0.

The simulator still has the source in hand when the queued effect runs.
Sampling the live four-card group (`random(3)`, skip source) desyncs every
earlier Madness (FIDL01474 unaffordable; FIDL01461 first-divs at Heavy Blade
energy). Exclude the source first, then `getRandomCard` on that remaining
group so the bound is `random(n-1)` on the same cards Java sees.

Zero-cost remaining cards stay in the pool and are retried; do not
pre-filter them out of the bound.

## Non-goals

- Do not change Madness into a turn-only cost. Java writes `cost` as well.
