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

Sources: [`research.md`](research.md), [`AGENTS.md`](../AGENTS.md), and simulator history from 2026-06-18.

## Trace-Driven Fidelity and Automated Collection

The simulator grew incrementally from starter combat into maps, rewards, shops,
events, potions, relics, and broader content. Real-game traces showed that
plausible mechanics were insufficient: action order, visual delays, hidden RNG
consumption, identity preservation, and importer semantics all caused
divergence. Work shifted toward first-divergence diagnosis and permanent replay
regressions. Observed state became expected output only, never a source from
which authoritative simulator state could silently repair itself. That rule was
held for state contents and lost for transition choice; see the retraction
below.

Manual traces were never expected to cover the combinatorial run space, so
automation was required. SlayTheData was the only large-scale online source of
run-level decisions the owner had identified. It was integrated to guide Neow,
routes, rewards, events, shops, and campfires while the local combat agent drove
combat.

Guided collection then exposed a structural limit: event, item, reward, and
other RNG trajectories often stop matching the source run. Keeping guidance
legal requires brittle mapping even when the simulator is coherent. In July
2026 the collection loop therefore switched to reproducible random actions
from a guarded CommunicationMod command set in the real game with 10,000
verification HP. Collection writes
immutable complete traces without waiting for the simulator. Independent
workers strictly replay those traces, retain the first valid prefix, minimize
the first divergence, and deduplicate repair tasks. Repairs can then replay the
same full trace farther to reveal its next divergence. This separation made
real-game action throughput and verifier throughput independently scalable and
removed SlayTheData legality from the fidelity-discovery loop.

Random collection subsequently acquired two explicitly separate roles.
Adaptive traces remain discovery evidence used to find and repair defects; they
cannot be reused as a certification holdout. The Phase 3A exit instead combines
a fresh frozen batch of 6,605 clean full runs with zero known in-scope failures,
green permanent regressions, and targeted coverage of rare or collector-excluded
mechanics. The run count gives a one-sided 3σ bound below one failure per 1,000
runs only for its declared test distribution.

That gate was retracted on 2026-08-18. Its green permanent regressions were not
evidence of simulator correctness, because the verifier had stopped verifying.
Faced with a boundary it could not derive, replay had grown the habit of
generating alternative next states — substituting a different action, permuting
behaviour flags, searching an RNG call count, settling only part of an end turn
— and keeping whichever one matched the recorded observation. By the end there
were 99 such generators behind 178 fallbacks, and a passing trace proved only
that one of N guesses reproduced the frame. Removing the mechanism outright
dropped the corpus from 537/539 to 55/539. The lesson is not that the guesses
were careless; each was written against real evidence. It is that once a
verifier can consult the answer, every unexplained divergence has a cheap local
fix, and the cheap fix is always available. Prohibiting the shape is the only
durable defence: transition code must not be able to see the post-state at all.

Leftover candidate apply-paths later survived as unused public wrappers
(skipped-retrieval confirms, Time Warp lag variants, deferred Colosseum
opening) and as production-path guesses that never consulted the observer
directly: skipping `initializeRelicList` at act boundaries to keep FIDL00241
green, and an encounter/hand-shape table for Discovery leftover
`generateCardChoices` pulses. Those were removed so the remaining mismatches
fail honestly. `confirm_*_skipped_retrieval` helpers still exist in combat
code for unit tests of CommunicationMod lag frames; they are not on the
`apply_run_action` path.

Two defects surfaced immediately underneath. Event obtains were being published
into the master deck a boundary early, which alone accounted for two thirds of
the honest failures. And `CommunicationMod` had been reporting ready while an
end turn was still resolving — the action queue is transiently empty after
`EndTurnAction` is popped and before its follow-ups are queued — so a command
could land mid-turn, duplicate the end turn, and silently drop a pending card
selection. Thirty of the 99 deleted generators existed to model that artifact;
with the affected traces quarantined, deleting the simulator model behind them
left the authoritative result byte-identical, which is what proved it was never
gameplay.

