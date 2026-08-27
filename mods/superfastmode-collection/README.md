# SuperFastMode (collection fork)

Local fork of [Skrelpoid/SuperFastMode](https://github.com/Skrelpoid/SuperFastMode)
for random-fidelity collection throughput.

## Why

Upstream SuperFastMode multiplies most game delta, but forces **raw 1× delta** for:

- `CardCrawlGame.updateFade`
- `AbstractDungeon.update`
- monster death/escape animations

CommunicationMod will not set `ready_for_command` while
`AbstractDungeon.isFadingOut/In` is true, and combat end waits on death
animations. That is the post-combat “wait for fade” tax during collection.

## Fork changes

Dungeon fades and monster death/escape paths are **removed** from the raw-delta
exemption list so they use the global multiplied `getDeltaTime()`.

`AbstractDungeon.update` remains on raw wall-clock delta. Its only direct
`getDeltaTime()` call in desktop 1.0 advances `CardCrawlGame.playtime`, and that
value controls Secret Portal eligibility. Accelerated rendering must not change
which events are legal. Collection `.1` and `.2` multiplied this clock and are
therefore not valid evidence for time-gated event selection.

Gameplay action state machines are different: they run with a fixed synthetic
`1/60` delta. The game is configured without VSync and executes those canonical
60 Hz updates much faster than wall-clock 60 Hz, while remaining independent of
host frame-time spikes and `deltaMultiplier`.

This distinction is required for reproducible collection. At `deltaMultiplier=100`, target
`ExhaustAction` could expire in the same update that opened a hand-selection
screen, before its later `wereCardsRetrieved` update. Depending on whether the
opening frame took more than 2.5 ms, the selected card was either exhausted or
lost screen ownership and surfaced in discard at end of turn. The canonical
gameplay tick removes that frame-rate-dependent branch for all
`AbstractGameAction.tickDuration()` users and for the small audited set of
actions that subtract `getDeltaTime()` directly.

`GremlinMatchGame.update` / `updateMatchGameLogic` stay on raw 1× delta. The
match minigame’s flip timer and hitbox path are one-frame click sensitive;
100× plus software GL left `CHOOSE` accepted with no completing boundary.

Map-screen and many UI flicker mitigations from upstream are kept.

## Install

Replaces `mods/SuperFastMode.jar` in the STS install (same `modid`).

```powershell
powershell -ExecutionPolicy Bypass -File mods/superfastmode-collection/install.ps1
```

Or from WSL after ensuring `javac`/`jar` are on PATH and the STS path is mounted:

```bash
# see install.sh
./mods/superfastmode-collection/install.sh
```

Restart the game/watchdog after install. Existing SuperFastMode config under
`%LOCALAPPDATA%/ModTheSpire/SuperFastMode/` is reused (`deltaMultiplier=100`).
The installed manifest must report `1.0.9-collection.3` before collecting a
promotable trace.

## License

Upstream SuperFastMode license applies to forked sources (see `LICENSE` if present).
