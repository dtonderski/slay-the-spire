# Asynchronous trace/repair loop

The current high-throughput fidelity workflow uses random legal actions rather
than SlayTheData guidance:

```text
random game collector -> immutable full trace -> verification queue
                                                 |
                                      strict verifier workers
                                                 |
                              first divergence -> minimized trace
                                                 |
                                    deduplicated repair task
```

`run_random_fidelity_campaign.js` runs the game-facing collector indefinitely.
Set `STS_RANDOM_DEFER_VERIFICATION=1` so collection never waits for replay.
`random_fidelity_verifier.js` consumes completed traces independently. A trace
after its first divergence is not parity evidence, but retaining the full trace
allows a repair to replay farther and expose the next divergence.
Deferred traces are created with exclusive writes under `traces/`; their names
contain the seed, policy seed, collection timestamp, and process id. Metadata
schema 1 records `source_version` (set `STS_RANDOM_SOURCE_VERSION` for a build
identifier), so restarting a seed cannot overwrite earlier evidence.
Every distinct unexpected simulator/real-game divergence is also minimized and
added to `simulator/verification/corpus/permanent_traces.json`. Manifest
promotion is serialized across verifier workers, so parallel verification
cannot drop a permanent regression entry. Typed gameplay divergences are
permanent too; only verifier crashes and trace-integrity failures are excluded.
The directory lock is stale-recovering, so killing a promoter during its
critical section cannot permanently block later promotion.
During rolling upgrades, run `random_fidelity_corpus_promoter.js` beside the
verifiers. It continuously reconciles repair tasks into the permanent corpus,
so a still-running older verifier cannot bypass promotion while workers are
being replaced. Queued/in-progress tasks retain an exact `expected_boundary`;
after a successful full-trace recheck resolves a task, the reconciler changes
that entry to a retained-prefix expectation at the minimized trace endpoint.
The batch corpus gate therefore accepts a known unresolved witness but requires
resolved fixes to replay cleanly.

Verifier workers use deterministic queue sharding when more than one is needed.
The normal topology uses one verifier with `STS_RANDOM_VERIFY_WORKERS=1` and
`STS_RANDOM_VERIFY_WORKER_INDEX=0`. It appends results to
`verification_results.jsonl`; per-fingerprint locks serialize updates under
`repair_tasks/`. One verifier commonly uses about 0.8-1 GB of RAM, so worker
count is an explicit resource choice. In measured collection runs, one verifier
completed a multi-minute trace in roughly 6-26 seconds and stayed ahead of the
single collector.

`random_fidelity_repair_queue.js` owns the repair-task lifecycle. Use
`status` to inspect backlog. Prefer `claim-ready WORKER` for unattended
dispatch: it replays queued evidence first, resolves stale fingerprints already
fixed by another repair, and returns only a currently reproducing task.
Use `claim WORKER [FINGERPRINT]` when deliberately selecting exact evidence,
then `recheck FINGERPRINT WORKER` after its focused tests pass. Recheck
runs the original full trace: it resolves the old fingerprint only when parity
is reached or replay advances to a newly deduplicated fingerprint; otherwise it
returns the task to the queue. Use `release` when an attempt is blocked before
recheck. Claims and verifier occurrence updates share the same per-fingerprint
locks, so a newly observed occurrence cannot be lost while a task changes
state.

## Start the random fidelity pipeline

Start Slay the Spire with the workshop ModTheSpire 3.30.3 and the five required
mods. From the game directory, the unattended Windows launch used by this
repository is:

```bash
./jre/bin/java.exe -jar \
  'D:\SteamLibrary\steamapps\workshop\content\646570\1605060445\ModTheSpire.jar' \
  --skip-launcher --skip-intro \
  --mods basemod,CommunicationMod,abandon-run-control,superfastmode,verification-bootstrap
```

For unattended runs started by Codex, do not leave either the game or pipeline
attached to a managed command terminal, and do not create a new tmux server
from that terminal. The command runner cleans up both direct and daemonized
descendants. Hand ownership to the user service manager and enable lingering:

```bash
loginctl enable-linger "$USER"
systemd-run --user --unit=sts-game \
  --property=Restart=always --property=RestartSec=5s \
  --property=KillMode=control-group --property=TimeoutStopSec=10s \
  --working-directory=/path/to/slay-the-spire \
  --setenv=STS_GAME_DIR=/path/to/SlayTheSpire \
  --setenv=STS_BRIDGE_SESSION_DIR=/path/to/slay-the-spire/tools/communication/session \
  /absolute/path/to/node \
  /path/to/slay-the-spire/tools/communication/random_fidelity_game_watchdog.js
```

