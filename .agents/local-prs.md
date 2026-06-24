# Local PR Queue

Integrator branch: `codex/integration-m32a`

Use this file to track parallel worker branches before integration. Each worker should own a narrow scope, avoid unrelated files, and report tests run.

| Branch | Owner | Scope | Expected write set | Status | Tests | Merge notes |
| --- | --- | --- | --- | --- | --- | --- |
| `codex/integration-m32a` | integrator | Stabilize current working tree and merge reviewed worker branches | repo-wide as needed for integration only | active | pending | Current dirty tree moved from `master` onto this branch. |

## Worker Contract

- Start from the current integration branch checkpoint.
- Own a disjoint write set where possible.
- Do not revert unrelated edits.
- Keep changes small and reviewable.
- Run focused tests plus the project gate when feasible: `cargo fmt`, `cargo clippy`, and `cargo test` from `simulator/`.
- Final report must include changed files, diff summary, tests run, and known risks.
