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
- Omniscient combat agents are acceptable early because they are used to get
  through combats and collect combat roots, not as the final fair policy.
- Once A0 works, the preferred next experiment is a direct jump to A20H. If that
  becomes too hard to debug, the fallback is incremental ascension gates.

## Architecture

- Rust simulator: authoritative game mechanics, deterministic state transition,
  legal action generation, snapshot/restore, and replay.
- Trace tooling: CommunicationMod-based real-game traces for parity validation.
- Combat agents: handcrafted search agent first, RL agent later.
- Human UI: a small Slay the Spire UI that can play against the simulator,
  connect to the real game through CommunicationMod, and ask an agent for the
  best move.
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
| 3A | Strict automated parity replay | Validate real-game parity using automated traces. | Full-run replay reports, first-divergence categories, mismatch metrics. | Exact replay succeeds often enough that remaining failures are understood. | Full-run exactness may be harder than combat exactness. |
| 3B | Guided trace and root collection | Use SlayTheData high-level choices and the combat agent to collect more complete runs. | Real-game traces and combat roots. | Illegal divergence rate is low enough for productive collection. | Legal-but-diverged runs may shift the root distribution. |
| 4 | Simulator-only root collection | Move the Phase 3B process into the simulator for speed. | Large corpus of simulator-ready combat roots. | Root corpus is reproducible, versioned, and validated against prior traces. | Simulator-only bugs can amplify silently. |
| 5 | Omniscient combat RL | Train an RL combat agent with fair inputs but omniscient search/planning. | Combat RL policy/value model, search loop, benchmark reports. | Beats human traces and strong handcrafted baselines on held-out roots. | Omniscient search may not transfer to fair play. |
| 6 | Fair combat RL | Train/search using only the visible game state. | Fair observation/action API, belief or latent-state method, fair combat agent. | Improves over non-cheating baselines under fixed compute budget. | Partially-observable search is the core research problem. |
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

Early combat agents optimize terminal combat outcome, not full-run value.

The initial objective should be lexicographic:

1. Win the combat.
2. Maximize max HP gain.
3. Maximize current HP after combat.
4. Use potions according to the SlayTheData floor-level potion budget.

SlayTheData tells whether a potion was used on a floor, not necessarily the
exact combat action. Therefore the combat agent is responsible for potion timing
and targets within the allowed budget.

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
| Fair-observable | Visible hand, HP, block, energy, relics, visible counters, monster HP and visible intent, potion slots, visible pile contents where the UI allows inspection. | Final fair policies, fair RL observations, fair action masks. |
| Hidden real state | Draw order without Frozen Eye, RNG streams, future monster moves, private AI counters, future rewards, unrevealed potion/relic/card outcomes. | Simulator internals and belief-state inference only. |
| Omniscient/debug state | Full snapshots, RNG state, exact pile order, private monster state, verifier diffs, trace metadata. | Parity validation, debugging, handcrafted omniscient search, omniscient RL search. |

The detailed visibility reference is
`simulator/docs/rl_visibility_matrix.md`.

## Omniscient vs Fair Agents

An omniscient agent may search using hidden state, exact RNG, and future
deterministic outcomes. Early omniscient agents exist to reduce engineering risk,
validate the simulator, and bootstrap combat-root collection.

A fair agent must act only from information a player could see or infer from
public history. Two candidate approaches for fair combat search are:

- particle search over hidden states feasible given the visible history
- latent-state search anchored by predictions of real outcomes such as HP,
  enemy intent, hand damage, hand block, and other public quantities

The project currently expects little guaranteed transfer from omniscient combat
RL to fair combat RL. Possible transferable pieces include simulator APIs,
search infrastructure, value/policy architecture, training loops, and root
datasets, but not necessarily the final policy itself.

## Omniscient RL Search Notes

For omniscient combat RL, the clean conceptual target is search to terminal
combat outcome. This is valid because the full simulator state, including RNG,
is available to the search.

However, terminal search may still be expensive because the branching factor
includes card choices, targets, end turn, potion actions, and long stall lines.
The first implementation should try terminal search on bounded combat roots and
introduce depth limits or learned value cutoffs only if full search is
impractical.

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
- Can a fair combat agent use search at inference time, or should the final
  agent eventually be a direct policy?
- What is the first strong baseline for fair combat: heuristic search, particle
  search, latent world model, or another approach?