Start the pipeline command below as a second `systemd-run --user` unit named
`sts-fidelity`, with `Restart=always`, its environment passed through
`--setenv`, and an absolute Node executable. Inspect the services with
`systemctl --user status sts-game sts-fidelity` and
`journalctl --user -u sts-game -u sts-fidelity`. The game watchdog treats both
a missing TCP bridge and a command pending for more than 30 seconds as a hang,
then kills the stale Windows interop wrapper so systemd can relaunch it.

Do not use the old game-directory ModTheSpire 3.6.3 or BaseMod jar; those are
incompatible with the current game. Once `live-trace bridges list` reports
`connected: true`, the recommended single-command data-plane supervisor is:

```bash
STS_BRIDGE_SESSION_DIR=tools/communication/session \
STS_RANDOM_OUTPUT_DIR=random_traces_loop \
STS_RANDOM_LOG_ACTIONS=0 \
STS_STARTING_HP=10000 \
STS_SEEN_BOSSES_PATH=/path/to/SlayTheSpire/preferences/STSSeenBosses \
STS_RANDOM_SOURCE_VERSION=<build-id> \
STS_RANDOM_VERIFY_WORKERS=1 \
STS_RANDOM_REPAIR_WORKERS=2 \
STS_RANDOM_REPAIR_MODEL=gpt-5.6-luna \
STS_RANDOM_REPAIR_EFFORT=xhigh \
STS_RANDOM_UV_BIN=/absolute/path/to/uv \
node tools/communication/run_random_fidelity_pipeline.js
```

It starts and independently restarts the indefinite campaign, corpus promoter,
verifier shards, and repair lanes. Service logs are under `pipeline_logs/`;
the atomic `pipeline_status.json` records PIDs, restart counts, and current
service state. Each repair lane uses `claim-ready`, runs one ephemeral Codex
process at a time, releases a claim if Codex exits before recheck, and
immediately takes the next reproducing task. Per-attempt model logs are under
`repair_agent_logs/`; current lane state is under `repair_worker_status/`.
The default repair configuration is two `gpt-5.6-luna` workers at `xhigh`
reasoning effort and priority service tier.
Repair candidates are never applied directly from their lane. The serialized
integrator snapshots the current permanent corpus, applies the candidate in a
fresh staging workspace, builds an isolated `sts_verify`, replays the focused
witness, and runs the broad corpus gate. A candidate may only be promoted when
it introduces no failing trace outside the latest authoritative baseline.
Set `STS_RANDOM_UV_BIN` explicitly for service-manager launches whose minimal
`PATH` does not include the user installation of `uv`. If the integrator is
restarted during a gate, it returns stranded `gating` candidates to its
serialized queue before resuming.
On campaign restart, the next policy seed is inferred from immutable trace
filenames and `skipped_policy_seeds.jsonl`, so a supervisor restart cannot
silently return to seed 1 or a known pathological seed. In indefinite mode,
three consecutive collector failures are recorded in `campaign_failures.jsonl`
and cause that policy seed to be durably skipped; override the threshold with
`STS_RANDOM_FAILURES_PER_SEED`. Repair
lanes likewise release any claim left under their stable worker identity before
claiming new work.

To operate the components separately, run the collector indefinitely:

```bash
STS_BRIDGE_SESSION_DIR=tools/communication/session \
STS_RANDOM_OUTPUT_DIR=random_traces_loop \
STS_RANDOM_MAX_RUNS=0 \
STS_RANDOM_DEFER_VERIFICATION=1 \
STS_RANDOM_LOG_ACTIONS=0 \
STS_STARTING_HP=10000 \
STS_RANDOM_SOURCE_VERSION=<build-id> \
node tools/communication/run_random_fidelity_campaign.js
```

Run one verifier process:

```bash
STS_RANDOM_OUTPUT_DIR=random_traces_loop \
STS_RANDOM_VERIFY_WORKERS=1 \
STS_RANDOM_VERIFY_WORKER_INDEX=0 \
node tools/communication/random_fidelity_verifier.js
```

If verification later falls behind collection, deterministic sharding supports
raising the worker count and starting one process per index. Also run the
idempotent corpus reconciler; this is required during rolling verifier upgrades:

```bash
STS_RANDOM_OUTPUT_DIR=random_traces_loop \
node tools/communication/random_fidelity_corpus_promoter.js
```

