# Run-level training investigation

> Historical investigation below: API prerequisites #78–#81 have since merged,
> and an experimental `train.py --task run` baseline is now implemented separately
> in [`../run_training/`](../run_training/README.md). The active training target is
> **A0**, not A20. Original measurements and proposed later work are preserved here;
> they are not claims about the new trainer's performance.

**Research design, not an implemented trainer or a strength claim.** Investigation
against repository base `d0a24577`, October 2, 2026. This concerns learned full-run
Slay the Spire policies, not LLM agents. The intended endpoint remains a **fair
Ironclad A20 Heart player**.

Machine-readable results: [run_level_training_evidence.json](run_level_training_evidence.json).
Small executable contracts/probes: [`../run_training/`](../run_training/).

## 1. Recommendation in one page

Build a **learned macro policy + frozen learned combat controller** first:

- Macro policy makes all meaningful map, reward, shop, rest, event, and permanent
  card-selection decisions. Combat includes potion use and combat selection panels.
- Inputs are fair public context, current screen, legal candidate descriptors, and
  explicit public history. Represent the permanent deck as a multiset, the visible
  map as a graph, and each offer/choice as an addressable token.
- Output **one logit for each complete legal candidate**, plus a separate **run
  value**. Do not use a fixed action-count head, internal IDs, or the combat HP
  value as a run-win value.
- Start with synchronous, full-episode on-policy actor-critic/REINFORCE using
  terminal binary objectives and `gamma=1`. Then add PPO only after rollout and
  recomputation equivalence tests. Sparse-success bootstrapping needs a curriculum,
  not a disguised floor-score objective.
- Begin an explicitly declared **A0 Act-1-clear curriculum**, then A0 Act-3/Heart,
  then source-and-trace-verified higher ascensions. Evaluate the endpoint separately.
- Maintain one training entry point, `rl/train.py`; promote any research prototype
  by integrating an explicit task mode and shared collection/targets, not by
  retaining another production trainer.

The neural network is **not the first blocker**. The current API can run through
many screens, but full-run training needs environment/configuration work:

1. Add fair public keys, visible boss, explicit terminal reason, and missing
   visible offer metadata to the run contract.
2. Expose ordinary initial profile configuration for Heart runs, without HP
   manipulation or reconstruction from game observations.
3. Implement trusted call-time environmental inputs for Courier; handle the
   advertised-but-unsupported Prismatic Shard surface explicitly.
4. Audit and establish higher-ascension **run** rules. Current A20 constructors
   and transitions omit important modifiers despite scaling many enemy mechanics.
5. Benchmark and strengthen the frozen combat controller on naturally reached
   starter/early-run decks. Existing synthetic-combat scores are not this metric.

## 2. What was actually investigated and tested

### Scope and evidence hierarchy

- Read the project boundary, simulator/fair-environment/Python types, action
  projection, numeric exporter, encoders, trainer, trajectory loss, synthetic
  roots, and prior design/research notes.
- Read an external full-run training implementation, not just its README:
  `Jialeiv/sts-rl-agent`, revision
  `039c199933b7d3a2e5d8fde51ae77a9175e1031f`.
- Executed **864 simulator-only interface cases**, including deliberately flawed
  initial probe-driver runs; **704 corrected-driver cases / 500,224 accepted
  actions** form the useful interface campaign.
- Executed another **384 frozen-checkpoint integration cases**. No optimizer was
  run. Used CPU FP32 inference so as not to contend for the already-active GPU.
- Replayed successful public-action journals and all eight rejected-action
  failures from initial state; verified rejected failures did not change the
  public state or revision.
- Found the reviewed corpus in the main checkout, not this worktree, and ran the
  actual strict verifier: **433/433 pass, 642,896 actions**, no divergence,
  incomplete traces, or invalid inputs.

**Critical coverage limitation:** all 433 reviewed traces use A0 and explicitly
configured 10,000 starting HP. They are real-game transition evidence for their
captured configurations, not natural-HP/A20 policy evaluation. Neither simulator
probes nor unit tests fill that coverage gap.

### Corrected interface campaigns

All seeds here are diagnostic, reusable development seeds, **not a sealed test
set**. Driver `exercise` prioritizes playing available cards, collecting rewards,
recalling keys, and progressing screens; it is not a strategic baseline. `mixed`
uses that combat driver and uniformly random noncombat legal candidates.

| Configuration | Cases | Result | Accepted actions | Maximum candidates |
|---|---:|---|---:|---:|
| Natural A0, exercise | 128 | 128 deaths | 21,181 | 35 |
| Natural A20, exercise | 128 | 128 deaths | 12,749 | 31 |
| Natural A0, uniform random | 128 | 128 deaths | 8,944 | 22 |
| Synthetic A20, HP/max HP 10,000, Heart profile, exercise | 64 | 24 modeled Heart clears, 23 Act-3-only clears, 17 deaths | 93,118 | 95 |
| Synthetic A0, HP/max HP 10,000, Heart profile, mixed | 128 | 61 Act-3-only clears, 61 deaths, 5 errors, 1 cutoff | 188,881 | 38 |
| Synthetic A20, HP/max HP 10,000, Heart profile, mixed | 128 | 31 Act-3-only clears, 94 deaths, 3 errors | 175,351 | 53 |

The synthetic clears demonstrate API traversal and terminal handling **only**.
They do not measure agent strength, full A20 fidelity, or a training result. Large
HP also changes percentage-based events, Neow outcomes, and combat behavior.

Initial `exercise` experiments reopened the merchant indefinitely. Their journals
were preserved; the **driver**, not simulator state or traces, was corrected in a
new campaign. This illustrates why UI cycles and cutoffs must be logged distinctly
from death. The corrected natural cases exposed roughly 21–36 median non-forced
macro choices before early death; the synthetic full-run exercise cases had 177
median and 215 maximum. These are workload measurements for these drivers, not
universal full-run action counts.

