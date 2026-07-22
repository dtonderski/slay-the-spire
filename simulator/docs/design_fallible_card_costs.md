# Fallible Card Costs

## Problem

Combat legality, queue construction, energy spending, Necronomicon, and Madness
each reconstructed card cost independently. Unknown content became zero in two
paths and effectively unaffordable in another. Blood for Blood reduction was
duplicated across three implementations, while Madness ignored it and could
select a card that already cost zero.

Imported card metadata was also under-validated. Negative or misplaced Blood for
Blood reductions, a turn-only flag without a temporary cost, and combat-local
cost state retained in the run deck could all survive as plausible state.

## Decision

One combat-owned cost module now provides fallible printed and effective costs.
It requires known content, validates per-instance cost metadata, applies temporary
cost before Blood for Blood reduction, clamps legitimate reduction at zero, and
handles Corruption explicitly for legality. All former cost reconstructions use
this authority.

Blood for Blood reduction must be nonnegative and may appear only on Blood for
Blood or Blood for Blood+. A turn-only cost flag requires a temporary cost.
Run-deck and run-choice validation reject temporary cost, combat-only cards, and
Blood for Blood reduction because all are combat-local.

Madness now tests effective cost when preferring cards whose cost can improve,
while retaining printed cost for its fallback rule. Cost preflight is fallible
and completes before random selection, so invalid imported state does not consume
RNG or silently remove candidates. Card-energy spending uses the same authority
and checked subtraction.

## Verification

Regressions cover malformed combat and run metadata, unknown content, reduction
clamping, zero-energy Blood for Blood legality and spending, and Madness excluding
an already-free Blood for Blood. Existing Bloodletting, Confusion, Corruption,
Necronomicon, and Madness tests remain required. Formatting, strict workspace
Clippy, workspace tests, snapshot round trip, and repeated permanent-corpus replay
remain commit gates.
