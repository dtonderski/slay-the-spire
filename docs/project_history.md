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

The original `simulator/` directory eventually accumulated far more than the
simulator engine: search, verification, Python bindings, live transport, a UI,
RL code, data, and documentation. Once beam-cloning training needed the same
planner as live automation, ownership mattered more than the umbrella. The
repository was reorganized into a root Rust workspace with mechanics,
verification, and search libraries, a PyO3 binding, a live application, one
root-level Python project, root-owned verification data, and consolidated docs.
The misleading umbrella was removed. This was intentionally a physical cleanup
rather than a compatibility or infrastructure program; recreatable artifacts
are regenerated after relocation.

Sources: [`research.md`](research.md), [`AGENTS.md`](../AGENTS.md),
[`repository_architecture_proposal.md`](repository_architecture_proposal.md),
and simulator history from 2026-06-18.

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
opening), while Discovery used an encounter/hand-shape table to guess leftover
`generateCardChoices` pulses. Those production-path guesses were removed so
remaining mismatches fail honestly. Target bytecode also settled a rejected
relic-pool experiment: only `Exordium` calls `initializeRelicList`; `TheCity`,
`TheBeyond`, and `TheEnding` retain the depleted pools. `confirm_*_skipped_retrieval`
helpers remain only for unit tests of CommunicationMod lag frames and are not
on the `apply_run_action` path.

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
window without inference. That flag is scoped to combat: the target can retain
the player boolean after a lethal end turn has already opened combat rewards,
and treating that stale value as unresolved deadlocks an otherwise ready
bridge. The two schemas are not comparable — a v1 trace can contain commands
the game should never have accepted — so the version is a corpus generation
marker, not a payload revision. Schema 3 withheld quiescent combat readiness
while any active monster still had `DEBUG` intent, closing the normal-speed
battle-start window before its first move was initialized. Its pilot exposed a
second lifecycle hole: an out-of-combat deferred-update flag could survive the
combat transition and publish the first card command before that queued card
resolved. Schema 4 clears that flag on entry to combat. Its replacement pilot
then exposed a transport-ordering hole at Knowing Skull: an additional state
from the preceding choice overtook execution of the next queued command, so the
bridge cleared its in-flight guard and built a new command from a state the game
had already superseded. Schema 5 carries a process-monotonic command-execution
sequence and does not complete gameplay commands until that sequence advances.
This is a protocol fence, not observed-state correction. Schema 5 then exposed
that quiescent action queues were still insufficient: `ObtainKeyEffect` and
`ShowCardAndObtainEffect` mutate keys and the master deck from dungeon effect
queues. Their duration-dependent completion varied across otherwise equivalent
reward choices. Schema 6 waits for pending `ObtainKeyEffect` and
`ShowCardAndObtainEffect` instances across the dungeon effect queues and
publishes their zero counts for audit. Its pilot clarified that the
no-queued-end-turn rule belongs to quiescent boundaries: Nilry's Codex
legitimately exposes an `interaction_ready` card reward while its source `END`
remains queued, and later lethal-end-turn traces proved that the target can
publish a terminal `GAME_OVER` boundary before its current damage action and
combat queues drain. In both cases the command fence still proves which command
reached the decision; residual target queues remain expected diagnostic output,
not simulator input. An independently audited exact-20 schema-6 pilot had valid
strict pairs, zero retrieval failures, zero pending gameplay effects, stable
repeated verifier output, and zero raw unexpected diffs; those immutable
payloads became the first post-legacy authoritative cohort. A later 208-trace
schema-6 campaign repeated those integrity checks across 309,676 transitions,
including zero failures in 2,880 audited hand retrievals and 5,491 card-reward
selections. It replaced the initial pilot as the active local and Hugging Face
cohort at that point; 123 traces passed completely and 85 retained honest
unsupported frontiers. A later audited promotion added 103 locally collected
schema-6 traces and removed 74 externally collected `working-tree` traces,
producing a 311-trace regression lock. A subsequent overnight collection added
77 more reviewed terminal traces (FIDL02009–FIDL02106, excluding one
under-specified pre-opening Colosseum publication), producing a 388-trace lock.
The next promotion added 43 reviewed FIDL02107–FIDL02154 captures and retained
two later failure-driving captures, FIDL02161 and FIDL02166. Their source-backed
repairs covered Steam Barrier's target card ID and Exhume's action-time full-hand
check, producing the current 433-trace lock.

A later 222-trace overnight cohort was frozen before its 213 passing payloads
were removed from live staging. Eight of its nine failure-driving traces exposed
generic combat gaps and now replay completely, but none entered the authoritative
lock: the collection fork's global 100× delta had also advanced dungeon playtime,
changing Act 3 Secret Portal eligibility, and one same-seed/action capture consumed
Omamori while still adding Writhing Mass's Parasite. That payload contradicts both
vanilla source and the earlier authoritative capture. All nine remain immutable
quarantined evidence pending recollection with an unmultiplied gameplay clock.

