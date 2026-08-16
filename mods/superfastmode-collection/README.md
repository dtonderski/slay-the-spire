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

## Fork change

Those paths are **removed** from the raw-delta exemption list so they use the
global multiplied `getDeltaTime()` like combat actions.

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

## License

Upstream SuperFastMode license applies to forked sources (see `LICENSE` if present).
