# CommunicationMod

Local CommunicationMod source used to expose Slay the Spire state and accept
external commands.

Upstream: <https://github.com/ForgottenArbiter/CommunicationMod>

## Requirements

- Slay the Spire
- [ModTheSpire](https://github.com/kiooeht/ModTheSpire)
- [BaseMod](https://github.com/daviscook477/BaseMod)

Copy the built JAR into the ModTheSpire mods directory and configure its external
command to launch `simulator/tools/communication/trace_client.js`. The child process must
print `ready` followed by a newline, then exchange one-line JSON states and
commands over stdin/stdout.

The protocol advertises currently legal command families in each state. Common
commands are `START`, `PLAY`, `POTION`, `END`, `CHOOSE`, `PROCEED`, `RETURN`,
`KEY`, `CLICK`, `WAIT`, and `STATE`. Do not construct commands from this list
alone; use the current state's advertised commands and choices.

### Stable offer indices and executable choices

`choice_index_schema: 1` declares an additive selectability contract. The existing
`game_state.choice_list` stays the UI offer list in its original order: affordable
shop potions remain listed with Sozu/full slots, and full-belt reward potions stay
in their original positions. `game_state.selectable_choice_indices` is a strictly
increasing list of currently executable indices into that same list. It never
compresses/reindexes labels. `CHOOSE i` (and named choices) rejects an index not
in that list before touching the game. The `choose` family is unavailable when
none can be selected; the offer list remains observable.

The random collector requires this producer contract before `START`, validates
the index list, and samples every advertised executable command without potion
heuristics or exceptions for known hangs/divergences. Schema-6/7 replay still
binds commands against the original offer indices; no schema reinterpretation,
observed-label-based transition selection, or gameplay repair is required.
Shared synthetic Java/client/policy/verifier fixtures cover a potion before a
card reward, Sozu, full/open belts, and an all-unselectable reward list. They do
not establish real-game parity.

This project also uses `PROFILE`, a one-time non-gameplay response carrying
persistent profile inputs such as the Note card and final-act availability.
Collectors copy it into trace metadata before `START`; replay never infers it
from later observed state.

Bridge operation and collection are documented in
`simulator/tools/communication/README.md`.

## Gameplay settlement

Readiness waits for naturally completing gameplay work, not merely empty action
queues. This includes unfinished key/card acquisition and Recall/Dig effects in
all four dungeon effect collections, runnable unfinished Smith/Toke effects after
closing/cancelling their grid, active card Souls (which clear card
attributes and reapply hand powers), Match's own wait timer, and Smoke Bomb's
combat-to-reward room transition. Newly entered DEATH retains priority over the
Soul and Smoke checks. These readiness checks do not advance timers or mutate
game state. Smith/Toke effects pause while a screen is up, so legitimate user
interaction remains available; once the screen closes, readiness waits for the
old effect to finish before another campfire choice can hide the UI.

The class-level `@SpirePatch` added to `CampfireDigEffectPatch` **enables hooks
that were inactive on master**. Its block at `addCampfireChoiceData` and resume
at the authored `isDone` branch now fence Dig's natural lifecycle; this is a
behaviour change, not just diagnostics. Target bytecode and locator tests verify
the resume site (duration < 0). Fresh diagnostics-off live Dig coverage is still
required for this candidate; headless tests alone do not certify it.

The legacy `CampfireSmithEffectDurationPatch`, `RestRoomSmithSelectionPatch`,
and `ShopRoomPurgePatch` repair chain has been removed. The producer no longer
resets Smith duration/completion or manually applies pending upgrades in room
updates. Use a clean build so deleted patch classes cannot remain in the JAR.
For accelerated trials, pair this producer with the collection.8 SuperFastMode
candidate (which includes the collection.5 Smith/Toke change): those updates use
raw delta rather than amplified visual
delta, letting the original effects own selection, upgrade/purge processing, and
completion. Raw playtime and canonical 1/60 action ticks remain unchanged.
Diagnostics-off full captures at 100×, 1×, and SFM disabled exercise Smith and
Toke confirmation plus repeated cancellation/re-selection; these bounded results
do not clear unattended batches.

The serialized effect counts are selective, not a complete pending-work count:
for example, Souls are outside the effect lists. Zero counts alone do not prove
settlement. Diagnostics remain opt-in; immutable live traces are required to
validate lifecycle behavior beyond the headless readiness fixtures. End-turn
external mutations are allowed to settle on an already-open screen only for
active CodexAction/CARD_REWARD or RetainCardsAction/HAND_SELECT owners. Well-Laid
Plans' RetainCardPower queues the latter. These are interaction-ready boundaries,
not quiescence: queued follow-up work and endTurnQueued remain untouched.

Raw-clock ExhaustCardEffect resets still require **same-command** exhaust then
retrieval coverage. Waiting for effects between commands cannot prove that
intra-command ordering, and it remains a live certification blocker.

## Campfire diagnostics (opt-in, observation-only)

The diagnostic build is disabled unless `communicationmod.diagnostics.path` is
set to an absolute writable path. Relative paths are rejected. It writes
bounded JSONL to a collision-safe sibling path (for example `window.jsonl.1`)
and never truncates an existing file. The optional
`communicationmod.diagnostics.maxRecords` JVM property defaults to 512 records
(invalid or non-positive values use the default). The final record is a
`{"stage":"truncated","truncated":true,"reason":"max_records"}` marker
when the cap is reached, so incomplete evidence is distinguishable; the cap
includes that marker. `communicationmod.diagnostics.tailUpdates` controls the
finite post-publication/update tail in actual dungeon-post updates and defaults
to 32 (invalid or non-positive values use the default). Tail exhaustion emits
`{"stage":"truncated","truncated":true,"reason":"tail_updates"}` instead,
so the two stopping causes remain distinguishable.

Diagnostics are gated to a selected `RecallOption`, `DigOption`, or `SmithOption` window and
record command/effect/update seams, listener state, screens/room/rewards, all
four dungeon effect collections (including `duration`, `starting_duration`, and
`is_done`), and delayed key/reward transitions. A card reward `CHOOSE` also
opens a bounded diagnostic window for `FastCardObtainEffect`, solely to observe
delayed card acquisition. `communicationmod.diagnostics.minCommandExecutionSeq`
optionally delays opening the output until a chosen command-counter threshold
(default zero); it does not delay or change gameplay. Smith observations include
effect identities, current command identity/counter and campfire visibility.
Publication does not end a campfire/card window:
the window closes only after a relevant natural effect has been observed and
no relevant pending effect remains, or at the finite tail cap. The game-pre
and dungeon-post callbacks are both recorded, but only dungeon-post callbacks
spend tail updates. They do not call delta-time, update an effect, clear a
queue, or affect readiness.

For slow 1x attribution, use a fresh output file per target and allow the
shared file budget for earlier card windows. Recommended bounded settings are:

```text
-Dcommunicationmod.diagnostics.maxRecords=4096
-Dcommunicationmod.diagnostics.tailUpdates=128
```

These are bounded settings, not a correctness fence: observer writes/flushes
can perturb frame timing and must be reported.

Run the normal baseline with no diagnostics property (or with
`-Dcommunicationmod.diagnostics.path=`). For a separate diagnostic JAR, build
without installing it into the game:

```bash
mvn test package
java -Dcommunicationmod.diagnostics.path=/tmp/recall-dig.jsonl \
  -Dcommunicationmod.diagnostics.maxRecords=512 -jar ModTheSpire.jar
```

The second command is illustrative: use the normal ModTheSpire launcher and
place the separately identified diagnostic artifact in a disposable test mods
location; do not overwrite an installed CommunicationMod or the reviewed
candidate. Diagnostic output is not protocol stdout.
