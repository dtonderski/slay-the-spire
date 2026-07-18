# Fair Belief Architecture

Status: design-only. No simulator mechanics or RL code are implemented by this
document. Last updated: 2026-07-09.

This document designs a fair agent architecture for Ironclad combat and run
play that handles hidden information without cheating. It consolidates and
extends [`research_tree.md`](../../docs/literature_review/research_tree.md),
[`combat_tree_search.md`](../../docs/literature_review/combat_tree_search.md), and the archived
`rl_visibility_matrix.md` / `fair_action_schema.md` drafts, grounded in the
current `sts_core` / `sts_verify` / `sts_live` / `py_sts` code.

Hard constraints restated:

- Runtime fair agents receive only fair observations and public action
  history.
- Hidden simulator state may be used for training labels, teacher signals,
  debugging, and calibration, but never as runtime input.
- The exact simulator remains the source of truth.
- Content completion is not the answer; the architecture must work on the
  content the simulator already verifies (A0 Ironclad Act 1 today) and grow
  with it.
- Prefer staged, falsifiable experiments over grand architecture.

## Summary Of Answers

1. **Is a learned latent belief/world model feasible and useful compared with
   exact hidden-state particle search?** Feasible, yes: observations are
   symbolic and compact, the simulator generates unlimited labeled data, and a
   few-million-parameter model fits the laptop budget. Useful, yes — but only
   as a second stage. Exact particle search must come first because Slay the
   Spire combat has a nearly *analytic* belief update (see the lazy-commitment
   observation in the sampler section): most hidden state is a uniformly
   shuffled multiset the agent can track exactly by card counting. A learned
   latent model buys speed and generalization, not correctness, and it can
   quietly hallucinate impossible states or learn simulator bugs. The latent
   model is trained, calibrated, and evaluated against the exact
   simulator/particle baseline, and is only adopted where it matches the
   teacher at a fraction of the compute.
2. **What particle sampler should we build as baseline and teacher?** A
   *constrained regeneration sampler*: particles are full authoritative
   `CombatState` values whose public-known fields are reconstructed exactly
   from public history via a knowledge tracker, whose draw-pile order is
   lazily committed (sampled only when a draw is about to be observed), and
   whose RNG streams are freshly re-seeded per particle. Filtering is needed
   only for the small set of committed-but-hidden values (for example intents
   under Runic Dome). POMCP-style search over these particles is the fair
   baseline planner and the teacher for all learned models.
3. **What is the formal fair-observation boundary?** A pure projection
   function `fair_view(authoritative_state, public_history)` plus the
   observational non-interference invariant: states that differ only in
   hidden fields must produce byte-identical fair observations, action
   descriptors, masks, public errors, and info — until normal gameplay reveals
   the difference. Seed/RNG-stream cryptanalysis from public history is
   declared out of protocol for reported agents.
4. **How do we test that fair observations never leak?** A layered invariance
   suite: type/serialization allowlisting, hidden-state mutation
   non-interference property tests, zero-RNG-draw guards, adversarial probes
   that try to predict hidden values from fair outputs, mutation testing of
   the projection itself, and dataset channel audits. Details below.
5. **What data should the exact simulator generate?** Combat roots (a new
   serialized format), paired-channel trajectories (fair channel vs hidden
   label channel), particle-posterior and analytic-distribution calibration
   targets, POMCP search targets (visit counts, values), and held-out
   evaluation suites with paired-seed variance reduction.
6. **What tree search should operate over latent belief states?** A
   stochastic MuZero-style search (decision nodes plus chance nodes) with
   Gumbel root action selection, over a recurrent belief latent encoded from
   fair observation history. Before that, the same networks should be used as
   priors/values *inside* particle POMCP, which is the lower-risk hybrid.
7. **First experiments?** E1 fair facade plus invariance suite, E2 sampler
   calibration, E3 fair POMCP vs baselines on a fixed root corpus, E4
   supervised belief model calibration, E5 latent search vs teacher at
   matched wall-clock. Metrics and kill criteria are in the roadmap section.

## 1. Information Taxonomy: Fair / Hidden / Public-History

Three classes, aligned with `PROJECT_OVERVIEW.md` State Visibility and refined
against current `sts_core` state types.

### Fair-observable (allowed as runtime input)

Combat:

- Player: current HP, max HP, block, energy, visible powers with amounts.
- Hand: ordered visible slots with card name, upgrade level, current visible
  cost, visible keywords/text-affecting state.
- Draw pile: count, and contents *as an unordered multiset* (the real pile
  viewer shows sorted contents, not order). Ordered prefix only under an
  explicit visibility rule (Frozen Eye; Headbutt-style placements the player
  performed).
- Discard/exhaust piles: counts and visible contents (UI-inspectable).
- Monsters per visible slot: kind, current HP, max HP, block, powers, current
  intent with displayed damage and hit count, `intent_visible` flag (Runic
  Dome), alive/escaped/minion status.
- Potion slots and identities; relics and visible counters (for example Pen
  Nib, Nunchaku, Kunai/Shuriken progress — visible in the real UI).
- Turn number, cards played this turn, active selection/grid/choice screens
  with their visible options.

Run level: whole-act map topology and room symbols, current node, floor, act,
gold, master deck as a multiset (deck viewer), relics, potions, boss identity
for the act, visible screen contents (shop inventory once entered, event text
and currently offered choices, reward offers once shown, Neow options).

### Public history (allowed as runtime input; derived values are fair)

- The agent's own action log.
- Public event log: cards drawn into hand, cards discarded/exhausted/shuffled
  as displayed, damage numbers dealt and received, monster moves actually
  executed (move history is public — the player watched every move), potion
  and relic triggers that animate, shuffle events.
- Anything *computed* from the above is fair: card-counting state, remaining
  draw-pile multiset, "top k cards known" tracking, monster move-history
  vectors, expected-damage arithmetic.

### Hidden (never runtime input; allowed for training labels, teachers,
debugging, calibration)

- Draw pile order (absent a visibility rule); discard/exhaust internal order
  beyond what the UI shows.
- All RNG state: seeds, `StsRng.counter` values, `JavaRng` state, every
  `RunState` per-stream seed/counter pair, combat-embedded `shuffle_rng`,
  `monster_rng` (aiRng), `monster_hp_rng`, `card_random_rng`.
- Future monster rolls; committed-but-hidden intents (Runic Dome); private
  monster fields not implied by public history (`rolled_attack_damage` before
  its intent number is displayed, `stasis_card` identity before reveal,
  internal mode counters not mirrored by a visible power).
- Unrealized generation: card reward contents before the screen opens
  (`card_reward_pending`), shop stock before entry, chest contents, potion
  drops, event identity before entering the room, all pool orders.
- Internal identity: `CardId`, `MonsterId`, `ContentId`, UUIDs, generation
  provenance.
- Simulator scaffolding: snapshots, hashes, event logs, verifier metadata,
  fidelity labels, action queue internals.

Classification rule for anything new: *would the real game's UI show it, or
could a careful player derive it from what the UI showed earlier this run?*
If neither, it is hidden. Ambiguous cases get a row in the visibility matrix
with an explicit decision, never a silent default.

### The seed-divination caveat

Given enough observed random outcomes, RandomXS128 state is in principle
recoverable, which would let a "fair" agent reconstruct the entire hidden
future from public history. This is technically inference from public inputs,
but it collapses the research problem into RNG cryptanalysis and does not
generalize to any game whose seed the agent does not control. The evaluation
protocol therefore declares: **reported fair agents must not attempt RNG
stream or seed reconstruction from observed outcomes.** The canonical belief
model treats unknown randomness as exchangeable — every not-yet-realized
random outcome is drawn fresh from the source-backed distribution, not from a
recovered stream. This convention is what makes the lazy-commitment sampler
below *exact* rather than approximate.

## 2. Formal Fair-Observation Boundary

The boundary is a function, not a habit:

```text
fair_view : (AuthoritativeState, PublicHistory) -> FairObservation
fair_actions : (AuthoritativeState, PublicHistory) -> Vec<FairActionDescriptor>
fair_mask : (AuthoritativeState, PublicHistory) -> FixedMask
```

Requirements:

- Pure and deterministic: no mutation, no RNG draws (verifiable via
  `StsRng.counter` deltas), no dependence on memory layout or map iteration
  order.