The collector, verifiers, reconciler, and repair lanes are independent
long-running processes, so model latency never blocks gameplay.
The campaign awaits each collector through an asynchronous child lifecycle;
this avoids the WSL/DrvFS synchronous-spawn cleanup failure that can otherwise
leave a live collector behind while the campaign mistakenly retries its seed.
Collector acquisition also cancels an old queued command after its controller
has disappeared and the five-second stale threshold has elapsed.

## Trace and task schemas

A completed trace is an immutable JSONL file under `random_traces_loop/traces/`.
Its first record is metadata schema 1:

```json
{
  "type": "metadata",
  "schema": 1,
  "source": "communication_mod",
  "client": "tools/communication/random_fidelity_collector.js",
  "source_version": "<build-id>",
  "collection": {
    "policy_seed": 82,
    "game_seed": "FIDL00082",
    "starting_hp": 10000
  }
}
```

The remaining records are ordered `action`, `state`, and `error` observations.
`verification_queue.jsonl` points to complete immutable traces.
`verification_results.jsonl` records strict replay disposition, elapsed time,
first boundary, fingerprint, and minimized/permanent trace paths.

Each `repair_tasks/<fingerprint>/task.json` is schema 1 and contains:

- `status`, timestamps, and the seed-independent fingerprint;
- normalized boundary, reason, first diff, and diff lines;
- all full/minimized trace occurrences;
- the current worker, claim time, attempt count, and completion note.

The lifecycle is:

```text
queued -> in_progress -> resolved
   ^          |
   +----------+  release, failed recheck, or explicit reopen
```

`recheck` resolves a task only when its original full trace reaches strict
parity or advances to a different deduplicated fingerprint. The new fingerprint
becomes its own queued task.

## Repair worker contract

Claim work before spawning an agent:

```bash
STS_RANDOM_OUTPUT_DIR=random_traces_loop \
node tools/communication/random_fidelity_repair_queue.js claim-ready luna-1
```

Give the worker this contract, substituting the returned fingerprint and worker:

```text
Read AGENT_RULES.md and RESEARCH.md before RNG/action-queue work. Diagnose the
first divergence from the claimed minimized/full trace. Implement the smallest
generic simulator fix: no seed-specific branches, observed-state hydration,
comparison weakening, or unrelated mechanics. Add focused regression coverage
where appropriate and run focused tests. Preserve unrelated dirty-worktree
changes and do not run Git state-changing commands. Finally run:

STS_RANDOM_OUTPUT_DIR=random_traces_loop node
tools/communication/random_fidelity_repair_queue.js recheck
<fingerprint> <worker>
```

The fast promotion gate is focused regression coverage plus full replay of the
original trace through `recheck`. Formatting, clippy, and the broad workspace
corpus are batched across compatible repairs; a task is never considered fixed
merely because a narrow unit test passes.

Before a repaired batch is promoted into a collector build, stop repair writers
and run from the repository root:

```bash
STS_RANDOM_OUTPUT_DIR=random_traces_loop \
node tools/communication/random_fidelity_batch_gate.js
```

The gate runs the parallel permanent-corpus replay through `uv`, appends its
result to `random_traces_loop/promotion_checkpoints.jsonl`, and converts failed
trace entries back into deduplicated repair tasks. Only a passing checkpoint
may be promoted.

## Demonstrated path

FIDL00082 produced an immutable 1,082-action trace. A verifier found fingerprint
`6bb06bc1b46cc683` at action 52, minimized it, created the deduplicated repair
task, and the reconciler added
`random-fidelity-6bb06bc1b46cc683.jsonl` to the permanent manifest. The queue
then claimed it for `luna-3`. At that point the verification backlog was zero,
demonstrating that collection, verification, permanent retention, and repair
dispatch all advance independently.

The older SlayTheData-guided supervisor remains described below for historical
and specialized guided collection use. It is not part of the current random
fidelity loop.

`repair_loop.js` is the outer supervisor for high-throughput fidelity
collection. It uses the resident `live-trace slaythedata agent` protocol so a
worker can finish an attempt and request the next one without starting a new
Rust process for every decision.

The normal collector lane never waits for repair:

```text
live-trace agent workers -> archive trace -> queue/dedupe task -> next_run
                                      \
                                       -> optional Codex repair + verification
```

Unsupported boss transitions are the single exception. They are rare, dense
sources of live data, so that worker pauses while a repair is made, verified,
and built. The supervisor then resumes the same CommunicationMod game and
session trace with the verified binary. Other divergences continue to use the
asynchronous lane.

Each worker gets its own bridge, journal, and repair-packet path. Use one
CommunicationMod/bridge process per worker when running in parallel. A single
bridge is the safe default.

## Queue-only collection

Build the native binary first, start Slay the Spire and CommunicationMod, then
run from the repository root:

