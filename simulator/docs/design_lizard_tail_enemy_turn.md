# Lizard Tail during an enemy turn

Lizard Tail resolves immediately when damage makes the player lethal. The
enemy action queue then continues; revival is not an end-of-turn repair.

The combat transition therefore owns the one-shot availability flag while a
combat is active. `RunState` seeds that flag from its persistent
`lizard_tail_used` value and synchronizes consumption back after each combat
action. Monster-turn processing revives immediately after a lethal monster
action and continues with later monsters. Mark of the Bloom suppresses the
revival. Fairy in a Bottle remains run-layer handling until potion inventory is
represented inside combat transitions.
