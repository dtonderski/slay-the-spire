# Combat Research Freeze — 2026-07-13

This directory is the immutable benchmark input for the overnight `sts_live`
combat-planner research run. After this freeze, planner experiments may change
`crates/sts_live/src/automation.rs` and directly related planner tests only.
They must not change `combat_research.rs`, root JSON, manifests, evaluator
scoring, simulator mechanics, verifier behavior, or split membership.

## Membership

| Split | Roots | Lineages | Act 1 | Act 2 | Act 3 | Low HP | Potions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| development | 124 | 15 | 94 | 30 | 0 | 20 | 91 |
| validation | 56 | 4 | 35 | 12 | 9 | 12 | 43 |
| held_out | 23 | 3 | 10 | 11 | 2 | 1 | 20 |
| challenge | 22 | 2 | 12 | 10 | 0 | 4 | 15 |

Only two independent non-challenge lineages contain verified Act 3 roots.
They are sealed in validation and held-out. Development therefore has no Act 3
roots; duplicating either lineage into development would leak a sealed split.
The challenge split is diagnostic development evidence and never promotion
evidence.

Extraction admitted 225 unique strict-replay combat-start roots and recorded
223 explicit exclusions. Once admitted, losses, nonterminal searches, illegal
plans, errors, and timeouts remain in the evaluation denominator.

## SHA-256

- `development.json`: `028DB01B966B712F9718C136C8683218E4565A19E42B7BC67B710AF95B7915C0`
- `validation.json`: `1FCF0EE9FA5F3493AC203B77DA8E86474AF8682DE06A9B06A33FADD4B6B31E73`
- `held_out.json`: `EBC99C574573D2A651C47B9392B55D38679CD5D475D9F3FE78162906EDB023AA`
- `challenge.json`: `6757095B55D39E941FE87D10DDA893935E3F8C44134BD0F91A6045C289211F2E`
- frozen evaluator source `combat_research.rs`: `8DA2896E8B396078F88CABBEF4417B3044E91D158A27C27EF055CF7B57FEE73B`
- incumbent planner source `automation.rs`: `A2228721CC4739294C568F7A638513D206199848D0F173E773174D8CC745E59A`
- evaluator binary: `2869E9B210EA956CE8B2D81A91C52F98832BD18051FDEA67E418652FC41FEF5F`
- incumbent live binary: `34DD8D0EB7459544BFE23922F7E5C0EEA09039569FCFB030CDCB0501ECCB35C3`

## Correctness fingerprint

`cargo test -p sts_live --lib --no-fail-fast` ran 301 tests. Exactly three
pre-existing failures are allowed:

1. `session_tests::slaythedata_blocks_same_command_on_wrong_live_phase_and_stays_blocked`
2. `slaythedata::tests::pending_room_resolution_allows_ambiguous_route_suffix`
3. `ui_contract_tests::static_ui_sends_backend_action_ids`

Every combat-research test passed. Any additional failure rejects a candidate.
Validation may be evaluated once after development selection. Held-out may be
evaluated once after validation passes. Neither result may feed another edit.
