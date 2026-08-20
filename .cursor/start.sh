#!/usr/bin/env bash
# Per-boot runtime step. The pre-collection.2 Hugging Face dataset is a legacy
# archive and must not be restored into the active authoritative corpus.
set -uo pipefail

active_corpus="simulator/verification/corpus/permanent_traces"
mkdir -p "$active_corpus"

echo "start: authoritative corpus ready at $active_corpus."
echo "start: the pre-collection.2 Hugging Face dataset is legacy and is not synced automatically."
exit 0
