"""Write-once source-epoch bundles that archive exact loaded native bytes."""

from __future__ import annotations

import json
import os
import platform
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from .. import _native
from .experiment import (
    _raise_if_symlink_ancestor,
    _read_regular_file_bytes,
    write_scientific_artifact,
)
from .provenance import (
    RepositoryVersion,
    canonical_bytes,
    digest_payload,
    require_digest,
    sha256_bytes,
)

SOURCE_EPOCH_BUNDLE_KIND = "source-epoch-bundle-v1"
SOURCE_EPOCH_DIRNAME = "source-epoch-bundle"
_MANIFEST_NAME = "manifest.json"
_BUNDLE_KEYS = frozenset(
    {
        "kind",
        "git_sha",
        "clean",
        "dirty_diff_digest",
        "native_extension_name",
        "native_extension_digest",
        "native_relative_path",
        "python_implementation",
        "python_version",
        "python_releaselevel",
        "platform",
        "rustc_version",
        "cargo_version",
        "bundle_digest",
    }
)


def loaded_native_path() -> Path:
    path = Path(_native.__file__)
    if path.is_symlink() or not path.is_file():
        raise ValueError("native extension must be a regular file")
    return path


def loaded_native_bytes() -> bytes:
    return _read_regular_file_bytes(loaded_native_path())


def loaded_native_digest() -> str:
    return sha256_bytes(loaded_native_bytes())


def _tool_version(name: str) -> str:
    completed = subprocess.run(
        [name, "--version"],
        check=True,
        capture_output=True,
        text=True,
    )
    version = completed.stdout.strip()
    if not version:
        raise ValueError(f"{name} version is empty")
    return version


@dataclass(frozen=True, slots=True)
class SourceEpochBundle:
    kind: str
    git_sha: str
    clean: bool
    dirty_diff_digest: str | None
    native_extension_name: str
    native_extension_digest: str
    native_relative_path: str
    python_implementation: str
    python_version: tuple[int, ...]
    python_releaselevel: str
    platform: dict[str, str]
    rustc_version: str
    cargo_version: str
    bundle_digest: str

    def relative_members(self) -> frozenset[str]:
        return _expected_relative_files(self)

    def to_dict(self) -> dict[str, object]:
        return {
            "kind": self.kind,
            "git_sha": self.git_sha,
            "clean": self.clean,
            "dirty_diff_digest": self.dirty_diff_digest,
            "native_extension_name": self.native_extension_name,
            "native_extension_digest": self.native_extension_digest,
            "native_relative_path": self.native_relative_path,
            "python_implementation": self.python_implementation,
            "python_version": list(self.python_version),
            "python_releaselevel": self.python_releaselevel,
            "platform": dict(self.platform),
            "rustc_version": self.rustc_version,
            "cargo_version": self.cargo_version,
            "bundle_digest": self.bundle_digest,
        }


