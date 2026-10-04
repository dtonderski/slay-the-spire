# Project knowledge

Start with the [objective and fair-state boundary](../PROJECT_OVERVIEW.md).
[Agent rules](../AGENTS.md) govern implementation and documentation maintenance.
This page routes readers to canonical documents; it does not duplicate them.

## Current contracts and workflows

| Need | Read |
|---|---|
| Simulator layout, build, and checks | [Simulator README](../simulator/README.md) |
| Fair observation/action boundary | [Fair API](../simulator/docs/fair_api.md) |
| Hidden-state exclusions and public-information gaps | [Boundary audit](../simulator/docs/fair_observation_hidden_state_audit.md) |
| Python usage and numeric transport | [Python API](../simulator/docs/python_api.md), [first-contact example](../rl/examples/README.md) |
| Replay rules and evidence | [Verification](../simulator/docs/verification.md), [corpus](../simulator/verification/corpus/README.md) |
| Real-game control and collection | [Communication bridge](../simulator/tools/communication/README.md), [support mods](../simulator/mods/README.md) |
| Synthetic training and evaluation | [RL README](../rl/README.md) |
| Loadout data and sampling tools | [RL tools](../rl/tools/README.md) |
| Interactive policy inspection | [Combat explorer](../rl/combat_explorer/README.md) |

## Durable findings and decisions

- [Parity research](../simulator/docs/research.md): hard-to-recover target-source
  findings. Source-backed rules are not automatically trace-validated behavior.
- [Project history](project_history.md): major decisions, rejected approaches,
  and settled experiments—not a description of every current implementation.

Code, tests, and immutable traces retain routine implementation and regression
evidence. A documentation summary never replaces them or establishes parity.

## Research proposals

[Literature review](literature_review/README.md) separates proposed fair belief
search from privileged-teacher research. These are reading material and possible
experiments, not the current training architecture or an approved work queue.

## Historical handoffs

- [Combat explorer implementation handoff](archive/combat_explorer_implementation_plan.md):
  original product scope and recommendations. The explorer README above documents
  the implemented tool; the handoff is not maintained as a second API guide.

Keep active knowledge small: update the canonical document near its owner, link
instead of copying, and retire obsolete guidance. Add a page only for durable
knowledge that does not fit an existing home. Git holds routine change history.
