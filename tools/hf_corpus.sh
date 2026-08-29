#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
corpus_dir="${STS_PERMANENT_CORPUS_DIR:-$repo_root/verification/corpus/permanent_traces}"

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

  declare -A expected_archives=()
  local trace archive temp stale_archive relative
  for trace in "${traces[@]}"; do
    expected_archives["$(basename "$trace").gz"]=1
  done
  while IFS= read -r -d '' stale_archive; do
    relative="${stale_archive#"$staging_dir"/}"
    if [[ "$relative" != "$(basename "$relative")" || -z "${expected_archives[$relative]:-}" ]]; then
      printf 'removing stale upload archive %s\n' "$relative"
      rm -- "$stale_archive"
    fi
  done < <(find "$staging_dir" -type f -name '*.jsonl.gz' -print0)

  for trace in "${traces[@]}"; do
    archive="$staging_dir/$(basename "$trace").gz"
    if [[ -s "$archive" ]]; then
      if gzip -t -- "$archive" && gzip -d -c -- "$archive" | cmp -s - "$trace"; then
        continue
      fi
      printf 'replacing stale same-name archive %s\n' "$(basename "$archive")"
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
    --include "*.jsonl.gz" \
    --delete "*.jsonl.gz"
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

  local manifest manifest_unique
  manifest="$(mktemp)"
  manifest_unique="$(mktemp)"
  HF_HUB_DISABLE_XET=1 \
    PYTHONPATH="$repo_root/tools/hf_ipv4${PYTHONPATH:+:$PYTHONPATH}" \
    uvx --from huggingface_hub hf download \
    "$repo" \
    --repo-type dataset \
    --include "*.jsonl.gz" \
    --dry-run \
    --format quiet >"$manifest"
  sort -u -- "$manifest" >"$manifest_unique"
  [[ "$(wc -l <"$manifest")" == "$(wc -l <"$manifest_unique")" ]] ||
    die "dataset returned duplicate trace archive names"
  mv -- "$manifest_unique" "$manifest"
  [[ -s "$manifest" ]] || die "dataset contains no .jsonl.gz traces"

  HF_HUB_DISABLE_XET=1 \
    PYTHONPATH="$repo_root/tools/hf_ipv4${PYTHONPATH:+:$PYTHONPATH}" \
    uvx --from huggingface_hub hf download \
    "$repo" \
    --repo-type dataset \
    --include "*.jsonl.gz" \
    --local-dir "$download_dir"

  declare -A remote_archives=()
  local archives=() relative archive target temp stale_archive stale_target
  while IFS= read -r relative; do
    [[ "$relative" == "$(basename "$relative")" && "$relative" == *.jsonl.gz ]] ||
      die "unsafe dataset trace archive path: $relative"
    remote_archives["$relative"]=1
    archive="$download_dir/$relative"
    [[ -s "$archive" ]] || die "downloaded archive is missing: $archive"
    archives+=("$archive")
  done <"$manifest"
  rm -f -- "$manifest" "$manifest_unique"

  shopt -s nullglob
  local stale_cache=0
  for stale_archive in "$download_dir"/*.jsonl.gz; do
    if [[ -z "${remote_archives[$(basename "$stale_archive")]:-}" ]]; then
      printf 'removing stale download archive %s\n' "$(basename "$stale_archive")"
      rm -- "$stale_archive"
      stale_cache=$((stale_cache + 1))
    fi
  done

  local quarantine_dir="${STS_HF_CORPUS_QUARANTINE_DIR:-$(dirname "$corpus_dir")/quarantined_traces/hf-remote-removed}"
  local quarantined=0
  for stale_target in "$corpus_dir"/*.jsonl; do
    relative="$(basename "$stale_target").gz"
    if [[ -z "${remote_archives[$relative]:-}" ]]; then
      mkdir -p "$quarantine_dir"
      target="$quarantine_dir/$(basename "$stale_target")"
      if [[ -e "$target" ]]; then
        cmp -s -- "$stale_target" "$target" ||
          die "quarantine collision for removed remote trace: $target"
        rm -- "$stale_target"
      else
        mv -- "$stale_target" "$target"
      fi
      printf 'quarantined remote-removed trace %s\n' "$(basename "$stale_target")"
      quarantined=$((quarantined + 1))
    fi
  done

  local extracted=0 existing=0
  for archive in "${archives[@]}"; do
    target="$corpus_dir/$(basename "${archive%.gz}")"
    if [[ -s "$target" ]]; then
      gzip -t -- "$archive"
      gzip -d -c -- "$archive" | cmp -s - "$target" ||
        die "existing trace differs from remote archive: $target"
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

  printf 'corpus ready: %s existing, %s extracted, %s total, %s stale cache removed, %s traces quarantined in %s\n' \
    "$existing" "$extracted" "${#archives[@]}" "$stale_cache" "$quarantined" "$corpus_dir"
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
the cached Hugging Face login or HF_TOKEN and replaces the remote root
`*.jsonl.gz` set exactly; download requires HF_TOKEN and quarantines local
traces no longer present remotely.
EOF
  exit 2
  ;;
esac