### Reproduced environment failures

The complete accepted prefixes are preserved under
`tmp/run-training-research/synthetic-mixed-{a0,a20}/`. Replaying them established:

- **3 Prismatic Shard purchases:** `buy_shop_relic` is in the current legal list;
  stepping fails with `decision unavailable`, atomically. The guard in
  `sts_env/src/environment.rs::decision_for` intentionally refuses ownership
  because off-color public costs are not fully represented.
- **5 colored-card purchases while owning The Courier:** `buy_shop_card` is in
  the legal list; stepping fails with `choice is invalid`, atomically. Core shop
  restocking requires typed `pending_external_rng` inputs; `FairEnvironment`
  maps the core failure to its public error, and Python provides no input service.

These are advertised candidate / supported-successor mismatches, not bad sampled
indices. Training cannot reinterpret them as game losses, retry a different
candidate, or reward avoiding an implementation error. A declared reduced-surface
experiment may use a public capability mask, but that changes the task and must
be reported. Full-game training requires completing the surface.

### Frozen combat integration

Pinned a read-only copy of the existing local checkpoint
`rl/wandb/fresh72h-20261001/latest.pt`, iteration **259,870**, SHA-256
`c12949adc98e5223ae3fc5f6a31adba0a900ade2f479f4872f806525d1c0c054`.
Architecture: width 64, two layers; trained on fresh independent A0 synthetic
combats. This is a weight-only integration experiment, **not strict training
continuation**: source configuration is BF16/CUDA, probes use CPU/FP32, and native
binary hashes differ. Inference retained the existing trainer's Smoke Bomb filter.

| Combat execution + macro driver | Cases | Act 2 reached | Final clears | Other outcomes |
|---|---:|---:|---:|---|
| Greedy combat, exercise, natural A0 | 32 | 6 | 0 | 16 deaths, 16 cutoffs |
| Greedy combat, exercise, natural A20 | 32 | 0 | 0 | 28 deaths, 4 cutoffs |
| Greedy combat, exercise, A0 Heart profile / initial HP 80 | 32 | 5 | 0 | 16 deaths, 16 cutoffs |
| Greedy combat, random macro, natural A0 | 32 | 1 | 0 | 27 deaths, 5 cutoffs |
| Sampled combat, exercise, natural A0 | 64 | 12 | 0 | 64 deaths |
| Sampled combat, exercise, natural A20 | 64 | 0 | 0 | 64 deaths |
| Sampled combat, exercise, A0 Heart profile / initial HP 80 | 64 | 7 | 0 | 64 deaths |
| Sampled combat, random macro, natural A0 | 64 | 1 | 0 | 64 deaths |

No Act-3 arrival occurred in these frozen experiments. Sampled A0 exercise mean
ending floor was 16.09, versus 12.73 with random macro; this is a small development
comparison with different induced trajectories, not a significance or learning
claim. Many deaths occurred against Act-1 bosses and elites.

Greedy cutoffs repeatedly toggled combat selection cards. **Greedy inference is
not an interchangeable substitute for this checkpoint's sampled evaluation.**
Sampled execution used one explicit per-run CPU generator and removed those
observed cutoffs, without changing the simulator or installing heuristic fixes.
A production controller still needs a declared budget and cutoff accounting.

Conclusion: A0 Act-1 clear has some positive examples with the existing combat
policy; direct A20/Heart binary-reward macro training has no demonstrated success
signal in these pilots. Macro learning and better natural-distribution combat
coverage are both necessary. Zero wins do not prove a policy can never win.

## 3. Current repository: reuse versus missing pieces

| Component | What exists | Run-level implication |
|---|---|---|
| `sts_core` | Run mechanics and state; four acts, keys, many events/relics | Supported transitions are not the same as verified full-game/A20 coverage |
| `sts_env::FairEnvironment` | Private authoritative state; atomic observation + choices; clone; revision checks | Correct ownership boundary; do not replace with raw RunState access |
| `sts_sim.State` | Typed `new`, `decision`, `step`, `clone`, numeric batches | Typed API already handles noncombat screens |
| `RunContext` | Ascension, act, floor, gold, HP/max HP, deck, owned relics/counters, potion slots | Useful macro common context; lacks explicit keys, visible boss and run goal/profile |
| Run screen types | Map/event/reward/treasure/rest/shop/grid/complete | Most macro representation plumbing already exists |
| Public actions | 46 complete kinds with local slots | Reuse exact native legal list; no new legality engine |
| Numeric transport | Actual payload version **3**, combat feature tables, all action rows, outcome headers | No macro feature tables; noncombat action rows alone are insufficient model input |
| `CombatValueModel` | Shared combat encoder, candidate scoring, scalar terminal-HP value | Good scoring pattern; action encoder rejects macro action kinds |
| `play_combats` | Numeric batches, independent clones, optional recomputation | Stops at combat outcome, not run outcome; cannot simply loop its reward logic |
| `Trajectories` | Sum decision losses / completed fights, terminal combat return | Need macro-owner records, real run returns, and truncation bootstrap contracts |
| Existing validation | Frozen synthetic A0 combats, native/data hashes | Keep as combat regression; add independent full-run seed validation/test |

Several older design/API paragraphs describe obsolete schemas or APIs. This
investigation uses current Rust/Python source: run schema **5**, combat schema **4**,
numeric payload **3**, typed public `State` with no public state JSON hydration.
Do not derive a new interface by copying old schema descriptions.

### Higher-ascension run-rule audit: a release gate

Direct public initialization probes at ascensions 0/5/6/11/14/16/20 all returned
**HP 80 / max HP 80 / three potion slots**. Source corroborates:

- `run/state.rs::ironclad_run_base` uses A0 base HP regardless of ascension;
  `potion_capacity` has no ascension branch.
