#!/usr/bin/env python3
"""Create and verify an immutable, byte-exact schema-v0 trace archive.

Schema is classified exclusively from each CommunicationMod state record's
``message.boundary_schema`` field.  Filenames and top-level metadata.schema are
not used.  Archive files retain their repository-relative path beneath the
archive root.  This script deliberately separates creation, verification, and
active-copy removal so no source is removed before a complete byte audit.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import sys
from typing import Any

ACTIVE_ROOTS = (
    "simulator/verification/corpus/permanent_traces",
    "simulator/verification/corpus/fidelity_regressions",
    "simulator/verification/corpus/open_failures",
    "simulator/verification/corpus/quarantined_traces",
    "random_traces_loop/traces",
    "random_traces_loop/minimized",
    "random_traces_loop/schema_v1_smoke/traces",
)
MANIFEST_NAME = "manifest.json"
ARCHIVE_SET_NAME = "archive-set.sha256"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def classify_trace(path: Path) -> dict[str, Any]:
    counts = {"records": 0, "actions": 0, "states": 0, "errors": 0, "external_rng": 0}
    saw_v0 = False
    saw_v1 = False
    explicit_communication_mod = False
    has_direct_records = False
    with path.open("r", encoding="utf-8") as source:
        for line_number, line in enumerate(source, 1):
            if not line.strip():
                continue
            counts["records"] += 1
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise RuntimeError(f"{path}:{line_number}: invalid JSON: {error}") from error
            record_type = record.get("type")
            if record_type == "metadata" and record.get("source") == "communication_mod":
                explicit_communication_mod = True
            elif record_type == "action" and type(record.get("step")) is int and isinstance(
                record.get("command"), str
            ):
                counts["actions"] += 1
                has_direct_records = True
            elif record_type == "state" and type(record.get("step")) is int and "message" in record:
                counts["states"] += 1
                has_direct_records = True
                message = record.get("message")
                if isinstance(message, str):
                    try:
                        message = json.loads(message)
                    except json.JSONDecodeError as error:
                        raise RuntimeError(
                            f"{path}:{line_number}: state message string is invalid JSON: {error}"
                        ) from error
                if not isinstance(message, dict):
                    raise RuntimeError(f"{path}:{line_number}: state message is not an object")
                boundary_schema = message.get("boundary_schema")
                if boundary_schema is None or (type(boundary_schema) is int and boundary_schema == 0):
                    saw_v0 = True
                elif type(boundary_schema) is int and boundary_schema == 1:
                    saw_v1 = True
                else:
                    raise RuntimeError(
                        f"{path}:{line_number}: unsupported state boundary_schema {boundary_schema!r}"
                    )
            elif record_type == "error" and type(record.get("step")) is int:
                counts["errors"] += 1
            elif record_type == "external_rng" and type(record.get("step")) is int:
                counts["external_rng"] += 1
    # Strict v1 provenance is explicit. Shape inference exists only to identify
    # historical v0 evidence whose old metadata was absent or used live_trace.
    communication_mod = explicit_communication_mod or (saw_v0 and has_direct_records)
    if not communication_mod or counts["actions"] == 0 or counts["states"] == 0:
        schema = "not_communication_mod"
    elif saw_v0 and saw_v1:
        schema = "mixed"
    elif saw_v1:
        schema = "v1"
    else:
        schema = "v0"
    return {"schema": schema, **counts}


def candidate_paths(repo: Path) -> list[Path]:
    paths: list[Path] = []
    for relative_root in ACTIVE_ROOTS:
        root = repo / relative_root
        if not root.is_dir():
            raise RuntimeError(f"required active discovery root is missing: {relative_root}")
        paths.extend(path for path in root.rglob("*.jsonl") if path.is_file())
    return sorted(set(paths))


def canonical_manifest_bytes(manifest: dict[str, Any]) -> bytes:
    return (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode("utf-8")


def archive_set_bytes(entries: list[dict[str, Any]]) -> bytes:
    return "".join(
        f"{entry['sha256']}  {entry['archive_path']}\n" for entry in entries
    ).encode("utf-8")


def load_manifest(archive_root: Path) -> tuple[dict[str, Any], bytes]:
    path = archive_root / MANIFEST_NAME
    raw = path.read_bytes()
    manifest = json.loads(raw)
    if raw != canonical_manifest_bytes(manifest):
        raise RuntimeError(f"{path}: manifest is not in canonical form")
    return manifest, raw


def create(repo: Path, archive_root: Path) -> None:
    if archive_root.exists() and any(archive_root.iterdir()):
        raise RuntimeError(f"archive root is not empty: {archive_root}")
    archive_root.mkdir(parents=True, exist_ok=True)
    entries: list[dict[str, Any]] = []
    schema_counts: dict[str, int] = {}
    for source in candidate_paths(repo):
        classification = classify_trace(source)
        schema = classification.pop("schema")
        schema_counts[schema] = schema_counts.get(schema, 0) + 1
        if schema != "v0":
            continue
        relative = source.relative_to(repo).as_posix()
        destination = archive_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        source_size = source.stat().st_size
        source_hash = sha256_file(source)
        shutil.copyfile(source, destination)
        with destination.open("rb") as archived:
            os.fsync(archived.fileno())
        archive_size = destination.stat().st_size
        archive_hash = sha256_file(destination)
        if archive_size != source_size or archive_hash != source_hash:
            raise RuntimeError(f"byte verification failed for {relative}")
        destination.chmod(stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
        entries.append(
            {
                "original_path": relative,
                "archive_path": relative,
                "size": source_size,
                "sha256": source_hash,
                **classification,
            }
        )
    unexpected = {
        schema: count
        for schema, count in schema_counts.items()
        if schema not in {"v0", "v1"} and count
    }
    if schema_counts.get("mixed", 0) or unexpected:
        raise RuntimeError(f"refusing to archive with unsupported active inputs: {schema_counts}")
    manifest = {
        "manifest_schema": 1,
        "classification": (
            "CommunicationMod state message.boundary_schema: missing/integer 0 is v0; "
            "integer 1 is v1; filenames and metadata.schema are ignored"
        ),
        "active_roots": list(ACTIVE_ROOTS),
        "archive_root": str(archive_root),
        "schema_counts_at_creation": schema_counts,
        "trace_count": len(entries),
        "total_size": sum(entry["size"] for entry in entries),
        "total_records": sum(entry["records"] for entry in entries),
        "total_actions": sum(entry["actions"] for entry in entries),
        "total_states": sum(entry["states"] for entry in entries),
        "total_errors": sum(entry["errors"] for entry in entries),
        "total_external_rng": sum(entry["external_rng"] for entry in entries),
        "entries": entries,
    }
    manifest_bytes = canonical_manifest_bytes(manifest)
    (archive_root / MANIFEST_NAME).write_bytes(manifest_bytes)
    set_bytes = archive_set_bytes(entries)
    (archive_root / ARCHIVE_SET_NAME).write_bytes(set_bytes)
    (archive_root / f"{MANIFEST_NAME}.sha256").write_text(
        f"{hashlib.sha256(manifest_bytes).hexdigest()}  {MANIFEST_NAME}\n",
        encoding="utf-8",
    )
    (archive_root / f"{ARCHIVE_SET_NAME}.sha256").write_text(
        f"{hashlib.sha256(set_bytes).hexdigest()}  {ARCHIVE_SET_NAME}\n",
        encoding="utf-8",
    )
    print(json.dumps({key: value for key, value in manifest.items() if key != "entries"}, indent=2))
    print(f"manifest_sha256={hashlib.sha256(manifest_bytes).hexdigest()}")
    print(f"archive_set_sha256={hashlib.sha256(set_bytes).hexdigest()}")


def verify(repo: Path, archive_root: Path, require_sources: bool) -> None:
    manifest, manifest_bytes = load_manifest(archive_root)
    manifest_hash_line = (archive_root / f"{MANIFEST_NAME}.sha256").read_text(encoding="utf-8")
    expected_manifest_hash = manifest_hash_line.split()[0]
    actual_manifest_hash = hashlib.sha256(manifest_bytes).hexdigest()
    if actual_manifest_hash != expected_manifest_hash:
        raise RuntimeError("manifest hash mismatch")
    expected_set = archive_set_bytes(manifest["entries"])
    stored_set = (archive_root / ARCHIVE_SET_NAME).read_bytes()
    if stored_set != expected_set:
        raise RuntimeError("archive-set listing mismatch")
    expected_set_hash = (archive_root / f"{ARCHIVE_SET_NAME}.sha256").read_text(
        encoding="utf-8"
    ).split()[0]
    if hashlib.sha256(stored_set).hexdigest() != expected_set_hash:
        raise RuntimeError("archive-set aggregate hash mismatch")
    checked_sources = 0
    for entry in manifest["entries"]:
        archived = archive_root / entry["archive_path"]
        if archived.stat().st_size != entry["size"] or sha256_file(archived) != entry["sha256"]:
            raise RuntimeError(f"archived bytes mismatch: {entry['archive_path']}")
        source = repo / entry["original_path"]
        if source.exists():
            checked_sources += 1
            if source.stat().st_size != entry["size"] or sha256_file(source) != entry["sha256"]:
                raise RuntimeError(f"source bytes mismatch: {entry['original_path']}")
        elif require_sources:
            raise RuntimeError(f"source missing before removal: {entry['original_path']}")
    print(
        f"verified {len(manifest['entries'])} archived traces; "
        f"source_copies_checked={checked_sources}; manifest_sha256={actual_manifest_hash}; "
        f"archive_set_sha256={expected_set_hash}"
    )


def audit_active(repo: Path) -> None:
    counts: dict[str, int] = {}
    for path in candidate_paths(repo):
        schema = classify_trace(path)["schema"]
        counts[schema] = counts.get(schema, 0) + 1
    print(json.dumps({"active_roots": ACTIVE_ROOTS, "schema_counts": counts}, indent=2))
    unsupported = {schema: count for schema, count in counts.items() if schema != "v1" and count}
    if unsupported:
        raise RuntimeError(f"unsupported files remain in active discovery roots: {unsupported}")


def audit_repository(repo: Path) -> None:
    skipped = {".git", ".pi-subagents", "node_modules", "target", "tmp"}
    counts: dict[str, int] = {}
    unsupported: list[str] = []
    for base, directories, files in os.walk(repo):
        directories[:] = [directory for directory in directories if directory not in skipped]
        for filename in files:
            if not filename.endswith(".jsonl"):
                continue
            path = Path(base) / filename
            schema = classify_trace(path)["schema"]
            counts[schema] = counts.get(schema, 0) + 1
            if schema in {"v0", "mixed"}:
                unsupported.append(path.relative_to(repo).as_posix())
    print(json.dumps({"repository_schema_counts": counts, "v0_or_mixed": unsupported}, indent=2))
    if unsupported:
        raise RuntimeError(f"schema-v0 or mixed traces remain in repository: {unsupported[:10]}")


def remove_active(repo: Path, archive_root: Path) -> None:
    verify(repo, archive_root, require_sources=True)
    manifest, _ = load_manifest(archive_root)
    for entry in manifest["entries"]:
        (repo / entry["original_path"]).unlink()
    remaining_v0 = []
    for path in candidate_paths(repo):
        if classify_trace(path)["schema"] == "v0":
            remaining_v0.append(path.relative_to(repo).as_posix())
    if remaining_v0:
        raise RuntimeError(f"schema-v0 active traces remain: {remaining_v0[:10]}")
    print(f"removed {len(manifest['entries'])} verified active copies; schema-v0 remaining=0")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "mode",
        choices=("create", "verify", "audit-active", "audit-repository", "remove-active"),
    )
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--archive-root", type=Path)
    parser.add_argument("--require-sources", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve()
    if args.mode == "audit-active":
        audit_active(repo)
        return
    if args.mode == "audit-repository":
        audit_repository(repo)
        return
    if args.archive_root is None:
        parser.error("--archive-root is required for create, verify, and remove-active")
    archive_root = args.archive_root.resolve()
    if repo == archive_root or repo in archive_root.parents:
        print("note: archive is outside active discovery roots but inside repository", file=sys.stderr)
    if args.mode == "create":
        create(repo, archive_root)
    elif args.mode == "verify":
        verify(repo, archive_root, args.require_sources)
    else:
        remove_active(repo, archive_root)


if __name__ == "__main__":
    main()