- **Observational non-interference (the core invariant):** for any two
  authoritative states `s1`, `s2` with identical public history whose
  differences are confined to hidden fields, `fair_view`, `fair_actions`,
  `fair_mask`, every public error produced by rejected actions, and every
  fair `info` field must be byte-identical, until normal gameplay reveals the
  difference. This is stricter than "don't expose the draw pile": hidden
  state can leak through descriptor ordering, mask shape, target lists, and
  validation outcomes (the Havoc case in the archived fair action schema).
- Fair action descriptors use visible slots (hand slot, monster slot, option
  slot), never internal IDs. The descriptor taxonomy from the archived
  `fair_action_schema.md` is adopted as-is; it does not need redesign, it
  needs implementation and tests.
- The fair runtime surface offers no snapshot/restore, no state hashes, no
  event logs. Reproducibility in fair mode is seed/config plus the visible
  action log, replayed through the authoritative simulator.
- Capability separation: fair APIs and omniscient/debug APIs live in visibly
  different types/modules (today `py_sts::OmniCombatEnv` and
  `sts_live::automation` are explicitly omniscient — they stay that way and
  keep the `Omni` naming). The fair facade should be a new crate (working
  name `sts_fair`) that depends on `sts_core` one-way, per the archived
  first-slice plan.

Formally, the fair game is the belief MDP induced by: hidden state drawn from
the game's generative distribution (source-backed RNG semantics with
exchangeable unrealized outcomes), observation function `fair_view`, action
set `fair_actions`. Fair planners may *internally* instantiate hidden states
(particles) — the constraint binds the policy's runtime inputs and the
information flow out of the facade, not the planner's internal machinery.
This matches the existing rule in `rl_visibility_matrix.md`: belief systems
may track hidden possibilities, but the policy network never inspects raw
particle internals directly; it sees fair observations and belief *summaries
computed only from fair inputs plus the sampler's declared prior*.

## 3. Combat-First Fair Observation Schema

Working shape (field names indicative, not final Rust):

```text
FairCombatObservation
  schema_version
  turn
  phase                      # NormalCombat | HandSelect | GridSelect | ... (decision substates)
  player
    hp, max_hp, block, energy
    powers: [ { power_kind, amount, secondary_amount_if_visible } ]
  hand: [ { name_id, upgrades, visible_cost, playable_shape } ]   # ordered, visible slots
  draw_pile
    count
    known_multiset: [ { name_id, upgrades, count } ]   # from card counting; equals true multiset
    known_top: [ name_id ... ]                         # only under Frozen Eye / player placements
  discard_pile { count, contents_multiset }
  exhaust_pile { count, contents_multiset }
  monsters: [ {
    slot, kind_name, hp, max_hp, block,
    powers: [...],
    intent { visible: bool, category, damage?, hits? },
    status flags: alive / escaped / minion
  } ]
  potions: [ { slot, identity | empty } ]
  relics: [ { name_id, visible_counter? } ]
  selection_view?            # SelectionView / GridSelectView per fair_action_schema.md
  public_counters            # cards played this turn, times attacked this combat if UI-derivable, etc.
```

Notes:

- `known_multiset` is computed by the wrapper's knowledge tracker from public
  history; because the starting deck and every zone transition are public in
  A0 Ironclad play, it equals the true multiset. It lives in the observation
  as a convenience so non-recurrent policies do not need to re-derive it.
- Intent carries displayed damage (already strength/weak-adjusted in the real
  UI). Under Runic Dome, `visible: false` and category/damage/hits are
  absent.
- No `CardId`/instance identity anywhere. Duplicate cards are
  indistinguishable in fair view, exactly as in the real UI.
- Public history is exposed to recurrent policies as a token stream of
  (fair observation delta, action, public events) — the facade emits the
  public event list per step so wrappers do not scrape it from diffs.

## 4. Full-Run Fair Observation Extensions

Additions for run-level play, in dependency order of likely implementation:

```text
FairRunObservation
  act, floor, ascension
  gold
  map { nodes: [ { row, x, symbol } ], edges, current_node, boss_name }
  master_deck_multiset
  relics, potions                     # same encoding as combat
  screen                              # exactly one primary decision substate
    MapChoice { options }
    RewardScreen { visible_offers }   # gold amount, card-reward-present, potion identity once shown, relic identity once shown
    CardRewardScreen { offered_cards }
    ShopRoom / ShopScreen { inventory with prices, purge cost }  # only after entry
    RestSite { visible_options }
    Event { event_name_as_displayed, visible_choice_labels }
    NeowScreen { visible_options }
    Chest { opened: bool, contents_if_shown }
    Terminal { outcome }
  keys/act4 state when in scope
```