- `run/reward.rs::enter_next_act_map` heals to max HP unless Mark of the Bloom,
  with no higher-ascension missing-HP healing branch.
- `map/target.rs` map configuration/generation has no ascension parameter; elite
  room rates are fixed, so higher-ascension map-density changes need investigation.
- Shop pricing source lacks an ascension price branch.
- `run/map.rs::boss_combat_monsters_for_run` spawns the selected Act-3 boss, and
  final boss Proceed goes to the Spire Heart event. There is no observed sequential
  second Act-3 boss transition. The similarly named `AscensionConfig::double_boss`
  helper elsewhere is **not proof** of the correct run flow.

Community documentation describes starting damage, fewer potion slots, lower max
HP, reduced between-act healing, shop inflation, increased elite density, and
sequential A20 bosses. It is **secondary evidence**, not authority for implementing
exact thresholds, rounding, RNG calls, or lifecycle order. Required next work is
an authoritative target-source audit plus immutable real-game higher-ascension
traces. No gameplay fixes were guessed or made in this investigation.

## 4. Input contract

### 4.1 Model boundary

A policy forward should receive:

```text
RunPolicyInput {
    feature_schema_version,
    objective: ACT1_CLEAR | ACT3_CLEAR | HEART_CLEAR,
    public_context,
    current_screen,
    explicit_public_history,
    legal_candidates: [PublicActionDescriptor ...]
}
```

Coordinator-only data: simulator handles, revisions, seed, episode ID, checkpoint
hash, native hash, rollout version, debug journals, timestamps, and environment
configuration not actually public. **Never embed these as policy features.**
Constructor seed exclusion alone is insufficient: slot/order/error/timing
non-interference and input-closure tests remain mandatory. Cloning actual state
and inspecting its futures is privileged search, not fair policy evaluation.

The included `PolicyAction` prototype strips native revisions and families while
preserving kind and visible references. This is a coding contract, not a sandbox
for hostile Python code.

### 4.2 Common context and permanent deck

Encode public scalars with both useful absolute and relative quantities:

- Current HP, max HP, HP fraction; gold and sensible bounded/log-scaled features.
- Act, floor, ascension, current declared public objective.
- Potion capacity/occupancy and individual potion identities, retaining holes.
- Owned relic identities and all exposed persistent counters, with optional-value
  presence bits. Absent combat-only counters are not fabricated zeros.
- Permanent deck **card instances**: content identity, upgrade count, bottle flag,
  relevant permanent growth values. Preserve duplicates and multiplicity.

Deck pooling must be permutation-invariant but not discard counts: use sum pooling
plus deck size, or set attention plus count/sum features. A normalized average alone
can alias a 10-card deck and its duplicated 20-card deck. Use context-conditioned
card/relic interactions; deck quality is not a sum of fixed card rankings.

Candidate references to context deck or visible grid must gather the exact local
card. Slot numbers are addresses, not learned persistent card identities.

### 4.3 Visible map and public map cache

The current map projection contains node kind, burning-elite marker, child edges,
current node, and reachable nodes. Encode the **whole currently revealed act graph**:

- Room-kind/burning-marker node features; distance from current position, graph
  connectivity, reachable descendants, and merge points.
- Bottom-up DAG message passing, or a cheap initial public lookahead feature set:
  reachable elite/rest/shop counts by distance and connectivity.
- Score the chosen node using its graph embedding, not `Embedding(node_slot)`.
- Do not fill an event node with its hidden realized room/encounter, or reveal
  shop contents and rewards ahead of entry.

Other screens currently do not include the map. Cache the last observed graph as
**explicit public history** so a card/relic/shop choice can consider upcoming
routes. Refresh on every map observation and act change; do not read `state.map`
inside the model. Public graph-derived distances are not a substitute for actual
floor metadata in runs with shortcuts.

A remembered map also needs updates from public accepted key acquisition; the
burning marker is conditional on emerald-key ownership. An old observed marker
must not be treated as fresh hidden truth.

### 4.4 Screen-specific inputs

| Screen | Required inputs and addressable entities | Important omission / trap |
|---|---|---|
| Reward | Visible card instances, gold/stolen gold, potion offers, relic offers, boss relic candidates, card-reward flow, queued overlay descriptors | Opening an unopened card reward does not entitle the policy to hidden contents |
| Shop | Visible card/relic/potion offers, full card upgrade/state metadata where displayed, price, sold flags, removal price, merchant state | Current shop card projection keeps identity/price but not the full card record |
| Rest | Available options, complete flag, deck cards for upgrade/removal, recall availability | Heal is not automatically superior to smith or saving recall for later |
| Grid | Purpose, offered cards, selected slots, confirmation/cancel candidates | Grid choices are not the entire permanent deck; toggle actions can cycle |
| Event | Event identity, current public choice labels, disclosed costs/outcomes, revealed memory-board cards, public stage history | A slot alone aliases multiple event stages; do not read private `event_data` |
| Treasure | Chest size/opened state, revealed linked rewards/key opportunity | Do not inspect unopened relic contents |
| Complete | Explicit terminal reason and continuation availability | Positive HP and kind `complete` do not necessarily terminate the run |

For event text, first preserve full visible labels and use a versioned tokenizer
or stable schema-backed public choice features. Numbers and changed labels matter.
Do not silently strip them, clamp event IDs, or guess semantic costs from private
state. A raw label parser is a representation choice, not authority for mechanics.

`TakeRelicRewardAt` can address pending/queued relic offers in core. The current
run screen only exports a singular `relic_offer` and boss choices. Audit and
expose **all actually visible** indexed offers with matching slots; never infer
unexposed identities from the legal action's index.

### 4.5 Public information to add or retain

Priority additions to the existing fair schema (with allowlist/decoder tests):