A larger apparent lifecycle ambiguity was ultimately a collection-speed defect,
not a missing trace input. The SuperFastMode collection fork multiplied the
delta used by gameplay `tickDuration`. `ExhaustAction` opens its hand screen on
one update and retrieves selected cards on a later update; at 100×, an opening
frame over 2.5 ms expired its 0.25-second duration immediately. The same
failure shape affected PutOnDeck, Gambling Chip, Forethought, Armaments, Dual
Wield, and Recycle. Across the expanded 602-file corpus, 2,222 of 8,288 audited
selected-card confirms skipped retrieval, contaminating 328 traces. Fork `.2`
gave gameplay action state machines fixed 60 Hz ticks, but mistakenly left
`AbstractDungeon.update` on multiplied delta. A later 163-trace cohort again
advanced playtime far faster than wall time and remains frozen, not promoted.
Fork `.3` restores raw wall-clock delta only for that dungeon clock while
leaving action ticks deterministic and visual transitions accelerated.

The hand-selection audit was subsequently shown to be only a narrow detector:
an unquarantined Discovery reward had the same skipped-retrieval artifact, and
other timing-sensitive paths could not be certified trace by trace. The entire
602-trace pre-`.2` cohort was therefore archived unchanged as legacy evidence
and removed from the authoritative gate. A paired `.2`/unmodded run disproved a
trace-fitted claim that generated draw-pile cards could be randomly inserted
four or seven times; target bytecode and three matched Wild Strike insertions
show exactly one insertion draw. That workaround was deleted. The same paired
run exposed a separate bridge defect at normal speed: CommunicationMod could
publish combat while a living monster still had `DEBUG` intent, allowing a
command before its first move was initialized. Current collection blocks that
boundary before beginning a small replacement pilot.

Sources: the July 2026 SlayTheData and fidelity history;
[`research.md`](research.md);
[`phase3a_statistical_fidelity_gate.md`](phase3a_statistical_fidelity_gate.md).

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
[`combat_search_benchmark_2026-07.md`](combat_search_benchmark_2026-07.md).

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
  added the queued-end-turn guard. Schema 3 (2026-08-20) added the
  uninitialized-monster-intent guard; schema 4 prevents deferred out-of-combat
  stabilization from completing a combat command; schema 5 requires a monotonic
  command-execution fence before a gameplay boundary can close; schema 6 also
  waits for gameplay-affecting dungeon effect queues. Replay accepts explicit
  metadata/state schemas 1 through 7 with typed profile/RNG input. Schema 7 adds
  target-echoed command identity plus separate attempt and settlement sequences
  so stale or rejected responses cannot be misattributed. The locked corpus
  remains schema 6. Old passes are evidence, not supported inputs.
- A verifier that may inspect the expected output will eventually select its
  transition to match it, and no amount of care in the individual cases
  prevents that. Replay advances from state and action alone; the observation
  is compared afterwards and may only report a diff. Explicitly captured
  environmental inputs are the sole exception, and a missing one must fail
  rather than be inferred.
- A green suite measures the verifier before it measures the simulator. Prefer
  a small honest number to a large one whose provenance nobody can state.
- Unreproducible captures are quarantined, never deleted or edited. An entire
  uncertified collection epoch may instead be archived as legacy evidence.
  Retaining both while excluding them from the active gate is what later makes
  it possible to prove which simulator behaviour was modelling a collection
  artifact.

## Open Strategic Questions

- What measured replay and coverage gate is sufficient for A0 simulator-only
  run training?
- Should run-level learning begin with SlayTheData imitation, curriculum RL, or
  a hybrid?
- What compute and decision-time budget should constrain the final claim of
  being the strongest player?
- The mixed `ExhaustAction` retrieval outcome was traced to multiplied gameplay
  delta and removed from future collection without adding lifecycle inputs.
  The pre-collection.2 corpus is legacy. The active external regression cohort
  now contains 433 reviewed schema-6 traces: the original 208-trace replacement,
  103 locally collected traces externally attested to the collection.2 artifact
  despite their stale metadata label, 77 terminal FIDL02009–FIDL02106 captures,
  and 45 traces from the next review waves. One additional capture published
  Colosseum's second combat before its opening queue was installed; because the
  trace lacks a pre-action scheduler input that distinguishes that race, it
  remains immutable outside the authoritative lock. The current simulator
  replays all 433 through EOF without unsupported frontiers. Post-state candidate
  selection remains prohibited.
