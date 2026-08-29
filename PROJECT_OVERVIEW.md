# Project Overview: Slay the Spire A20H Ironclad RL

## Purpose

This project aims to create the strongest Slay the Spire player in the world for
Ironclad A20 Heart runs, measured by win rate under a defined evaluation
protocol.

The project starts with a faithful Rust simulator and progressively builds
toward reinforcement-learning agents. The early phases intentionally use A0 and
omniscient combat agents to reduce research risk before tackling fair,
partially-observable A20H play.

## Constraints

- Character scope starts with Ironclad only.
- The full training and iteration workflow must be feasible on a laptop with an
  NVIDIA 5080-class GPU.
- Simulator mechanics must remain deterministic, reproducible, and separate
  from RL feature extraction.
- Future fair agents must not receive hidden simulator state. Omniscient tools
  are allowed only when explicitly labeled as verifier, debugger, or planning
  tools.

## Current Strategic Assumptions

- A0 Ironclad is the first validation target because A20H is too difficult to
  attack directly.
- The A0 simulator is already close to complete. Remaining parity gaps should
  be driven down with manual traces and then automated traces.
- Exact RNG reproduction is required for seeded replay and is expected to be
  mostly implemented already.
- Combat can be treated as the first learning/control problem, even though
  full-run optimal play eventually requires run-level decisions.
- Privileged combat search is acceptable early because it is used to get
  through combats, collect roots, and teach a fair-input policy/value network.
  Raw hidden state is not a neural-network input; the privilege is confined to
  planning over the one true simulator state.
- Once A0 works, the preferred next experiment is a direct jump to A20H. If that
  becomes too hard to debug, the fallback is incremental ascension gates.

## Architecture

- Rust simulator: authoritative game mechanics, deterministic state transition,
  legal action generation, snapshot/restore, and replay.
- Trace tooling: CommunicationMod-based real-game traces for parity validation.
- Combat agents: handcrafted beam search first, then AlphaZero-style Expert
  Iteration with a fair-input policy/value network and privileged search.
- Live trace UI: a small operator console for collecting real-game traces,
  managing bridge sessions, and monitoring simulator fidelity. See
  `docs/live_trace_ui_design.md`.
- Replay/root pipeline: SlayTheData-guided high-level decisions plus simulator
  or real-game combat execution to produce validated traces and combat roots.
- Training environments: wrappers around the simulator for omniscient and fair
  RL experiments.
- Evaluation harness: fixed protocol for A20H Ironclad win-rate measurement.

## Phase Roadmap

| Phase | Name | Purpose | Main outputs | Success gate | Main risks |
|---|---|---|---|---|---|
| 1 | A0 Rust simulator parity | Build an Ironclad A0 simulator with exact game behavior. | Deterministic simulator, legal actions, RNG streams, snapshots, parity tests. | Manual and automated traces show rare, explainable divergence. | Hidden RNG call-order bugs, incomplete edge cases, trace instrumentation gaps. |
| 2 | Omniscient handcrafted combat agent | Use the simulator to search for strong combat play and help collect traces. | Combat search agent, benchmarks against human combat traces, small human UI. | Agent matches or beats human combat outcomes on held-out combat roots. | Objective too narrow, branching too high, agent exploits simulator bugs. |
| 3A | Strict automated parity replay | Validate real-game parity using automated traces. | Full-run replay reports, first-divergence categories, mismatch metrics, statistical residual-rate and coverage reports. | Combined fidelity confidence gate in `docs/phase3a_statistical_fidelity_gate.md`: zero known in-scope failures, green permanent and targeted regressions, and 6,605 consecutive clean prospective full runs under a frozen balanced natural/deep distribution. The run batch gives a one-sided 3σ bound of p < 0.001 under that distribution; all gates together support “high confidence in full fidelity within declared scope.” | Random policies undersample rare/skilled lines; collector or comparison exclusions narrow the claim; later agents require on-policy differential audits. |
| 3B | Guided trace and root collection | Use SlayTheData high-level choices and the combat agent to collect more complete runs. | Real-game traces and combat roots. | Illegal divergence rate is low enough for productive collection. | Legal-but-diverged runs may shift the root distribution. |
| 4 | Simulator-only root collection | Move the Phase 3B process into the simulator for speed. | Large corpus of simulator-ready combat roots. | Root corpus is reproducible, versioned, and validated against prior traces. | Simulator-only bugs can amplify silently. |
| 5 | Privileged-search combat RL | Train an AlphaZero-style combat agent whose network consumes fair information while search follows the one true hidden simulator state. | Fair symbolic decision API, policy/value model, Expert Iteration loop, benchmark reports. | Network-guided search beats equal-budget handcrafted and unguided baselines on held-out roots. | Search targets may conflict across hidden-equivalent public states; privileged search is not a fair deployable planner. |
| 6 | Fair particle-search combat RL | Replace the single hidden search root with a belief over hidden states and aggregate search by public action-observation history. | Particle/belief method, fair planner, calibrated information-gap reports. | Improves over visible-only and non-cheating baselines under a fixed compute budget. | Partial observability, particle quality, and strategy fusion are the core risks. |
| 7+ | Run-level agents | Extend beyond combat into full-run card, relic, route, event, shop, and potion decisions. | Full-run RL system. | A20H Ironclad win rate under the final evaluation protocol. | Run-level credit assignment and compute requirements. |