1. Public ruby/emerald/sapphire inventory and public final-act eligibility/goal.
2. Currently visible boss identity, only after the game reveals it; no premature
   second-boss identity or future encounter list.
3. Explicit run terminal reason: death, Act-3-only ending, Heart victory. Preserve
   victory-screen continuation as a separate field.
4. Complete currently visible reward/merchant card metadata and indexed relic offers.
5. Public selection cardinality constraints/semantics where displayed.
6. Consistent public run-history fields needed for memory (e.g. remembered map,
   previously revealed Match-and-Keep identities), generated from public transitions.

From a genuine initial state, accepted `take_*_key` / `rest_recall` actions can
already support a public key-history tracker. That is an interim consumer-side
solution, not permission to inspect hidden RunState or reconstruct arbitrary
mid-run inventory. Direct public inventory is cleaner and auditable.

Useful run-history features include encountered monsters, public fight damage/HP
changes, combat lengths, visible choices previously skipped, and potion use. Do
not reconstruct private encounter exclusions or reward RNG counters from core.

### 4.6 Numeric representation and batching

Start with the typed API to validate semantics. Then **extend the existing numeric
transport** with run-context and per-screen tables, preserving a single versioned
feature path, rather than keeping permanent Python-object and numeric trainers.

Proposed ragged groups:

```text
run_context; deck_cards; owned_relics; relic_counters; potion_slots
map_nodes; map_edges; map_current/reachable
reward_cards; reward_relics; reward_potions; reward_overlays
shop_cards; shop_relics; shop_potions; shop_state
event_choices; event_public_board; grid_cards; grid_selection; rest_state
action_rows + current list index/revision (transport only)
```

Use stable content-vocabulary mappings and presence bits. Existing transport
owners/offsets/revisions are not embedding features. Empty groups are valid.
Reference gathers require the correct group **and** slot; different action kinds
reuse slot field names with different meanings.

No fixed deck size, relic count, or action padding limit should silently discard
information. Bucket batches by token and candidate counts; a hard capacity needs
an explicit unsupported result. The observed maximum of 95 candidates is not a
proven global bound.

## 5. Outputs and network

### 5.1 Complete-candidate policy

For state/history embedding `z`, current candidate `a_i`, and its referenced public
objects `x_i`:

```text
q = policy_query(z)
c_i = candidate_encoder(kind(a_i), x_i, public_candidate_features)
logit_i = dot(q, c_i) / sqrt(width) + contextual_MLP(z, c_i)
pi = softmax(logits over this exact legal list)
V_run = value_head(z)
```

A dot-product-only scorer is the closest extension of current combat architecture;
the contextual MLP is an ablation, not mandatory. One candidate distribution avoids
independent kind/target/card sampling that can compose illegal actions. Do not
normalize once per action family and then uniformly choose families: that changes
the policy and its log probabilities.

Padding is masked to `-inf` before every probability/log-probability/entropy
calculation, including PPO recomputation. A real decision must have a nonempty
candidate list; zero candidates is not an all-masked categorical distribution.

### 5.2 Candidate feature resolution

- `choose_map_node` -> referenced visible map node/descendant embedding.
- `take_card_reward` -> reward card; `choose_boss_relic_reward` -> boss offer.
- `buy_shop_{card,relic,potion}` -> correct offer group + price + current resources.
- `toggle_grid_card` -> grid card + selected state + purpose, not raw context index.
- `rest_smith` / `rest_remove_card` -> context deck reference where that action uses it.
- `choose_event_option` -> current event + **current** choice label/stage-visible context.
- Key acquisition -> public keys + opportunity cost shown in current reward/rest.
- `use_potion_slot` / `discard_potion_slot` -> actual inventory slot, including holes.
- Open/close/proceed/skip/confirm -> learned kind/purpose constants plus public state.

Never reinterpret an unrecognized kind or invalid reference as a zero vector.
An unknown encoder capability is an infrastructure error, not a legal-action
sample, game defeat, or invitation to drop a candidate silently.

### 5.3 Initial model and memory

Pilot candidates, not claimed optimal hyperparameters:

- Cheap baseline: count-based permanent-deck/relic encoder, public scalars and
  graph lookahead, shared per-candidate MLP, one value head.
- Main baseline: width 128, two set-attention layers, four heads, card/relic/offer
  tokens plus a small DAG map encoder. Compare to width 64 for throughput.
- Explicit public history cache first; then a small GRU over **macro transitions**
  if event/route memory yields a measured benefit.

Do not assume recurrence restores information never delivered. Update memory
through forced actions and combat summaries even when no macro policy loss is
recorded. For recurrent PPO, store sequence boundaries, reset masks and burn-in;
recompute contiguous histories under current weights instead of shuffling isolated
steps with stale hidden states.

The combat checkpoint's card vocabulary/features are useful initialization.
Load shared weights only into compatible modules, initialize macro heads and
**run value** separately, and leave the frozen combat copy immutable. Sharing a
trainable encoder with the nominally frozen combat head would move the tactical
policy and break stationarity.

Auxiliary heads may predict public future outcomes from observation/action:
next-fight death/HP distribution, act-clear probability, resource usage. Targets
may be realized rollout outcomes; future outcomes must not be policy inputs.
Avoid privileged critics and hidden-state auxiliaries in the baseline.

## 6. Rollout and episode contract

### Ownership

Route by **current screen**, not action family: combat potion and selection
choices have `family='run'` but belong to the combat controller.

```text
coordinator owns simulator handles and revisions
    |
    +-- recognized training terminal -> outcome, stop/reset
    +-- no actions / unexpected settlement -> infrastructure failure
    +-- exactly one action -> execute forced transition, update history
    +-- combat screen -> frozen combat policy, update history/summaries
    +-- otherwise -> learned macro policy, record policy transition
```

