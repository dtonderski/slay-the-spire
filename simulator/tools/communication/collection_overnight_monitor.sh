#!/usr/bin/env bash
# No lock cleanup, PID-based kills, launch, retry, or stale-owner takeover.
# Keep this historical entrypoint fail-closed rather than silently restoring
# the old unattended restart workflow.
printf '%s\n' 'Automatic collection recovery is disabled. Launch the game/bridge explicitly; inspect failures and use a fresh campaign after review.' >&2
exit 1
