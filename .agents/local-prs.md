# Local PR Queue

Integrator branch: `codex/integration-m32a`

Use this file to track parallel worker branches before integration. Each worker should own a narrow scope, avoid unrelated files, and report tests run.

| Branch | Owner | Scope | Expected write set | Status | Tests | Merge notes |
| --- | --- | --- | --- | --- | --- | --- |
| `codex/integration-m32a` | integrator | Stabilize current working tree and merge reviewed worker branches | repo-wide as needed for integration only | active | pending | Current dirty tree moved from `master` onto this branch. |
| audit/cards | Lagrange | Read-only M32A audit of cards, statuses, curses, mappings, tests, and evidence | none | completed | n/a | Recommends one row per `ContentId`; separate trace mapping from effect modeling. |
| audit/relics-potions | Dewey | Read-only M32A audit of relic/potion mappings, pools, tests, and evidence | none | completed | n/a | Recommends one row per `RelicKey`/`Relic`/`Potion`; split proof strength by row. |
| audit/run-world | Descartes | Read-only M32A audit of monsters, bosses, encounters, events, rooms, rewards, shops, rest, map, ascension, and corpus coverage | none | completed | n/a | Recommends run/world rows by surface group and explicit RNG stream/caveat columns. |
| `codex/m32a-matrix-shell` | integrator | Add matrix vocabulary and empty schema row | `simulator/docs/content_support_matrix.md`, `.agents/local-prs.md` | ready for commit | docs only | Base artifact for future matrix workers. |

## Recommended Next Local PRs

| Branch | Scope | Write set |
| --- | --- | --- |
| `codex/m32a-card-inventory` | Fill card, colorless, curse, and status rows. | `simulator/docs/m32a_cards_matrix.md` only. |
| `codex/m32a-relic-potion-inventory` | Fill relic and potion rows. | `simulator/docs/m32a_relic_potion_matrix.md` only. |
| `codex/m32a-run-world-inventory` | Fill monster, boss, encounter, event, room, reward, shop, rest, map, ascension, verifier, and corpus rows. | `simulator/docs/m32a_run_world_matrix.md` only. |

## Worker Contract

- Start from the current integration branch checkpoint.
- Own a disjoint write set where possible.
- Do not revert unrelated edits.
- Keep changes small and reviewable.
- Run focused tests plus the project gate when feasible: `cargo fmt`, `cargo clippy`, and `cargo test` from `simulator/`.
- Final report must include changed files, diff summary, tests run, and known risks.
