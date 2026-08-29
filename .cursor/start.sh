#!/usr/bin/env bash
# Per-boot runtime step. The Rust workspace is rooted at the repository root;
# verification data remains under simulator/ during the staged physical move.
# Download the current reviewed permanent corpus when the runtime-only Hugging
# Face read token is available.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root" || exit 1

active_corpus="$repo_root/simulator/verification/corpus/permanent_traces"
mkdir -p "$active_corpus"

if [[ -n "${HF_TOKEN:-}" ]]; then
  STS_PERMANENT_CORPUS_DIR="$active_corpus" \
    tools/hf_corpus.sh download dtonderski/sts-permanent-traces
  echo "start: authoritative corpus downloaded to $active_corpus."
else
  echo "start: HF_TOKEN is unset; authoritative external corpus was not downloaded."
  echo "start: use committed fixtures for verifier smoke tests."
fi
