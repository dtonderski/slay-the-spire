# Simulator Combat Explorer — Implementation Handoff

## 1. Purpose and authority

Implement a local, simulator-only combat exploration UI for inspecting trained policies and experimenting with alternative moves.

This document records the user-approved product direction and supplies concrete engineering recommendations. Requirements marked **required** reflect the agreed scope. Specific framework choices, endpoint names, layouts, and file names below are recommendations, not claims that these facilities already exist.

Read `AGENTS.md` and `PROJECT_OVERVIEW.md` before implementation. Preserve the fair-state boundary and the dependency direction `rl -> simulator`. Follow project instructions for validation and report limitations honestly.

The implementing agent should perform the work directly unless the operator separately authorizes delegation. If delegation is authorized, follow the project's non-Astra child-model policy.

## 2. Product summary

The central workflow is:

1. Load a saved synthetic validation root or generate a new synthetic combat root.
2. Inspect the initialized combat.
3. Either make a legal move manually or ask a trained model to finish the combat.
4. Inspect the generated history by stepping between player decisions.
5. Return to any earlier decision and make a different move.
6. Let the model finish from the new position, or continue manually.
7. Retain all alternatives as a navigable tree.
8. Save the tree locally and reopen it later.

The intended outcome is a practical tool for understanding model strengths, weaknesses, and action preferences. It is not a game recreation, a training dashboard, or a real-game trace viewer.

### Required scope

- Saved synthetic validation roots and newly generated roots.
- Optional model checkpoint: manual exploration works without a model.
- Manual legal actions, including targets and nested selection decisions.
- Explicit model continuation to combat completion, bounded by a decision limit.
- Convenient single-model-decision execution as well.
- Greedy selection and sampling with a temperature control.
- Action probabilities at decision states.
- Immutable branching history and arbitrary node selection.
- One history step per player decision, plus terminal states.
- Readable state presentation and summaries of changes between decisions.
- Prominent turn-end and enemy-kill navigation markers, with honest handling of uncertain kill detection.
- Basic local session save/load.

### Explicitly excluded

- Real-game integration, CommunicationMod, sockets to a running game, capture tools, or real-game trace playback.
- Automatic playback, timers that advance the displayed state, play/pause, or playback speed.
- Full-run navigation through shops, maps, rewards, etc. Stop at combat completion.
- Training or updating weights inside the UI.
- Batch evaluation dashboards, checkpoint leaderboards, and side-by-side multi-model comparison in the first release.
- Elaborate graphics, card artwork acquisition, animations, or reproducing the original game's layout exactly.
- General compatibility with every historical root/checkpoint format.
- A privileged hidden-state inspector in the first release. Do not expand the fair API just to expose hidden draw order.
- Gameplay-rule changes merely to make this UI work.

**Important distinction:** “Finish combat” computes a continuation on demand. It is not playback. While computation is running, the user's selected history node should remain stable; show progress separately. When finished, offer a clear jump to the result rather than animating through generated nodes.

## 3. Verified repository starting points

These were inspected when preparing this plan. Recheck exact APIs before implementing; do not assume this document supersedes code.

### Simulator and Python bindings

- `simulator/crates/sts_core/`: Rust simulator mechanics/state.
- `simulator/crates/sts_env/`: fair observation and action boundary.
- `simulator/bindings/py_sts/`: PyO3 bindings.
- `simulator/python/sts_sim/__init__.py`: typed Python wrapper.
- `simulator/python/sts_sim/_native.pyi`: native API/action properties.
- `simulator/python/sts_sim/observations/combat.py`: typed combat projections.

Existing Python methods include:

```python
State.from_synthetic_spec(spec_json: str) -> State
state.clone() -> State
state.decision() -> Decision
state.observation() -> Observation
state.legal_actions() -> list[Action]
state.step(action: Action) -> Decision
state.revision -> int
```

`Decision` contains `schema_version`, `revision`, `observation`, and `actions`.

There is no public Python snapshot export/import method in the inspected `State` wrapper. Do not plan around a nonexistent `serialize()` or `restore()` API. The first implementation can use clones in memory and root-plus-actions for durable reconstruction.

