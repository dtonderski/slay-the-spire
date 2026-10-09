# Research Tree

Research proposals, not an implementation status report or committed work queue.
The [RL README](../../rl/README.md) describes the current trainer. Paper details
live in the [tree-search review](combat_tree_search.md).

## Root Question

How should a fair Slay the Spire combat agent plan when it only sees public
combat information, while the simulator contains hidden draw order, RNG streams,
monster private state, and future random choices?

## Candidate Trunk: Exact Simulator + Belief Search

Proposed experiment: a practical fair combat planner.

Read before implementing:

1. POMCP: particle beliefs plus MCTS from public history.
2. MCTS survey: UCT mechanics, backups, rollouts, priors, tree reuse.
3. ISMCTS: information-set vocabulary and determinization warnings.

Implementation direction:

- Maintain a belief as particles of exact hidden simulator states.
- Search from histories/visible action descriptors, not hidden-state IDs.
- At new observations, filter/reweight/resample particles.
- Rebuild or partially reuse the tree at observation boundaries.
- Start with heuristic rollouts or a simple value function.

Research recommendation:

- If pursuing fair search, start with this bounded experiment rather than
  waiting for latent-model papers. It is separate from the current trainer.
- Belief updates operate on independently sampled hypotheses, never by repairing
  authoritative simulator replay from an observed game state.

## Branch A: Better POMDP / Particle Planning

Use this branch when the first POMCP-style planner works but particle quality,
tree stability, or sample efficiency becomes a problem.

Core follow-ups:

- DESPOT: scenario-tree planning and regularization against overfitting sampled
  futures.
- Particle-belief approximation guarantees: understand what particle count and
  resampling are costing us.
- POMCPOW / progressive widening variants: useful if observations or actions
  become too large or continuous-like.

Project questions:

- How many particles are needed for early Ironclad combats?
- Can we update beliefs cheaply after draws, shuffles, and monster intents?
- When is tree reuse worth the complexity?
- How do we measure belief collapse?

## Branch B: Policy/Value Guided Search

Use this branch after exact belief search can generate decent decisions or
training targets.

Core papers:

- AlphaZero: policy/value-guided MCTS with search-improved training targets.
- MuZero: policy/value/reward prediction with learned latent dynamics.
- EfficientZero: later sample-efficiency refinements.

Project questions:

- Should the policy/value consume only fair observations, a belief summary, or
  search statistics?
- Can search visit counts become policy targets?
- How much exact search is needed before a learned value replaces rollouts?

## Branch C: Latent World Models

Use this branch when exact search is too slow or when we want a fast learned
agent that imagines futures.

Core papers:

- PlaNet: recurrent stochastic latent dynamics for planning.
- Dreamer: learning behavior from imagined latent rollouts.
- MuZero: latent tree search without reconstructing full state.

Project questions:

- Can a latent state represent uncertainty over draw order and RNG without
  leaking hidden truth?
- Should the model predict visible observations only, or also private simulator
  state for auxiliary training?
- How do we detect impossible latent rollouts?
- Should latent search be trained by exact simulator rollouts, POMCP targets,
  or both?

Decision:

- This is not the first implementation path.
- Treat it as a research/distillation branch after exact search exists.

## Branch D: Fairness, Visibility, And Dataset Boundaries

Use this branch continuously. It governs all other branches.

Core project docs:

- [Fair API contract](../../simulator/docs/fair_api.md)
- [Boundary audit](../../simulator/docs/fair_observation_hidden_state_audit.md)
- [Current training architecture](../../rl/README.md)
- [Major decisions and rejected approaches](../project_history.md)

Project questions:

- Do public choice lists or their ordering leak hidden top-deck or
  future-choice information?
- Are policy/value inputs purely fair?
- Are debug fields, hidden particles, RNG state, and simulator snapshots kept
  out of agent-observable outputs?
- Are training datasets clearly labeled as fair, omniscient, or auxiliary?

## Branch E: Omniscient Baselines

Use this branch for diagnostics, not fair agent claims.

Purpose:

- Compare fair belief search against search that sees the exact hidden state.
- Estimate the value lost to hidden information.
- Debug simulator/search bugs with full snapshots and RNG logs.

Project questions:

- How much better is exact-state MCTS than fair POMCP on the same combats?
- Does a fair policy accidentally learn hidden-state shortcuts from dataset
  metadata?
- Which combats require belief tracking versus simple visible heuristics?

## Smallest Fair-Search Experiment

If this research branch is selected, a tiny combat POMCP skeleton would test the
idea before deeper reading:

1. Fixed combat fixture.
2. Particle type wrapping exact simulator state.
3. Visible observation/history key.
4. Visible action descriptors.
5. UCT tree over visible histories.
6. Particle filter after card draws.
7. A minimal heuristic rollout/value.

Then use failures to choose the next reading branch.

