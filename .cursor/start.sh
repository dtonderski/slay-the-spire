#!/usr/bin/env bash
# Per-boot runtime step: runs when an agent pod starts, where runtime secrets
# (HF_TOKEN) are available but build-time secrets are not. Downloads the
# permanent trace corpus once, before the agent starts working. The download is
# incremental and skips traces that already exist, and it never aborts boot.
set -uo pipefail

corpus_dir="simulator/verification/corpus/permanent_traces"

if ls -A "$corpus_dir"/*.jsonl >/dev/null 2>&1; then
  echo "start: permanent corpus already present in $corpus_dir; skipping download"
  exit 0
fi

if [ -z "${HF_TOKEN:-}" ]; then
  echo "start: HF_TOKEN not set; skipping corpus download. sts_verify parity/status" \
       "need the corpus — set HF_TOKEN (read access to dtonderski/sts-permanent-traces)," \
       "or use the committed manual/milestone1.jsonl fixture." >&2
  exit 0
fi

echo "start: downloading permanent trace corpus from Hugging Face..."
if ! bash tools/hf_corpus.sh download dtonderski/sts-permanent-traces; then
  echo "start: corpus download failed; continuing without it (it will retry on the next boot)." >&2
fi
exit 0