### Root creation and validation datasets

- `rl/synthetic_roots.py`: `build_root(...)`, `sample_root(...)`, `SyntheticRoot`.
- `rl/loadout_sampling.py`: `LoadoutSampler.load(...)` and sampled loadouts.
- `rl/scenarios.py`: encounter/configuration sampling.
- `rl/validation_set.py`: frozen synthetic validation dataset creation/loading.

`SyntheticRoot` contains the live state, encounter/loadout metadata, `spec_json`, and rejection metadata.

A synthetic root specification contains seed, floor, encounter kind/name, deck/upgrades, relics, potions, HP/max HP, gold, and counters. Constructing it calls `State.from_synthetic_spec`, which executes ordinary combat initialization. It is an initialization recipe, not a captured trace or an arbitrary mid-combat observation.

The inspected validation format uses:

- `schema: 1`
- `protocol: synthetic_pre_entry_hp_A0`
- `native_sha256`
- evaluation settings
- `cases[]` with ID, label, act, kind, specification, and initial-observation hash.

`load_validation(path)` validates protocol, native binary hash, case constraints, and initial observations, returning document plus grouped `Root` objects. Retain case identity from the original document; do not lose it through a metadata-poor adapter. Prefer reusing existing validation rather than reimplementing weaker checks.

Older rollout-derived `roots.json` files reconstruct combats from initial run configuration and action prefixes. They are not the initial UI target. Return a useful unsupported-format error if given one; never silently treat them as synthetic specs.

### Model and combat task

- `rl/model.py`: `CombatModel`.
- `rl/train.py`: typed and numeric rollout paths; inspect actual evaluation sampling behavior.
- `rl/train_synthetic.py`: current synthetic trainer/checkpoint writing.
- `rl/combat_task.py`: shared action filtering and terminal semantics.

The model currently supports:

```python
logits, valid_mask = model([combat_observation], [candidate_actions])
```

It returns padded batched logits and a valid-action mask. A single observation is still a batch of length one.

The inspected synthetic trainer saves a dictionary containing `model` state dict, optimizer, iteration, sampling/Torch RNG states, and config to `wandb/<run-id>/latest.pt`; it also writes `config.json` and `validation.json`.

This is not a general cross-version model interchange format. First support this current checkpoint shape with strict loading and clear errors. Do not infer arbitrary architectures by guessing tensor shapes.

`combat_task.action_indices(decision)` excludes Smoke Bomb use while preserving native indices. `combat_outcome(...)` recognizes completion without taking postcombat actions. Reuse these semantics.

## 4. Architecture

### Recommended deployment

A local browser frontend served by a Python backend in the RL environment:

```text
Browser UI
  -> loopback HTTP API
    -> Python session/root/model adapters
      -> existing sts_sim wrapper
        -> Rust simulator
```

Place the application under `rl/combat_explorer/` so simulator code does not depend on RL. Prefer a small server and static frontend over a large new framework/toolchain. FastAPI/Uvicorn plus plain modular JavaScript/CSS is a reasonable default; add dependencies explicitly using the existing `uv` project. A different lightweight implementation is acceptable if justified.

Suggested structure:

```text
rl/combat_explorer/
  __init__.py
  __main__.py          # documented local launch command
  server.py           # HTTP routing and validation
  roots.py            # datasets and synthetic generator adapter
  policy.py           # checkpoint loading, inference, sampling
  sessions.py         # immutable tree and transactional stepping
  persistence.py      # versioned JSON and replay reconstruction
  presentation.py     # display DTOs and honest transition summaries
  static/
    index.html
    app.js
    api.js
    history.js
    combat.js
    styles.css
rl/tests/test_combat_explorer_*.py
```

Avoid modifying the existing CommunicationMod `trace_ui`; it serves a different purpose and would muddy the simulator-only boundary.

### Layer boundaries

- Browser: rendering, selection/navigation, user input; no mechanics or RNG-driven gameplay.
- Session service: owns simulator instances, tree structure, selected-action validation, jobs, persistence.
- Policy adapter: receives only fair observations and permitted actions, never roots/specifications, hidden state, seeds, or internal simulator handles.
- Root service: generates/loads initial states outside the policy boundary.
- Rust: sole executor of game rules and gameplay RNG.

