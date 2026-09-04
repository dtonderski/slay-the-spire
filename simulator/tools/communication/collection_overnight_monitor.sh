#!/usr/bin/env bash
# Overnight health monitor for random-fidelity collection.
# - Checks every 2 minutes
# - Restarts at most one game + one campaign
# - Uses a lock file so only one monitor instance runs
set -u

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT" || exit 1

SESSION="${STS_BRIDGE_SESSION_DIR:-/mnt/d/dev/slay-the-spire/simulator/tools/communication/session}"
GAME_DIR="${STS_GAME_DIR:-/mnt/d/SteamLibrary/steamapps/common/SlayTheSpire}"
OUT="${STS_RANDOM_OUTPUT_DIR:-$ROOT/simulator/tools/random_traces_loop}"
NODE="${NODE_BIN:-$(command -v node)}"
LOG="$OUT/collection_monitor.log"
LOCK="$OUT/collection_monitor.lock"
GAME_PIDFILE="$OUT/game_supervisor.pid"
CAMP_PIDFILE="$OUT/campaign_supervisor.pid"
STATUS="$OUT/campaign_status.json"
GAME_LOG="$OUT/game_supervisor.log"
CAMP_LOG="$OUT/campaign_supervisor.log"
INTERVAL="${STS_COLLECTION_MONITOR_INTERVAL_SEC:-120}"

SOURCE_VERSION="${STS_RANDOM_SOURCE_VERSION:-collection.3-schema6}"

mkdir -p "$OUT"

log() { printf '%s %s\n' "$(date -Is)" "$*" | tee -a "$LOG"; }

# Single instance via mkdir lock
if mkdir "$LOCK" 2>/dev/null; then
  echo $$ >"$LOCK/pid"
  trap 'rm -rf "$LOCK"' EXIT
else
  old="$(cat "$LOCK/pid" 2>/dev/null || true)"
  if [[ -n "$old" ]] && kill -0 "$old" 2>/dev/null; then
    echo "monitor already running pid=$old" >&2
    exit 0
  fi
  rm -rf "$LOCK"
  mkdir "$LOCK"
  echo $$ >"$LOCK/pid"
  trap 'rm -rf "$LOCK"' EXIT
fi

alive() { [[ -n "${1:-}" ]] && kill -0 "$1" 2>/dev/null; }

java_alive() {
  /mnt/c/Windows/System32/tasklist.exe /FI "IMAGENAME eq java.exe" /FO CSV /NH 2>/dev/null | grep -qi 'java\.exe'
}

bridge_info() {
  STS_BRIDGE_SESSION_DIR="$SESSION" python3 - <<'PY'
import json, os, socket
from pathlib import Path
base = Path(os.environ["STS_BRIDGE_SESSION_DIR"])
status_path = base / "status.json"
summary_path = base / "summary.json"
try:
    d = json.loads(status_path.read_text())
except Exception as e:
    print(f"DOWN status_read {e}")
    raise SystemExit(1)
c = d.get("control") or {}
host = c.get("host") or "127.0.0.1"
port = c.get("port")
if not port:
    print("DOWN no_port")
    raise SystemExit(2)
s = socket.socket(); s.settimeout(2)
try:
    s.connect((host, int(port)))
except Exception as e:
    print(f"DOWN tcp {e}")
    raise SystemExit(3)
finally:
    s.close()
sm = {}
if summary_path.exists():
    try:
        sm = json.loads(summary_path.read_text())
    except Exception:
        sm = {}
print(
    f"UP port={port} bridge_status={d.get('status')} in_game={sm.get('in_game')} "
    f"ready={sm.get('ready_for_command')} floor={sm.get('floor')} "
    f"room={sm.get('room_type')} boundary={sm.get('boundary_kind')}"
)
PY
}

kill_java() {
  local pids
  pids="$(/mnt/c/Windows/System32/tasklist.exe /FI "IMAGENAME eq java.exe" /FO CSV /NH 2>/dev/null | sed -n 's/^"java\.exe","\([0-9]*\)".*/\1/p' || true)"
  for p in $pids; do
    /mnt/c/Windows/System32/taskkill.exe /PID "$p" /T /F >/dev/null 2>&1 || true
  done
}

restart_game() {
  log "RESTART game watchdog"
  local opid
  opid="$(cat "$GAME_PIDFILE" 2>/dev/null || true)"
  if alive "$opid"; then
    kill "$opid" 2>/dev/null || true
    sleep 2
    kill -9 "$opid" 2>/dev/null || true
  fi
  # only one game tree
  pkill -f 'random_fidelity_game_watchdog.js' 2>/dev/null || true
  sleep 1
  kill_java
  sleep 3
  nohup env \
    STS_GAME_DIR="$GAME_DIR" \
    STS_BRIDGE_SESSION_DIR="$SESSION" \
    PATH="$PATH" \
    "$NODE" "$ROOT/simulator/tools/communication/random_fidelity_game_watchdog.js" \
    >>"$GAME_LOG" 2>&1 </dev/null &
  echo $! >"$GAME_PIDFILE"
  disown || true
  log "game pid=$(cat "$GAME_PIDFILE")"
  local i out
  for i in $(seq 1 36); do
    sleep 5
    if out="$(bridge_info 2>/dev/null)"; then
      log "bridge recovered: $out"
      return 0
    fi
    if ! alive "$(cat "$GAME_PIDFILE" 2>/dev/null || true)"; then
      log "game watchdog died during startup"
      return 1
    fi
  done
  log "bridge did not recover in time"
  return 1
}

