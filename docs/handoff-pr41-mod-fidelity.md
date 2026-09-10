# Handoff: PR #41 review and mod fidelity repair

## Task and current outcome

The user collected a new real-game trace dataset:
<https://huggingface.co/datasets/dtonderski/sts-random-traces-schema7>.
An overnight model raised replay passes from roughly 1/600 to 485/635 in PR #41:
<https://github.com/dtonderski/slay-the-spire/pull/41>.

The task has two parts:
1. Review which PR changes are legitimate simulator fixes versus compensation
   for SuperFastMode/CommunicationMod bugs; recommend what to keep/rework/remove.
2. Fix proven bugs in the mods themselves, then validate with fresh captures.
   Do not accommodate broken captures in the collector or simulator.

**Current verdict: do not merge PR #41 as-is.** Review and corpus replay were
performed, but the gameplay-source audit remains incomplete. **No simulator,
collector, or mod implementation changes were made in this session.** This
handoff is the only project-file change made by the parent. No PR was merged,
commented on, or pushed. No game installation was changed.

Work moved to a local machine because the review host has no game JARs, no
mounted game installation (`/mnt` is empty), and no live game. Java exists;
Maven was not found. The next session should implement and test, not merely
repeat an unbounded review.

## Safety and authority

Read `AGENTS.md`, `PROJECT_OVERVIEW.md`, `simulator/docs/research.md`,
`simulator/docs/verification.md`, `simulator/mods/README.md`, and
`simulator/tools/communication/README.md` before implementation.

- Existing captures are immutable. Never rewrite, truncate, or regenerate them
  to make replay pass.
- Replay must not hydrate, repair, or re-anchor from observed game state.
- Do not weaken comparisons or add seed/trace/corpus-specific behavior.
- Preserve natural gameplay ordering. Do not execute effects out of order and
  restore fields afterward to match an observation.
- A mod's existing workaround is not authoritative evidence of vanilla rules.
  Inspect the matching target game bytecode/source before porting behavior.
- Preserve raw dungeon playtime and explicit gameplay RNG streams. No arbitrary
  RNG burns or synthetic time inferred from post-state.
- Use the documented CommunicationMod bridge for live control, not handcrafted
  game socket commands. Coordinate with the owner before stopping collection,
  installing JARs, restarting the game, or taking live control.
- Explicitly select and verify a non-Astra model for every subagent. Never
  inherit Astra implicitly. Follow the local subagent protocol.

## Git state: do not accidentally use the dirty master checkout

Reviewed refs:
- Base: `1e224d6ba0893ea6abdce1323d7f2b18c4bdb036` (`master` at review time).
- PR head: `5f94dfeddbdb71f57e904337573351025d93db81`.
- PR branch: `fix/schema7-communicationmod-fidelity`.
- Review branch: `wt/trace-review-worktree-20260910`, based on the above base.

Review-host paths (not assumed to exist locally):
- Review worktree:
  `/home/davton/.local/share/pi-worktrees/20260910135956/trace-review-worktree-20260910`
- PR worktree:
  `/home/davton/.local/share/pi-worktrees/20260910042525/trace-repair-grok-worktree-20260910`
- Main checkout: `/home/davton/dev/slay-the-spire`.

Both review and PR worktrees were clean before this handoff was written.
However, the main checkout on `master` had **40 modified tracked files and
5 untracked files**, spanning simulator and RL work. Its HEAD was still
`1e224d6b`. Their origin is unknown; this review's recorded edit/write calls
only wrote reports outside the repository. Do not reset, stash, commit, copy,
or otherwise take ownership of those changes without the owner's direction.

Start local implementation in a clean, separate mod-fix worktree. Verify its
base explicitly. Do not merge PR #41 wholesale just to begin mod repair.
If local/remote refs have moved, inspect that delta before applying these
findings; line numbers below refer to the reviewed versions.

## Evidence: checks actually run at PR head

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | Fail |
| `cargo test -p sts_core --lib -- --test-threads=1` | 1,080 pass |
| `cargo test --workspace -- --test-threads=1` | Fails to compile `sts_env` |
| New schema-7 corpus | 635 traces: 485 pass, 118 divergent, 20 incomplete, 12 invalid; 878,921 actions |
| Reviewed permanent corpus | 433 traces: 400 pass, 33 divergent, 0 incomplete, 0 invalid; 609,760 actions |