def _require_git_sha(value: object) -> str:
    if (
        type(value) is not str
        or len(value) not in {40, 64}
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError("git SHA must be a lowercase object digest")
    return value


def _require_native_basename(value: object) -> str:
    if type(value) is not str or not value:
        raise TypeError("native extension name must be a nonempty string")
    if (
        value in {".", ".."}
        or "/" in value
        or "\\" in value
        or "\x00" in value
    ):
        raise ValueError("native extension name must be a single safe basename")
    return value


def _parse_bundle(payload: object) -> SourceEpochBundle:
    if type(payload) is not dict:
        raise TypeError("source-epoch-bundle manifest must be an object")
    source = cast(dict[str, object], payload)
    if set(source) != _BUNDLE_KEYS:
        raise ValueError("source-epoch-bundle manifest has missing or unknown fields")
    if source["kind"] != SOURCE_EPOCH_BUNDLE_KIND:
        raise ValueError("unsupported source-epoch-bundle kind")
    python_version = source["python_version"]
    if type(python_version) is not list or not python_version:
        raise TypeError("python version must be a nonempty array")
    version_parts: list[int] = []
    for part in cast(list[object], python_version):
        if type(part) is not int:
            raise TypeError("python version components must be integers")
        version_parts.append(part)
    if len(version_parts) != 4:
        raise TypeError("python version must be major, minor, micro, and serial")
    major, minor, micro, serial = version_parts
    if major <= 0 or minor < 0 or micro < 0 or serial < 0:
        raise ValueError("python version components are invalid")
    platform_value = source["platform"]
    if type(platform_value) is not dict:
        raise TypeError("platform must be an object")
    platform_map = cast(dict[str, object], platform_value)
    if set(platform_map) != {"system", "release", "machine"}:
        raise ValueError("platform has missing or unknown fields")
    platform_fields: dict[str, str] = {}
    for key, value in platform_map.items():
        if type(key) is not str or type(value) is not str or not value:
            raise TypeError("platform fields must be nonempty strings")
        platform_fields[key] = value
    if set(platform_fields) != {"system", "release", "machine"}:
        raise TypeError("platform fields must be strings")
    native_name = _require_native_basename(source["native_extension_name"])
    native_relative = source["native_relative_path"]
    if type(native_relative) is not str:
        raise TypeError("native relative path must be a string")
    if native_relative != f"native/{native_name}":
        raise ValueError("native relative path is not canonical")
    for name in (
        "python_implementation",
        "python_releaselevel",
        "rustc_version",
        "cargo_version",
    ):
        value = source[name]
        if type(value) is not str or not value:
            raise TypeError(f"{name.replace('_', ' ')} must be a nonempty string")
    if source["python_releaselevel"] not in {"alpha", "beta", "candidate", "final"}:
        raise ValueError("python releaselevel is unsupported")
    rustc_version = cast(str, source["rustc_version"])
    cargo_version = cast(str, source["cargo_version"])
    if not rustc_version.startswith("rustc "):
        raise ValueError("rustc version must carry the canonical rustc prefix")
    if not cargo_version.startswith("cargo "):
        raise ValueError("cargo version must carry the canonical cargo prefix")
    git_sha = _require_git_sha(source["git_sha"])
    clean = source["clean"]
    if type(clean) is not bool:
        raise TypeError("clean flag must be boolean")
    if not clean:
        raise ValueError("source-epoch bundles require a clean repository")
    if source["dirty_diff_digest"] is not None:
        raise ValueError("clean source-epoch bundles must omit a dirty digest")
    dirty = None
    bundle = SourceEpochBundle(
        SOURCE_EPOCH_BUNDLE_KIND,
        git_sha,
        clean,
        dirty,
        native_name,
        require_digest(source["native_extension_digest"], "native extension digest"),
        native_relative,
        cast(str, source["python_implementation"]),
        tuple(version_parts),
        cast(str, source["python_releaselevel"]),
        platform_fields,
        cast(str, source["rustc_version"]),
        cast(str, source["cargo_version"]),
        require_digest(source["bundle_digest"], "source-epoch-bundle digest"),
    )
    if bundle.bundle_digest != digest_payload(bundle.to_dict(), "bundle_digest"):
        raise ValueError("source-epoch-bundle digest is invalid")
    return bundle


def _python_version_sidecar(bundle: SourceEpochBundle) -> bytes:
    major, minor, micro, serial = bundle.python_version
    return (
        f"{bundle.python_implementation} {major}.{minor}.{micro} "
        f"{bundle.python_releaselevel} {serial}\n"
    ).encode()


def _expected_relative_files(bundle: SourceEpochBundle) -> frozenset[str]:
    name = bundle.native_extension_name
    return frozenset(
        {
            _MANIFEST_NAME,
            f"native/{name}",
            f"native/{name}.sha256",
            "source/git-sha.txt",
            "toolchain/python-version.txt",
            "toolchain/platform.json",
            "toolchain/rustc-version.txt",
            "toolchain/cargo-version.txt",
        }
    )


def _nofollow_relative_files(bundle_dir: Path) -> tuple[str, ...]:
    _raise_if_symlink_ancestor(bundle_dir)
    if bundle_dir.is_symlink() or not bundle_dir.is_dir():
        raise ValueError("source-epoch-bundle must be a directory")
    found: list[str] = []
    found_dirs: set[str] = set()
    for dirpath, dirnames, filenames in os.walk(bundle_dir, followlinks=False, topdown=True):
        directory = Path(dirpath)
        rel_dir = directory.relative_to(bundle_dir).as_posix()
        if rel_dir != ".":
            found_dirs.add(rel_dir)
        for name in (*dirnames, *filenames):
            child = directory / name
            if child.is_symlink():
                raise ValueError("source-epoch-bundle must not contain symlinks")
        for name in filenames:
            child = directory / name
            if not child.is_file():
                raise ValueError("source-epoch-bundle members must be regular files")
            found.append(child.relative_to(bundle_dir).as_posix())
    expected_dirs = frozenset({"native", "source", "toolchain"})
    if found_dirs != expected_dirs:
        raise ValueError("source-epoch-bundle members are not the exact allowlisted set")
    return tuple(sorted(found))


def _verify_sidecars(bundle_dir: Path, bundle: SourceEpochBundle) -> None:
    expected = _expected_relative_files(bundle)
    found = frozenset(_nofollow_relative_files(bundle_dir))
    if found != expected:
        raise ValueError("source-epoch-bundle members are not the exact allowlisted set")
    native_bytes = _read_regular_file_bytes(bundle_dir / bundle.native_relative_path)
    if sha256_bytes(native_bytes) != bundle.native_extension_digest:
        raise ValueError("archived native bytes do not match the source-epoch-bundle digest")
    declared_native = _read_regular_file_bytes(
        bundle_dir / "native" / f"{bundle.native_extension_name}.sha256"
    )
    if declared_native != f"{bundle.native_extension_digest}\n".encode():
        raise ValueError("archived native digest sidecar is not canonical")
    git_sidecar = _read_regular_file_bytes(bundle_dir / "source" / "git-sha.txt")
    if git_sidecar != f"{bundle.git_sha}\n".encode():
        raise ValueError("archived git SHA sidecar is not canonical")
    python_sidecar = _read_regular_file_bytes(bundle_dir / "toolchain" / "python-version.txt")
    if python_sidecar != _python_version_sidecar(bundle):
        raise ValueError("archived python version sidecar is not canonical")
    platform_sidecar = _read_regular_file_bytes(bundle_dir / "toolchain" / "platform.json")
    if platform_sidecar != canonical_bytes(bundle.platform):
        raise ValueError("archived platform sidecar is not canonical")
    rustc_sidecar = _read_regular_file_bytes(bundle_dir / "toolchain" / "rustc-version.txt")
    if rustc_sidecar != f"{bundle.rustc_version}\n".encode():
        raise ValueError("archived rustc version sidecar is not canonical")
    cargo_sidecar = _read_regular_file_bytes(bundle_dir / "toolchain" / "cargo-version.txt")
    if cargo_sidecar != f"{bundle.cargo_version}\n".encode():
        raise ValueError("archived cargo version sidecar is not canonical")


def write_source_epoch_bundle(output_dir: Path, repository: RepositoryVersion) -> SourceEpochBundle:
    if not repository.clean or repository.dirty_diff_digest is not None:
        raise ValueError("source-epoch bundles require a clean repository")
    native_path = loaded_native_path()
    native_bytes = loaded_native_bytes()
    native_digest = sha256_bytes(native_bytes)
    native_name = native_path.name
    native_relative = f"native/{native_name}"
    unsigned: dict[str, object] = {
        "kind": SOURCE_EPOCH_BUNDLE_KIND,
        "git_sha": repository.git_sha,
        "clean": repository.clean,
        "dirty_diff_digest": repository.dirty_diff_digest,
        "native_extension_name": native_name,
        "native_extension_digest": native_digest,
        "native_relative_path": native_relative,
        "python_implementation": platform.python_implementation(),
        "python_version": [
            sys.version_info.major,
            sys.version_info.minor,
            sys.version_info.micro,
            sys.version_info.serial,
        ],
        "python_releaselevel": sys.version_info.releaselevel,
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
        },
        "rustc_version": _tool_version("rustc"),
        "cargo_version": _tool_version("cargo"),
    }
    bundle = _parse_bundle(
        {
            **unsigned,
            "bundle_digest": digest_payload(unsigned, "bundle_digest"),
        }
    )
    output_dir.mkdir(parents=True, exist_ok=True)
    write_scientific_artifact(output_dir / native_relative, native_bytes)
    write_scientific_artifact(
        output_dir / "native" / f"{native_name}.sha256",
        f"{native_digest}\n".encode(),
    )
    write_scientific_artifact(output_dir / "source" / "git-sha.txt", f"{repository.git_sha}\n".encode())
    write_scientific_artifact(
        output_dir / "toolchain" / "python-version.txt",
        _python_version_sidecar(bundle),
    )
    write_scientific_artifact(
        output_dir / "toolchain" / "platform.json",
        canonical_bytes(unsigned["platform"]),
    )
    write_scientific_artifact(
        output_dir / "toolchain" / "rustc-version.txt",
        f"{bundle.rustc_version}\n".encode(),
    )
    write_scientific_artifact(
        output_dir / "toolchain" / "cargo-version.txt",
        f"{bundle.cargo_version}\n".encode(),
    )
    write_scientific_artifact(output_dir / _MANIFEST_NAME, canonical_bytes(bundle.to_dict()))
    return bundle


