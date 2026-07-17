# Combat Tree Search Literature Review

## Purpose

This note collects the papers that are most worth reading before implementing
combat-first tree search for this project. The project-specific question is:

- Can we plan fairly from visible Slay the Spire combat state when hidden state
  includes draw order, RNG, monster private counters, and future random choices?
- Should search happen over sampled concrete hidden states, over belief states,
  or inside a learned latent world model?

Short recommendation: start with particle/belief-state search over exact
simulator states. Treat latent search as a later research branch trained against
the exact simulator.

## Reading Order

1. Silver and Veness, POMCP.
2. Cowling, Powley, and Whitehouse, ISMCTS.
3. Browne et al., MCTS survey.
4. AlphaZero.
5. MuZero.
6. PlaNet and Dreamer.
7. DESPOT or particle-belief guarantees.
8. ReBeL or Student of Games, only if we later care about learned belief-state
   value/policy systems beyond single-agent combat planning.

## Need To Read

### 1. Monte-Carlo Planning in Large POMDPs

Paper: David Silver and Joel Veness, 2010.

Link: https://proceedings.neurips.cc/paper/2010/hash/edfbe1afcf9246bb0d40eb4d8027d90f-Abstract.html

Why it matters:

- This is the closest match to "sample feasible hidden states and plan from
  them."
- POMCP combines particle belief updates with MCTS from the current history.
- It only needs a black-box simulator, which fits our Rust simulator perfectly.
- The tree is over histories, not raw hidden states, which is exactly the mental
  shift needed for fair play.

Key ideas to steal:

- Represent belief as particles: concrete hidden simulator states consistent
  with visible history.
- At each real observation, update/filter/resample the particle set.
- Run UCT simulations from particles sampled from the current belief.
- Reuse the same simulations to both estimate action values and maintain
  beliefs when possible.

Project notes:

- For early Slay the Spire combat, a particle can include draw pile order, RNG
  stream state, monster move history, and any hidden counters.
- After a card draw, particles whose predicted draw disagrees with the observed
  draw are no longer feasible. We do not need to philosophically "restart";
  practically, the root belief changes and the search tree can be pruned or
  rebuilt.
- This should be our first serious planner design.

### 2. Information Set Monte Carlo Tree Search

Paper: Peter I. Cowling, Edward J. Powley, and Daniel Whitehouse, 2012.

Link: https://eprints.whiterose.ac.uk/id/eprint/75048/

Why it matters:

- This is the classic MCTS treatment for games with hidden information.
- It frames search over information sets instead of fully specified states.
- It is useful vocabulary for avoiding accidental "cheating MCTS" that sees the
  draw pile or future RNG.

Key ideas to steal:

- Use the player's information set as the public search root.
- Sample determinizations, but aggregate statistics at information-set actions.
- Keep action descriptors visible: hand slot, monster slot, choice slot, not
  internal simulator IDs.

Project notes:

- Slay the Spire combat is single-agent, so we do not need opponent strategy
  machinery at first.
- The main warning is that naive determinization can leak hidden state or
  overfit to sampled futures. For this project, POMCP-style belief updates are
  probably cleaner than pure ISMCTS.

### 3. A Survey of Monte Carlo Tree Search Methods

Paper: Cameron Browne et al., 2012.

Link: https://repository.essex.ac.uk/4117/1/MCTS-Survey.pdf

Why it matters:

- This is the practical map of MCTS variants: UCT, rollout policy, backup
  choices, progressive widening, transpositions, priors, tree reuse, and
  domain heuristics.
- It gives enough background to implement a boring correct MCTS before adding
  neural policy/value guidance.

Key ideas to steal:

- Start with simple UCT and clean instrumentation.
- Add priors, value evaluation, progressive widening, or transpositions only
  after there is a measured bottleneck.
- Separate tree policy, rollout/default policy, backup rule, and root action
  selection.

Project notes:

- Combat action branching can be high because every playable card can have
  multiple targets and choice screens. A clean MCTS implementation lets us add
  action priors later without rebuilding the planner.

### 4. AlphaZero

Paper: David Silver et al., 2017.

Link: https://arxiv.org/abs/1712.01815

Why it matters:

- This is the canonical policy/value-guided MCTS loop.
- It shows how search can become both an action selector and a training target:
  train policy to match improved search visit counts and value to match
  outcomes.

Key ideas to steal:

- Use a policy prior to focus search.
- Use a value function instead of long random rollouts.
- Train from search-improved targets.

Project notes:

- AlphaZero assumes perfect information and a known simulator. For fair Slay
  the Spire, the policy/value should consume fair observations or belief
  summaries, not hidden state.
- The AlphaZero loop is more immediately useful after we have POMCP/particle
  MCTS producing decent root policies.

### 5. MuZero

Paper: Julian Schrittwieser et al., 2019/2020.

Link: https://arxiv.org/abs/1911.08265

Why it matters:

- This is the main paper for latent-space tree search.
- MuZero learns a representation, a dynamics model, and prediction heads for
  policy/value/reward. Search runs in learned hidden state rather than true game
  state.

Key ideas to steal:

- Predict only quantities needed for planning: reward, policy, and value.
- Search can run over latent states without reconstructing the full simulator
  state.
- A learned model can be trained from search/self-play targets.

Project notes:

- This is attractive later, but risky first. The learned model may invent
  impossible states, miss rare card/relic interactions, or quietly learn bugs.
- For this repo, MuZero should be a second-stage experiment trained against
  exact simulator rollouts and compared against exact-search baselines.

