# Combat Agent Research Protocol — 2026-07-15

This protocol is frozen before changing live replanning or combat search. It
reuses the immutable, strict-replay combat roots frozen on 2026-07-13; it does
not regenerate roots from the current corpus.

## Fixed population

Development is the union of the old `development`, opened `validation`, and
diagnostic `challenge` manifests: 202 roots from 21 independent seed lineages.
It contains 141 Act 1, 52 Act 2, and 9 Act 3 roots; 143 hallway fights, 40
elites, 15 bosses, 36 low-HP starts, and 149 potion opportunities.

The untouched holdout is the old `held_out` manifest: 23 roots from three
independent lineages, containing 10 Act 1, 11 Act 2, and two Act 3 roots; 16
hallway fights, five elites, two bosses, one low-HP start, and 20 potion
opportunities. Holdout per-root results must not guide edits. It is run only for
the incumbent baseline and final promotion candidates.

Manifest SHA-256 values:

- development: `028DB01B966B712F9718C136C8683218E4565A19E42B7BC67B710AF95B7915C0`
- opened validation: `1FCF0EE9FA5F3493AC203B77DA8E86474AF8682DE06A9B06A33FADD4B6B31E73`
- challenge: `6757095B55D39E941FE87D10DDA893935E3F8C44134BD0F91A6045C289211F2E`
- untouched holdout: `EBC99C574573D2A651C47B9392B55D38679CD5D475D9F3FE78162906EDB023AA`

Every referenced root filename is its FNV-64 content hash. Evaluation rejects
a root whose content does not match its filename. Split membership is by seed
lineage, so states from one run cannot cross development/holdout.

## Information boundary

The planner receives only the serialized `RunState` at the combat start and
states produced by applying its own legal actions. It may not read manifest
metadata, source traces, observed trace actions, later trace observations,
SlayTheData choices or outcome, root IDs, split names, or evaluation results.
No root-, seed-, encounter-, floor-, or split-specific behavior is permitted.

The source trace fields in manifests exist only for provenance and must never
be passed to the planner. Human trace outcomes/actions are not comparison
targets. A root remains in the denominator after loss, nonterminal search,
illegal action, error, or timeout.

## Fixed compute budget

Incumbent and candidate use exactly:

- beam depth 100;
- beam width 300;
- 100,000 generated transitions per combat search;
- 10,000 ms hard wall-clock timeout per root;
- the same executable build, machine, process isolation, and manifest order for
  paired comparisons.

Changing a budget creates a different experiment and cannot promote over this
baseline. Timeout, crash, illegal replay, and nonterminal search are failures,
not exclusions.

## Lexicographic objective and promotion gate

Aggregate and paired comparisons use this order, never a weighted sum across
tiers:

1. More combat wins. A candidate may not turn an incumbent win into a non-win.
2. On roots both agents win, higher final HP / max HP. Development promotion
   requires a positive mean paired delta and no unexplained catastrophic HP
   regression (more than 10 percentage points on a root).
3. If survival and HP are tied within measurement noise, preserve more potion
   value. Potion value is fixed before experiments as common=1, uncommon=2,
   rare=3, using the simulator's source-backed rarity table. Other finite
   resources are compared by exact remaining inventory when exposed.
4. If still tied, fewer combat actions, generated transitions, and elapsed
   milliseconds are better, in that order.

Development promotion requires a strict lexicographic improvement and zero
new illegal actions, errors, or timeouts. A final candidate must then preserve
holdout wins (no more than one net loss only if a two-sided paired bootstrap
95% interval includes zero), avoid new correctness failures, and satisfy the
same lower-tier ordering. Holdout failures do not feed another edit.

## Replanning policy

At the first actionable state of every player turn, search starts from the
fresh current state. Actions within that turn consume the selected principal
variation. Search runs again after `EndTurn` resolves and the next hand,
intents, powers, enemies, energy, and potions are known. Mid-turn search repeats
only when the observed state cannot bind to the predicted next action or a
choice reveals information absent from the predicted state.

Valid cached search data may be reused only when keyed by the complete planner
state and configuration. Cached principal-variation suffixes may seed the new
search, but cannot replace it. Cache hits count toward reporting, and incumbent
and candidate receive identical cache policy and budgets.

## Experiment discipline

Record every attempted candidate in append-only `EXPERIMENTS.jsonl`, including
source hash, hypothesis, configuration, exact commands, runtime, aggregate and
paired metrics, correctness results, and keep/reject decision. Make one
principled planner change at a time. Do not alter these manifests, roots,
protocol, scoring order, or gates after observing candidate results.