Hidden at run level: unentered room contents (event identity, shop stock,
chest contents), unopened card rewards, future encounter assignment beyond
map symbols, all pools and pool orders, Neow outcomes behind unchosen
options. Event stages already revealed (for example Cursed Tome pages seen)
are public history.

Run-level fairness has one structural difference from combat: decisions are
sparse and hidden information mostly resolves *between* decisions (you learn
the card reward when you open it, before choosing). Belief tracking matters
much less; the hard part is value estimation over long horizons. This is why
the architecture is combat-first: combat is where belief quality changes
per-action decisions.

## 5. Hidden-State Particle Sampler (Baseline And Teacher)

### Key structural observation: lazy commitment

In Slay the Spire combat, almost all hidden state is *not yet committed*
under the exchangeable-randomness convention:

- Draw pile order: after any shuffle, the order is a uniform permutation of a
  multiset the agent knows exactly (card counting). The posterior over the
  unseen suffix remains uniform over the remaining multiset after every
  observed draw. Therefore the sampler never needs to commit an order in
  advance: represent the draw pile as `known_top_prefix + unknown_multiset`,
  and sample the identity of a drawn card only at the moment a draw resolves.
  Particles can never be "wrong about" draw order, so there is no depletion
  from draws — the dominant failure mode of naive particle filters in card
  games disappears by construction.
- Future RNG (monster rolls, Whirlwind-style card randomness, potion drops):
  not committed until consumed; each particle consumes freshly seeded streams.
- Monster intents: rolled at end of the previous turn and *displayed*, so
  they are observed the moment they are committed. No inference needed except
  under Runic Dome.

The truly persistent hidden state — values committed before observation — is
small and enumerable:

| Committed hidden value | When committed | When revealed | Belief treatment |
|---|---|---|---|
| Intent under Runic Dome | end of prior turn | when the move executes | filter/weight particles by the source-backed move distribution given public move history; state space per monster is tiny (move table) |
| `stasis_card`-style captures | on effect | on reveal | uniform over the known multiset of the source pile |
| Unrevealed spawn rolls (for example Louse damage before first intent display) | combat entry | first display | sample from source-backed range, filter on reveal |
| Frozen Eye absent, Headbutt-style placements by *monsters* (rare) | on effect | on draw | tracked as known-position unknown-identity slots |

### Particle representation

A particle is a full authoritative `CombatState` (plus the minimal `RunState`
context needed by relic/potion interactions), constructed as:

1. Copy all public-known fields directly from the knowledge tracker: player
   stats, powers, monster HP/block/powers/move history, hand identities,
   discard/exhaust multisets, relic counters, potion belt.
2. Fill the draw pile with the known multiset in a *freshly sampled* uniform
   order (or keep it symbolically unordered and let the facade's draw hook
   commit lazily — the implementation may choose either; lazy commitment is
   the preferred design because it also makes belief updates free).
3. Sample each committed-hidden value from its filtered posterior (table
   above).
4. Re-seed every RNG stream from the sampler's own RNG. Counters start
   fresh; no attempt is made to match the true streams.

Because steps 1–3 sample from the *exact* conditional distribution given
public history (uniform shuffle is source-backed: `Collections.shuffle` over
a fresh `JavaRng` is uniform), the particle set is unbiased. This is a
stronger property than generic POMCP particle filtering and should be
protected by calibration tests (E2).

### Belief update at a real step

- Apply the real action to each particle; step the simulator.
- Compare each particle's public projection to the real observation.
- For lazily committed values there is nothing to filter (identity by
  construction). For committed-hidden values, drop/reweight inconsistent
  particles and regenerate replacements from the posterior (regeneration is
  cheap because the constructor above is a direct sampler, not rejection).
- Particle count: start with 256–1024; the E2 experiment measures what is
  actually needed. Expected to be small precisely because most uncertainty is
  analytic.

### Planner: fair POMCP

- UCT tree over public histories from the current root belief; node keys are
  fair observation/action digests, never hidden state.