### 6. Learning Latent Dynamics for Planning from Pixels

Paper: Hafner et al., PlaNet, 2018/2019.

Link: https://arxiv.org/abs/1811.04551

Why it matters:

- PlaNet is a cleaner latent dynamics/planning reference than MuZero if the
  focus is "learn a compact state and plan inside it."
- It uses stochastic latent dynamics, which is relevant to hidden draw/RNG
  uncertainty.

Key ideas to steal:

- Learn a recurrent latent state from observation history.
- Include stochastic latent variables to represent uncertainty.
- Optimize the model for multi-step prediction, not just one-step accuracy.

Project notes:

- We do not need pixel reconstruction. The useful part is latent belief dynamics
  over symbolic observations.
- If we later build a learned belief-state model, PlaNet is a better conceptual
  starting point than a pure deterministic embedding.

### 7. Dream to Control / Dreamer

Paper: Hafner et al., Dreamer, 2019.

Link: https://arxiv.org/abs/1912.01603

Why it matters:

- Dreamer trains behavior through imagined latent rollouts rather than explicit
  MCTS.
- It is relevant if we want a fast learned agent that uses latent imagination
  after exact search has produced data.

Key ideas to steal:

- Train value and policy from imagined trajectories in latent space.
- Keep the world model separate from the behavior learner.
- Use compact latent rollouts as a speed layer, not as the first source of
  truth.

Project notes:

- Dreamer is not the first tree-search implementation, but it is a strong later
  candidate for distilling exact simulator/search data into a fast policy.

### 8. DESPOT: Online POMDP Planning with Regularization

Paper: Nan Ye, Adhiraj Somani, David Hsu, and Wee Sun Lee, 2016/2017.

Link: https://arxiv.org/abs/1609.03250

Why it matters:

- DESPOT is another major online POMDP planner using sampled scenarios.
- It is useful when thinking about overfitting to a finite set of hidden-state
  particles.

Key ideas to steal:

- Plan over a sparse sampled scenario tree.
- Regularize policy complexity to avoid overfitting sampled futures.
- Treat the planner as anytime.

Project notes:

- POMCP is the simpler first implementation. DESPOT is worth reading if
  particle MCTS becomes unstable or too sample-hungry.

### 9. Optimality Guarantees for Particle Belief Approximation of POMDPs

Paper: Lim, Becker, Kochenderfer, Tomlin, and Sunberg, 2022/2023.

Link: https://arxiv.org/abs/2210.05015

Why it matters:

- This is less of an implementation paper and more of a theory guardrail.
- It clarifies what particle belief approximation is buying and what errors it
  introduces.

Key ideas to steal:

- Treat particle belief planning as solving an approximate belief-MDP.
- Think explicitly about particle count, resampling, and observation likelihood.
- Separate simulator correctness from belief approximation error.

Project notes:

- Read this before making claims about particle search quality.
- For early experiments, empirical checks will matter more than theory, but the
  paper helps name the failure modes.

## Useful Later

### ReBeL

Paper: Brown et al., 2020.

Link: https://arxiv.org/abs/2007.13544

Why it matters:

- Combines deep RL and search in imperfect-information games.
- Uses public belief states and learned value/policy machinery.

Project notes:

- More relevant to poker-like multi-agent equilibrium problems than
  single-agent Slay the Spire combat.
- Still useful if we want a learned value function over belief states rather
  than over exact hidden states.

### Student of Games

Paper: Schmid et al., 2021/2023.

Link: https://arxiv.org/abs/2112.03178

Why it matters:

- A modern attempt to unify perfect-information and imperfect-information game
  learning with guided search.

Project notes:

- Too heavy for the first combat planner, but useful as a long-horizon north
  star if this project grows into general hidden-information search research.

### EfficientZero

Paper: Ye et al., 2021.

Link: https://arxiv.org/abs/2111.00210

Why it matters:

- A sample-efficient MuZero descendant.

Project notes:

- Read after MuZero, not before. The first question for us is whether latent
  search is faithful enough, not whether it is sample efficient.

## Project-Specific Initial Design

For the first combat planner:

1. Define fair observation and visible action schema.
2. Maintain a belief as particles of exact `CombatState` plus hidden RNG/pile
   state consistent with public history.
3. At each public observation, filter/reweight/resample particles.
4. Run UCT/POMCP-style search from particles sampled from the root belief.
5. Back up values to visible action descriptors, not internal IDs.
6. Optionally use a simple heuristic rollout/value function: lethal check, HP
   delta, block survival, expected damage next turn.
7. Emit training data: fair observation, action mask, search visit
   distribution, value target, and hidden/debug metadata only in a separate
   non-agent dataset channel.

Important design constraint:

- The fair planner may use hidden simulator state inside sampled particles, but
  the policy/value network trained for fair play must never receive raw hidden
  state unless it is explicitly an omniscient/debug baseline.

## Open Questions For This Project

- What is the exact particle representation for combat-only Ironclad?
- Do we infer RNG stream state, or initially sample only draw-pile order from
  visible pile contents?
- Should card draw observations cause full tree rebuild, root subtree reuse, or
  particle-set filtering plus partial reuse?
- How do we avoid action masks leaking top-deck information for cards like
  Havoc?
- What is the first rollout/value heuristic before neural value exists?
- What metrics compare planners: win rate, expected HP loss, survival through
  N fights, agreement with exact omniscient search, or downstream RL target
  quality?

