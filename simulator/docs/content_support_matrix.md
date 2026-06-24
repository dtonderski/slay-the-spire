# Ironclad A0 Content Support Matrix

Milestone: 32A Content Inventory and Coverage Matrix

This matrix tracks content completeness, unit coverage, and parity evidence separately. A row marked implemented means the simulator has local behavior for the surface; it does not imply seed-start parity unless the evidence columns say so.

## Status Values

| Status | Meaning |
| --- | --- |
| `implemented` | Behavior exists in `sts_core` and is expected to work within the stated scope. |
| `placeholder` | Behavior exists but is simulator-only or compatibility scaffolding with no parity claim. |
| `inventory_only` | Known surface is listed, but behavior has not been implemented. |
| `unsupported` | Known surface is out of current executable support and should be reported clearly when encountered. |
| `waived` | Known surface is intentionally excluded, with a named reason. |
| `not_in_scope` | Surface exists in the game but is outside Ironclad A0 core-game scope for this matrix. |

## Evidence Values

| Evidence | Meaning |
| --- | --- |
| `none` | Listed for inventory only; no supporting implementation/evidence yet. |
| `unit_only` | Covered by local unit or integration tests, but not proven against target-game behavior. |
| `wiki_reference` | Basic facts come from public references only; not enough for timing/RNG parity. |
| `source_backed` | Behavior was decoded from target bytecode/source or a trusted source-backed prior implementation. |
| `trace_backed` | Behavior appears in a captured CommunicationMod trace with a passing verifier scope. |
| `source_and_trace_backed` | Has both source/bytecode evidence and passing trace evidence. |
| `waived` | Evidence is intentionally not required because the row is waived or out of scope. |

## Columns

| Column | Description |
| --- | --- |
| Category | Broad surface type: card, relic, potion, monster, boss, encounter, event, room, reward, shop, rest, map, ascension, key, verifier, corpus. |
| Content ID / Key | Stable simulator/game identifier where one exists. Use `ContentId` for cards, `RelicKey`/`Relic` for relics, `Potion` for potions, and encounter/event/map keys for run surfaces. |
| Name | Human-readable surface name. |
| Surface Details | Compact metadata such as card type/rarity/cost/target, relic tier/pool/hook surfaces, potion rarity/targeting, act, ascension scope, or RNG streams. |
| Status | One of the status values above. |
| Unit Tests | `yes`, `partial`, `no`, or named focused test(s). |
| Seed-Start Trace | `yes`, `partial`, `no`, or named trace/test. |
| Evidence | One of the evidence values above. |
| Caveats | Short notes for unsupported branches, hidden state, RNG/timing uncertainty, or scope limits. |
| Source Files | Primary implementation or verifier files. |

## Inventory Guidance

- Card rows should be one row per `ContentId`, not one row per display name. Track definition presence, reward/shop/colorless pool reachability, upgrade mapping, legal action coverage, combat effect status, and trace mapping separately.
- Relic rows should be one row per modeled or referenced `RelicKey`/`Relic`. Track pool tier/order, spawn filters, acquisition surfaces, effect hooks, state counters, RNG streams, verifier mapping, and proof strength separately.
- Potion rows should be one row per `Potion`. Track rarity, pool index, combat/target requirements, active/passive behavior, RNG use, reward/shop/Entropic surfaces, selection UI, relic interactions, and verifier mapping separately.
- Run/world rows should be one row per distinct surface such as monster, boss, encounter key, room kind, event, event choice, reward screen, shop action, rest action, map topology, map path choice, ascension delta, verifier trace, or corpus boundary.
- Trace recognition and behavior implementation are different claims. A row may be trace-mapped but unsupported, implemented but unit-only, or source-backed without trace coverage.
- Placeholder, legacy fixed, and captured-branch behavior must remain visible in row caveats.

## Matrix

| Category | Content ID / Key | Name | Surface Details | Status | Unit Tests | Seed-Start Trace | Evidence | Caveats | Source Files |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| meta | n/a | Matrix schema | status/evidence vocabulary and row contract | implemented | no | no | unit_only | Schema only; content rows will be filled by follow-up M32A inventory workers. | `simulator/docs/content_support_matrix.md` |