A forced Proceed may move from `complete` Victory UI into the Act-3 Spire Heart
event or Act-4 continuation. Check continuation and the declared objective.
Death can be a training terminal before its optional UI Proceed. A final Act-3
ending without keys has reward **0 for Heart**, even with positive HP.

The prototype recognizes these current modeled cases from public phase/HP/act
and candidates; production should use the explicit public terminal-reason field.

### Macro transition

From one sampled macro choice to the next macro decision (or genuine terminal),
execute its native action plus any subsequent forced/combat decisions. Store:

```text
public observation/history; revision-free candidate features; chosen row
behavior log-probability; behavior value; macro/combat policy versions
reward; next public value input; terminated; collector boundary
primitive duration; declared discount; trace decay; controller ownership
```

No autograd graphs spanning an entire game. Store public numeric inputs under
inference mode and recompute chosen log probabilities/value predictions during
learning. Keep per-run generators, histories and accepted prefixes distinct even
when active rows shrink or refill.

### Collector pseudocode

```python
freeze_behavior_macro_weights()
freeze_combat_checkpoint()
while batch_not_ready:
    for each live run:
        read atomic decision
        if terminal_for_declared_task:
            settle pending transition with terminal reward; reset
        elif no_candidates:
            journal infrastructure failure; discard affected learning unit
        elif forced:
            execute; update public history
        elif combat:
            enqueue fair numeric combat row
        else:
            settle previous macro transition at this public decision
            enqueue macro observation + legal descriptors
    run batched inference by controller and width bucket
    sample using each run's explicit generator
    apply native actions once with matching revisions
    journal attempts and accepted transitions
finish trajectories or mark nonterminal collection boundaries
compute returns/advantages from this behavior version
recompute public inputs -> losses -> one or bounded PPO updates
validate gradients; checkpoint; evaluate separately
```

Do not pass State into the model or mutate a naturally reached run to normalize
HP. A naturally reached combat clone is a legitimate future training root if
constructed from initial state/actions; it must not be confused with the current
synthetic `spec_json` constructor, which cannot resume an arbitrary full run.

### Failures and cutoffs

Separate `death`, `success`, `objective_failure`, `collector_cutoff`,
`simulator_error`, and `infrastructure_error`.

- Bootstrap ordinary nonterminal collection cutoffs; cut the trace at the boundary.
- A simulator failure does **not** become return zero or a value-bootstrap label.
  Retain prefix/debug evidence and report unavailable coverage. If continuing is
  allowed, do not reuse contaminated transitions as if sampled normally.
- Native numeric batch stepping is **not atomic across states**. Current safe
  behavior is discard the whole affected batch; accepted states are never retried.
- Hard run/decision budgets protect against UI cycles and possible infinite fights.
  Reaching a budget is not a game defeat. Do not mask toggles after observing a
  loop unless defining a separate declared policy restriction.
- On-policy batching must avoid collecting only the first/fastest completed runs;
  that induces completion-time selection bias. Finish assigned episodes or use
  fixed fragments with correct nonterminal bootstrap/versioning.

The probe's case timer is **cooperative between calls**, not a subprocess hard
watchdog. Production needs per-worker/process supervision for stuck native calls.

## 7. Objective, targets, and optimizer

### True objective

For final training:

```text
J = P(fair Ironclad A20 Heart clear | declared seed distribution and budget)
r_terminal = 1 for Heart victory; 0 for legitimate game failure
r_intermediate = 0
```

A0 Act-1, A0 Act-3, and A0 Heart objectives are separate public task labels and
curriculum metrics, not endpoint wins. Natural full-run simulation supplies the
correlated deck/relic/HP distribution missing from independent loadout marginals.

No primary rewards for HP preservation, number of cards, gold, floor, or elite
count. Those resources have context-dependent exchange rates. A combat HP-optimal
policy may waste a potion today that would save the run tomorrow; conversely it
may refuse useful damage trades for permanent growth. Eventually introduce public
run context/objective into combat inputs and fine-tune deliberately.

### Discount clock

Use **`gamma=1`** initially for finite episodic win probability. The SMDP/option
view is useful because frozen combat takes variable primitive duration, but it
**does not require discounting combat duration** when the actual objective is
undiscounted success.

If deliberately choosing a discounted objective, specify the clock:

- Primitive action clock: `d_k = gamma ** duration_k`; rewards inside the interval
  must be aggregated with the same clock.
- Macro decision clock: `d_k = gamma` per sampled macro transition.
- Undiscounted success: `d_k = 1` between decisions, `0` at genuine terminals.

For example, `.99 ** 1400` is approximately `7.7e-7`, while `.99 ** 177` is about
`.169`. Blindly copying primitive-step gamma can practically erase early decisions'
terminal signal and penalize harmless UI interactions/long combat sequences.
Wall time is a reported compute constraint, not a hidden-state-dependent reward.

### Monte Carlo baseline first

For completed runs and `gamma=1`, every macro action gets `G_t = terminal outcome`:

```text
A_t = G_t - V_run(public_history_t)
L_policy = -mean_over_runs(sum_t log_pi(a_t) * stopgrad(A_t))
L_value = mean_over_runs(sum_t (V_run_t - G_t)^2)
L_entropy = -coef * mean_over_runs(sum_t entropy(pi_t))
```

A critic baseline reduces variance without changing the target. Mean over runs of
summed policy gradients matches the episodic score-function estimator; dividing
each run by its own decision count changes the objective weighting. Uniform
transition averaging is a practical batch-wide rescaling but not per-run/per-screen
rebalance. Make value/entropy normalization explicit so UI verbosity does not
silently dominate regularization.

Forced one-candidate actions contribute zero policy gradient/entropy; still
advance history and environmental transitions. They may supply value supervision.
Losses should not pretend a run with no sampled macro actions has policy decisions.

### GAE and bootstrap

For arbitrary declared transition discount `d_t`, trace decay `l_t`, and a
nonterminal successor value:

