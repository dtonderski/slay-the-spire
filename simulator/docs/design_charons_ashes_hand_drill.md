# Charon's Ashes THORNS can trigger Hand Drill

## Source behavior

`CharonsAshes.onExhaust` `addToTop`s `DamageAllEnemiesAction` with
`DamageType.THORNS`. That damage hits block. When block goes from positive
to zero, `AbstractCreature.brokeBlock` notifies `HandDrill.onBlockBroken`,
which `addToBot`s `VulnerablePower(target, 2, isSourceMonster=false)`.

FIDL01673: Runic Pyramid keeps Ghostly Armor until end of turn. Ethereal
exhaust fires Ashes into Writhing Mass's 3 Malleable block, Hand Drill
applies 2 Vulnerable, and the following monster-turn tick leaves 1.

## Non-goals

- Do not treat Ashes as HP_LOSS (it does not ignore block).
- Do not trigger Malleable on THORNS.
