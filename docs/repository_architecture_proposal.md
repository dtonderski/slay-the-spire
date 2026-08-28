# Lean Repository Architecture Proposal

**Status:** Proposed three-PR cleanup

**Scope:** Make repository ownership outside `simulator/` easier to understand.

## Decision

Replace the misleading `simulator/` umbrella with a small root Rust workspace,
one Python project, one broad live application, and root-owned verification and
documentation directories.

This is a repository cleanup, not a platform migration. Bugs are expected and
can be fixed through the existing local workflow. Overengineering and excess
complexity are the larger risks. The proposal therefore extracts only search,
where there is an immediate dependency benefit, then performs two physical move
PRs. Trace parsing remains verifier-owned until another real consumer justifies
a crate boundary.

There is no preparatory governance program, compatibility mode, or production
migration framework.

## Current factual diagnosis

The current Rust workspace is rooted at `simulator/Cargo.toml`:

```text
sts_core   -> []
sts_verify -> [sts_core]
sts_live   -> [sts_core, sts_verify]
py_sts     -> [sts_core, sts_live, sts_verify]
```

The edges reflect mixed responsibilities:

- `sts_core` already owns deterministic mechanics, state, rules, named RNG,
  snapshots, content, fair/public observations, and transition results.
- `sts_verify` owns replay, diff, minimization, seed conversion, the roughly
  1,899-line generic trace DTO/parsing implementation, and SlayTheData
  import/preflight/CLI commands.
- `sts_live` combines CommunicationMod transport, bridge/session handling,
  traces, fidelity, SlayTheData, HTTP/CLI, automation, production beam search,
  combat research, and the Vite UI.
- `py_sts` calls the production beam teacher through `sts_live`, uses verifier
  seed and SlayTheData APIs, contains a second Rust/PyO3 search surface, and owns
  concrete privileged beam-clone episode orchestration.
- Repository search finds `slaythedata_preflight_json` only at its PyO3
  definition, base-package export, and type stub. Apart from their definitions
  and tests, `rust_greedy_combat_search` and `rust_beam_combat_search` appear
  only in the native type stub. None has a repository caller, so PR 1 deletes
  all three surfaces.
- The 18 live automation integration tests cover orchestration, mapping, safety,
  and lifecycle, but do not freeze an exact production recommendation,
  principal variation, score, or complete tie-break result. Trace replay cannot
  detect search-only behavior changes.
- `sts_verify` has real `slaythedata-plan` and `slaythedata-preflight` CLI
  commands, while live has a large SlayTheData module. Moving that subsystem is
  not a small prerequisite for clearer repository paths.
- Rust, Python, verification data, component docs, and the supported UI all sit
  under `simulator/`; path assumptions also occur in Maturin, notebooks,
  collectors, corpus tooling, Cursor setup, ignores, READMEs, and agent rules.

One premise needs explicit correction: base `import sts_sim` is already
Torch-free because `sts_sim/__init__.py` does not import `sts_sim.rl`. Torch is
loaded only through explicit RL modules. This proposal preserves that behavior;
it does not claim to create it.

## Hard invariants

The goal is simply clearer structure plus the minimum extraction needed to
remove inappropriate binding-to-app dependencies. Four domain invariants remain:

1. **No observed-state hydration.** Replay advances only from initial
   seed/state, accepted actions, explicit external inputs, and game rules.
   Observations are expected output and never repair simulator state.
2. **Immutable captured traces.** Real-game payloads are never edited,
   normalized, regenerated, or truncated to make verification pass.
3. **Deterministic core.** RNG stays named and tracked; observation, legal
   action generation, serialization, hashing, and display consume no RNG;
   snapshot/restore preserves replay behavior.
4. **Fair neural inputs.** Models and tensors receive fair observations,
   ordered public choices, and public history only. Hidden authoritative state
   may be used by an explicitly privileged teacher, never as model input.

These are behavior constraints, not reasons to build governance machinery.

## Target layout