def load_source_epoch_bundle(bundle_dir: Path) -> SourceEpochBundle:
    _raise_if_symlink_ancestor(bundle_dir)
    if bundle_dir.is_symlink() or not bundle_dir.is_dir():
        raise ValueError("source-epoch-bundle must be a directory")
    content = _read_regular_file_bytes(bundle_dir / _MANIFEST_NAME)
    try:
        payload = json.loads(content)
    except json.JSONDecodeError as error:
        raise ValueError("source-epoch-bundle manifest is not JSON") from error
    bundle = _parse_bundle(payload)
    if content != canonical_bytes(bundle.to_dict()):
        raise ValueError("source-epoch-bundle manifest is not canonical")
    _verify_sidecars(bundle_dir, bundle)
    return bundle


def verify_loaded_native_bytes(bundle: SourceEpochBundle) -> None:
    digest = loaded_native_digest()
    if digest != bundle.native_extension_digest:
        raise ValueError("loaded native extension bytes do not match the source-epoch-bundle")


def copy_source_epoch_bundle(source_dir: Path, destination_dir: Path) -> SourceEpochBundle:
    bundle = load_source_epoch_bundle(source_dir)
    verify_loaded_native_bytes(bundle)
    destination_dir.mkdir(parents=True, exist_ok=True)
    for relative in _nofollow_relative_files(source_dir):
        write_scientific_artifact(
            destination_dir / relative,
            _read_regular_file_bytes(source_dir / relative),
        )
    loaded = load_source_epoch_bundle(destination_dir)
    if loaded.bundle_digest != bundle.bundle_digest:
        raise ValueError("copied source-epoch-bundle digest mismatch")
    return loaded


def source_epoch_relative_files(bundle_dir: Path) -> tuple[str, ...]:
    bundle = load_source_epoch_bundle(bundle_dir)
    files = tuple(
        sorted(
            f"{SOURCE_EPOCH_DIRNAME}/{relative}"
            for relative in _nofollow_relative_files(bundle_dir)
        )
    )
    if not files:
        raise ValueError("source-epoch-bundle is empty")
    if frozenset(path.split("/", 1)[1] for path in files) != _expected_relative_files(bundle):
        raise ValueError("source-epoch-bundle members are not the exact allowlisted set")
    return files
