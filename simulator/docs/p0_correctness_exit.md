# P0 correctness exit audit

Date: 2026-07-22

This note records the bounded correctness off-ramp from the simulator tech-debt
program. It is not a claim that all simulator debt is complete. The exit
criterion is narrower: there is no known remaining production path that turns
missing, unsupported, observed, or fabricated authority into a false verifier
pass or a plausible substitute simulation result.

## Audit result

- Verifier simulation is driven from pre-state plus typed core actions. The
  production verifier contains no observed-to-simulator state assignments, no
  alternative-outcome comparator, and no known-card/unknown-card diff filter.
  Simulated projection functions take simulator state rather than observed
  post-state. Observation-derived legacy reconstruction helpers are test-only.
- Verification outcomes are typed as complete pass, retained-prefix pass,
  expected boundary, invalid input, or failure. Assessment rejects unexpected
  diffs, unsupported transitions, ignored tails, inconsistent or unexpected
  boundaries, malformed input, incomplete or duplicate action dispositions,
  rejected actions in complete traces, missing terminal state, and unresolved
  transient assertions.
- Combat comparison exclusions are limited to explicit visibility contracts:
  Runic Dome intent hiding, missing CommunicationMod debug move IDs, dead-state
  fields that cannot affect another decision, and transient frames that must be
  reconciled at a later stable frame.
- Encounter generation and monster conversion reject missing spawns, empty
  encounters, unknown content, unsupported powers, unsupported acts, pending AI
  rolls, and the remaining approximate Act 3/4 intent definitions. Production
  combat entry does not substitute fixtures or representative monsters.
- `CombatRngState` contains four mandatory RNG streams and is flattened into
  `CombatState`; missing streams fail deserialization. Authoritative combat
  mechanics no longer use seed-zero, first-item, unshuffled, or no-roll
  fallbacks.
- Imported grid state validates its phase/event owner, matching relic owner,
  canonical deck-derived payload, generated event payload, Calling Bell curse,
  Pandora's Box RNG replay, Falling RNG provenance, and bounded remove/transform
  counts. Mandatory relic grids cannot be cancelled, and confirmation-only
  grids cannot be selected.

The final grid slices are committed as:

- `47b08c462` - validate relic-created grid authority
- `4fccae26c` - validate card-grid action contracts

Both slices passed formatting, strict workspace Clippy, the full workspace test
gate, all 36 corpus tests, snapshot regressions, and permanent/fidelity replay.

## Explicitly deferred debt

The following work remains useful but is not required for this P0 exit:

- exhaustive reachability validation for every serialized run/combat field and
  causal-history modeling for already-owned relic effects;
- further snapshot schema migration and removal or labeling of unchecked debug
  imports;
- one consolidated core action API across every Python, verifier, and live path;
- relic/card/reward metadata deduplication and valid-by-construction decision
  enums;
- typed SlayTheData guidance persistence and binder decomposition;
- dead-code/privacy cleanup, fixture relocation, generated status reporting,
  hotspot splitting, and test-module moves.

These items should be handled as reviewable P1/P2 work when they directly
support product development. They must not be described as remaining known
false-pass or substitute-state P0 defects without new evidence.