## Parity vs Collection

The project has two related but distinct replay modes.

Strict parity validation means the simulator is expected to match the real game
exactly from the same seed and same actions. This is the mode used to validate
simulator correctness. It should track exact actions, state transitions, RNG
streams, monster intents, card orders, relic counters, rewards, and first
divergence.

Guided replay/root collection means high-level choices are taken from
SlayTheData where legal, while combat decisions are made by the combat agent.
This mode is allowed to diverge legally from the source run. If a required
high-level choice becomes illegal, the trace is discarded. Legal divergence may
continue, but should be tagged so later analysis can separate exact replays from
guided runs.

This split matters because strict replay proves parity, while guided replay
produces useful combat roots. They should not be treated as the same evidence.

## Trace and Replay Metrics

Track these metrics during Phase 2 and Phase 3:

- strict replay completion rate
- guided replay illegal-divergence rate
- first-divergence category: RNG, monster AI, card/relic effect, reward/event,
  legality, instrumentation, or unknown
- floor reached before divergence
- combat-level exact match rate
- full-run exact match rate
- root-state validity rate
- root corpus size by act, floor type, enemy encounter, deck size, and relic set

The current informal target for guided replay is an illegal-divergence rate well
below "1 / pi". The serious requirement is that divergence is rare enough and
well understood enough that root collection remains productive.

## Combat Objective

Early combat agents optimize a versioned handcrafted terminal proxy because a
run-level value network does not exist yet. Survival must dominate resource
preferences. Within winning outcomes, the proxy may value terminal HP, max HP,
gold, and exact remaining potion inventory. Store the full outcome vector even
when search consumes one normalized scalar.

There is no hard potion budget in the learned-agent architecture. Potion use is
an ordinary legal combat choice whose opportunity cost is represented by the
resulting inventory. SlayTheData floor-level potion metadata may constrain
guided trace collection, where matching source-run resource use is useful, but
it is not an RL observation, policy command, or simulator legality rule.

Eventually terminal combat states are evaluated by a run-level network:
`V_run(post_combat_state)`. That evaluator, rather than fixed combat weights or
a pre-combat permission, determines whether consuming a potion improves A20H
run-win probability.

The simulator must still track relic counters, potion inventory, card order,
exhaust/discard/draw piles, powers, and all other gameplay state exactly. Early
combat objectives may ignore some of their future value, but the authoritative
state must not.

## Combat Root Schema

A combat root should be a serialized state immediately before the first player
decision in combat. It should be sufficient to reproduce the combat under both
fair and omniscient APIs.

At minimum, a root should include:

- schema version and simulator/content version
- source label: manual trace, strict replay, guided replay, or simulator-only
- seed and all relevant RNG stream states
- ascension, act, floor, room type, encounter id, and combat turn
- player HP, max HP, block, energy, powers, stance-like state if ever relevant
- deck and all combat piles, with card instance data and pile order where real
  game state has order
- hand, draw pile, discard pile, exhaust pile, limbo or action-queue state if
  combat has already begun
- relics and exact relic counters
- potions and potion slots
- monsters, HP, block, powers, intents, private AI state, move history, and
  targetability