```text
Cargo.toml
crates/
  sts_core/
  sts_verify/
  sts_search/
bindings/
  py_sts/
apps/
  sts_live/
    src/
    ui/
python/
  pyproject.toml
  uv.lock
  sts_sim/
  tests/
  notebooks/
verification/
docs/
mods/       # unchanged
tools/      # unchanged
```

The existing Cargo lockfile policy continues unchanged.

## Target dependency graph

Project-internal edges are:

```text
sts_core   -> []
sts_verify -> [sts_core]
sts_search -> [sts_core]
py_sts     -> [sts_core, sts_search]
sts_live   -> [sts_core, sts_verify, sts_search]
```

External dependencies such as Serde, PyO3, NumPy, and Torch are omitted.

## Component boundaries

- **`sts_core`** owns existing mechanics, state transitions, named RNG,
  snapshots, content, fair/public APIs, transition results, and the seed codec.
  It has no internal dependency. This move adds no canonical combat outcome
  framework or outcome semantics beyond existing transition results. Seed
  conversion belongs here because simulator construction and base Python use it.
- **`sts_verify`** owns trace DTOs/parsing/serialization, replay, strict
  comparison, first divergence, diff, minimization, corpus operations, and its
  CLI. It also retains SlayTheData import, preflight, and CLI commands. Trace and
  SlayTheData are imperfectly placed, but splitting either now would add churn
  without removing an internal dependency edge.
- **`sts_search`** owns incumbent `RunState`-typed beam/greedy planning,
  search policy/configuration, scoring, budgets, tie-breaking, deduplication,
  diagnostics, typed warm suffixes, and teacher decisions with unchanged
  semantics. Live and Python teacher labeling both use it, so extraction removes
  `py_sts -> sts_live`. `LiveState` observation binding, wire/session config,
  command projection, and warm-plan lifecycle remain in live.
- **`bindings/py_sts`** owns PyO3 conversion and the concrete privileged
  beam-clone episode loop, including outcome, truncation, decision/turn, and
  delta accounting. Keep this known binding-local orchestration until PUCT or
  another Rust consumer justifies sharing it. Python `_terminal_status` cleanup
  is also deferred. Privileged naming is not a security boundary; fair
  enforcement remains in model/tensor input tests proving neural code receives
  only fair observations and public choices.
- **`apps/sts_live`** remains broad: CLI/HTTP, CommunicationMod bridge/sessions,
  SlayTheData, fidelity, automation composition, trace writing, combat research,
  and Vite UI. Existing verifier/live SlayTheData modules stay accepted debt.
- **`python`** remains one project and `sts_sim` package with `sts_sim.rl`. Base
  import stays Torch-free; explicit RL still uses py_sts, NumPy, and Torch.

## Current-to-target mapping

| Current path or owner | Target/disposition |
|---|---|
| `simulator/Cargo.toml` | `/Cargo.toml` |
| `simulator/crates/sts_core` | `/crates/sts_core` |
| `simulator/crates/sts_verify/src/seed.rs` | `/crates/sts_core` |
| verifier trace/replay/diff/minimize/corpus and SlayTheData CLI | `/crates/sts_verify` |
| `RunState`-typed production search in `sts_live/src/automation.rs` | `/crates/sts_search` |
| search fields in `AutomationConfig`/`AutomationPolicy` | search-owned `SearchConfig`/`SearchPolicy`; live wire/session adapter stays in `sts_live` |
| concrete privileged episode loop in `py_sts` | stays in `/bindings/py_sts` |
| `simulator/crates/py_sts` | `/bindings/py_sts` |
| `simulator/crates/sts_live/src` | `/apps/sts_live/src` |
| `simulator/crates/sts_live/ui` | `/apps/sts_live/ui` |
| verifier/live SlayTheData modules | stay with their current components |
| `simulator/python` | `/python` |
| `simulator/verification` | `/verification` |
| `simulator/docs` | root `/docs` or relevant component docs |
| `simulator/AGENTS.md` | merge useful rules into root/component rules; remove original |
| `simulator/README.md` | merge/move useful content to root/component docs; remove original |
| root `mods` and `tools` | unchanged |

