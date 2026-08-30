# Combat RL Architecture

Status: design contract; the initial fair tensorizer, tiny policy/value model, and naive synchronous privileged PUCT are implemented.
Last updated: 2026-08-30.

The first learned combat agent uses AlphaZero-style Expert Iteration over
collected combat roots. Its policy/value network receives only fair public
information. During the bootstrap phase, tree search is privileged: it follows
the one true authoritative simulator state, including hidden state and exact
RNG. The network is therefore reusable when privileged singleton search is
later replaced by particle-belief search.

See [`fair_combat_api_design.md`](fair_combat_api_design.md) for the symbolic
Rust boundary and [`combat_search_benchmark_2026-07.md`](combat_search_benchmark_2026-07.md)
for the July 2026 fixed-budget planner conclusions. This document specifies the
model and training direction. Tensor code remains an optional Python RL layer;
it does not become simulator mechanics or part of the durable symbolic API.

[`combat_agent_phase2_plan.md`](combat_agent_phase2_plan.md) is the active
execution plan against this design: it records the measured baseline, settles
where search and tensorization run, and stages the remaining work behind gates.

## Terminology

- **Fair network:** policy and value inputs are `FairCombatObservation` plus
  public history or summaries derived only from it.
- **Privileged search:** search transitions one true full simulator state. This
  is a teacher/planning tool, not a fair deployable agent.
- **Fair search:** search acts over a belief or public history and cannot select
  different root actions merely because hidden states differ.

Calling the first learned agent "omniscient" refers only to its search tree.
Raw hidden fields are not neural inputs.

## Training Loop

Combat roots are starting states, not the entire training set. Legally generated
seeded simulator runs are the scalable training source; immutable real-trace
roots remain fixed distribution anchors for development, sealed evaluation, and
audit. Root generation must advance only through accepted legal transitions and
must never randomize raw state fields independently.

```text
sample training combat root
  -> run neural-guided search
  -> choose an action from root visits
  -> continue to combat termination
  -> store every visited (fair input, legal choices, search policy, outcome)
  -> train policy/value network
  -> repeat with the improved network
```

For network parameters `theta`:

```text
(p_theta, v_theta) = network(fair_information_state, public_choices)
pi                  = improved root policy from search visits
z                   = terminal proxy utility
```

The policy trains toward `pi`; the scalar value trains toward `z`. Combat has
one decision-maker, so value backups never alternate or negate by tree depth.

Split combat roots by source run/seed before generating examples. Descendant
decisions from one combat must not cross train/development/test boundaries.
Keep a sealed test split and a real-trace audit split in addition to the
development split used for model and search tuning.

## Beam-Search Bootstrap

The existing beam planner is the initial expert:

1. Search a training root with a versioned deterministic transition budget.
2. Execute its first selected public choice and replan, at least at turn
   boundaries; a cheaper initial corpus may also record validated plan suffixes.
3. Store the fair input, legal public choices, selected teacher action, terminal
   outcome vector, planner version, and budget.
4. Pretrain the policy by behavior cloning and the value/auxiliary heads from
   completed outcomes.
5. Initialize PUCT with that network, then replace beam one-hot targets with
   MCTS visit distributions.

The implemented trainer remains supervised imitation. Beam cloning writes
Record V2 / manifest V5 one-hot labels and terminal `combat_proxy_v1` values.
Privileged PUCT distillation writes Record V3 / manifest V6 labels: raw root
visit counts as the policy target and the backed-up root-mean as
`privileged_puct_root_mean_v1`, still bound to the terminal reward contract used
inside search. Optional offline W&B tracking records scalar `step`/`loss` plus
symbolic config and provenance digests; it is opt-in, never uploads, and does
not change checkpoint bytes. Each offline `--resume` is a separate W&B run
segment. Source or lockfile changes intentionally invalidate source-bound
checkpoints. `target/wandb` is removed by `cargo clean`.

A naive privileged PUCT now exists: it expands public `PlayerChoice` rows over
authoritative `RunState` clones, evaluates one fair leaf at a time, and selects
the root action by visit count. Search requires a finite positive `c_puct` and
positive simulation and transition budgets, always stops at the first exhausted
bound, uses `sqrt(parent_visits+1)` exploration, reports the root-mean backup
value, and scores terminals against the public episode-root max HP/gold
baseline. Revisiting an expanded terminal is standard MCTS backup and can
overweight short terminal lines in the visit target. The leaf callback is
synchronous and GIL-blocking; `fair_leaf_batch_v1` is batch-size 1 and not an
extensible request protocol. It does not yet batch leaves, apply virtual loss,
or share transpositions. `sts-combat-data puct-label` and
`sts-combat-puct-rollout` consume that teacher. Equal per-decision transition
budgets for beam and PUCT do not imply equal compute; PUCT also has a separate
simulation budget. This teacher is privileged, not a fair deployable agent.

## State Encoder

Do not use a sparse vector indexed by every possible card, relic, potion, or
monster. Encode the entities actually present as dense tokens:

```text
[STATE] [PLAYER]
[PILE summaries] [one token per visible card record]
[one token per monster]
[one token per relic]
[one token per potion/empty slot]
[PUBLIC HISTORY summary]
```