```text
delta_t = r_t + d_t * V_next - V_t
A_t = delta_t + d_t * l_t * A_next    # only within this contiguous trajectory
value_target_t = A_t + V_t
```

- True terminal: zero successor value/discount and stop recursion.
- Collector fragment boundary: **retain successor bootstrap**, stop recursion
  into another fragment/reset. No treating time limits as deaths.
- Run reset: never bootstrap from the next run's starting observation.
- Lambda may be macro-clock or duration-clock; declare it independently of gamma.

Start with completed-episode Monte Carlo / `lambda=1`. An uncalibrated critic and
`.95 ** 176` attenuation can leave very early choices with little terminal signal;
then compare `.97–1.0` GAE after value calibration. There is no universal best lambda.
The included `targets.py` exercises terminal masks, cutoffs, reset separation,
variable discount, finite checks, and lambda-one Monte Carlo equivalence.

### PPO second

Store behavior log probability/value and a fixed behavior version. Recompute the
**same complete legal-candidate distribution**, not a newly regenerated candidate
list against a stepped state:

```text
ratio = exp(new_logp - behavior_logp)
L_clip = -mean(min(ratio*A, clip(ratio, 1-eps, 1+eps)*A))
```

Pilot `eps=.1–.2`, 2–4 epochs, modest learning rate, gradient clipping, and logged
KL/clip fraction. These are tunable defaults, not research results. Monitor value
calibration, entropy by screen, and action-confirmation behavior. Advantage
normalization or per-screen sampling needs an explicit weighting protocol.

Do not recompute old REINFORCE data after changing weights and call it on-policy.
Do not asynchronously mix frozen-combat versions in one return/value objective.
If actor throughput later requires stale policies, use declared policy-lag bounds
and a principled off-policy algorithm such as IMPALA/V-trace, not accidental APPO.

## 8. Sparse-success curriculum and data

### Why direct terminal Heart reward is not yet enough

The frozen sampled pilots found **no full-run successes**, and no A20 Act-1 clears.
With all outcomes zero, a well-fit zero critic leaves no useful outcome advantage;
entropy can continue exploration but does not create successful examples. A
random nonzero critic can produce noisy gradients, not a success signal.

Recommended stages:

1. **A0 Act-1-clear**, sampled frozen combat, native natural HP, declared macro
   policy. The pilot has 12/64 Act-2 arrivals with exercise, so positive outcomes
   are available, though this is not proof a learned macro policy succeeds.
2. A0 Act-3 clear, then A0 Heart with legitimate key acquisition and initial profile.
3. Once higher-ascension run rules are verified, progressively add difficulty with
   objective/difficulty-conditioned inputs and retain final-task evaluation.
4. Harvest naturally reached combat roots to adapt tactical training to early
   decks, curses, small max HP, realistic potion scarcity, relic histories, and
   high-ascension fights. Keep synthetic-combat validation separate.
5. Alternate controlled combat improvement phases with macro improvement; freeze
   one combat version per macro phase and evaluate matched pairs.

Advance on preregistered success/coverage criteria, not just optimizer count or a
pretty floor curve. Higher-HP synthetic runs can test plumbing, not graduate the
curriculum. Cloning accepted simulator prefixes is allowed; synthetic rehydration
of historical decks/maps or observed states is not.

### Data and warm starts

The existing SlayTheData fit supplies **A0 final-deck marginals**, not complete
state-action demonstrations. Its independent card/relic draws intentionally lose
synergy/path/resource correlations; final potion acquisitions are not inventory.
It cannot directly teach which offered card was declined, what shop prices were,
or what alternatives were visible at each decision.

For behavior cloning, require true public decision records with complete visible
candidate sets and selected actions. Run summaries are not enough. All teacher
sources need fairness/protocol labels; privileged beam/MCTS trajectories are not
fair action labels without a declared teacher/student research experiment.

Checkpoint initialization is weight-only transfer with a new optimizer/RNG when
changing objective, schema, native gameplay, controller version, or curriculum.
Do not restore the combat HP-value optimizer into the run-win critic.

### Shaping and advanced alternatives

Potential-based shaping uses `F = d_t*Phi(next)-Phi(current)` with the same clock
and zero potential on every true terminal; it telescopes to an initial-state
constant. It can help bootstrapped local learning with a good potential, but is
**not automatically an additional full-episode REINFORCE success signal**.
Uncancelled terminal HP/floor/deck-quality potentials change the objective.

Floor score, ranked progress rewards, or hindsight "reached floor X" goals can be
explicit curricula/auxiliary tasks, not silently relabeled Heart successes.
Return decomposition (RUDDER) is a later credit-assignment experiment once diverse
successful trajectories exist; prediction uncertainty and policy shifts need
measurement. Learned world models/search are not prerequisites for the first macro
policy and need separate hidden-state/belief audits if introduced.

## 9. Throughput and operational design

The workstation has 24 logical CPUs and an RTX 5080 with about 16 GiB VRAM; a
pre-existing GPU job was active. All new checkpoint probes were inference-only on
CPU. No long optimizer job or game collection was started.

Diagnostic serial interface campaigns, including journals, took approximately
5 s / 21k actions for natural A0 exercise, 42 s / 93k actions for synthetic A20
exercise, and 51–53 s / 175–189k actions for mixed full-run synthetic cases.
These are **not learner throughput benchmarks**: workloads vary radically, and
journal I/O, card/deck growth, cheap UI cycles and inference costs differ. Frozen
sampled 64-run A0 integration took about 14 s with one-thread CPU inference.

Before selecting an overnight learning budget, measure:

- completed runs/hour and **useful sampled macro decisions/s**;
- natural-run reachability by act and fraction of terminal successes;
- native transition, observation export, tokenization/transfer, combat inference,
  macro inference, recomputation/backward, evaluation and journal time separately;
