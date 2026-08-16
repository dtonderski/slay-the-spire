#!/usr/bin/env bash
# Per-boot runtime step: runs when an agent pod starts, where runtime secrets
# (HF_TOKEN) are available but build-time secrets are not. Syncs the permanent
# trace corpus before the agent starts working. Never aborts boot.
set -uo pipefail

corpus_dir="simulator/verification/corpus/permanent_traces"

if [ -z "${HF_TOKEN:-}" ]; then
  if ls -A "$corpus_dir"/*.jsonl >/dev/null 2>&1; then
    echo "start: HF_TOKEN unset; keeping the corpus already present in $corpus_dir."
  else
    echo "start: HF_TOKEN unset and no corpus on disk; skipping download." \
         "sts_verify parity/status need the corpus — set HF_TOKEN (read access to" \
         "dtonderski/sts-permanent-traces), or use the committed manual/milestone1.jsonl" \
         "fixture." >&2
  fi
  exit 0
fi

# Delegate to the incremental downloader: it fetches only compressed traces that
# are missing and extracts only traces not already present, so this both resumes
# a partial corpus and picks up newly added traces on later boots. Persist the
# compressed cache under the (persisted) workspace so reboots re-fetch only new
# archives instead of the whole dataset.
export STS_HF_CORPUS_CACHE_DIR="${STS_HF_CORPUS_CACHE_DIR:-$PWD/simulator/verification/corpus/.hf_download_cache}"

echo "start: syncing permanent trace corpus from Hugging Face (incremental)..."
if ! bash tools/hf_corpus.sh download dtonderski/sts-permanent-traces; then
  echo "start: corpus sync failed; continuing without a complete corpus (retries next boot)." >&2
fi
exit 0