No exhaustive path inventory is required. Physical move PRs use targeted old
path searches and ordinary rename/path diff review.

## Explicit non-goals and accepted debt

This proposal does not add or design:

- CI workflows, required statuses, branch protection, or provisioned runners;
- reliability programs or production-grade migration machinery;
- compatibility layers, deprecation windows, artifact epochs, or path
  inventories;
- a Cargo lockfile policy change;
- trace, shared rollout, bridge, SlayTheData, guidance, or evaluation crates;
- duplicate core-free trace wire types;
- generic policy/rollout traits or a public-versus-privileged trait framework;
- optional bindings or separate `sts_data`/`sts_eval` Python packages;
- checkpoint/dataset migration readers or quarantine systems;
- SlayTheData relocation, legacy UI consolidation, or gameplay/API redesign.

Accepted debt is explicit: the concrete episode loop stays in PyO3, Python
terminal fallback stays for now, SlayTheData remains split across verifier and
live, and trace parsing stays in verifier despite its file size. Each can be
revisited when a concrete consumer or maintenance problem supplies value.

Small repository-local surface deletions are allowed only where needed to
remove `py_sts` app/verifier dependencies. There is no compatibility layer for
untracked scripts.

## Three-PR sequence

### PR 1 — Extract production search and clean binding dependencies

**Risk/value:** Medium-high risk, high RL value.

- Create `sts_search` in the existing nested workspace.
- Apply one type-level split rule: **`RunState`-typed planning moves;
  `LiveState`-typed orchestration stays.** Move greedy/beam search, scoring,
  frontier ordering, typed warm-suffix validation, budgets, deduplication,
  diagnostics, benchmarks, and teacher decisions while preserving behavior.
- Split the current live model types instead of importing them wholesale.
  `sts_search::SearchPolicy` and `SearchConfig` own greedy/beam policy and the
  depth, width, allowed-potion, transition/time-budget, and deduplication fields.
  Live keeps its serialized `AutomationPolicy`/`AutomationConfig` adapter,
  including `FakePlayFirstCard`, `auto_action_limit`, HTTP/CLI/UI compatibility,
  and session snapshots.
- Keep `observed_run_state`, fidelity/session fencing, live legal-action and
  command projection, warm-plan reuse/invalidation, and action sending in
  `sts_live`. Live converts its config to search config and typed search results
  back to live plan snapshots.
- Expose the concrete beam teacher decision from search so the binding no longer
  reaches through live.
- Move `sts_verify/src/seed.rs` and its tests to core.
- Delete `slaythedata_preflight_json` from PyO3, the base export, and the stub.
  Repository search shows definition/export/stub only; possible untracked-script
  breakage is accepted. Do not run an external compatibility project.
- Delete the second PyO3 `rust_greedy_combat_search` and
  `rust_beam_combat_search` implementations and their stub/tests. Repository
  search finds no caller, so production search should have one owner.
- Add no forwarding or compatibility layer.
- End with `py_sts -> core + search`, with no live or verifier dependency. Keep
  the concrete privileged episode loop in the binding, calling search directly.

**Characterization before moving code:**

The current integration tests are not an exact regression net for search. Add a
small deterministic fixture against the incumbent implementation first:

- fixed `RunState` root and search config with no wall-clock deadline;
- exact first typed action, complete principal variation, value, terminal
  reason, and deterministic diagnostics, asserted identically across two runs;
- finite transition-budget variant pinning exhaustion and node accounting;
- valid warm-suffix variant pinning recommendation, value, cache hit, and fresh
  expansion; and
- a small synthetic comparator/frontier table pinning the full tie-break chain
  and first-action diversity.

Capture expected values from the current implementation rather than deriving
or inventing them during extraction. Do not assert elapsed wall-clock time.

