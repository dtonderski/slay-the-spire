#!/usr/bin/env bash
# Build-time bootstrap: runs during the environment Build and is baked into the
# snapshot. No Cloud secrets are available here, so only source-derived build
# steps belong in this file. Must stay idempotent.
set -euo pipefail

# Build the PyO3 extension so `import sts_sim` works (canonical command from
# simulator/docs/python_fair_api.md).
uv sync --project simulator/python --reinstall-package sts-sim