- peak retained trajectory bytes, padded token/action distributions and fragment
  lengths, not just transitions/s;
- failure/cutoff/unavailable fractions and completed-run latency percentiles.

Parallel environment workers should own disjoint runs. Batch combat observations
from all workers at the shared inference server, and batch macro observations
separately; do not block a whole group until its slowest fight ends. Avoid
`workers * native_threads * torch_threads` oversubscription. Start conservatively
and measure. Existing numeric stepping uses native threads and non-atomic batches;
coordinator ownership/error handling must match that contract.

For an eventual unattended pilot: fresh output directory, fixed hashes and budgets,
periodic atomic checkpoints, disk/worker supervision, no retries of partially
advanced batches, and a terminal report distinguishing completed, cut off, and
unavailable runs. Exact resume must preserve each run's simulator progression,
public history, controller versions and per-run RNGs; current public Python clone
is in-memory, not a persisted full-run resume API. Alternatively checkpoint only
at a drained rollout/update boundary and document lost in-flight work on restart.

## 10. Evaluation and acceptance gates

### Report real objectives

Use three disjoint seed roles: training, repeatedly inspected validation, sealed
final test. Split by canonical numeric game seed so aliases like case variants or
`O`/`0` cannot cross partitions. Exclude synthetic combat validation seeds from
any training use where the contract requires it. Pin public profile, ascension,
initial HP rules, native/content/feature hashes, both controller hashes, sampling
protocol, and compute/decision budgets.

Full-run primary metrics:

- Heart clears / attempted seeds, with confidence interval and unavailable count.
- Act reach and Act-3-only endings; key acquisition and failure-to-enter-Act-4 rate.
- Death location/encounter, HP/potion resource trajectories, combats escaped.
- Macro action distribution/entropy, shop spend/removal choices, card skip/upgrades,
  confirmation/UI-cycle behavior, decision/compute costs.

Keep a raw observed success/attempts count and uncertainty bounds when outcomes
are unavailable; do not report only wins among completed runs. For `w` wins and
`u` unavailable out of `N`, empirical completed outcomes permit an attempted-set
win interval `[w/N, (w+u)/N]` before sampling uncertainty, not a made-up resolution
of the missing outcomes. Explain how a declared timeout budget is scored without
using that evaluation convention as a death training target.

Compare macro policies with the **same combat checkpoint, sampling and budget**;
compare combat policies under the same macro policy. Repeat stochastic policies
per seed using independent public-policy generators. Paired seeds reduce some
variance but paths/RNG consumption diverge legitimately after different actions.
Test runs must not influence curriculum, checkpoint selection, seed-specific policy
memory, or simulator behavior.

Synthetic-combat win/HP, privileged beam references and simulator floor curves
stay labeled diagnostic. Final fair strength requires real-game natural-HP A20
Heart validation with declared seed/compute budgets and no observed-state syncing.

### Concrete gates

1. **Environment gate:** supported legal candidate implies a supported successor;
   complete profile/external-input contract; correct terminal discrimination;
   immutable trace regression and higher-ascension coverage.
2. **Input gate:** every noncombat screen/action kind represented, unresolved
   references rejected, no hidden fields/transport metadata in model inputs.
3. **Learning gate:** finite masked losses; recomputation equals inference logits
   under a fixed protocol; successful toy/curriculum learning and calibrated values.
4. **Rollout gate:** selection/shop cycles, death Proceed, Act-3 continuation,
   queued rewards, grids, escapes, keys, and batch failures covered end to end.
5. **Reproducibility gate:** checkpoint/config/native/data/controller hashes,
   seed separation, RNG independence, deterministic accepted-prefix replay.
6. **Strength gate:** genuine natural-HP final-task evaluation; no promotion based
   on high-HP synthetic clears or on A0 corpus passage alone.

Test permutation equivariance of deck/offer tokens with correspondingly renamed
candidate references; preserve hand/potion/visible grid address semantics. Repeat
hidden-order/RNG/internal-ID non-interference tests over run screens, and test
that public keys/boss/offer costs/history changes do affect outputs.

## 11. Implementation order and decisions not to make yet

### Phase A: environment contracts

Modify `sts_env` observations/allowlists, typed Python decoders and bindings to
support public strategic fields and ordinary profile initialization. Add core
external-input integration with source-backed provenance, not hidden fallback RNG.
Audit higher-ascension run rules against target source and real traces before
calling the task A20. Keep dependency `rl -> simulator` only.

### Phase B: transport and routing

Extend the existing numeric exporter/encoder groups; add a tested run collector
behind `train.py --task ...`. Keep current combat path and checkpoints working.
Use typed-vs-numeric equivalence tests for every new table and reference. Add
controller ownership, terminal/cutoff handling, candidate/history storage, and
recomputation before adding a sophisticated model.

### Phase C: smallest learning experiment

One macro candidate scorer and one **new** run-value head; frozen sampled combat;
A0 Act-1 binary objective; completed-episode Monte Carlo first. Start with a small
batch/model and log the actual positive-return/coverage rate. Compare count-MLP
against set/graph encoder under matched rollout budgets.

No claim that the provided prototype already implements this learner. It implements
small contracts/targets and provides empirical integration evidence only.

### Phase D: longer horizons

Add correct nonterminal fragment bootstrapping, optionally PPO/GAE, then A0 Act-3
and Heart. Adapt combat using natural roots; only then train the source-and-trace
supported higher ascensions. Consider recurrent history or return decomposition
only when specific failures justify them.

Do **not** decide a giant unified model, learned simulator, omniscient search
teacher, or distributed off-policy learner is required before these gates. Do not
start an unattended full-run optimizer job while Courier/Prismatic support and
terminal semantics remain unresolved.

## 12. Literature and external implementation findings

Sources are evidence for algorithm structure or external reported results, not
authorities for this simulator's game mechanics.

