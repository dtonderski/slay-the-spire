# Live Trace Real-Game Smoke Checklist

This checklist validates `sts_live` against a real Slay the Spire process with
CommunicationMod. It is intentionally manual/quarantined: do not require it for
ordinary CI, and do not use it as a substitute for fake-bridge tests or verifier
replay tests.

## Preconditions

- Start Slay the Spire through ModTheSpire.
- Enable BaseMod and CommunicationMod.
- If abandon-run validation is in scope, also enable Abandon Run Control.
- Confirm CommunicationMod is writing bridge session files under the expected
  session directory, or set `STS_LIVE_BRIDGE_SESSION_DIR`.
- Use a throwaway game/profile state. The smoke should not depend on a
  particular save file.

From WSL, set the shared Windows/WSL session path and a Linux trace root:

```bash
cd /mnt/d/dev/slay-the-spire/simulator
export STS_LIVE_TRACE_ROOT=/mnt/d/dev/slay-the-spire/live_traces_smoke
export STS_LIVE_BRIDGE_SESSION_DIR=/mnt/d/dev/slay-the-spire/tools/communication/session
```

If the game launched CommunicationMod from another worktree, use that
worktree's mounted `tools/communication/session` path instead.

Equivalent Windows environment:

```powershell
$env:STS_LIVE_TRACE_ROOT = "D:\dev\slay-the-spire\live_traces_smoke"
$env:STS_LIVE_BRIDGE_SESSION_DIR = "D:\dev\slay-the-spire\tools\communication\session"
```

Leave `STS_LIVE_ALLOW_FILE_COMMANDS` unset unless intentionally testing the
legacy `next_command.txt` fallback. The default path should use guarded TCP
control.

## Commands

Build and start the backend:

```bash
cargo run -p sts_live --bin live-trace -- serve --addr 127.0.0.1:8799
```

In another shell, inspect bridges without using the browser:

```bash
cargo run -p sts_live --bin live-trace -- bridges list
cargo run -p sts_live --bin live-trace -- bridges state communication-mod
```

Expected evidence:

- JSON output is compact and parseable.
- At least one connected bridge is listed.
- No plain-text CLI error is emitted.

## Manual UI Path

Open <http://127.0.0.1:8799/>.

1. Select the live bridge.
2. Start an Ironclad A0 run with a known throwaway seed.
3. Confirm the UI shows a session id, trace path, current phase, and fidelity.
4. Use only legal-action buttons rendered by the backend.
5. Click `Request state` once and confirm the trace continues to grow.
6. Play through Neow and at least one combat decision.
7. Stop after a useful prefix; if testing abandon support, click `Abandon run`.

Expected evidence:

- The UI renders one button per current backend legal action.
- The trace JSONL contains `metadata`, `action`, and `state` records.
- Explicit state polling appears as a `request-state` action followed by a
  state.
- If fidelity is lost or a bridge command fails, the UI shows the backend error
  and the session becomes blocked or fidelity-lost rather than continuing
  silently.
- Mid-run attach without prior trace history reports `unverified_start`.

## Operator-Only Path

The same smoke must be possible without browser automation:

```bash
cargo run -p sts_live --bin live-trace -- sessions list
cargo run -p sts_live --bin live-trace -- actions list session-1
cargo run -p sts_live --bin live-trace -- actions send session-1 <action-id>
cargo run -p sts_live --bin live-trace -- sessions request-state session-1
cargo run -p sts_live --bin live-trace -- fidelity status session-1
cargo run -p sts_live --bin live-trace -- trace path session-1
```

If a throwaway run exists without a `live-trace` session, reset it through the
same Linux CLI before starting a tracked session:

```bash
cargo run -p sts_live --bin live-trace -- bridges abandon communication-mod
cargo run -p sts_live --bin live-trace -- sessions start \
  --bridge communication-mod --character ironclad --ascension 0 --seed CODEX04
```

Expected evidence:

- Each command returns compact structured JSON.
- A session started in one CLI invocation can be listed and continued in a later
  invocation.
- Action ids, not labels, are used for `actions send`.

## Existing-Trace Replay

Validate a source trace without changing the game:

```bash
cargo run -p sts_live --bin live-trace -- replay <source-trace.jsonl> --dry-run
```

Replay a bounded prefix into a start-ready game:

```bash
cargo run -p sts_live --bin live-trace -- replay <source-trace.jsonl> \
  --bridge communication-mod --max-actions 1
```

If a throwaway run may already be active, explicitly authorize resetting only
that bridge:

```bash
cargo run -p sts_live --bin live-trace -- replay <source-trace.jsonl> \
  --bridge communication-mod --reset-bridge --max-actions 1
```

Expected evidence:

- `--dry-run` returns `status: validated` and no session id.
- A live replay creates a new append-only trace rather than modifying the
  source.
- `replayed_actions` counts commands after the recorded `START`.
- `status` is `completed` when the source endpoint is reached or `action_limit`
  when `--max-actions` stops a valid prefix.
- Replay stops before sending if the recorded command is not a unique enabled
  legal action.
- Replay stops immediately if the new session's fidelity is not `ok`.
- A captured source profile must match the profile reported by the live game.
- An active run is rejected unless `--reset-bridge` is present.

Only normal `START` traces are live-replayable. `START_VERIFY` changes test-only
starting HP and is rejected by the normal game launcher. Traces with explicit
boss-unlock inputs are also rejected because the live bridge cannot assert that
persistent profile state.

## Trace Validation

The verifier has one strict replay contract: reconstruct from `START`, typed
commands, and simulator state. For a trace containing enough history for that
contract, run:

```bash
uv run -- cargo run -p sts_verify --bin sts_verify -- parity <trace-path>
```

Do not claim seed-start fidelity unless `sts_live` reports seed-start `ok` for
that trace or this command passes manually. A trace without the required
`START` history is not eligible for strict parity; observed state is comparison
evidence and must not be used to hydrate simulator state.

## Pass Criteria

Record the smoke as passed only when all applicable evidence is present:

- bridge discovery works
- start or attach works
- manual legal-action sending works
- request-state works and is recorded
- trace file is append-only and recoverable after backend restart
- CLI can list and continue the recovered session
- fidelity status is honest (`unknown`, `ok`, `lost`, or `unverified_start`)
- errors are structured and do not permit silent continuation

## Failure Handling

Keep failed traces if they contain useful evidence. If a trace exposes simulator
divergence, preserve the JSONL and minimize it with the verifier workflow in
[`verification.md`](verification.md). Fix the generic simulator, verifier, or bridge behavior; do
not add seed-specific UI/backend workarounds.
