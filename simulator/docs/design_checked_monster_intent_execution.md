# Checked Monster Intent Execution

Monster intent execution is an authoritative transition boundary. It must not
turn an unrepresentable imported counter, power, block value, damage total, or
move count into wrapped or saturated gameplay state.

`apply_monster_intent_with_card_rng` therefore evaluates an intent against
staged copies of the monster, player, card piles, and card RNG. It commits all
four only after damage scaling, debuffs, thorns, status generation, power
changes, and the move counter succeed. Any checked-arithmetic failure returns
`InvalidState` with every caller-owned input unchanged.

Weak and Vulnerable damage scaling uses exact rational arithmetic and truncates
once, matching the existing observable behavior without relying on a float cast
that could silently clamp an out-of-range value.

This boundary covers the generic single-monster intent executor. The surrounding
whole-monster-turn dispatcher, group buffs, summons, cleanup powers, and revival
helpers remain separate follow-up audit surfaces.
