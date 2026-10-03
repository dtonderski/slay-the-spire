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

`AbstractImageEvent.update` uses raw delta in collection.6. The installed target
shows an image-event dialog only when its wait timer crosses strictly below zero,
but stops decrementing at zero. Multiplied delta can land exactly on zero and
permanently suppress the dialog; the raw target clock restores the target's
natural transition behavior without changing timer state or synthesizing choices.

`ShowCardAndObtainEffect.update` uses raw delta in collection.7. Omamori marks a
blocked curse effect done in its constructor, but the game still invokes one
update before removal. At target delta that update leaves the normal obtain
duration positive; 100x delta can cross below zero and add the curse despite
Omamori. Keeping this effect on the raw clock preserves its authored done/removal
ordering without deleting a card or restoring the relic afterward.

`ExhaustCardEffect.update` uses raw delta in collection.8. Its visual countdown
also performs `resetAttributes()` on the exhausted card object; the same object
can later be retrieved. CommunicationMod must wait for this natural completion
before publishing a settled command boundary. Neither patch manually resets a
card or changes the authored effect ordering within an active action chain.

`CampfireSmithEffect.update` and `CampfireTokeEffect.update` use raw delta in
collection.5. Their target implementations open a grid below duration 1 and
complete below 0; amplified delta can cross both thresholds in one frame and
discard the effect that should consume the later selection. Exempting those
calls preserves original timer/selection ownership instead of resetting fields
or applying upgrades/purges from another room. The shared campfire UI hide timer
retains its existing clock; slowing it independently would change ordering
relative to other accelerated options. CommunicationMod separately waits for
unfinished Smith/Toke work after cancellation while permitting an open grid.

Map-screen and many UI flicker mitigations from upstream are kept.

## Install

Replaces `mods/SuperFastMode.jar` in the STS install (same `modid`).

From WSL, after ensuring `javac`/`jar` are on `PATH` and the STS path is
mounted:

```bash
NO_INSTALL=1 ./simulator/mods/superfastmode-collection/install.sh
```

This builds/tests the candidate without changing the installation. Omit
`NO_INSTALL=1` only for an explicitly authorized deployment. The build uses `uv`
for its Python audits. Restart the game manually after an authorized install. Existing SuperFastMode config under
`%LOCALAPPDATA%/ModTheSpire/SuperFastMode/` is reused (`deltaMultiplier=100`).
The current candidate manifest reports `1.0.9-collection.8`. Pair it with the
producer that removes the legacy Smith state-repair patches and fences Toke. Deployment alone
is not collection qualification: fresh staged collection gates remain required.
Previously captured payloads and their original artifact identities stay intact.

## License

Upstream SuperFastMode license applies to forked sources (see `LICENSE` if present).