```bash
node tools/communication/repair_loop.js run \
  --bridge communication-mod \
  --output-root /mnt/d/dev/slay-the-spire/live_traces_loop \
  --trace-root /mnt/d/dev/slay-the-spire/live_traces_loop/sessions \
  -- \
  --ascension 0 --victory --min-floor 51 --target-floor 60
```

Collection is indefinite by default and continues until it receives a shutdown
signal. Bridge/backend failures use exponential retry backoff (one second up to
thirty seconds) so a transient game reset, bridge restart, or backend outage
does not terminate the service. Use `--runs N` only for bounded tests and
demonstrations.

The supervisor archives every session trace under `traces/`. Fidelity and
mapping failures are grouped by a seed-independent fingerprint under `tasks/`.
Archived trace and occurrence names include a timestamp and nonce, so restarting
the supervisor cannot overwrite prior evidence. `loop.jsonl` is the complete,
buffered machine-readable event stream; the console prints only lifecycle
events to avoid backpressuring game workers. Pass `--source-version` (or set
`STS_REPAIR_LOOP_SOURCE_VERSION`) when the collector binary comes from an
uncommitted build; otherwise the supervisor records the repository HEAD.

Combat beam search is bounded to 250 ms by default, including when collection
arguments are supplied after `--`. Override it explicitly with
`--combat-search-time-budget-ms` when stronger but slower play is more important
than collection throughput.

For parallel collection, provide one bridge id per worker:

```bash
node tools/communication/repair_loop.js run \
  --workers 2 \
  --bridge communication-mod-1 \
  --bridge communication-mod-2 \
  --bridge-session-dir /path/to/session-1 \
  --bridge-session-dir /path/to/session-2 \
  -- ...
```

Each worker also gets a separate live-trace session-trace subdirectory. Do not
point multiple workers at one CommunicationMod session directory.

## Codex repair lane

Repair execution is opt-in because it needs an isolated worktree. Prepare that
worktree separately, then point the supervisor at it:

```bash
node tools/communication/repair_loop.js run \
  --bridge communication-mod \
  --repair-agent codex \
  --repair-cwd /path/to/isolated/simulator-worktree \
  -- ...
```

One Codex process uses the configured isolated worktree at a time. Each task
receives a packet and prompt, writes `agent.log`, then the supervisor runs the task's
verification gate and writes `verification.log`. A task reaches `verified`
only when formatting, clippy, workspace tests, and strict parity for an
archived failing trace pass. The supervisor does not merge or promote a repair;
that remains an explicit review step.

The repair prompt enforces the project invariants: no seed-specific branches,
no observed-state hydration, and no weakening of comparisons. A repair task is
deduplicated by blocker kind plus normalized first-difference text, so repeated
failures keep producing trace occurrences without launching duplicate agents.

## Unsupported boss repair lane

Unsupported boss failures default to `--boss-repair-agent codex`. The active
worker is stopped without abandoning the game, the immutable failing trace is
handed to Codex, and the supervisor serializes boss repairs so two workers
cannot modify one repair worktree concurrently. Use
`--boss-repair-cwd /path/to/worktree` to select the worktree; the repository
root is the default. Use `--boss-repair-agent queue` to pause and preserve the
session without launching Codex.

Before the repair starts, the supervisor records the workspace test baseline.
Before a boss repair becomes the active collector binary, the gate then runs:

1. formatting;
2. strict parity against the archived failing trace;
3. workspace clippy with warnings denied;
4. the full workspace test corpus, rejecting every newly failing test while
   tolerating an explicitly recorded pre-existing red baseline;
5. a fresh `live-trace` build.

The exact built binary is then selected for the one-shot
`slaythedata resume` command and all later attempts. A resumed session retains
the loop's bounded combat-search configuration.

The live trace contains a `fidelity_recheck` marker before repair resumption.
This marker supersedes only an earlier verifier-generated `fidelity_lost`
diagnostic and forces a complete replay with the repaired simulator. It does
not remove observations, alter actions, hydrate simulator state, or weaken
comparison. If the repair is incomplete, the full replay loses fidelity again
and the session remains paused.

## Operational rules

- Collectors stay on the current known-good build while asynchronous repairs
  are in flight. A fully verified unsupported-boss repair is promoted to the
  active collector binary.
- A bridge/backend failure backs that worker off and retries; simulator fidelity
  and guided mapping failures schedule repair and continue with the next source
  run.
- The current Rust `live-trace` binary must be built before starting the loop.
- The UI is an operator console, not the inner debugging loop. Use the verifier
  and archived traces for repair iteration.
