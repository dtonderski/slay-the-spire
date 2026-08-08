#!/usr/bin/env python3
"""Verify the aggregate immutable schema-v0 forensic archive."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical(value: dict) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args()
    manifest_path = args.manifest.resolve()
    raw = manifest_path.read_bytes()
    manifest = json.loads(raw)
    if raw != canonical(manifest):
        raise RuntimeError("aggregate manifest is not canonical")
    expected = manifest_path.with_suffix(manifest_path.suffix + ".sha256").read_text().split()[0]
    actual = hashlib.sha256(raw).hexdigest()
    if actual != expected:
        raise RuntimeError("aggregate manifest hash mismatch")
    expected_entries = []
    expected_totals = {
        key: 0
        for key in (
            "total_size",
            "total_records",
            "total_actions",
            "total_states",
            "total_errors",
            "total_external_rng",
        )
    }
    for archive in manifest["archives"]:
        root = Path(archive["root"])
        archive_manifest = root / "manifest.json"
        archive_raw = archive_manifest.read_bytes()
        archive_data = json.loads(archive_raw)
        if archive_raw != canonical(archive_data):
            raise RuntimeError(f"archive manifest is not canonical: {root}")
        if sha256(archive_manifest) != archive["manifest_sha256"]:
            raise RuntimeError(f"archive manifest mismatch: {root}")
        if sha256(root / "archive-set.sha256") != archive["archive_set_sha256"]:
            raise RuntimeError(f"archive set mismatch: {root}")
        if archive_data["trace_count"] != archive["trace_count"]:
            raise RuntimeError(f"archive trace count mismatch: {root}")
        expected_entries.extend(
            {"archive_root": str(root), **entry} for entry in archive_data["entries"]
        )
        for key in expected_totals:
            expected_totals[key] += archive_data[key]
    expected_entries.sort(key=lambda entry: entry["original_path"])
    if manifest["entries"] != expected_entries:
        raise RuntimeError("aggregate entries do not exactly cover every archive manifest entry")
    if any(manifest[key] != value for key, value in expected_totals.items()):
        raise RuntimeError("aggregate totals do not match archive manifests")
    identities = {
        (entry["archive_root"], entry["archive_path"]) for entry in manifest["entries"]
    }
    if len(identities) != len(manifest["entries"]):
        raise RuntimeError("aggregate contains duplicate archive entries")
    total_size = 0
    for entry in manifest["entries"]:
        path = Path(entry["archive_root"]) / entry["archive_path"]
        if path.stat().st_size != entry["size"] or sha256(path) != entry["sha256"]:
            raise RuntimeError(f"archived trace mismatch: {path}")
        total_size += entry["size"]
    if len(manifest["entries"]) != manifest["trace_count"] or total_size != manifest["total_size"]:
        raise RuntimeError("aggregate accounting mismatch")
    print(
        f"verified {manifest['trace_count']} immutable schema-v0 traces; "
        f"bytes={total_size}; aggregate_manifest_sha256={actual}"
    )


if __name__ == "__main__":
    main()