Both corpus runs used `cargo run -p sts_verify --bin sts_verify -- <directory>`
from the PR repository root. No baseline full-corpus comparison was run in
this session: do not label every failure a newly introduced regression.
Incomplete/invalid traces are failures, not passes or permission to repair
payloads. Their exact causes are not all attributed.

Trace locations on review host:
- New corpus: `<PR worktree>/simulator/verification/corpus/random_traces_schema7`
  (about 18 GB).
- Permanent corpus:
  `/home/davton/dev/slay-the-spire/simulator/verification/corpus/permanent_traces`
  (about 13 GB).

Ignored traces/build artifacts do not follow Git. Transfer immutable captures
or download the dataset separately; preserve any supplied manifests/hashes.

## PR findings and disposition

### 1. P1: workspace integration does not compile

PR adds `pending_actions` to `CombatDecisionState::PotionCardReward` and
`ToolboxCardReward`, but the patterns in
`simulator/crates/sts_env/src/combat_observation.rs:931,944` omit it.
Fix integration before merge, preserving the fair observation boundary.
(The child report incorrectly called this file `action.rs`; the actual
compiler log names `combat_observation.rs`.)

Clippy also reports an unused `target_uniform_random_potion` import in
`sts_core/src/run/shop.rs:15`, and `manual_isolate_lowest_one` in
`sts_core/src/rng.rs:564`. The latter is in unchanged code and may be a
current-toolchain baseline issue; do not attribute it to this PR without proof.

### 2. P1: Match and Keep WAIT model is not a valid timing contract

`sts_verify/src/sim_real/replay.rs:433-444` interprets `WAIT n` as milliseconds
and calls `tick_match_and_keep_wait`; `sts_core/src/run/event.rs` introduces
`wait_remaining_ms` and an 800 ms pair timer.

CommunicationMod's `CommandExecutor.executeWaitCommand` calls
`GameStateListener.setTimeout(n)`. That API documents frames, and its timeout
is decremented on listener updates, with early-return paths that can postpone
it. This is not a declaration of elapsed wall-clock/gameplay milliseconds.
Simply converting frames to milliseconds is also not justified: Match and
Keep currently uses raw delta, and uncapped rendering is not a fixed 60 Hz clock.

**Remove/rework the unsupported replay-time assumption.** Fix the mod's
interaction/settlement boundary rather than inventing simulator timing from
collector WAIT values. Do not change mod WAIT units to make the PR correct.

### 3. Match and Keep choice behavior fails existing immutable evidence

Permanent trace `FIDL01966-p1966-2026-08-23T20-42-29-652Z-68806.jsonl`
diverges at step 101, `CHOOSE 9`, after `CHOOSE 0`, `CHOOSE 5`, `CHOOSE 9`.
Observed revealed names such as `dark shackles` disagree with PR card-slot
labels and omitted choices. Its final click at step 109 produces `leave`
without an explicit WAIT. Thirty of the permanent failures were categorized
as choice-label differences and three as deck/choice differences, all in
event contexts.

PR filtering in `run/event.rs:3876-3926` hides revealed cards/names while
unrevealed cards or waits remain. This is not supported by the reviewed trace
or current `CommunicationMod/ChoiceScreenUtils.java` choice-label projection.

**Important parent correction to the child review:** the child recommended
modeling current mod `finishPendingMismatchWait` behavior. Do NOT accept that
recommendation as vanilla authority. Existing `GremlinMatchGamePatch.java`
forcibly zeros timers, flips cards, clears selected-card fields, and invokes
`updateMatchGameLogic` reflectively outside the natural update path. These
are precisely suspect mod shortcuts that must be audited/replaced, not
translated into simulator rules to chase old trace pass rates.

### 4. P1: ordinary reward exit exposes an unsupported Return action

PR `run/reward.rs:2885-2904` creates a dismissable map overlay on ordinary
reward exits. `run/decision.rs:98-125` then advertises `ReturnFromMap`.