### Closest concrete full-run implementation

[Jialeiv/sts-rl-agent](https://github.com/Jialeiv/sts-rl-agent) uses a small shared
candidate scorer with MCTS combat. Inspected code:

- `agent/armG_train.py`: state vector concatenated with candidate descriptor;
  map room lookahead, card choices, rest/shop/event descriptors; complete-candidate
  softmax; combat remains search.
- `agent/armG_train_parallel.py`: spawned CPU actors, synchronous weight snapshots,
  graph-free trajectories, recomputed summed log probabilities.
- **Actual training reward is final floor / 50**, not binary run victory.
  Moving-average baseline; no learned run-value head in that training script.
- Documentation reports 14% A0 wins at 50k MCTS simulations and 4% at 2k on 50
  repeatedly inspected seeds. Those are author-reported, not independently
  reproduced here; validation, not untouched final-test evidence.
- Steam integration imports exact RNG/BattleContext for combat. This is
  **privileged exact-state search**, incompatible with our fair-policy boundary
  and no-hydration replay rules.
- The README explicitly says its published table predates a subsequent combat
  rules patch and was not remeasured.

Useful lessons: macro/combat decomposition, candidate scoring, snapshot actors,
recompute rather than keep full-run graphs, and honest evaluation roles. Do not
inherit the floor objective, opaque NNInterface fairness assumption, exact-state
hydration, silent cutoff handling, or headline numbers as this project's target.
The author's negative combat distillation results are evidence about those
experiments, not proof that fair learned combat is impossible.

### Algorithm reading list

| Source | Why it matters here | Limit / practical decision |
|---|---|---|
| Sutton, Precup & Singh, *Between MDPs and semi-MDPs* (1999), [DOI](https://doi.org/10.1016/S0004-3702(99)00052-1) | Frozen combat is temporal abstraction | Duration discount only if the chosen objective/clock uses it |
| Schulman et al., [GAE](https://arxiv.org/abs/1506.02438) | Macro credit/variance and cutoff bootstrap | A bad critic + short lambda can suppress distant success signal |
| Schulman et al., [PPO](https://arxiv.org/abs/1707.06347) | Reuse public rollouts with bounded policy updates | Exact behavior candidate distribution/version must be retained |
| Espeholt et al., [IMPALA](https://proceedings.mlr.press/v80/espeholt18a.html) | Actor/learner throughput and V-trace | Later scalability option, not permission for stale on-policy gradients |
| Huang & Ontañón, [invalid action masking](https://arxiv.org/abs/2006.14171) | Normalize over actual legal candidates, not illegal-action penalties | Mask cannot substitute for missing supported mechanics |
| Zaheer et al., [Deep Sets](https://arxiv.org/abs/1703.06114) | Permutation-invariant deck/relic encoding with multiplicity | Preserve count; sum-pooling is expressive, not inherently synergy-blind |
| Lee et al., [Set Transformer](https://proceedings.mlr.press/v97/lee19d.html) | Card/relic/offer interactions | Quadratic attention cost needs token-width measurement |
| Ng, Harada & Russell, [reward-shaping paper](https://people.eecs.berkeley.edu/~russell/papers/icml99-shaping.pdf) | Distinguish potential shaping from changed objectives | Terminal potential and discount clock must make the cancellation valid |
| Arjona-Medina et al., [RUDDER](https://arxiv.org/abs/1806.07857) | Long-delayed credit and return decomposition | Additional sequence learning/calibration; not the first sparse-success fix |

A web search also suggested early Slay the Spire RL blog experiments and several
LLM projects. The Kai Brewer-Krebs primary blog URL did not resolve from this
machine. Unverified search-generated dates, paper placeholders and performance
numbers were **not adopted as evidence**. This is not a proof of exhaustive
coverage of all published STS agents.

## 13. Deliverables and verification

Committed-source candidates from this investigation:

- `rl/docs/run_level_training.md`: this design and findings.
- `rl/docs/run_level_training_evidence.json`: compact campaign/config/hash record.
- `rl/run_training/contracts.py`: revision-free descriptors, controller routing,
  modeled terminal/objective discrimination.
- `rl/run_training/targets.py`: reference macro GAE/bootstrap calculations.
- `rl/run_training/probe.py`: append-only interface campaigns, no learning.
- `rl/run_training/replay.py`: native-hash-checked initial-state/action replay of
  these research journals, including atomic rejected-action checks.
- `rl/tests/test_run_training_{contracts,replay}.py`: 29 infrastructure tests.

Large journals, pinned local checkpoint copy, downloaded upstream source and the
one-off frozen integration script are under ignored
`tmp/run-training-research/`. They are local research artifacts, **not captured
real-game traces** and not permanent-corpus additions. Summary hashes and exact
paths are retained in the compact evidence file. No source mechanics, captured
traces, active training checkpoint or main-checkout state was modified.

Checks run:

- `cargo fmt --all -- --check`: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo test --workspace -- --test-threads=1`: pass.
- Strict corpus replay against the main checkout's absolute corpus path: 433 pass.
  The first attempt against the absent worktree-relative corpus failed with
  `INVALID ... is not a file or directory`; the valid directory was then found
  and checked, not replaced by fixtures.
- Python binding suite: 26 tests pass.
- Full RL suite after fixing a new test's malformed potion capacity: 161 tests
  pass, one existing optional browser test skipped.
- Existing documented RL type-check command: pass.
- Targeted prototype lint/format/type checks and 29 new tests: pass. An expanded
  type check caught an optional native module file path; an explicit guard was
  added and the full existing-plus-prototype check then passed.

Remaining limits: no new real-game collection, no authoritative higher-ascension
source bundle locally, no learned macro-policy optimization, no sealed endpoint
strength evaluation, and no persistent full-run resume implementation. These are
explicit next-work gates, not claims that an A20 Heart trainer is ready.