Some boundaries are not reproducible at all. Nineteen of 539 permanent traces
carry a command accepted mid-turn and are now quarantined rather than deleted:
retained as real-game evidence, excluded from the gate. More pointedly, the same
Neow reward under an identical command sequence shows its card in the deck at
the choice in fourteen traces and only at the map transition in five. No
deterministic rule satisfies both, so collection timing demonstrably changes
recorded outcomes rather than merely observation granularity. “Full fidelity”
must therefore be re-earned against a no-guess verifier, and any future claim
needs a corpus collected under a certified-deterministic collector.

One later fidelity discovery narrowed an important exception to the named-RNG
model: the UI-advanced, process-global `MathUtils.random` also chooses the
identity of a colored Courier restock. That state cannot be reconstructed from
a run seed or named-stream counters, so source-backed gameplay draws of this
kind are now captured as typed call-time trace inputs rather than inferred from
post-state. The vanilla audit found this exact card-selection path only for
Courier; any later path needs separate evidence and metadata.

Timing-dependent action screens later exposed a second trace-contract problem:
CommunicationMod can emit a natural state while a command is still crossing the
process pipe, so arrival order is not causal order. Boundary schema 1 labelled
poll, interaction-ready, quiescent, and terminal responses and recorded action
identity/update and queue metadata. The bridge keeps one command in flight,
settles it only on the matching declared boundary class, and ignores overtaking
transport states without semantic state guessing. Complete live runs validated
this contract before continuous random collection resumed.

Schema 1 turned out to under-specify quiescence: an empty action queue does not
mean the turn finished. Schema 2 withholds readiness while an end turn is still
resolving and publishes `end_turn_queued` so traces can be audited for that
window without inference. The two schemas are not comparable — a v1 trace can
contain commands the game should never have accepted — so the version is a
corpus generation marker, not a payload revision.

Sources: the July 2026 SlayTheData and fidelity history;
[`research.md`](research.md);
[`phase3a_statistical_fidelity_gate.md`](../simulator/docs/phase3a_statistical_fidelity_gate.md).

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
data and learning loop develops. July 2026 sealed-root experiments rejected a
pure complete-turn beam and accepted turn-boundary replanning plus a
budget-neutral complete-turn fallback; see
[`combat_search_benchmark_2026-07.md`](../simulator/docs/combat_search_benchmark_2026-07.md).

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
- Communication boundary schema made command settlement contractual rather than
  observational: `STATE` closes only on `poll`, gameplay closes only on an
  explicit ready/quiescent/terminal boundary, and steps and timing metadata fail
  closed. Schema-v0 compatibility ended on 2026-08-07; schema 2 (2026-08-18)
  added the queued-end-turn guard. Replay accepts explicit metadata/state schema
  1 or 2 with typed profile/RNG input. Old passes are evidence, not supported
  inputs.
- A verifier that may inspect the expected output will eventually select its
  transition to match it, and no amount of care in the individual cases
  prevents that. Replay advances from state and action alone; the observation
  is compared afterwards and may only report a diff. Explicitly captured
  environmental inputs are the sole exception, and a missing one must fail
  rather than be inferred.
- A green suite measures the verifier before it measures the simulator. Prefer
  a small honest number to a large one whose provenance nobody can state.
- Unreproducible captures are quarantined, never deleted or edited. Retaining
  them as evidence while excluding them from the gate is what later makes it
  possible to prove which simulator behaviour was modelling a collection
  artifact.

## Open Strategic Questions

- What measured replay and coverage gate is sufficient for A0 simulator-only
  run training?
- Should run-level learning begin with SlayTheData imitation, curriculum RL, or
  a hybrid?
- What compute and decision-time budget should constrain the final claim of
  being the strongest player?
- Can accelerated collection be shown to advance the same sequence of vanilla
  logical ticks, or must authoritative traces be collected unaccelerated? The
  throughput cost is real, but an unproven accelerator makes every residual
  divergence ambiguous between simulator defect and collection artifact.
- Should the pre-schema-2 corpus be recollected wholesale rather than repaired
  trace by trace? Traces are cheap to collect and simulator simplicity is not,
  which argues for recollection once a collector is certified.