Permanent `FIDL01934-p1934-2026-08-23T19-44-26-672Z-39439.jsonl`, lines
492-493, shows combat reward `PROCEED` -> `MAP` without `return` in available
commands. In contrast, FIDL01966 step 110 (event Leave) does advertise return.
CommunicationMod only exposes map cancel when `dungeonMapScreen.dismissable`
is true (`ChoiceScreenUtils.java:202-218`). Preserve this distinction.

FIDL01934 passes replay (842 actions) despite the extra simulator action:
the trace never selects it and the map projection does not compare the full
command set. A replay pass therefore does not establish legal-action parity.
Similar PR shop/treasure branches need target-source inspection; they were
not independently proven wrong by this review.

### 5. P2: event schema validation was broadened indiscriminately

`sts_verify/src/trace.rs:877-920` discards `command_ready` and permits missing
or null `choice_list` for every EVENT state. The justification names Match
and Keep but the exception does not identify that event or a wait boundary.
Rework this broad relaxation; establish any allowed exception from an explicit
schema/protocol contract, not merely the presence of a bad capture.

### 6. Combat ordering inconsistency requiring target-source verification

Generic attack damage now queues Shell Parasite stun and Lagavulin wake
reactions (`combat/transition/damage_actions.rs:87-155`). Alternate paths
still apply them inline, including all-living damage
(`combat/transition.rs:2147-2152`) and random-enemy damage
(`damage_actions.rs:255-260`), plus other specialized damage paths.

This is inconsistent with the PR's own queued-reaction rationale and can
matter for multi-hit cards. Exact vanilla callback ordering was not verified
without the target JAR. Inspect the actual game action/monster methods before
unifying paths; no recorded divergence was causally attributed to this issue.

### Potentially salvageable changes, not blanket approval

The combat reviewer found no concrete blocker in several changes: card-in-use/
limbo handling, some Dual Wield/True Grit selector changes, Awakened One
first-death/Feed handling, Gremlin Mask opening Weak, and Prismatic card
registration/queues. These remain candidates to keep after source audit,
not certified parity. Potion discard and explicit shop/rest choice bindings
are also legitimate categories of adapter work, but the entire implementation
was not independently cleared. Separate these from timing accommodations.

## Mod investigation: what is known versus only proposed

Source roots:
- `simulator/mods/CommunicationMod/`
- `simulator/mods/superfastmode-collection/`

1. **Pending gameplay-effect coverage:** `GameStateListener` scans
   `effectList`, `topLevelEffects`, and `topLevelEffectsQueue`, for unfinished
   `ObtainKeyEffect` and `ShowCardAndObtainEffect`. A separate dungeon
   `effectsQueue` is a candidate missing queue. Verify that field/type and its
   producer/drain lifecycle against the actual target JAR before adding it.
   Then test both readiness and emitted pending-effect diagnostics. Do not
   indiscriminately block on decorative effects.
2. **Event countdown check:** readiness currently tests `event.waitTimer != 0`.
   A negative countdown would remain blocked. Inspect actual countdown/clamp
   behavior before calling this a proven reachable bug. A `> 0` pending
   predicate with positive/zero/negative tests is a candidate bounded fix;
   distinguish inherited timers from Match and Keep's shadowing private timer.
3. **Match and Keep:** audit/removal of forced timer/selected-card mutations
   needs natural click-consumption, mismatch resolution, obtain publication,
   and final-dialog settlement tests. An empty-choice boundary appears in
   new-corpus FIDL00415, but no new root-cause implementation was completed.
4. **Smoke Bomb:** inspect escape transition and active/queued actions, including
   WaitAction. Traces alone did not establish a safe missing guard or patch site.
5. **Recall/Dig:** existing campfire fences do not by themselves prove end-to-end
   settlement. Concrete replay leads: new FIDL00035 step109 `CHOOSE 2`, ruby
   false versus true; FIDL00046 step264 `CHOOSE 2`, REST versus COMBAT_REWARD
   with Gremlin Horn. Inspect those exact raw boundaries and source lifecycle.