- Each simulation draws a particle from the root belief, then rolls the exact
  simulator forward, consuming the particle's own RNG.
- Statistics aggregate at fair action descriptors.
- Leaf evaluation: first a handcrafted heuristic (lethal check, HP delta,
  incoming-damage-vs-block), later a learned value network with fair inputs.
- Anytime budgeted; per-decision time budget is part of the evaluation
  protocol.

### Teacher roles

- Generates training targets: root visit distributions (policy targets),
  root value estimates, and per-step belief summaries.
- Defines the fair-performance baseline every learned system must beat or
  match at lower cost.
- Paired with the existing omniscient search (`automation.rs` greedy/beam,
  later omniscient MCTS) to measure the *value of hidden information*: the
  gap between omniscient and fair search on identical roots. That gap is the
  ceiling on what any belief model can recover, and combats with a near-zero
  gap do not need belief sophistication at all.

## 6. Latent Belief / World-Model Architecture

Second stage, gated on E2/E3 results. Purpose: amortize belief tracking and
simulation into a fast network so that search or a direct policy runs at a
fraction of particle-POMCP cost, and (later) so run-level play can share the
representation.

### Components

```text
h_t = f_enc(o_1, a_1, e_1, ..., o_t)        # belief encoder over fair history
        f_enc: Transformer or GRU over tokenized symbolic observations/events
        h_t is the latent belief state (deterministic summary)
(pi_t, v_t) = f_pred(h_t)                    # policy and value heads (fair)
z ~ f_chance(h_t, a)                         # chance-outcome head: discrete codes for
                                             # stochastic public events (which card is drawn,
                                             # which intent is rolled, damage rolls)
h_{t+1} = f_dyn(h_t, a, z)                   # latent dynamics (afterstate -> chance -> next)
aux heads (training only):
  next-draw distribution over known multiset
  next-intent distribution per monster
  hidden-state marginals (e.g., Runic-Dome intent) supervised from labels
```

Design decisions:

- **Symbolic tokens, no reconstruction.** Observations are already compact
  and structured; the model predicts planning-relevant quantities (reward,
  value, policy, public event distributions), MuZero-style, not observation
  reconstruction.
- **Stochasticity is explicit.** Combat transitions are stochastic given fair
  information; a deterministic latent dynamics model (vanilla MuZero) would
  average over draws and intents. Use afterstate/chance factorization
  (Stochastic MuZero) with discrete chance codes aligned to the game's real
  event structure. This also makes calibration testable: the chance head's
  next-draw distribution must match the analytic card-counting distribution,
  which is a rare luxury — the ground-truth belief is computable.
- **Hidden labels as auxiliary heads only.** Training may supervise hidden
  marginals from the simulator's hidden channel (this sharpens the belief
  representation), but those heads are stripped at inference and are never
  inputs to policy/value/dynamics — information flows from hidden labels into
  `h_t` only through gradients, which is fair (the runtime input is still
  only public history).
- **Scale.** Order 1–10M parameters, context of a few hundred tokens per
  combat; comfortably trainable and servable on a 5080-class laptop. This is
  not the risk; fidelity is.

### Why this is worth having despite exact particles

- A forward pass is microseconds; a particle simulation step is a full
  simulator rollout. At equal wall-clock, latent search explores far more.
- A belief latent is a differentiable, fixed-size object a policy can consume
  directly — needed if the final evaluation protocol restricts inference-time
  search.
- Run-level value estimation (Phase 7+) needs long-horizon function
  approximation regardless; sharing the belief encoder is the natural path.

And why it must not come first: it can assign probability to impossible
states, silently miss rare relic/card interactions, and inherit simulator
bugs invisibly. Every one of those failure modes is detectable only against
the exact baseline, which therefore has to exist first.

## 7. Simulator-Generated Dataset Plan

All datasets are generated by the exact simulator; every file carries
simulator version, content version, schema version, and a source label
(matching the fidelity discipline in [`design.md`](../../docs/design.md)). Scope is the currently
verified content surface (A0 Ironclad, Act 1 executable set); the corpus
grows as verification grows — no content brute-forcing to feed the model.

### D0: Combat root corpus (new format, prerequisite for everything)

No `CombatRoot` type exists today; define one:

