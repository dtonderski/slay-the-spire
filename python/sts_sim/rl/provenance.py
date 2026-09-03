"""Reproducibility metadata for generated learning artifacts."""

from __future__ import annotations

import hashlib
import json
import os
import stat
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import cast

_HEX = "0123456789abcdef"
_REPOSITORY_KEYS = {"git_sha", "clean", "dirty_diff_digest"}


def canonical_bytes(payload: object) -> bytes:
    return json.dumps(
        payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    ).encode()


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def require_digest(value: object, label: str) -> str:
    if (
        type(value) is not str
        or len(value) != 64
        or any(character not in _HEX for character in value)
    ):
        raise ValueError(f"{label} must be a lowercase SHA-256 digest")
    return value


def digest_payload(payload: dict[str, object], digest_key: str) -> str:
    unsigned = dict(payload)
    unsigned.pop(digest_key, None)
    return sha256_bytes(canonical_bytes(unsigned))


@dataclass(frozen=True, slots=True)
class RepositoryVersion:
    git_sha: str
    clean: bool
    dirty_diff_digest: str | None = None

    def __post_init__(self) -> None:
        if (
            type(self.git_sha) is not str
            or len(self.git_sha) not in {40, 64}
            or any(character not in "0123456789abcdef" for character in self.git_sha)
        ):
            raise ValueError("git SHA must be a lowercase object digest")
        if type(self.clean) is not bool:
            raise TypeError("clean flag must be boolean")
        if self.dirty_diff_digest is not None and (
            type(self.dirty_diff_digest) is not str
            or len(self.dirty_diff_digest) != 64
            or any(character not in "0123456789abcdef" for character in self.dirty_diff_digest)
        ):
            raise ValueError("dirty digest must be a lowercase SHA-256 digest")
        if self.clean == (self.dirty_diff_digest is not None):
            raise ValueError(
                "clean repositories must omit a dirty digest and dirty ones must include it"
            )

    def to_dict(self) -> dict[str, object]:
        return {
            "git_sha": self.git_sha,
            "clean": self.clean,
            "dirty_diff_digest": self.dirty_diff_digest,
        }

    @classmethod
    def from_dict(cls, payload: object) -> RepositoryVersion:
        if type(payload) is not dict:
            raise TypeError("repository version must be an object")
        source = cast(dict[str, object], payload)
        if set(source) != _REPOSITORY_KEYS:
            raise ValueError("repository version has missing or unknown fields")
        sha = source["git_sha"]
        clean = source["clean"]
        digest = source["dirty_diff_digest"]
        if type(sha) is not str or type(clean) is not bool:
            raise TypeError("repository version has invalid field types")
        if digest is not None and type(digest) is not str:
            raise TypeError("dirty repository digest must be a string")
        return cls(sha, clean, digest)


def _git(repo: Path, *args: str) -> bytes:
    completed = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        capture_output=True,
    )
    return completed.stdout


def _blob_id(algorithm: str, content: bytes) -> bytes:
    digest = hashlib.new(algorithm)
    digest.update(f"blob {len(content)}\0".encode())
    digest.update(content)
    return digest.hexdigest().encode("ascii")


def _current_manifest(repo: Path, paths: bytes) -> bytes:
    algorithm = _git(repo, "rev-parse", "--show-object-format").strip().decode("ascii")
    payload = bytearray()
    for encoded_path in sorted(filter(None, paths.split(b"\0"))):
        relative = encoded_path.decode("utf-8", errors="surrogateescape")
        path = repo / relative
        try:
            metadata = path.lstat()
        except FileNotFoundError:
            payload.extend(b"000000 missing -\t")
            payload.extend(encoded_path)
            payload.extend(b"\0")
            continue
        if stat.S_ISLNK(metadata.st_mode):
            mode = b"120000"
            content = os.fsencode(os.readlink(path))
        elif stat.S_ISREG(metadata.st_mode):
            mode = b"100755" if metadata.st_mode & 0o111 else b"100644"
            content = path.read_bytes()
        else:
            raise ValueError(f"cannot attest unsupported file type: {relative}")
        payload.extend(mode)
        payload.extend(b" blob ")
        payload.extend(_blob_id(algorithm, content))
        payload.extend(b"\t")
        payload.extend(encoded_path)
        payload.extend(b"\0")
    return bytes(payload)


def _head_manifest(repo: Path) -> bytes:
    return _git(repo, "ls-tree", "-rz", "--full-tree", "HEAD")


def _index_manifest(repo: Path) -> bytes:
    payload = bytearray()
    entries = _git(repo, "ls-files", "--stage", "-z")
    for entry in filter(None, entries.split(b"\0")):
        metadata, encoded_path = entry.split(b"\t", 1)
        mode, object_id, stage = metadata.split(b" ")
        if stage == b"0":
            payload.extend(mode + b" blob " + object_id + b"\t" + encoded_path + b"\0")
        else:
            payload.extend(b"unmerged " + metadata + b"\t" + encoded_path + b"\0")
    return bytes(payload)


def capture_repository_version(
    repo: Path,
    *,
    allow_dirty: bool = False,
) -> RepositoryVersion:
    """Capture exact source, refusing dirty generation by default."""

    sha = _git(repo, "rev-parse", "HEAD").decode().strip()
    tracked_paths = _git(repo, "ls-files", "-c", "-z")
    untracked_paths = _git(repo, "ls-files", "-o", "--exclude-standard", "-z")
    head_manifest = _head_manifest(repo)
    index_manifest = _index_manifest(repo)
    tracked_manifest = _current_manifest(repo, tracked_paths)
    untracked_manifest = _current_manifest(repo, untracked_paths)
    if (
        index_manifest == head_manifest
        and tracked_manifest == head_manifest
        and not untracked_manifest
    ):
        return RepositoryVersion(sha, True)
    if not allow_dirty:
        raise ValueError("repository is dirty; commit changes or explicitly allow a dirty digest")
    digest = hashlib.sha256(
        b"index\0"
        + index_manifest
        + b"tracked\0"
        + tracked_manifest
        + b"untracked\0"
        + untracked_manifest
    ).hexdigest()
    return RepositoryVersion(sha, False, digest)


def file_digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()
