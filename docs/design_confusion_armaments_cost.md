# Confusion + Armaments cost

## Source behavior

`ConfusionPower.onCardDraw` calls `AbstractCard.setCostForTurn(random(3))`.
That helper also writes `cost` when the new value differs from the current
`cost`, so a Snecko/Confusion roll of 2 on Havoc makes both `cost` and
`costForTurn` equal 2.

`Armaments` later calls `upgrade()`. `AbstractCard.upgradeBaseCost` computes
`diff = costForTurn - cost` against those already-modified values, then
`newCost = upgradedBase + diff`. For Havoc (printed 1 → 0) after a
Confusion 2, that difference is 0, so Havoc+ stays 0.

The previous simulator stored only `costForTurn` and treated printed
definition cost as `cost`. That produced `diff = 2 - 1 = +1` and wrote
Havoc+ `temp_cost = 1`. Playing it from 0 energy then failed closed.

A current cost of 0 is still sticky: `upgradeBaseCost` keeps 0.

## Evidence

- FIDL01816 step 689: Armaments upgrades confused Havoc (cost 2) to Havoc+
  with observed cost 0. Playing that Havoc+ from 0 energy succeeds.
- Existing unit coverage keeps a true zero current cost at zero after
  upgrade.

## Non-goals

- Do not invent a second persistent cost field unless a later witness
  requires it.
- Do not change Corruption or X-cost handling.