6. **Dual Wield/hand retrieval:** a HAND_SELECT interaction with queued work can
   be a legitimate input boundary, not premature quiescence. Do not globally
   require empty queues when a screen genuinely owns input. New FIDL00232
   step1158 CONFIRM has a hand-order difference; establish whether it is a mod,
   simulator, or projection issue rather than changing indexing speculatively.
7. **SuperFastMode:** preserve the existing fixed 1/60 gameplay action tick and
   raw `AbstractDungeon.update` playtime clock. Global accelerated visual delta
   previously caused skipped ExhaustAction retrieval; the documented action
   tick fix is already present in collection.3, not a new fix from this review.
   Current Match and Keep delta exemptions/patch interactions need auditing.

## Local restart plan and acceptance

1. Identify the actual game installation, target build, ModTheSpire launcher,
   BaseMod, CommunicationMod, and SuperFastMode versions/configs. Record JAR
   hashes. Match the capture environment rather than silently substituting
   latest upstream dependencies.
2. Read each mod's README/build script. Required JARs are `desktop-1.0.jar`,
   `ModTheSpire.jar`, and `BaseMod.jar`. CommunicationMod's Maven POM currently
   expects them in sibling `simulator/mods/lib/`. SuperFastMode's script uses
   `STS_DIR` and `MTS_JAR` and supports `NO_INSTALL=1`.
3. Build without installing first. Inspect only targeted game packages/methods;
   keep searches in `tmp/decompiled-sts/` package-targeted as AGENTS requires.
4. Prove and fix one small mod settlement/lifecycle bug at a time. Start with
   pending-effect queue/timer hypotheses if target source confirms them.
   Add regression tests that fail for real premature-ready or stuck-boundary
   cases, not only tests of a new helper in isolation.
5. Audit Match and Keep's existing state-reset shortcuts separately; do not
   increase old-trace passes by encoding those shortcuts in Rust.
6. Get independent review of the actual mod diff. Coordinate installation and
   live runs with the owner; preserve/record build provenance for new captures.
7. Collect new immutable regression traces through the existing bridge, without
   collector compensation. Check effect completion, command identities and
   settlement fences, legal commands, selection ownership, and terminal paths.
   Compare multiple speeds/frame conditions where relevant. Unit tests and
   successful compilation are not real-game parity evidence.
8. Revisit PR #41 against proven rules; split/revise rather than merge wholesale.
   For any Rust/replay changes, run the full AGENTS checks and both corpora.
   Old bug-bearing captures may legitimately remain divergent; report why,
   never edit them or hide gameplay differences.

The owner has not authorized discarding unrelated dirty files, merging PR #41,
or silently replacing a live collection installation.

## Prior run artifacts (optional, review-host only)

Everything needed to restart is summarized above. Supporting logs are
`/tmp/combat-validation-{fmt,clippy,test,core-test,random-corpus,permanent-corpus}.log`.
Detailed child reports live under the review host's Pi session artifacts at:

`/home/davton/.pi/agent/sessions/--home-davton-.local-share-pi-worktrees-20260910135956-trace-review-worktree-20260910--/subagent-artifacts/outputs/cc07b77d-0442-419d-893b-a4d6d484de38/`

Files: `review/run-replay.md`, `review/combat-validation.md`, and
`implementation/mod-fixes.md`. Apply the parent corrections in this handoff
if a child recommendation conflicts with it.

The initial combat-review and mod-worker runs hit a 30-minute timeout. The
workflow was stopped; both worktrees were checked clean; same-protocol,
report-only resumes recovered their evidence. No implementation resulted.
Use bounded local work now that authoritative dependencies are available.

## Suggested opening prompt for the local chat

> Read `docs/handoff-pr41-mod-fidelity.md` and the required project docs. Continue
> the PR #41 review and mod fixes described there. First inspect local Git state
> and locate the exact installed game/mod JARs. Do not touch unrelated dirty
> work, merge PR #41, modify captured traces, or compensate in the collector.
> Work in a clean mod-fix worktree, prove the first bounded root cause against
> target source, implement/test it, then coordinate fresh real-game validation
> with me. The game installation is: [insert local path].
