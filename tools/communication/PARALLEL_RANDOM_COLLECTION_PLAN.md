# Parallel random-fidelity collection plan

Status: **proposed; say `GO` to implement**

## Goal

Run the existing random-fidelity collector against `N` independent Slay the
Spire instances. Start with `N=2`, but do not hard-code two anywhere.

This remains adaptive discovery collection, not Phase 3A holdout evidence.

## Keep the design small

The first version will **not** add:

- systemd integration;
- automatic game restart/watchdogs;
- Job Objects or elaborate crash recovery;
- morning-report machinery;
- automatic repair agents;
- automatic permanent-corpus promotion;
- migration of old queues/campaigns;
- a general distributed-worker system.

If a game or bridge dies, that lane stops and the other lanes continue. The
operator can restart the failed lane or the whole command.

## Necessary isolation

Each worker still requires its own:

- Slay the Spire process;
- game directory, including `preferences/`, `runs/`, and `saves/`;
- Windows `LOCALAPPDATA` and temp directories used by ModTheSpire/mod configs;
- CommunicationMod bridge process and TCP endpoint;
- bridge session directory;
- worker ID.

Sharing a bridge is not allowed: the current collector can take control and
abandon an existing run. Sharing a session directory also corrupts the bridge
files.

A small preparation command will copy the approximately 550 MB game directory
once per worker, copy the required mod configuration, and create paired Windows
and WSL session paths. It will verify path mapping by writing a nonce through
Windows Node and reading it from WSL.

## Configuration

Use one manifest:

```json
{
  "schema": 1,
  "campaign_id": "random-discovery-1",
  "master_seed": "random-discovery-1",
  "workers": [
    {
      "id": "worker-0",
      "game_dir_windows": "D:\\sts-workers\\worker-0\\game",
      "game_dir_linux": "/mnt/d/sts-workers/worker-0/game",
      "session_dir_windows": "D:\\sts-workers\\worker-0\\session",
      "session_dir_linux": "/mnt/d/sts-workers/worker-0/session",
      "windows_state_dir": "D:\\sts-workers\\worker-0\\state"
    },
    {
      "id": "worker-1",
      "game_dir_windows": "D:\\sts-workers\\worker-1\\game",
      "game_dir_linux": "/mnt/d/sts-workers/worker-1/game",
      "session_dir_windows": "D:\\sts-workers\\worker-1\\session",
      "session_dir_linux": "/mnt/d/sts-workers/worker-1/session",
      "windows_state_dir": "D:\\sts-workers\\worker-1\\state"
    }
  ]
}
```

The worker count is `workers.length`. Validation rejects duplicate IDs or
paths. The manifest is copied into the campaign output directory. Worker count
and order may not change when resuming that campaign because they define seed
partitioning.

## Seed partitioning

Give every run a global trial index. For worker index `i` out of `N`:

```text
trial_index = i + local_run_number * N
```

Game seed and policy seed are deterministic functions of the campaign master
seed and `trial_index`, using separate domains. This guarantees that workers do
not collect the same trial and that completion order does not affect future
assignment.

Use the already specified collision-free affine/HMAC derivation and test
vectors from the earlier draft, implemented in one small helper. The exact
formula matters less than these properties:

- no overlap for the supported trial range;
- stable after restart;
- game and policy seeds are distinct derivations;
- changing `N` for an existing campaign is rejected.

Each worker has a tiny atomic cursor file containing its next local run number.
The cursor advances only when a collector returns a durable trace or an
explicit failed/skipped result. If the process stops mid-run, restarting retries
that one trial from the beginning. No live-state restoration is attempted.

## One campaign process, N collector loops

Do not launch `N` copies of `run_random_fidelity_pipeline.js` or
`run_random_fidelity_campaign.js`.

Extend the existing campaign service so one Node parent creates `N` asynchronous
worker loops. Each loop:

1. reads only its configured bridge session;
2. derives its assigned trial;
3. invokes `random_fidelity_collector.js` with worker/session/seed/output env;
4. waits for that collector to finish;
5. reports the result to the parent;
6. advances its cursor and starts its next trial.