## 5. State tree and transition semantics

### Node definition

A node represents a settled decision state or the state reached when combat ends. Its immutable gameplay contents include:

- Unique node ID and parent ID (null only at root).
- Ordered child IDs (branch metadata can grow without changing the node's gameplay state).
- Incoming edge/action record, absent at root.
- Frozen fair observation, decision schema/revision, and action descriptors.
- Root-relative decision depth and turn-group metadata.
- Combat outcome if terminal.
- Derived markers and transition summary.
- Policy diagnostics referenced by model/configuration identity when available.
- An owned `State` clone in server memory, or an explicit recoverable cache reference if later optimizing.

Keep UI selection and preferred-child navigation separate from gameplay node contents. A policy diagnostic computed later is a separately identified analysis record, not a mutation of what historically happened.

### Edge definition

Each successful action creates an edge and a child node. Record:

- Parent and child IDs.
- Actor: human/model.
- Native action index within the parent's decision.
- Semantic action descriptor and readable label.
- Model checkpoint identity, mode, temperature, and sampling metadata when applicable.
- Reference to probabilities used for the actual choice.
- Transition summary and flags.

Do not identify actions by label alone. Duplicate cards and different targets can have identical-looking text.

### Transactional execution

For every manual/model step:

1. Resolve the source node explicitly; never use an implicitly changing global current state.
2. Confirm it is nonterminal and reconstructable.
3. Obtain its current decision and validate the submitted revision/action identity.
4. Apply the combat-task filter and reject disallowed actions.
5. Clone the source state.
6. Apply the selected native `Action` from that clone's matching decision.
7. If successful, construct and attach the complete child node atomically.
8. If rejected or raised, discard the working clone and leave all existing nodes unchanged.

Never reuse a stale native `Action` object from another node. Never mutate a historical node and attempt to undo selected fields afterward.

### Branch behavior

- Selecting a node is read-only and consumes no gameplay or sampling RNG.
- Back goes to parent; forward follows the selected/preferred child.
- At a fork, show choices clearly rather than choosing an arbitrary child silently.
- Making a move from any historical node appends a new branch. Existing children remain intact.
- Repeating the same action may create another child initially; do not add state deduplication/transposition merging in v1.
- “Model finish from here” always has a named source node and creates a new continuation; it does not silently follow or overwrite existing descendants.
- Terminal nodes permit inspection/navigation/save, but not further actions or model continuation.

### Limits and failures

The continuation job has a configurable maximum number of additional decisions (default 512 is consistent with current validation, but make the chosen bound visible).

Distinguish job completion due to:

- Combat victory.
- Combat defeat.
- User cancellation.
- Decision limit.
- Model error.
- Simulator error.

A cancelled or limited job leaves a valid nonterminal leaf that can be continued. Do not mark that state as dead or terminal. Do not assign invented reward to unfinished jobs.

Keep every accepted prefix before a failure. Record the attempted action and error separately; do not attach a fictional successful child.

## 6. Roots workflow

### Saved roots

- Configure or select a local validation manifest.
- Validate using existing validation contracts.
- Present a compact root list with case ID, main/stress label, act, floor, encounter, HP, deck size, and relic count where available.
- Basic text search and encounter/act filters suffice.
- Selecting a root constructs an independent session; the frozen validation root is never mutated.
- Preserve original root spec, source manifest hash, case ID, and native fingerprint in session provenance.
- A mismatching manifest must produce a specific error, not an override that suppresses validation.

### Generated roots

- Load an existing loadout-distribution artifact via `LoadoutSampler.load`.
- Use existing `sample_root` / `ScenarioConfig`, not a new distribution invented by the UI.
- Minimal controls: generation seed and supported floor/range controls.
- Keep advanced scenario controls collapsed or defer them.
- Use a service-owned `random.Random` for generation, separate from policy sampling.
- Freeze the resulting `spec_json` in the session so future reconstruction needs neither the distribution artifact nor another random draw.
- Preserve generation settings and source artifact hash as provenance.
- Unsupported/incompatible generation errors remain explicit. Use only existing sampler retry behavior; do not add silent replacement/filtering based on policy performance.

Initial support is synthetic A0, matching the constructor/protocol. Do not advertise A20 or other ascensions without implemented support.

## 7. Policy execution and probabilities

### Loading

- Models are optional until inference/model execution is requested.
- Load a trusted local checkpoint on CPU by default; optionally expose CUDA when available.
- Prefer restricted/weights-only loading supported by the installed Torch version. Never silently fall back to unsafe arbitrary pickle loading.
- Verify expected dictionary/state-dict shape and perform strict weight loading.
- Use the known supported architecture/configuration; reject incompatible architecture clearly.
- Call `eval()` and use `torch.inference_mode()`.
- Fingerprint checkpoint bytes and record architecture/config identity. The mutable path `latest.pt` is not identity.
- Read/hash/load a stable artifact consistently; avoid hashing one training-time version and loading another. An explicit checkpoint copy is acceptable.
- Keep historical checkpoint identity even if the file is replaced later.

### Candidate mapping

1. Obtain native decision actions.
2. Apply `combat_task.action_indices`.
3. Build the policy candidate tuple in that exact order.
4. Run the typed batch-of-one path first; numeric batching is an unnecessary optimization for v1.
5. Retain both candidate position and original native index.
6. Use the valid mask and check finite logits for valid candidates.
7. Preserve every candidate, including duplicate-looking actions; do not merge probabilities by card label.

For v1, apply the same combat-only filter to manual and model choices. Explain that Smoke Bomb use is excluded by the training task; do not change simulator inventory or mechanics. If full native manual actions are added later, treat that as a separately specified mode with escape semantics.

### Selection modes

- Greedy: choose argmax over valid candidates; deterministic tie break in native candidate order.
- Sampling: `p_i = softmax(logit_i / temperature)` for finite positive temperature.
- Temperature 1 means the original model distribution.
- Use explicit greedy mode, not temperature zero.
- Validate finite positive temperature; choose a documented practical slider range and enforce API validation too.
- Default to sampling at temperature 1 if code inspection confirms that matches current evaluation. Document the discovered evaluation behavior rather than guessing.

Show base model probabilities and, when temperature differs, adjusted sampling probabilities. In greedy mode distinguish the model distribution from the deterministic selection rule; don't suggest the model's underlying probability becomes 100%.

### Randomness

Gameplay RNG stays entirely inside Rust state clones. Policy sampling uses a dedicated local generator, not an untracked process-global RNG.

A simple reproducible contract:

- Each explicit model continuation request gets a recorded sampling seed and generator implementation/version identifier.
- Samples consume that job's generator only.
- Node inspection and probability display never sample.
- Persist accepted actions so reviewing/loading history does not require resampling.
- A fresh continuation can use a fresh seed; show it in metadata and allow reusing a seed for controlled experiments if inexpensive.
- Sampling does not change Torch global RNG used by training or other sessions.

Probability cache keys must include node, checkpoint fingerprint, model adapter version, task candidate mapping, and relevant settings. Do not show an old checkpoint's probabilities as current.

### Diagnostic truthfulness

Historical choice diagnostics describe what was used at execution time. Re-analysis using another model/settings is visibly separate. Missing historical probabilities remain unavailable; never backfill them silently and imply they were recorded.

Probabilities are preferences, not causal explanations or calibrated confidence. No value estimates or predicted card damage should be invented if the model/API does not provide them.

## 8. UI design

Optimize for readable state and fast navigation, not visual imitation of Slay the Spire.

### Recommended desktop layout

- Header: root/scenario, model identity, greedy/sample control, temperature, save/load, job status.
- Left: collapsible branch history with a flattened active path and fork affordances.
- Center: combat board (player, enemies, hand, selection state, piles, relics/potions).
- Right: legal-action probabilities and transition summary.

Allow panels to resize/collapse if inexpensive. Avoid making a graph-layout library a prerequisite. An indented collapsible tree plus active-path step list meets the tree requirement more simply than a large canvas graph.

### Combat board

Always expose HP/max HP, block, energy, enemy HP/block/intents, powers, cards/costs, potions, and current selection prompt.

- Render all relevant target and selection information from the fair observation.
- Use stable readable labels from existing content catalogs where available; fallback to enum/content keys rather than invented descriptions.
- Text cards are acceptable; no artwork dependency.
- Legal-action list is the authoritative interaction surface, even if card/target clicking is also added.
- Nested selections (discard, exhaust, choose generated card, confirm/skip, etc.) must be fully actionable.
- Respect the observation's unordered pile semantics and known-position metadata. Do not render an unordered draw-pile listing as the future draw order.
- On terminal/reward observations, show the outcome plus any available final public state; do not assume a `CombatScreen` still exists. Any retained prior combat view must be labeled as preceding state, not current.

### Action panel

For each allowed candidate show action label, target, base probability, adjusted probability when relevant, and a human-action button. Sort visually by probability if desired, but keep the original action identity/mapping immutable.

Highlight the actual outgoing action when following a historical edge. With multiple children, show which continuation is selected.

If no model is loaded, show legal actions normally and explain that probabilities require a checkpoint.

### Navigation

- Left/right arrow: previous/next node on the selected path.
- Held-key repeat should remain responsive and must only navigate existing nodes.
- Click any tree node to select it directly.
- Explicit previous/next turn-boundary controls.
- Explicit previous/next enemy-kill marker controls, scoped to selected path.
- Decision and turn labels with branch identity visible.
- Optional discrete scrubber over the current root-to-leaf path.
- No keyboard actions while typing in text/number fields.
- Never make an unmodified navigation shortcut execute a gameplay move.
- Avoid wheel interception initially unless clearly scoped; ordinary page/panel scrolling must remain usable.

Use text/icons as well as color for markers and outcomes. Show a clear focus state for keyboard navigation.

### Model continuation UX

“Model step” executes one decision. “Model finish from here” starts a bounded job. Show source node, generated decision count, and cancellation control. Do not move selection on every generated node. On completion show a result link/button.

A session may allow navigation during computation. For v1, reject concurrent gameplay mutations in that session while one job is writing, with an explicit busy message. Keep reads responsive. Capture checkpoint and settings at job start so changing UI controls does not silently alter a running continuation.

## 9. Transition summaries and important markers

This needs careful implementation: the inspected fair decision API exposes states, not a chronological event log. Do not promise exact enemy action narration based only on before/after fields.

### Baseline summary without simulator changes

Show facts supported by the accepted action and public state delta:

- Submitted action and target.
- End-turn submitted / enemy phase resolved before the next decision, when supported by control flow.
- Player HP, block, and energy changes (label as net changes).
- Enemy HP/block/power changes where identity can be matched reliably.
- Hand/pile count changes and reliably identifiable card changes.
- Powers gained/lost/changed.
- Terminal outcome.

A loss of 8 HP is a **net HP change**, not necessarily “enemy dealt 8 damage.” Healing, block resets, effects, and multiple events can occur in one transition. Previous visible intent is not proof of the action actually executed.

### Enemy-action summaries

First investigate whether an existing trustworthy transition-event facility can be exposed read-only. If it exists, consume it. If not, deliver the accurate aggregate summary above and clearly label that individual enemy actions are unavailable.

If exact per-enemy narration is essential to acceptance and unavailable, flag that gap rather than fabricate details. A narrow separate event-instrumentation change may be proposed, but do not casually expand this UI task into action-queue work; read `simulator/docs/research.md` before any such changes.

### Turn and kill markers

Use accepted end-turn actions and documented settlement behavior to group turns. Do not assume every action increments a turn. Nested choices remain separate decisions within the relevant turn group. Handle terminal end-turn transitions.

Enemy kills cannot always be inferred from disappearance or slot reuse: escape, split, revive, replacement, and terminal screen changes complicate this. Use reliable death events if available. Otherwise:

- Mark confirmed observable deaths only where identity/alive transition is unambiguous.
- Mark uncertain removal as “enemy removed/changed,” not “kill.”
- Exclude explicit escape from kill markers.
- Record a combat-win marker independently when final enemy details are absent.
- Document marker coverage limitations.

Never let summary generation or marker failures alter gameplay state or block a successful transition from being retained.

## 10. Persistence

### Format

Use a versioned, inspectable JSON session document, not Python pickle or raw native pointers.

Recommended fields:

- Format name and schema version.
- Session ID and timestamps.
- Root specification and provenance.
- Simulator/native fingerprint and observation schema metadata.
- Task filter/protocol version.
- Checkpoint references with hashes/configuration, not embedded model tensors.
- Nodes and edges with accepted native indices and semantic descriptors.
- Recorded public observations/action manifests/hashes for display and replay checks.
- Historical model probabilities and sampling settings/seeds.
- Job outcomes/limits/errors separate from combat outcomes.
- Selected node and preferred path as optional UI metadata.

Persist large numeric values such as seeds as strings at the browser boundary where needed; JavaScript numbers cannot safely represent every 64-bit integer. Round-trip seed fidelity is mandatory.

### Loading and reconstruction

1. Validate schema, types, sizes, IDs, parent references, exactly one root, reachability, and absence of cycles.
2. Check simulator/task compatibility before enabling continued simulation.
3. Recreate root from its specification.
4. Check initial public observation/action manifest.
5. Traverse the tree in parent-first order, cloning reconstructed parents and replaying stored accepted actions.
6. At each step verify native index plus semantic descriptor against the current decision and compare the recorded observation/action manifest.
7. On mismatch stop reconstruction for the affected session and present a precise error. Never patch simulator state from saved observations.
8. Only publish a fully validated runnable session; do not leave a half-loaded session masquerading as healthy.

Recorded public hashes are drift checks, not proof that hidden state is equivalent. Reproducibility depends on compatible code and deterministic reconstruction, not observation equality alone.

Loading history must not need the original model: replay accepted actions directly. New manual moves can work without it; new model moves require the referenced or explicitly selected compatible checkpoint.

Cross-version replay is not promised. An optional read-only opening of saved observations is acceptable if clearly labeled incompatible/non-runnable, but is not required for v1.

### File safety

Use atomic save via temporary file plus rename. Validate filenames/paths inside configured roots/session directories. Never expose arbitrary filesystem reads or shell commands through the API. Treat checkpoint loading as trusted-local-input functionality and explain that clearly.

## 11. HTTP and job contract

Suggested API shape (adapt names as needed):

- `GET /api/capabilities`: versions, supported root/checkpoint protocols, configured artifact sources.
- `GET /api/roots`: validated case summaries.
- `POST /api/sessions/from-root`: create from manifest/case ID.
- `POST /api/sessions/generated`: generate and freeze a spec.
- `GET /api/sessions/{id}/tree`: lightweight tree metadata.
- `GET /api/sessions/{id}/nodes/{node}`: observation, action descriptors, markers.
- `POST /api/sessions/{id}/nodes/{node}/analyze`: explicit policy analysis.
- `POST /api/sessions/{id}/nodes/{node}/act`: manual action by identity/revision.
- `POST /api/sessions/{id}/nodes/{node}/model-step`.
- `POST /api/sessions/{id}/nodes/{node}/continue`: start bounded job.
- `GET /api/jobs/{id}` and `POST /api/jobs/{id}/cancel`.
- `POST /api/sessions/{id}/save` and `POST /api/sessions/load`.

State-changing requests must not be GETs. Use client request IDs/idempotency protection or equivalent to prevent double clicks/retries from producing accidental branches. Validate source session/node/revision every time.

Use one serialized writer per session. Keep clone/mutation ownership explicit. Do not block the web event loop with a long synchronous rollout. A worker thread with serialized model access is a reasonable first implementation; verify compatibility with the binding's thread constraints before choosing the execution strategy. Do not hold a broad session lock while performing the entire job if it prevents inspection/cancellation.

Bind to `127.0.0.1` by default. Serve frontend and API same-origin, avoid permissive CORS, validate Origin/Host as appropriate, and protect mutation endpoints from cross-origin requests. This is local-only tooling, not an authenticated internet service.

## 12. Implementation stages

### Stage A — API reconnaissance and backend proof

- Read relevant root, model, observation, and action code.
- Locate available validation manifests, distribution artifacts, and a current checkpoint without scanning/copying huge data trees.
- Determine typed-model support for nested selection decisions and actual evaluation sampling defaults.
- Confirm clone independence and deterministic replay with existing public APIs.
- Determine summary/event availability and binding concurrency constraints.
- Record blockers rather than inventing unavailable behavior.

Deliverable: focused tests proving root -> legal action -> child clone -> terminal detection, plus batch-of-one inference if a checkpoint is available.

### Stage B — Branching engine

- Root adapters, immutable node/edge structures, transaction-safe execution.
- Manual action validation and combat-task filtering.
- Branch creation and reconstruction tests.
- Basic derived summaries/markers.

Deliverable: headless service can construct a tree with two continuations without mutating ancestors.

### Stage C — Policy and continuation jobs

- Strict checkpoint adapter, fair candidate mapping.
- Base/temperature probabilities, greedy and isolated seeded sampling.
- One model step and bounded continuation/cancellation.
- Preserve accepted prefixes and classify errors honestly.

Deliverable: complete combat from root, fork at an earlier node, complete alternative.

### Stage D — Browser vertical slice

- Root picker/generator and model controls.
- Combat board, authoritative legal-action list, selection UI.
- Tree/path navigation and probability panel.
- Model completion progress with stationary selection.
- Turn/kill marker navigation and effect summaries.

Deliverable: the user's central workflow works end to end in a browser without developer console intervention.

### Stage E — Persistence and usability

- Versioned JSON export/import and replay validation.
- Exact large-seed handling.
- Atomic writes, safe artifact paths, clear errors.
- Keyboard shortcuts, readable layout, loading/empty/error states.
- Launch documentation and screenshots if tooling permits.

### Stage F — Verification and handoff

Run applicable tests, record exact results, and list remaining limitations. Do not claim completion based only on mocked backend tests or a server starting successfully.

## 13. Test plan

### Root tests

- Valid synthetic manifest loads and preserves case identity.
- Native hash, observation hash, malformed case, and legacy format errors remain explicit.
- Fixed generated seed/settings produce the same frozen root spec.
- Root generation does not consume policy sampling RNG.
- Starting a session never mutates a shared validation root.
- 64-bit seeds round-trip through API/browser/save/load exactly.

### Branching tests

- Two different moves from the same parent create siblings.
- Parent and existing descendants remain unchanged.
- Navigating arbitrary nodes consumes no RNG or steps.
- Replay of one branch matches its recorded observations/actions.
- Stale revision, wrong node, wrong candidate index, and descriptor mismatch are rejected atomically.
- Nested target/selection decisions are preserved, not auto-skipped.
- A failed step adds no successful edge and does not corrupt the parent.
- Terminal state cannot be advanced.

### Policy tests

- Policy sees only typed public observation and filtered candidates.
- Native index mapping is correct after Smoke Bomb exclusion, including newly generated potions.
- Duplicate-looking candidates remain distinct.
- Valid probabilities sum to one; padded entries never become candidates.
- Temperature 1 matches base softmax; lower/higher temperature behavior is correct on fixed logits.
- Zero/negative/nonfinite temperature is rejected.
- Greedy tie-breaking is deterministic.
- Seeded sampling repeats under the supported same-version contract.
- Analysis/navigation does not advance sampling RNG.
- Recorded diagnostics remain tied to the original checkpoint/settings after control changes.
- Bad checkpoints and nonfinite valid logits fail explicitly.

### Jobs and API tests

- Continuation reaches victory/defeat when possible without postcombat actions.
- Limit/cancel/error outcomes are distinct and leave valid prefixes.
- Concurrent mutation is rejected/serialized while reads remain possible.
- Double-submit protection does not duplicate branches accidentally.
- Job settings/model are fixed at submission.
- Source node selection changing in the browser does not redirect a running job.
- Traversal paths, hostile cross-origin requests, and malformed payloads are rejected.

### Persistence tests

- Save/load a multi-branch tree; compare every observation/action record and branch relationship.
- Continue a loaded branch and compare with continuing the original node under the same inputs.
- Historical loading succeeds without a checkpoint.
- Incompatible simulator/task version fails closed.
- Corrupted action records, cycles, missing parents, duplicate IDs, and invalid selected node are handled explicitly.
- JSON serialization contains no NaN/Infinity and preserves large seeds.
- Failed load does not publish partial runnable state.

### UI/end-to-end tests

Automate key workflows with a browser test tool if feasible and document its setup:

1. Load saved root and inspect state.
2. Generate a root and start a separate session.
3. Execute a targeted card and a nested selection.
4. Load model, inspect probabilities, complete combat.
5. Return to an earlier decision, act manually, complete a new branch.
6. Navigate both original and alternative outcomes.
7. Jump to turn boundaries and available confirmed-kill markers.
8. Save/reload and continue from a prior node.
9. Navigate while a job runs; verify no automatic playback.
10. Exercise no-model, terminal, error, and cancelled-job states.

Use genuine supported simulator states for integration tests. Mocks can test UI/error plumbing but do not prove gameplay behavior or model compatibility.

## 14. Validation commands and project rules

Use `uv` for Python/PyO3 operations. Follow `simulator/README.md` for binding setup:

```bash
uv sync --project simulator/python --reinstall-package sts-sim
cd simulator/python
uv run maturin develop --uv
uv run ty check
uv run ruff check sts_sim examples
uv run python examples/showcase.py
```

For RL tests, inspect the existing tests' runner conventions and document the exact commands used; add focused explorer tests plus relevant existing model/root/task tests. Do not assume pytest is installed when it is not declared. Add any necessary development/test dependencies explicitly.

If changing Rust, run from repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
```

For gameplay/replay changes also run:

```bash
cargo run -p sts_verify --bin sts_verify -- simulator/verification/corpus/permanent_traces
```

UI tests and synthetic simulator tests do not establish real-game parity. Report unavailable artifacts, corpus, device, or browser tooling honestly. Do not change immutable traces or weaken existing tests to obtain success.

No `docs/project_history.md` update is needed for routine UI implementation. Only update it if a major architectural assumption or rejected approach genuinely warrants it under project rules.

## 15. Acceptance checklist

The first version is done when:

- [ ] A documented local command launches the UI and backend.
- [ ] Both saved synthetic validation roots and generated roots are supported.
- [ ] The UI is usable without a model for manual legal moves.
- [ ] A supported real checkpoint loads and displays action probabilities, or missing external artifacts are explicitly reported as an unverified blocker rather than completion.
- [ ] Greedy and positive-temperature sampled actions work with correct identity mapping.
- [ ] Model completion creates navigable decision history without automatic playback.
- [ ] Any existing node can be selected and inspected.
- [ ] A manual alternative plus model continuation produces a retained sibling branch.
- [ ] Ancestors/original continuations remain unchanged.
- [ ] Nested selection decisions and targets are usable.
- [ ] Turn ends and reliably known enemy kills are easy to find; marker limitations are explicit.
- [ ] Between-decision summaries are factual and do not invent enemy event details.
- [ ] Victory, defeat, limit, cancellation, and errors are distinguished.
- [ ] Save/load preserves a multi-branch session and supports compatible reconstruction.
- [ ] No model input receives hidden state or debug metadata.
- [ ] No real-game bridge or gameplay-rule implementation is added.
- [ ] Tests and a browser walkthrough verify the central workflow.

## 16. Handoff report expected from implementer

Provide:

1. What was implemented and any deviations from this plan.
2. Exact launch command and example artifact arguments.
3. Supported root/checkpoint formats and compatibility limits.
4. How branching, sampling, save/load, and cancellation behave.
5. Tests/checks run with results.
6. Browser walkthrough evidence or explicit reason it could not run.
7. Known limitations, especially enemy-event/kill-summary coverage.
8. Changed file summary and any new dependencies.

Keep the implementation small and understandable. The hardest correctness requirement is preserving independent simulator histories while enabling reproducible alternate continuations—not drawing a fancy tree.