**Focused existing local validation:**

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets`
- `cargo test --workspace -- --test-threads=1`
- the new frozen search characterization plus existing automation tests
- relevant binding/Python API and teacher-labeling tests
- committed verifier fixture:
  `cargo run -p sts_verify --bin sts_verify -- corpus manual/milestone1.jsonl`
- permanent corpus parity when locally available, while recognizing that trace
  replay does not exercise search output

### PR 2 — Move the Rust workspace, binding, and live app

**Risk/value:** Medium risk, structural value.

- Move the workspace manifest to root, libraries to `/crates`, binding to
  `/bindings/py_sts`, and live source/UI to `/apps/sts_live`.
- Preserve the current Cargo lockfile policy.
- Update manifests, Rust path helpers, temporary nested-Python Maturin paths,
  backend/Vite assets, live/combat-research defaults, collector defaults,
  `tools/hf_corpus.sh`, `.gitignore`, `.cursor`, README commands, and targeted
  old-path references.
- Update root `AGENTS.md` for the new Rust/live paths and update
  `docs/project_history.md` for the changed repository ownership assumption.
- Do not move `mods`/`tools`, consolidate the legacy UI, or edit gameplay,
  search, trace, or SlayTheData algorithms.

**Focused existing local validation:**

- Cargo fmt, clippy, and workspace tests from root
- binding build and relevant Python tests after manifest updates
- committed verifier fixture and permanent corpus when locally available
- Vite UI build/tests because its location and assets move
- existing live fake-bridge/help/health tests; no real-game gate
- targeted stale-path search

### PR 3 — Move Python, verification, docs, and remove `simulator/`

**Risk/value:** Low-to-medium risk, structural completion.

- Move `simulator/python` to `/python` without splitting `sts_sim` or
  `sts_sim.rl`.
- Move `simulator/verification` to `/verification` without changing captured
  payload bytes.
- Move `simulator/docs` into root or component-local docs.
- Merge useful `simulator/AGENTS.md` rules into root/component `AGENTS.md` files
  and useful `simulator/README.md` content into root/component documentation;
  remove both originals.
- Update uv, Maturin, corpus, notebook, docs, agent-rule, Cursor, ignore, and
  README paths. Update root `AGENTS.md` and `docs/project_history.md` for the
  completed physical layout.
- Delete the now-empty `simulator/` directory.
- Add no Python packages, compatibility shims, or artifact readers.

**Focused existing local validation:**

- Cargo fmt, clippy, and workspace tests
- uv sync/build plus relevant pytest, Ruff, and ty checks from `/python`
- base `import sts_sim` smoke confirming Torch is not imported
- explicit `sts_sim.rl` tests in the RL environment
- committed verifier fixture at its new path and permanent corpus when available
- notebook, documentation-link, and stale-path searches
- no Node validation unless a UI reference is actually touched

## Artifact policy

Delete and regenerate disposable generated roots, derived datasets, and
checkpoints after relocation. Do not build a general migration system for
artifacts that can be recreated.

Immutable real-game traces stay byte-identical and path-independent: a move may
change where a payload is found, never its contents. If a specific old dataset
or checkpoint later proves valuable, solve migration for that artifact and its
actual consumer then. Never weaken or reinterpret source digests.

## Definition of done

The cleanup is complete when:

- the target layout exists and `simulator/` is gone;
- internal dependencies match the target graph;
- production search has one owner in `sts_search`, with its exact
  recommendation/PV/value/tie-break behavior characterized before extraction;
- search configuration is search-owned while live wire/session configuration
  remains a thin adapter;
- the seed codec is in core;
- `py_sts` depends on core and search, not live or verifier, while concrete
  privileged episode orchestration stays binding-local;
- trace parsing and SlayTheData remain deliberately in verifier/live without
  new subsystems, and unchanged traces pass existing verification;
- `sts_sim` and `sts_sim.rl` remain one project and base import stays Torch-free;
- root/component agent rules, READMEs, project history, Cursor setup, ignores,
  tooling, manifests, notebooks, UI assets, and corpus paths describe the new
  layout;
- existing focused local tests and available trace corpora pass; and
- no CI, compatibility framework, artifact epoch system, inventory, or
  deferred package split has been introduced.
