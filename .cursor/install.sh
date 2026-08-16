#!/usr/bin/env bash
# Cursor Cloud environment bootstrap. Runs from the repo root (/workspace) after
# checkout, on the image built from .cursor/Dockerfile. Must stay idempotent.
set -euo pipefail

# Build the PyO3 extension so `import sts_sim` works (canonical command from
# simulator/docs/python_fair_api.md).
uv sync --project simulator/python --reinstall-package sts-sim

# Download the permanent trace corpus from the private Hugging Face dataset into
# simulator/verification/corpus/permanent_traces/. Requires the HF_TOKEN secret
# to be available at build time; see AGENTS.md (### Cursor Cloud).
bash tools/hf_corpus.sh download dtonderski/sts-permanent-traces
