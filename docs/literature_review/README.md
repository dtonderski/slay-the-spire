# Literature review and research proposals

These notes collect prior art and possible experiments, not the current training
implementation or a committed roadmap. For what runs today, see the
[RL README](../../rl/README.md); for policy constraints, see the
[fair API contract](../../simulator/docs/fair_api.md).

## Fair planning

- [Research tree](research_tree.md): open questions and possible experiment branches.
- [Combat tree-search review](combat_tree_search.md): paper summaries and rationale
  for particle/belief search over exact simulator hypotheses.

Belief hypotheses must depend only on public information/history, not the live
environment's true hidden state. This proposal is not permission to repair
simulator replay from observations.

## Privileged teacher/search research

- [Reading list and local papers](papers/README.md): UCT, Expert Iteration,
  AlphaZero, and related policy-improvement work.

Privileged search is a distinct experimental direction, not fair online planning
or the current policy-loss implementation. Preserve that distinction when using
these papers to propose an experiment.

Return to the [knowledge index](../README.md) for current workflows and durable
project decisions.