```text
CombatRoot
  schema_version, sim_version, content_version
  source_label: manual_trace | strict_replay | guided_replay | simulator_only
  seed + all RNG stream seeds/counters at combat entry
  ascension, act, floor, room kind, encounter key, combat turn (0 for entry roots)
  full RunState/CombatState snapshot (hidden allowed: this is a root, per PROJECT_OVERVIEW)
  fair_view digest at root (for split integrity checks)
  provenance: trace id / session id where applicable
```

Sources: the existing permanent-trace corpus (10 passing traces, 2475
verified transitions), SlayTheData-guided collection sessions, and
simulator-only rollouts under a declared policy. Stratify by encounter key,
deck size, relic set, and floor; version and deduplicate.

### D1: Paired-channel trajectories

Per decision step, two physically separate channels:

- **Fair channel** (model input): fair observation, fair descriptors, fixed
  mask, chosen action, public event list, terminal outcome and public reward
  signals.
- **Hidden/label channel** (training targets and debugging only): full
  snapshot, RNG stream states, committed hidden values, next-draw identity,
  next intents, POMCP root visit counts and value, particle-belief summaries,
  omniscient-search value for the same root.

Channel separation is schema-level (different files/streams), so a dataset
audit can prove fair-channel files contain no hidden keys.

### D2: Calibration targets

For sampled decision points: the *analytic* conditional distributions that
the ground-truth belief implies — next-draw distribution from the knowledge
tracker, next-intent distribution from source-backed move tables given public
move history, damage-roll distributions. These are exact labels for both the
sampler (E2) and the latent chance head (E4). Slay the Spire is unusual in
that these ground-truth belief marginals are computable; use that.

### D3: Evaluation suites

- Held-out combat roots split by encounter key and by deck archetype (no
  leakage of near-duplicate roots across splits; use the fair-view digest).
- Paired-seed evaluation: fair and baseline agents play the same hidden
  realizations for variance reduction.
- A small human-trace benchmark subset (from CommunicationMod corpus) for
  outcome comparison, per the Phase 2 success gate.

## 8. Latent Tree-Search Design

Adopt in two steps, cheapest risk first:

### Step A (hybrid, recommended before pure latent search): learned priors
and values inside particle POMCP

Policy/value networks consume fair observations (or `h_t`) and steer the
exact-particle search: priors focus expansion (PUCT), the value head replaces
rollouts. Search remains over exact simulator states, so there is no model
fidelity risk; the networks are trained from D1 teacher targets (AlphaZero
loop over POMCP). This will likely be the strongest fair agent for a long
time and is the fallback if pure latent search fails.

### Step B: pure latent belief search

- Tree over latent belief states rooted at `h_t` from the current real
  history.
- Node structure mirrors the game's factorization: decision node → afterstate
  (via `f_dyn` on action) → chance node sampling/expanding discrete codes
  from `f_chance` (progressive widening over chance outcomes if needed) →
  next decision node.
- Root action selection with Gumbel top-k (strong at small simulation
  budgets, which is the target regime — the point of latent search is small
  budgets).
- Legality comes only from the fair mask at the root and from a
  mask-prediction head in the tree (never from querying the simulator's
  hidden state mid-tree).
- Sanity guards: reject/penalize latent rollouts whose predicted public
  events have near-zero probability under the analytic belief (D2 gives us
  this check almost for free at training and evaluation time).
- Evaluation is always against Step A and raw POMCP at matched wall-clock on
  the same held-out roots.

## 9. Fairness Invariance Test Suite

Layered defenses; each catches leaks the previous layer misses.

1. **Type and serialization boundary.** `FairObservation` and descriptors
   live in the fair crate; construction only via `fair_view`. Serde output is
   allowlisted field-by-field; a schema test walks the serialized form and
   fails on any key not in the allowlist. No `Debug`-format escape hatches in
   fair `info`/errors.
2. **Non-interference property tests (the core).** Generate a state, then
   produce hidden-equivalent variants: permute draw pile order, re-seed all
   RNG streams and perturb counters, replace committed-hidden values
   (Runic-Dome intent within the legal move set, unrevealed rolls), swap
   unrealized pools/pending rewards, renumber internal IDs. Assert
   byte-identical `fair_view`, descriptor lists, fixed masks, and — via a
   scripted probe that attempts every descriptor and a set of invalid ones —
   identical public errors and info. Run as property tests over randomly
   generated combats (AGENT_RULES already calls for property tests on
   randomly generatable state).