- action history and provenance needed for trace debugging

Fair observations derived from a root must hide unavailable information. The
root itself is allowed to contain full simulator state.

## State Visibility

The simulator may contain full hidden state. APIs must make the information
boundary explicit.

| State class | Examples | Allowed use |
|---|---|---|
| Fair-observable | Visible hand, HP, block, energy, relics, visible counters, monster HP and visible intent, potion slots, visible pile contents where the UI allows inspection. | Final fair policies, fair RL observations, public player-choice lists. |
| Hidden real state | Draw order without Frozen Eye, RNG streams, future monster moves, private AI counters, future rewards, unrevealed potion/relic/card outcomes. | Simulator internals, privileged teacher search, and belief-state inference only. |
| Omniscient/debug state | Full snapshots, RNG state, exact pile order, private monster state, verifier diffs, trace metadata. | Parity validation, debugging, handcrafted search, and privileged RL planning; never policy/value input. |

Detailed fair-observation schemas should be added as permanent project docs
when the fair API is implemented.

## Omniscient vs Fair Agents

The first learned combat system deliberately separates network information from
planner information. The policy/value network consumes only a fair observation
and public history. Its privileged teacher search may use hidden state, exact
RNG, and future deterministic outcomes while following the one true simulator
root. This reduces engineering risk and makes the network architecture reusable,
but the resulting searched agent is still not fair at inference time.

A fair agent must act only from information a player could see or infer from
public history. Two candidate approaches for fair combat search are:

- particle search over hidden states feasible given the visible history
- latent-state search constrained by predictions of real outcomes such as HP,
  enemy intent, hand damage, hand block, and other public quantities

The fair planner later replaces the singleton hidden root with particles
consistent with public history and keys its tree by public histories. The fair
observation encoder, dynamic public-choice scorer, policy/value heads, training
loop, and root datasets should transfer. Search targets will change because the
fair planner must choose one action across the belief rather than optimize each
hidden state independently.

## Combat RL Search Notes

The learning loop is AlphaZero-style Expert Iteration: search is the expert;
the policy/value network is the apprentice; root visit counts are policy
targets; completed combat outcomes provide value targets. Combat roots seed
episodes, and every visited decision state becomes an example. Roots and all
their descendants must be split by source run/seed across train, development,
and sealed test sets.

Bootstrap the network from the existing beam planner, then use the network as a
PUCT prior and leaf value. Gradually replace beam demonstrations with PUCT visit
targets so the learned system can exceed its initial teacher. Terminal search
may still be expensive because choices include cards, targets, potions,
selection screens, end turn, and stall lines; introduce learned cutoffs only
after terminal/beam baselines and fixed-budget benchmarks exist.

The policy scores the current variable-length list of public player choices
rather than allocating a global output neuron to every possible action. The
state encoder consumes dense tokens for visible cards, piles, monsters, powers,
relics, potions, and public history. Detailed contracts live in
`docs/fair_combat_api_design.md` and
`docs/combat_rl_architecture.md`.

## Final Evaluation Protocol

The final project claim should be measured on Ironclad A20 Heart runs.

Open evaluation details to define before the final push:

- seed policy, initially proposed as 100 random seeds
- confidence interval and minimum run count needed to compare against top human
  and bot baselines
- allowed information: fair only for final reported agent
- time budget per action, per combat, or per full run
- whether inference may use tree search or must be direct policy execution
- crash, timeout, abandon, and illegal-action handling
- comparison baselines and reporting format

Without a time or compute budget, "best player" risks collapsing into "largest
tree search". The project should therefore separate research/evaluation mode,
practical play mode, and laptop-iteration mode.

## Open Questions

- What is the exact phase gate for moving from A0 to A20H?
- Should A20H be attempted directly after A0, or should the project add
  incremental ascension gates when debugging becomes difficult?
- What is the precise weighting between max HP and current HP for combat
  objective scoring?
- Which relic counters should be valued by early combat agents, even if all are
  tracked exactly by the simulator?
- How should legal-but-diverged guided runs be tagged and analyzed?
- What is the final per-decision or per-run time budget?
- What fixed simulation and wall-clock budgets should gate beam, PUCT, and
  particle-search comparisons?
- When is the run-level value model calibrated well enough to replace the
  handcrafted terminal combat proxy?
