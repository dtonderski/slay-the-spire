#!/usr/bin/env bash
# Build/provision-time bootstrap. Runs from the repo root on Cursor's default
# image (no custom Dockerfile), so it also works for just-in-time agents that
# don't build an environment image. Must stay idempotent.
set -euo pipefail

# Newest stable Rust. The repo commits no Cargo.lock and resolves edition2024
# transitive deps, so the image's default 1.83 toolchain is too old.
rustup toolchain install stable --profile minimal --no-self-update
rustup default stable
rustup component add rustfmt clippy

# uv drives the Python/PyO3 build (and provides uvx for the corpus download).
if ! command -v uv >/dev/null 2>&1; then
  curl -LsSf https://astral.sh/uv/install.sh | sh
fi
export PATH="$HOME/.local/bin:$PATH"

# System lib needed to build/link the sts_sim (py_sts) extension.
sudo DEBIAN_FRONTEND=noninteractive apt-get update -y
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends libpython3.12-dev

# Build the PyO3 extension so `import sts_sim` works (canonical command from
# simulator/docs/python_fair_api.md).
uv sync --project simulator/python --reinstall-package sts-sim