3. **Divergence-timing tests.** After the non-interference pair *should*
   diverge (the hidden difference becomes visible through a draw or move),
   assert the fair views differ — this catches an over-redacted facade that
   hides public information, which would silently cripple the agent.
4. **RNG and mutation guards.** `fair_view`/`fair_actions`/`fair_mask`
   consume zero RNG draws (assert all `StsRng.counter`s unchanged) and do not
   mutate state (hash before/after on the authoritative snapshot — debug-mode
   only, since fair mode has no hashes).
5. **Known-exception tests.** Frozen Eye exposes order and only then; Runic
   Dome hides intent everywhere it appears (observation, descriptors, info);
   Havoc masks are target-shape-invariant across hidden top cards, per the
   archived schema's analysis.
6. **Adversarial probes (statistical).** Train a small classifier to predict
   a hidden bit (top-of-deck identity, Runic-Dome intent) from the fair
   channel of D1. Its accuracy must match the analytic belief baseline within
   confidence bounds; better-than-belief accuracy indicates a leak in the
   observation, the mask, or dataset construction (for example ordering
   artifacts). Run this as a dataset-release gate, not just once.
7. **Mutation testing of the projection.** Deliberately introduce leaks
   (include one hidden field, sort a multiset by internal ID, expose an exact
   error) in a test harness and assert the suite fails. A leak suite that
   cannot detect a planted leak is decoration.
8. **Runtime capability audit.** The fair environment API surface has no
   snapshot/restore/hash/log methods at all (not merely "unused"), and the
   Python fair wrapper cannot reach `OmniCombatEnv` internals without an
   explicit, separately imported omniscient module.

## 10. Experiment Roadmap

Ordered, falsifiable, each with success metrics and kill criteria. Budgets
are per-decision wall-clock on the laptop target.

### E1: Fair facade and invariance suite (foundation)

Build `fair_view`, descriptors, masks over the existing combat fixture set;
implement suite layers 1–5.

- Success: all invariance tests pass over randomly generated verified-content
  combats; probe layer 6 passes on a small pilot dataset.
- Kill criterion: none — this is prerequisite infrastructure. But if
  non-interference cannot be satisfied for some mechanic without redesigning
  core state, stop and write the design note first (AGENT_RULES rule 7).

### E2: Sampler calibration

Implement the knowledge tracker and constrained regeneration sampler; verify
against analytic distributions (D2) across many seeds and encounters.

- Metrics: chi-squared / exact-test agreement of sampled next-draw and
  next-intent distributions with analytic ground truth; effective sample size
  of the particle set after T turns of real play; regeneration cost per step.
- Success: no detectable bias at p > 0.01 across the verified encounter set;
  ESS stays above 50% of particle count through full Act 1 combats with ≤ 1k
  particles.
- Kill criterion: if committed-hidden filtering (Runic-Dome-style cases)
  degenerates or the sampler cannot match analytic marginals, stop and fix
  the belief representation before building any planner on top — a
  miscalibrated sampler poisons both the baseline and every teacher target.

### E3: Fair POMCP baseline vs ladder on fixed roots

Run on a frozen D0 root corpus: (a) random-legal, (b) visible-only greedy
heuristic (port of current `automation.rs` scoring but restricted to fair
inputs), (c) fair POMCP, (d) omniscient beam/greedy, (e) omniscient
full-state MCTS where affordable.

- Metrics: win rate, mean HP loss, lexicographic combat objective from
  `PROJECT_OVERVIEW.md`, per-decision time, and the fair-vs-omniscient value
  gap per encounter.
- Success: fair POMCP strictly dominates (b) at a 1-second-per-decision
  budget and closes a meaningful fraction (target ≥ 50%) of the (b)→(d) gap
  on encounters where the gap is nonzero.
