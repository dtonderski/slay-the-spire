#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
corpus_dir="${STS_PERMANENT_CORPUS_DIR:-$repo_root/simulator/verification/corpus/permanent_traces}"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

ensure_uvx() {
  if command -v uvx >/dev/null 2>&1; then
    return
  fi
  command -v curl >/dev/null 2>&1 ||
    die "uvx is missing and curl is unavailable; install uv first"
  curl -LsSf https://astral.sh/uv/install.sh | sh
  export PATH="$HOME/.local/bin:$PATH"
  command -v uvx >/dev/null 2>&1 || die "uv installation did not provide uvx"
}

repo_id() {
  local value="${2:-${HF_CORPUS_REPO:-}}"
  [[ -n "$value" ]] ||
    die "set HF_CORPUS_REPO or pass the dataset repo as the second argument (owner/name)"
  printf '%s\n' "$value"
}

upload() {
  local repo
  repo="$(repo_id "$@")"
  [[ -d "$corpus_dir" ]] || die "corpus directory does not exist: $corpus_dir"

  shopt -s nullglob
  local traces=("$corpus_dir"/*.jsonl)
  ((${#traces[@]} > 0)) || die "no .jsonl traces found in $corpus_dir"

  local staging_dir="${STS_HF_CORPUS_UPLOAD_DIR:-$repo_root/tmp/hf-permanent-traces}"
  mkdir -p "$staging_dir"

  local trace archive temp
  for trace in "${traces[@]}"; do
    archive="$staging_dir/$(basename "$trace").gz"
    if [[ -s "$archive" && "$archive" -nt "$trace" ]]; then
      continue
    fi
    printf 'compressing %s\n' "$(basename "$trace")"
    temp="$archive.tmp"
    gzip -n -1 -c -- "$trace" >"$temp"
    mv -- "$temp" "$archive"
  done

  ensure_uvx
  printf 'uploading %s compressed traces to private dataset %s\n' "${#traces[@]}" "$repo"
  HF_HUB_DISABLE_XET=1 \
    PYTHONPATH="$repo_root/tools/hf_ipv4${PYTHONPATH:+:$PYTHONPATH}" \
    uvx --from huggingface_hub hf upload \
    "$repo" "$staging_dir" . \
    --repo-type dataset \
    --private \
    --include "*.jsonl.gz"
}

download() {
  local repo
  repo="$(repo_id "$@")"
  [[ -n "${HF_TOKEN:-}" ]] ||
    die "HF_TOKEN is required to download the private corpus dataset"

  ensure_uvx
  local safe_repo="${repo//\//--}"
  local download_dir="${STS_HF_CORPUS_CACHE_DIR:-$HOME/.cache/sts-permanent-traces-hf/$safe_repo}"
  mkdir -p "$download_dir" "$corpus_dir"

  HF_HUB_DISABLE_XET=1 \
    PYTHONPATH="$repo_root/tools/hf_ipv4${PYTHONPATH:+:$PYTHONPATH}" \
    uvx --from huggingface_hub hf download \
    "$repo" \
    --repo-type dataset \
    --include "*.jsonl.gz" \
    --local-dir "$download_dir"

  shopt -s nullglob
  local archives=("$download_dir"/*.jsonl.gz)
  ((${#archives[@]} > 0)) || die "dataset contains no .jsonl.gz traces"

  local archive target temp extracted=0 existing=0
  for archive in "${archives[@]}"; do
    target="$corpus_dir/$(basename "${archive%.gz}")"
    if [[ -s "$target" ]]; then
      existing=$((existing + 1))
      continue
    fi
    printf 'extracting %s\n' "$(basename "$target")"
    temp="$target.tmp.$$"
    gzip -t -- "$archive"
    gzip -d -c -- "$archive" >"$temp"
    mv -- "$temp" "$target"
    extracted=$((extracted + 1))
  done

  printf 'corpus ready: %s existing, %s extracted, %s total in %s\n' \
    "$existing" "$extracted" "${#archives[@]}" "$corpus_dir"
}

case "${1:-}" in
upload)
  upload "$@"
  ;;
download)
  download "$@"
  ;;
*)
  cat >&2 <<'EOF'
usage:
  tools/hf_corpus.sh upload [owner/dataset]
  tools/hf_corpus.sh download [owner/dataset]

Set HF_CORPUS_REPO instead of passing owner/dataset if preferred. Upload uses
the cached Hugging Face login or HF_TOKEN; download requires HF_TOKEN.
EOF
  exit 2
  ;;
esac