The parent is the only writer of shared campaign status and verification queue
records, eliminating concurrent JSONL writers. Per-worker logs and status remain
separate.

If one lane hits three consecutive bridge/infrastructure failures, mark that
lane stopped. Do not stop healthy lanes and do not attempt to restart its game.

## Trace paths

Parallel mode always uses deferred verification and immutable trial-based names:

```text
<campaign>/traces/<trial-id>.jsonl
<campaign>/workers/<worker-id>/status.json
<campaign>/workers/<worker-id>/log.txt
```

Write a trace to a temporary file and rename it only after close. Use exclusive
creation and reject an existing different file. This is enough for a local
single-parent process; no distributed lock/lease layer is planned.

The collector prints one final machine-readable result to the parent. The
parent adds successful traces to the verification queue.

## Verification

Keep the existing independently configurable verifier pool. For the first run,
use one verifier.

Because only the parent campaign process adds queue entries, the existing queue
can remain simple. Verifier worker assignment continues to use the current
worker-index scheme.

Add an explicit `promotion=false` option so the verifier does not call
`promoteDistinctFailure` during this collection run. It may verify, minimize,
and create repair-task artifacts, but it must not edit the permanent corpus or
source tree.

## Game launcher

Add a small parallel game launcher using native Windows Node:

- loop over the manifest workers;
- set each worker's game directory, `LOCALAPPDATA`, `APPDATA`, `TEMP`, `TMP`,
  `TRACE_SESSION_DIR`, `TRACE_BRIDGE_ID`, and `TRACE_CONTROL_PORT=0`;
- spawn that worker's Java/ModTheSpire process;
- print PID and bridge readiness per worker;
- do not scan for or kill unrelated Java processes;
- do not restart failed games automatically.

The launcher remains running while its game children run. On an intentional
shutdown it may terminate only the exact child PID tree it spawned. No watchdog
or system service is needed.

## Implementation steps after `GO`

1. Add manifest parsing/validation and deterministic seed-partition tests.
2. Add the preparation helper for isolated worker directories/config/session
   paths.
3. Add bridge IDs to `trace_client.js` status and TCP responses; make collectors
   verify their expected bridge ID.
4. Extend `run_random_fidelity_campaign.js` to run `N` worker loops inside one
   parent process, with worker cursors and parent-owned queue/status writes.
5. Change collector output paths to immutable trial IDs and return one JSON
   result to the parent.
6. Add `promotion=false` to verifier operation used by this mode.
7. Add the simple Windows Node multi-game launcher.
8. Update `run_random_fidelity_pipeline.js` to start one parallel campaign and
   the configured verifier pool, with repair/promotion services disabled.
9. Run the existing communication tests plus new tests for:
   - duplicate worker path rejection;
   - disjoint seeds for several `N` values;
   - stable restart cursors;
   - bridge-ID mismatch;
   - one failed lane not stopping another;
   - no corpus promotion.
10. Provision two workers and run two trials per worker as a real smoke test.
11. If the smoke test passes, run the launcher and collection pipeline in the
    background with `N=2`.

## Acceptance criteria

Before unattended collection:

- two real game instances expose different bridge IDs, ports, and session
  directories;
- worker seed sets are disjoint;
- both workers produce valid immutable traces;
- one stopped bridge only stops its own lane;
- the verifier consumes both workers' traces;
- no permanent-corpus or simulator source files are changed by the run;
- rerunning the campaign resumes from the worker cursors without changing seed
  partitioning.

## Scaling to N workers

The design scales to arbitrary manifest length:

- game/bridge/profile cost is linear in `N`;
- collector loops are independent asynchronous children of one lightweight
  parent;
- seed partitioning works for any frozen `N`;
- verifier count is configured separately from collector count;
- no worker-specific code or fixed worker names are introduced.

Practical scaling is limited by Slay the Spire process memory/CPU and verifier
throughput, not the scheduler. Start at two, measure, then increase the manifest
length if the machine has capacity.

## Non-goals

- No simulator mechanic changes.
- No comparison weakening or observed-state hydration.
- No certification claim.
- No automatic repairs or corpus promotion.
- No self-healing distributed service.