- Kill criteria: if fair POMCP cannot beat the visible-only greedy baseline
  at any reasonable budget, the planner or belief is broken — do not proceed
  to learned models on top of it. If instead the omniscient-vs-fair gap is
  near zero across the corpus, *deprioritize belief sophistication entirely*
  (hidden information doesn't matter enough at this content scope) and jump
  to policy/value learning with simple fair observations.

### E4: Supervised belief model calibration

Train `f_enc` + auxiliary heads on D1/D2 (no control yet): predict next-draw,
next-intent, next-turn damage taken, and combat outcome from fair history.

- Metrics: log-loss/KL vs analytic belief marginals (the irreducible floor is
  known — a unique advantage of this domain); calibration curves; held-out
  encounter generalization.
- Success: within 5% relative log-loss of the analytic floor on held-out
  roots for draw/intent heads; outcome prediction beats a no-history baseline
  by a clear margin.
- Kill criterion: if the model cannot approach the analytic floor on
  *in-distribution* combats with ample data, latent dynamics search (E5) is
  dead on arrival — stop, keep particle POMCP as runtime, and restrict
  learning to Step A priors/values.

### E5: Latent value/policy in search, then latent search

First Step A (networks inside POMCP), then Step B (pure latent search),
each vs the raw POMCP teacher at matched wall-clock on held-out roots.

- Metrics: win rate and HP loss at equal per-decision budgets (test at 50 ms,
  250 ms, 1 s); speedup at equal strength; frequency of impossible-event
  rollouts in latent search (from the D2 sanity guard).
- Success (Step A): strictly stronger than raw POMCP at every tested budget.
  Success (Step B): matches Step A at ≥ 10x lower budget on at least the
  small-budget end.
- Kill criteria: if Step A networks do not improve POMCP, the training
  targets or architecture are wrong — fix before Step B. If Step B never
  approaches Step A at any budget, or impossible-rollout rates stay high
  after calibration-guided training, abandon pure latent search as runtime
  and keep it as a research branch; the reported agent becomes Step A.

## 11. Risks, Failure Modes, Do-Not-Do-Yet

### Risks and likely failure modes

- **Simulator bugs become policy.** Learned models and search exploit
  divergences from the real game invisibly. Mitigation: the trace-parity
  discipline already in place; freeze corpus/simulator versions per
  experiment; re-run E3 ladders after fidelity fixes.
- **Leak through side channels.** Mask shape, descriptor order, error
  variance, dataset artifacts. Mitigation: suite layers 2, 6, 7; treat any
  probe success as a release blocker.
- **Over-redaction.** Hiding genuinely public information (relic counters,
  pile contents) silently caps agent strength and is not caught by leak
  tests. Mitigation: divergence-timing tests (layer 3) and the visibility
  matrix discipline — every field gets an explicit decision.
- **Particle filtering complacency.** Lazy commitment removes draw-order
  depletion, but future mechanics (scry-like effects if scope widens, more
  committed hidden state at higher ascension or other acts) can reintroduce
  it. Mitigation: E2 metrics are permanent regression gates, not one-off.
- **Latent model hallucination.** Impossible states, missed rare
  interactions, averaging over stochasticity. Mitigation: explicit chance
  factorization, D2 sanity guards, kill criteria in E4/E5.
- **Budget-shaped conclusions.** "Best agent" claims are meaningless without
  the per-decision budget; all comparisons are at declared matched budgets,
  per the evaluation-protocol concerns in `PROJECT_OVERVIEW.md`.
- **Teacher ceiling.** Distilling POMCP caps the student at the teacher.
  Mitigation: AlphaZero-style iteration (Step A improves the teacher itself)
  before any pure-distillation claim.

### Do not do yet

- Do not implement any of this before Phase 2 omniscient collection and the
  root corpus exist; the fair facade (E1) is the only piece with no
  dependency on new data and may be scheduled first.
- Do not build the full-run fair facade beyond the schema sketch here until
  combat E1–E3 are done; run-level screens multiply surface area without
  advancing the core research question.
- Do not attempt A20H fair play, Runic Dome/Frozen Eye completeness, or
  other-act content in the fair scope before the A0 Act 1 ladder is measured.
- Do not train MuZero-style dynamics end-to-end from scratch (skipping E4
  supervised calibration); the analytic belief floor is the cheapest
  falsifier this project has — use it before spending GPU-weeks.
- Do not add belief features for mechanics the simulator does not yet verify;
  no content brute-forcing to feed models.
- Do not let the fair crate grow snapshot/restore "for tests" — test through
  the omniscient side, keep the fair surface clean.
- Do not report any fair-agent result before the invariance suite and the
  adversarial probe gate pass on the exact model inputs used.