The initial implementation pools visible powers and counters onto their owner
entity as deterministic vocabulary-indexed aggregate value, presence-mask, and
occurrence-count features. This deliberately preserves owner association and
aggregate duplicate/OOV statistics without adding separate tokens whose owner
references must be maintained. Exact power/counter vocabulary identity is
checkpoint-owned; changing it changes the model input width and therefore
requires a new checkpoint. Separate power/counter tokens remain a valid future
experiment, not the current tensor contract.

A small Transformer or equivalent permutation-aware set encoder contextualizes
the tokens. The `[STATE]` output feeds the value head; contextual entity outputs
feed the action scorer.

### Card tokens

Start with a learned content embedding plus visible structured features:

```text
content/family embedding
card type and target type
upgrade level
visible current cost
zone/pile
publicly known draw rank, if any
visible instance counters and keyword flags
```

Embeddings begin random and learn end-to-end from beam/MCTS policy targets and
value targets. Structured fields prevent the model from spending data to
rediscover basic facts such as cost, card type, Exhaust, Ethereal, Retain, or
X-cost. Do not encode English text or maintain a second handwritten mechanics
database; static features come from authoritative content definitions and the
fair projection.

Hand, discard, exhaust, and ordinary draw contents are unordered collections
for model semantics. Do not add generic sequence positions. Add positions only
for publicly known draw order or event history. Include pile sizes and sum
pooling so duplicate-card multiplicity is not erased by normalized attention.

### Other entity tokens

- Player: HP/max HP, block, energy, turn, and pooled public powers/counters.
- Monster: identity, HP/max HP, block, targetability, visible intent, public
  status, and pooled public powers/counters. Hidden intent is represented as
  unknown.
- Relic: identity and explicitly public counter/state only, pooled on the relic
  token.
- Card: visible instance counters are pooled on the card token.
- Potion: identity and visible slot; represent empty capacity as well.

Public slots are references used by choices, not semantic positions. Permuting
an unordered entity collection and remapping its choice references must leave
the value unchanged and permute the corresponding policy logits.

## Dynamic Legal-Choice Scoring

The policy does not need a global output neuron for every possible action.
Encode and score only the current public choices:

```text
h_state  = state_encoder(observation)
h_action = action_encoder(choice, contextual entity references)
logit(a) = score(h_state, h_action)
P(a|s)   = softmax over current choices
```

Examples:

- `PlayHandSlot`: state token + referenced card token + optional monster token
  + action-kind embedding.
- `UsePotionSlot`: state token + potion token + optional monster token + kind.
- `EndTurn`: state token + action-kind embedding + learned no-source/no-target
  embeddings.

For batching, either pad to the largest number of choices in a batch and mask
padding, or pack all choices with a choice-to-state segment index. Search caches
the resulting prior by serialized `PlayerChoice`.

## Value and Temporary Combat Objective

PUCT consumes one scalar value. Initially it predicts a versioned handcrafted
terminal proxy in which survival dominates every resource preference. Within a
win, the proxy may combine terminal HP, max HP, gold, and the exact remaining
potion inventory. Exact weights are experiment configuration, not a permanent
game rule.

Store the full outcome vector even when search uses one scalar:

```text
won / lost / escaped
terminal HP and max HP
max-HP and gold changes
remaining potion identities and slots
relic/card counters changed by combat
terminal / truncated status
```

Auxiliary heads may predict win probability, terminal HP, max-HP change, gold
change, and potion inventory value. These improve diagnostics and preserve
information for the future evaluator; only the configured scalar head/value is
backed up by initial PUCT.

There is no hard potion budget in the learned-agent architecture. Potion use is
an ordinary legal choice. SlayTheData potion metadata may constrain guided
trace collection, but it is not an RL policy input or simulator legality rule.

## Run-Level Value Handoff

The handcrafted combat utility is temporary. The final objective is A20 Heart
run win rate. Once a calibrated run-level evaluator exists:

```text
terminal combat line
  -> complete post-combat RunState
  -> V_run(post-combat state)
  -> value backed up through combat search
```

The combat value then approximates expected downstream run value, not merely
HP preservation. The run-level network evaluates resulting potion inventory;
it does not issue a pre-combat potion permission. Counterfactual post-combat
states must be included in run-value training/evaluation to control
out-of-distribution errors.

## Transition to Fair Particle Search

The privileged phase searches one true hidden root while its network sees fair
inputs. Hidden-equivalent roots can produce conflicting search targets; the
network learns their average, while privileged search resolves each using the
true state. This is expected and must be measured.

The later fair planner changes:

```text
one true hidden root
        -> belief over hidden roots consistent with public history

state-keyed privileged tree
        -> public action-observation-history tree
```

The fair observation encoder, public choice encoder, policy/value heads,
training record format, batching, and most PUCT infrastructure remain reusable.
The fair planner must aggregate decisions across particles; independently
optimizing each hidden state and averaging actions would leak information
through strategy fusion.

## Implemented Initial Tensor Decisions and Deferred Experiments

The initial optional RL tensor layer now specifies checkpoint-owned
vocabularies, normalization and missing-value encoding, dynamic batch padding,
and action references. It deliberately does not persist a tensor schema version.
The stacked model layer owns the tiny Transformer configuration and tensorizes
durable symbolic records on demand.

Public-history representation, alternative power/counter tokenization,
Transformer/pooling experiments, scalar utility objectives, and auxiliary loss
weights remain experiment decisions. None of these choices belongs in simulator
mechanics; the symbolic fair API remains the durable prerequisite.
