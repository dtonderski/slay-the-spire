# Secret Portal design

The source event is available only in the Beyond, only once, and only after
800 seconds of game play. The simulator previously had no playtime field and
disabled the event unconditionally.

This implementation adds a serialized `playtime_seconds` input to `RunState`.
It defaults to zero for existing snapshots and is intentionally not inferred
from floor number: source playtime is wall-clock state, not seeded gameplay
RNG. Integrations must record the target game's actual
`CardCrawlGame.playtime` value as an explicit transition input before event
selection. Wall-clock trace timestamps are not a substitute because the target
timer pauses while the game is backgrounded.

Taking the portal enters the selected act-three boss through the existing boss
combat/reward pipeline and sets a temporary Boss room override. This preserves
boss relic/chest behavior without fabricating a map node or silently changing
the current map topology. Declining the portal uses the normal staged event
leave flow.
