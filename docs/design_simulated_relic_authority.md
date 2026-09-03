# Simulated relic authority

## Problem

Seed-start simulated projections currently merge `RunState::relics` with a
verifier-owned carry list. The carry began as a way to keep Neow's Lament
visible after its three-combat counter reached zero and to retain its position
before later pickups. Although the list is normally derived from earlier
simulated transitions, it is state outside the authoritative simulator and is
accepted by every simulated screen projector. A projector can therefore emit a
relic identity or order that is absent from `RunState`.

## Contract

`RunState::relics` is the sole ordered authority for stable simulated screen
projections whenever a core run state exists.
Neow's Lament is a normal core relic identity acquired after the current
starter relic and retained after its effect is spent, matching captured traces.
`neow_lament_combats_remaining` remains the separate gameplay counter. A
positive counter without the relic identity is invalid core state.

Reusable simulated screen projectors derive relic identity and order directly
from `RunState::relics`. They do not accept an observed or verifier-carried
relic list. Starter-relic replacement remains core behavior and therefore
preserves the replaced slot without projection repair.

Inline Neow projections use the typed Ironclad starter identity until a core
run state exists, then derive identity and order from that state. Transient
boss-relic overlays snapshot the pre-command core relic projection explicitly;
they do not maintain a second verifier-owned relic authority.

## Snapshot schema

Current snapshots are schema 8 only. Restore requires that exact schema and
does not migrate older bytes. Historical schema-seven Neow identity repair is
not applied at load time; git history is the archive. Current-schema snapshots
and raw states with a positive counter but no Neow's Lament identity fail
validation. Duplicate identity remains invalid under the existing owned-relic
invariant. Simulated projection never invents, reorders, or carries relics to
make an observation match.
