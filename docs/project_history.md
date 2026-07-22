# Project History

This is a curated account of why the project took its current shape, not a
changelog or status report. Git records implementation details; focused design
and experiment documents hold supporting evidence.

The early history was reconstructed on 2026-07-11 from repository history,
project documents, and discussion with the project owner. Development sequences
are repository-backed; retrospective motivations should be corrected if better
firsthand context appears.

## Current Thesis

The objective from the beginning has been to build the strongest A20 Heart
Ironclad player in the world, although that remains distant. The current working
strategy is to establish credible simulator fidelity with real-game traces, use
the existing deterministic combat search as an adequate executor, and develop a
fair run-level policy. Run-level decisions are presently considered the main
bottleneck to generating varied, internally consistent simulator runs and later
combat roots. This ordering is a working hypothesis, not an experimental result.

## Origin and Simulator Choice

The project also began as a software-engineering experiment: could current LLMs
build a substantial, faithful Slay the Spire simulator and its surrounding
verification, data, and operator tooling? Building that system was intrinsically
interesting rather than merely a prerequisite for RL.

`sts_lightspeed` was considered as an implementation base or oracle. Rust was
preferred for LLM-driven development because its type system and compiler can
enforce invariants during iterative changes, while the existing C++ code was
hard for the owner to inspect. At least one concrete `sts_lightspeed` bug was
found; once it could not be authoritative, real-game traces and independent
verification were necessary regardless. It remains valuable prior art and a
secondary differential oracle.

The simulator therefore preserved action queues, named RNG streams, snapshots,
and deterministic replay instead of simplifying them for early RL. This made
development slower but allowed the same core to support verification, search,
and learning. Real-game state observed through CommunicationMod became the
primary parity evidence.

Sources: [`research.md`](research.md), [`AGENT_RULES.md`](../AGENT_RULES.md), and simulator history from 2026-06-18.

## Trace-Driven Fidelity and Automated Collection

The simulator grew incrementally from starter combat into maps, rewards, shops,
events, potions, relics, and broader content. Real-game traces showed that
plausible mechanics were insufficient: action order, visual delays, hidden RNG
consumption, identity preservation, and importer semantics all caused
divergence. Work shifted toward first-divergence diagnosis and permanent replay
regressions. Observed state became expected output only, never a source from
which authoritative simulator state could silently repair itself.

Manual traces were never expected to cover the combinatorial run space, so
automation was required. SlayTheData was the only large-scale online source of
run-level decisions the owner had identified. It was integrated to guide Neow,
routes, rewards, events, shops, and campfires while the local combat agent drove
combat.

Guided collection then exposed a structural limit: event, item, reward, and
other RNG trajectories often stop matching the source run. Keeping guidance
legal requires brittle mapping even when the simulator is coherent. The current
decision is to use SlayTheData to obtain broad Act 1-3 fidelity traces without
making exact reconstruction the permanent data architecture. Strict replay
validates fidelity; guided collection is a temporary bridge to varied states.
Roughly 100 long runs may provide useful coverage, but readiness depends on
measured, understood divergence rather than trace count alone.

Sources: `simulator/SLAYTHEDATA_CLI_COLLECTION_STATUS.md` and the July 2026
SlayTheData and fidelity history.

## The Trace UI Rewrite

The clearest engineering failure was implementing too much of the first trace UI
at once while keeping substantial workflow state in the frontend. Multiple
integrations were delegated under an assignment close to "write the frontend,"
and synchronization became unreliable. The prototype was archived and rewritten
with the backend as authoritative owner of session, automation, and collection
state; the frontend became a minimal operator view.

The durable lesson is to keep one authoritative state machine for synchronized
tools and grow integrations in tested slices rather than commissioning the whole
workflow at once.

## The First Combat Agent

The deterministic combat agent was built to automate trace collection and reach
more states for simulator debugging. It was never intended as the final player.
Its current role is pragmatic: it is good enough to execute combats while the
data and learning loop develops.

The adopted learned-combat direction is AlphaZero-style Expert Iteration, but
with a deliberate information split: policy/value networks receive only public
observations and history, while the first teacher search follows the one true
hidden simulator state. This keeps the learned representation reusable when the
privileged root is later replaced by a particle belief. The existing beam search
bootstraps policy/value training; it is scaffolding rather than a permanent
teacher ceiling. Public choices reference visible slots and map back to the one
authoritative action type, rather than creating parallel fair legality. Potion
budgets were rejected as a learned-agent abstraction: provisional combat value
scores resulting resources, and the eventual run-level value model evaluates
the complete post-combat state.

## Run-Level RL Moved Ahead of Combat RL

The repository roadmap placed combat learning before run-level learning. By July
2026, deterministic combat search appeared adequate for bootstrapping runs,
while high-level choices remained the obstacle to reaching later acts and
creating a broad on-policy distribution. Improving combat from good to excellent
would not remove that bottleneck.

The adopted working sequence is therefore: validate enough A0 full-run fidelity;
use search as the combat executor; train a fair run-level policy/value model;
generate varied simulator runs and roots; then train omniscient and eventually
fair combat agents on the improved distribution. A20 Heart remains the final
target, not the first environment used to debug this loop.

## LLM Development Method

Bounded, verifiable loops have worked well, including long overnight runs with
objective stopping conditions. Large ambiguous whole-system assignments have
not: they require more human steering and tighter slices. Early attempts to
maximize explicit sub-agent use also provided no inherent quality advantage, so
delegation is now left to the working agent when it genuinely helps. Evidence,
scope, and feedback-loop quality matter more than task duration or agent count.

## Durable Lessons

- Exact replay and productive data collection are different modes and provide
  different evidence.
- Observations may validate simulator state but must never re-anchor it.
- Synthetic RL will seek simulator exploits, so it needs held-out real-trace
  audits.
- Fairness belongs at explicit observation boundaries; debug state may remain
  omniscient while final policies receive only public information.

## Open Strategic Questions

- What measured replay and coverage gate is sufficient for A0 simulator-only
  run training?
- Should run-level learning begin with SlayTheData imitation, curriculum RL, or
  a hybrid?
- What compute and decision-time budget should constrain the final claim of
  being the strongest player?