count_campaigns() {
  ps -ef | grep -F 'run_random_fidelity_campaign.js' | grep -v grep | wc -l
}

restart_campaign() {
  log "RESTART campaign supervisor"
  # Kill every campaign/collector so we never run two controllers
  pkill -f 'run_random_fidelity_campaign.js' 2>/dev/null || true
  pkill -f 'random_fidelity_collector.js' 2>/dev/null || true
  sleep 2
  pkill -9 -f 'run_random_fidelity_campaign.js' 2>/dev/null || true
  pkill -9 -f 'random_fidelity_collector.js' 2>/dev/null || true
  sleep 1
  nohup env \
    STS_BRIDGE_SESSION_DIR="$SESSION" \
    STS_RANDOM_OUTPUT_DIR="$OUT" \
    STS_RANDOM_MAX_RUNS=0 \
    STS_RANDOM_LOG_ACTIONS=0 \
    STS_STARTING_HP=10000 \
    STS_SEEN_BOSSES_PATH="$GAME_DIR/preferences/STSSeenBosses" \
    STS_RANDOM_GAME_SEED_PREFIX=FIDL \
    STS_RANDOM_SOURCE_VERSION="$SOURCE_VERSION" \
    PATH="$PATH" \
    "$NODE" "$ROOT/simulator/tools/communication/run_random_fidelity_campaign.js" \
    >>"$CAMP_LOG" 2>&1 </dev/null &
  echo $! >"$CAMP_PIDFILE"
  disown || true
  log "campaign pid=$(cat "$CAMP_PIDFILE")"
}

log "monitor start pid=$$ interval=${INTERVAL}s root=$ROOT"
echo $$ >"$OUT/collection_monitor.pid"

STALL_LOOPS=0
LAST_TRACE_COUNT=0
LAST_PROGRESS_TS="$(date +%s)"

while true; do
  traces="$(find "$OUT/traces" -maxdepth 1 -type f -name '*.jsonl' 2>/dev/null | wc -l | tr -d ' ')"
  newest="$(find "$OUT/traces" -maxdepth 1 -type f -name '*.jsonl' -printf '%T@ %f\n' 2>/dev/null | sort -n | tail -n1)"
  camp_status="missing"
  if [[ -f "$STATUS" ]]; then
    camp_status="$(STATUS="$STATUS" python3 - <<'PY'
import json
import os
from pathlib import Path
p=Path(os.environ["STATUS"])
d=json.loads(p.read_text())
print(f"{d.get('status')}|run={d.get('run_number')}|seed={d.get('game_seed')}|updated={d.get('updated_at')}|fail={d.get('consecutive_failures')}")
PY
)"
  fi

  gpid="$(cat "$GAME_PIDFILE" 2>/dev/null || true)"
  cpid="$(cat "$CAMP_PIDFILE" 2>/dev/null || true)"
  gok=no; cok=no; jok=no; bup=no; bout=""
  alive "$gpid" && gok=yes
  alive "$cpid" && cok=yes
  java_alive && jok=yes
  if bout="$(bridge_info 2>/dev/null)"; then bup=yes; fi
  ncamp="$(count_campaigns | tr -d ' ')"

  log "health game=$gok java=$jok bridge=$bup ($bout) campaign=$cok n_campaigns=$ncamp status=$camp_status traces=$traces newest=$newest"

  # Progress watchdog: if no new traces for 45+ minutes while supposedly running, bounce
  now="$(date +%s)"
  if [[ "$traces" -gt "$LAST_TRACE_COUNT" ]]; then
    LAST_TRACE_COUNT="$traces"
    LAST_PROGRESS_TS="$now"
    STALL_LOOPS=0
  else
    STALL_LOOPS=$((STALL_LOOPS + 1))
  fi
  stuck_sec=$((now - LAST_PROGRESS_TS))

  need_game=0
  need_camp=0
  if [[ "$gok" != yes || "$jok" != yes || "$bup" != yes ]]; then
    need_game=1
  fi
  if [[ "$cok" != yes || "$ncamp" -eq 0 ]]; then
    need_camp=1
  fi
  if [[ "$ncamp" -gt 1 ]]; then
    log "multiple campaigns detected ($ncamp); collapsing to one"
    need_camp=1
  fi
  if [[ "$stuck_sec" -ge 2700 && "$bup" == yes ]]; then
    log "no new traces for ${stuck_sec}s; restarting campaign"
    need_camp=1
    LAST_PROGRESS_TS="$now"
  fi
  # long infrastructure outage
  if [[ "$stuck_sec" -ge 3600 ]]; then
    log "severe stall ${stuck_sec}s; full game+campaign restart"
    need_game=1
    need_camp=1
    LAST_PROGRESS_TS="$now"
  fi

  if [[ "$need_game" -eq 1 ]]; then
    restart_game || true
    need_camp=1
  fi
  if [[ "$need_camp" -eq 1 ]]; then
    # only restart campaign when bridge is up
    if out="$(bridge_info 2>/dev/null)"; then
      restart_campaign || true
    else
      log "defer campaign restart until bridge is up"
    fi
  fi

  sleep "$INTERVAL"
done
