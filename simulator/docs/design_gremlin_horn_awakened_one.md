# Gremlin Horn on Awakened One half-death (FIDL00378)

## Observation

Killing Awakened One form 1 with Uppercut (Gremlin Horn held) grants **+1 energy** and
**draws 1** (Seeing Red) while combat continues into form 2. Simulator skipped Horn
because `queue_monster_death_hooks` required `any(monster.alive)` after
`mark_awakened_one_half_dead` sets `alive = false`.

## Rule

Gremlin Horn fires when combat **continues** after a death, including:

- another living monster, or
- Awakened One in the half-dead phase (`awakened_one_is_half_dead`)

Same gate for immediate `apply_monster_death_hooks` and deferred
`pending_monster_death_relic_triggers`.

## Non-goals

Does not change Darkling half-death gold / true-fatal filters.
