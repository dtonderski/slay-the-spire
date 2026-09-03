"""Content-addressed experiment provenance, integrity, and reproduction."""

from __future__ import annotations

import json
import os
import re
import secrets
import stat
from collections.abc import Iterator, Mapping
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from types import MappingProxyType
from typing import cast

from .provenance import (
    canonical_bytes,
    capture_repository_version,
    digest_payload,
    read_regular_file_bytes,
    sha256_bytes,
)

PREDECLARATION_KIND = "experiment_predeclaration_v1"
PREDECLARATION_SCHEMA_VERSION = 1
ARTIFACT_INVENTORY_NAME = "artifact-inventory.sha256"
UNDECLARED_POLICY_STRICT = "strict"
_HEX = "0123456789abcdef"
_V1_KEYS = frozenset(
    {
        "kind",
        "schema_version",
        "name",
        "source_commit",
        "source_worktree_must_be_clean",
        "promotion_claim",
        "consumed_evidence_policy",
        "inputs",
        "outputs",
        "environment",
    }
)
_ARTIFACT_REF_KEYS = frozenset({"role", "path", "sha256"})
_EVIDENCE_POLICY_KEYS = frozenset(
    {"sealed_test", "real_trace_audit", "development_only_for_assessment"}
)
_ENVIRONMENT_KEYS = frozenset(
    {
        "runtime_identity_digest",
        "encoder_contract_digest",
        "vocabulary_fingerprint",
        "source_digest",
        "cohort_digest",
        "root_manifest_digest",
        "dataset_manifest_digest",
        "checkpoint_file_digest",
        "checkpoint_config_digest",
    }
)
_INVENTORY_LINE = re.compile(r"^([0-9a-f]{64}) [ *](.+)$")
_JSON_DIGEST_FIELDS = (
    "manifest_digest",
    "cohort_digest",
    "report_digest",
    "source_digest",
    "checkpoint_file_digest",
    "checkpoint_config_digest",
    "root_manifest_digest",
    "dataset_manifest_digest",
    "runtime_identity_digest",
    "encoder_contract_digest",
    "vocabulary_fingerprint",
)
_FILE_DIGEST_ROLES = {
    "checkpoint": "checkpoint_file_digest",
}
_LIVE_ENVIRONMENT_KEYS = frozenset({"source_digest", "runtime_identity_digest"})
_SELF_DIGEST_KEYS = frozenset({"report_digest", "manifest_digest"})
_DIRECTORY_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_DIRECTORY | os.O_CLOEXEC
_FILE_READ_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
_FILE_CREATE_FLAGS = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
_PREDECLARATION_NAME = "predeclaration.json"


def _require_mapping(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        raise TypeError(f"{label} must be an object")
    result = cast(dict[str, object], value)
    if any(type(key) is not str for key in result):
        raise TypeError(f"{label} keys must be strings")
    return result


def _require_string(value: object, label: str) -> str:
    if type(value) is not str or not value:
        raise TypeError(f"{label} must be a nonempty string")
    return value


def _require_bool(value: object, label: str) -> bool:
    if type(value) is not bool:
        raise TypeError(f"{label} must be a boolean")
    return value


def _require_digest(value: object, label: str) -> str:
    if (
        type(value) is not str
        or len(value) != 64
        or any(character not in _HEX for character in value)
    ):
        raise ValueError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _require_git_sha(value: object, label: str) -> str:
    if (
        type(value) is not str
        or len(value) not in {40, 64}
        or any(character not in _HEX for character in value)
    ):
        raise ValueError(f"{label} must be a lowercase git object digest")
    return value


def _fsync_directory_fd(descriptor: int) -> None:
    os.fsync(descriptor)


def _fsync_directory(directory: Path) -> None:
    descriptor = os.open(os.fspath(directory), _DIRECTORY_FLAGS)
    try:
        _fsync_directory_fd(descriptor)
    finally:
        os.close(descriptor)


def _open_path_root(path: Path) -> tuple[int, tuple[str, ...]]:
    if path.is_absolute():
        return os.open(b"/", _DIRECTORY_FLAGS), path.parts[1:]
    return os.open(b".", _DIRECTORY_FLAGS), path.parts


def _symlink_parent_error(path: Path, error: OSError) -> ValueError:
    del error
    return ValueError(
        f"refusing to write scientific artifact through a symlink parent: {path}"
    )


def _ensure_directory_nofollow(path: Path) -> int:
    """Create missing parents and return an O_NOFOLLOW directory fd for `path`."""

    current, components = _open_path_root(_lexically_normalized(path))
    if not components:
        return current
    try:
        for component in components:
            encoded = os.fsencode(component)
            try:
                nxt = os.open(encoded, _DIRECTORY_FLAGS, dir_fd=current)
            except FileNotFoundError:
                try:
                    os.mkdir(encoded, dir_fd=current)
                except FileExistsError:
                    pass
                try:
                    nxt = os.open(encoded, _DIRECTORY_FLAGS, dir_fd=current)
                except OSError as error:
                    raise _symlink_parent_error(path, error) from error
            except OSError as error:
                raise _symlink_parent_error(path, error) from error
            os.close(current)
            current = nxt
        return current
    except BaseException:
        os.close(current)
        raise


@contextmanager
def held_parent_directory(path: Path) -> Iterator[int]:
    """Hold the parent directory fd of `path` without following any component."""

    descriptor = _ensure_directory_nofollow(path.parent)
    try:
        yield descriptor
    finally:
        os.close(descriptor)


def _require_basename(path: Path) -> str:
    name = path.name
    if not name or name in {".", ".."} or "/" in name or "\\" in name or "\x00" in name:
        raise ValueError(f"scientific artifact name is not a single basename: {path}")
    return name


def _open_existing_regular(parent_fd: int, name: str, path: Path) -> int | None:
    try:
        descriptor = os.open(os.fsencode(name), _FILE_READ_FLAGS, dir_fd=parent_fd)
    except FileNotFoundError:
        return None
    except OSError as error:
        raise ValueError(
            f"refusing to write scientific artifact through a symlink: {path}"
        ) from error
    try:
        info = os.fstat(descriptor)
        if not stat.S_ISREG(info.st_mode):
            raise ValueError(f"refusing to write scientific artifact through a symlink: {path}")
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _read_fd_bytes(descriptor: int) -> bytes:
    with os.fdopen(descriptor, "rb") as handle:
        return handle.read()


def _create_exclusive_temp(parent_fd: int, prefix: str) -> tuple[int, str]:
    for _ in range(128):
        name = f".{prefix}.{os.getpid()}.{secrets.token_hex(8)}"
        try:
            descriptor = os.open(
                os.fsencode(name), _FILE_CREATE_FLAGS, 0o600, dir_fd=parent_fd
            )
        except FileExistsError:
            continue
        return descriptor, name
    raise RuntimeError("could not allocate a temporary scientific artifact")


def _unlink_temp(parent_fd: int, name: str) -> None:
    try:
        os.unlink(os.fsencode(name), dir_fd=parent_fd)
    except FileNotFoundError:
        return


def write_bytes_via_parent_fd(
    parent_fd: int,
    name: str,
    content: bytes,
    *,
    replace: bool,
    path: Path,
) -> None:
    """Write `content` relative to a held parent directory fd."""

    encoded_name = os.fsencode(name)
    existing = _open_existing_regular(parent_fd, name, path)
    if existing is not None:
        if replace:
            os.close(existing)
        else:
            existing_bytes = _read_fd_bytes(existing)
            if existing_bytes == content:
                return
            raise ValueError(f"refusing to mutate scientific artifact: {path}")
    tmp_fd, tmp_name = _create_exclusive_temp(parent_fd, name)
    try:
        with os.fdopen(tmp_fd, "wb") as output:
            tmp_fd = -1
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        encoded_tmp = os.fsencode(tmp_name)
        if replace:
            os.replace(
                encoded_tmp,
                encoded_name,
                src_dir_fd=parent_fd,
                dst_dir_fd=parent_fd,
            )
        else:
            try:
                os.link(
                    encoded_tmp,
                    encoded_name,
                    src_dir_fd=parent_fd,
                    dst_dir_fd=parent_fd,
                    follow_symlinks=False,
                )
            except FileExistsError:
                current = _open_existing_regular(parent_fd, name, path)
                if current is None:
                    raise ValueError(f"refusing to mutate scientific artifact: {path}") from None
                existing_bytes = _read_fd_bytes(current)
                if existing_bytes == content:
                    return
                raise ValueError(f"refusing to mutate scientific artifact: {path}") from None
        _fsync_directory_fd(parent_fd)
    finally:
        if tmp_fd >= 0:
            os.close(tmp_fd)
        _unlink_temp(parent_fd, tmp_name)
        _fsync_directory_fd(parent_fd)


def _absolute_without_follow(path: Path) -> Path:
    if path.is_absolute():
        return path
    return Path.cwd() / path


def is_mutable_synchronization_path(relative: Path) -> bool:
    """Mutable observational metadata: timing files written beside immutable artifacts."""

    if relative.is_absolute():
        raise ValueError("artifact paths must be relative to the experiment directory")
    return relative.name.endswith(".time.txt")


def normalize_inventory_relative_path(path_text: str) -> str:
    """Normalize a sha256sum path. Reject absolute, traversal, and misleading forms."""

    if type(path_text) is not str or not path_text:
        raise ValueError("inventory path must be a nonempty string")
    if "\x00" in path_text:
        raise ValueError("inventory path contains NUL")
    if path_text.startswith(" ") or path_text.endswith(" ") or " \n" in path_text:
        raise ValueError("inventory path has misleading whitespace")
    if "\\" in path_text:
        raise ValueError("inventory path must use posix separators")
    if path_text.startswith(("/", "~")):
        raise ValueError("inventory path must not be absolute")
    if len(path_text) >= 2 and path_text[1] == ":":
        raise ValueError("inventory path must not be absolute")
    if path_text.startswith("././") or path_text == "./" or path_text == ".":
        raise ValueError("inventory path has misleading separators")
    if "//" in path_text or "/./" in path_text or path_text.endswith(("/", "/.")):
        raise ValueError("inventory path has misleading separators")
    relative = path_text.removeprefix("./")
    if relative.startswith(("./", "/")):
        raise ValueError("inventory path has misleading separators")
    parts = relative.split("/")
    if any(part == "" or part == "." or part == ".." for part in parts):
        raise ValueError("inventory path must not contain '.' or '..' segments")
    return "/".join(parts)


def _lexically_normalized(path: Path) -> Path:
    absolute = _absolute_without_follow(path)
    parts: list[str] = []
    for part in absolute.parts:
        if part == "..":
            if parts:
                parts.pop()
        elif part != ".":
            parts.append(part)
    if not parts:
        return Path("/")
    return Path(*parts)


def _raise_if_symlink_ancestor(path: Path) -> None:
    current = _absolute_without_follow(path).parent
    while True:
        info = _lstat(current)
        if info is not None and stat.S_ISLNK(info.st_mode):
            raise ValueError(
                f"refusing to write scientific artifact through a symlink parent: {path}"
            )
        nxt = current.parent
        if nxt == current:
            return
        current = nxt


def _require_contained_path(root: Path, path: Path, label: str) -> Path:
    root_abs = _lexically_normalized(root)
    path_abs = _lexically_normalized(path)
    if ".." in root_abs.parts or ".." in path_abs.parts:
        raise ValueError(f"{label} must not contain '..' segments")
    try:
        path_abs.relative_to(root_abs)
    except ValueError as error:
        raise ValueError(f"{label} must reside under the experiment directory") from error
    current = path_abs
    while True:
        info = _lstat(current)
        if info is not None and stat.S_ISLNK(info.st_mode):
            raise ValueError(f"{label} must not be a symlink: {current}")
        if current == root_abs:
            return path_abs
        nxt = current.parent
        if nxt == current:
            raise ValueError(f"{label} escapes the experiment directory: {path}")
        current = nxt


def resolve_inventory_path(root: Path, relative: str) -> Path:
    """Resolve a declared inventory path under root without following symlinks."""

    normalized = normalize_inventory_relative_path(relative)
    root_abs = _absolute_without_follow(root)
    candidate = root_abs.joinpath(*normalized.split("/"))
    try:
        candidate.relative_to(root_abs)
    except ValueError as error:
        raise ValueError(f"inventory path escapes experiment root: {relative}") from error
    return candidate


def _lstat(path: Path) -> os.stat_result | None:
    try:
        return os.lstat(path)
    except FileNotFoundError:
        return None


def _read_regular_file_bytes(path: Path) -> bytes:
    return read_regular_file_bytes(path)


def _read_contained_regular_file_bytes(container: Path, relative: str) -> bytes:
    """Read a declared descendant without following symlinks."""

    container_abs = _absolute_without_follow(container)
    _raise_if_symlink_ancestor(container)
    container_info = _lstat(container_abs)
    if container_info is not None and stat.S_ISLNK(container_info.st_mode):
        raise ValueError(f"scientific artifact container must not be a symlink: {container}")
    path = resolve_inventory_path(container, relative)
    current = path
    while True:
        info = _lstat(current)
        if info is not None and stat.S_ISLNK(info.st_mode):
            raise ValueError(f"scientific artifact must not be a symlink: {current}")
        if current == container_abs:
            break
        nxt = current.parent
        if nxt == current:
            raise ValueError(f"scientific artifact escapes container: {relative}")
        current = nxt
    return _read_regular_file_bytes(path)


def write_scientific_artifact(path: Path, content: bytes) -> str:
    """Write immutable scientific bytes once. Identical content is idempotent."""

    digest = sha256_bytes(content)
    name = _require_basename(path)
    with held_parent_directory(path) as parent_fd:
        existing = _open_existing_regular(parent_fd, name, path)
        if existing is not None:
            existing_bytes = _read_fd_bytes(existing)
            if existing_bytes == content:
                return sha256_bytes(existing_bytes)
            raise ValueError(f"refusing to mutate scientific artifact: {path}")
        write_bytes_via_parent_fd(parent_fd, name, content, replace=False, path=path)
    return digest


def replace_file_bytes(path: Path, content: bytes) -> None:
    """Replace a regular file relative to a held parent directory fd."""

    name = _require_basename(path)
    with held_parent_directory(path) as parent_fd:
        write_bytes_via_parent_fd(parent_fd, name, content, replace=True, path=path)


@dataclass(frozen=True, slots=True)
class ArtifactRef:
    role: str
    path: str
    sha256: str

    def to_dict(self) -> dict[str, object]:
        return {"role": self.role, "path": self.path, "sha256": self.sha256}


@dataclass(frozen=True, slots=True)
class ExperimentPredeclaration:
    kind: str
    schema_version: int
    name: str
    source_commit: str
    source_worktree_must_be_clean: bool
    promotion_claim: bool
    consumed_evidence_policy: Mapping[str, bool]
    inputs: tuple[ArtifactRef, ...]
    outputs: tuple[ArtifactRef, ...]
    environment: Mapping[str, str | None]

    def to_dict(self) -> dict[str, object]:
        return {
            "kind": self.kind,
            "schema_version": self.schema_version,
            "name": self.name,
            "source_commit": self.source_commit,
            "source_worktree_must_be_clean": self.source_worktree_must_be_clean,
            "promotion_claim": self.promotion_claim,
            "consumed_evidence_policy": dict(self.consumed_evidence_policy),
            "inputs": [ref.to_dict() for ref in self.inputs],
            "outputs": [ref.to_dict() for ref in self.outputs],
            "environment": dict(self.environment),
        }


@dataclass(frozen=True, slots=True)
class IntegrityMismatch:
    relative_path: str
    declared_sha256: str
    actual_sha256: str | None

    def to_dict(self) -> dict[str, object]:
        return {
            "relative_path": self.relative_path,
            "declared_sha256": self.declared_sha256,
            "actual_sha256": self.actual_sha256,
        }


@dataclass(frozen=True, slots=True)
class ArtifactIntegrityReport:
    ok: bool
    experiment_dir: str
    inventory_path: str
    checked: int
    skipped_mutable: tuple[str, ...]
    missing: tuple[str, ...]
    mismatches: tuple[IntegrityMismatch, ...]
    undeclared_scientific: tuple[str, ...]
    undeclared_policy: str
    symlink_violations: tuple[str, ...]

    def to_dict(self) -> dict[str, object]:
        return {
            "ok": self.ok,
            "experiment_dir": self.experiment_dir,
            "inventory_path": self.inventory_path,
            "checked": self.checked,
            "skipped_mutable": list(self.skipped_mutable),
            "missing": list(self.missing),
            "mismatches": [item.to_dict() for item in self.mismatches],
            "undeclared_scientific": list(self.undeclared_scientific),
            "undeclared_policy": self.undeclared_policy,
            "symlink_violations": list(self.symlink_violations),
        }

    def summary_message(self) -> str:
        if self.ok:
            return f"artifact integrity ok: {self.checked} scientific files"
        parts: list[str] = []
        if self.missing:
            parts.append(f"missing={list(self.missing)}")
        if self.mismatches:
            parts.append("mismatches=" + ", ".join(item.relative_path for item in self.mismatches))
        if self.symlink_violations:
            parts.append(f"symlinks={list(self.symlink_violations)}")
        if self.undeclared_policy == UNDECLARED_POLICY_STRICT and self.undeclared_scientific:
            parts.append(f"undeclared={list(self.undeclared_scientific)}")
        return "artifact integrity failed: " + "; ".join(parts)


class ArtifactIntegrityError(ValueError):
    def __init__(self, report: ArtifactIntegrityReport) -> None:
        self.report = report
        super().__init__(report.summary_message())


class ExperimentReproductionError(ValueError):
    """Raised when source, artifact, or evidence-policy identities drifted."""


def parse_sha256sum_inventory(content: bytes) -> tuple[tuple[str, str], ...]:
    if type(content) is not bytes:
        raise TypeError("artifact inventory must be bytes")
    if not content:
        return ()
    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError("artifact inventory is not UTF-8") from error
    if "\r" in text:
        raise ValueError("artifact inventory must use canonical newline separators")
    if not text.endswith("\n"):
        raise ValueError("artifact inventory must end with a newline")
    entries: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line_number, raw in enumerate(text[:-1].split("\n"), start=1):
        if not raw or raw.strip() != raw:
            raise ValueError(f"inventory line {line_number} is blank or noncanonical")
        matched = _INVENTORY_LINE.fullmatch(raw)
        if matched is None:
            raise ValueError(f"inventory line {line_number} is not sha256sum format")
        digest = _require_digest(matched.group(1), f"inventory line {line_number} digest")
        relative = normalize_inventory_relative_path(matched.group(2))
        if relative in seen:
            raise ValueError(f"inventory lists {relative} more than once")
        seen.add(relative)
        entries.append((relative, digest))
    ordered = tuple(sorted(entries, key=lambda item: item[0]))
    rebuilt = "".join(f"{digest}  ./{relative}\n" for relative, digest in ordered)
    if rebuilt.encode("utf-8") != content:
        raise ValueError("artifact inventory bytes are not canonical")
    return ordered


def _relative_to_root(root: Path, path: Path) -> Path:
    return _absolute_without_follow(path).relative_to(_absolute_without_follow(root))


def _symlink_violation_relative(root: Path, path: Path) -> str | None:
    root_abs = _absolute_without_follow(root)
    current = _absolute_without_follow(path)
    while True:
        if current.is_symlink():
            if current == root_abs:
                return "."
            try:
                return current.relative_to(root_abs).as_posix()
            except ValueError:
                return current.as_posix()
        if current == root_abs:
            return None
        parent = current.parent
        if parent == current:
            return None
        current = parent


def _iter_tree_entries(root: Path) -> Iterator[tuple[Path, str]]:
    root_abs = _absolute_without_follow(root)
    if root_abs.is_symlink():
        yield root_abs, "symlink"
        return
    for dirpath, dirnames, filenames in os.walk(root_abs, followlinks=False, topdown=True):
        directory = Path(dirpath)
        for name in list(dirnames):
            child = directory / name
            if child.is_symlink():
                yield child, "symlink"
        for name in filenames:
            child = directory / name
            if child.is_symlink():
                yield child, "symlink"
            elif child.is_file():
                yield child, "file"


def _symlink_is_allowed(root: Path, path: Path, relative: Path) -> bool:
    del root, path, relative
    return False


def _scientific_file_digests(root: Path) -> dict[str, str]:
    mapping: dict[str, str] = {}
    for path, kind in _iter_tree_entries(root):
        if kind != "file":
            continue
        relative = _relative_to_root(root, path)
        if is_mutable_synchronization_path(relative):
            continue
        mapping[relative.as_posix()] = sha256_bytes(_read_regular_file_bytes(path))
    return mapping


def _symlink_violations(root: Path) -> tuple[str, ...]:
    found: set[str] = set()
    root_hit = _symlink_violation_relative(root, root)
    if root_hit is not None:
        found.add(root_hit)
        return tuple(sorted(found))
    for path, kind in _iter_tree_entries(root):
        relative = _relative_to_root(root, path)
        if kind == "symlink" or path.is_symlink():
            if not _symlink_is_allowed(root, path, relative):
                found.add(relative.as_posix())
            continue
        if is_mutable_synchronization_path(relative):
            continue
        violation = _symlink_violation_relative(root, path)
        if violation is not None:
            found.add(violation)
    return tuple(sorted(found))


def _undeclared_policy_for(root: Path) -> str:
    path = root / "predeclaration.json"
    info = _lstat(path)
    if info is None:
        raise ValueError("experiment directory is missing predeclaration.json")
    if stat.S_ISLNK(info.st_mode):
        raise ValueError("predeclaration.json must not be a symlink")
    load_experiment_predeclaration(path)
    return UNDECLARED_POLICY_STRICT


def write_artifact_inventory(experiment_dir: Path) -> Path:
    root = _absolute_without_follow(experiment_dir)
    if not root.is_dir() or root.is_symlink():
        if root.is_symlink():
            raise ValueError(f"experiment directory must not be a symlink: {root}")
        raise FileNotFoundError(f"experiment directory does not exist: {root}")
    violations = _symlink_violations(root)
    if violations:
        raise ValueError(f"refusing to inventory symlink paths: {list(violations)}")
    lines: list[str] = []
    for path, kind in sorted(_iter_tree_entries(root), key=lambda item: item[0].as_posix()):
        if kind != "file":
            continue
        relative = _relative_to_root(root, path)
        if relative.as_posix() == ARTIFACT_INVENTORY_NAME or is_mutable_synchronization_path(
            relative
        ):
            continue
        digest = sha256_bytes(_read_regular_file_bytes(path))
        lines.append(f"{digest}  ./{relative.as_posix()}\n")
    lines.sort(key=lambda line: line.split("  ./", 1)[1])
    destination = root / ARTIFACT_INVENTORY_NAME
    write_scientific_artifact(destination, "".join(lines).encode())
    return destination


def _load_inventory_entries(inventory_path: Path) -> tuple[tuple[str, str], ...]:
    return parse_sha256sum_inventory(_read_regular_file_bytes(inventory_path))


def verify_artifact_integrity(experiment_dir: Path) -> ArtifactIntegrityReport:
    """Check declared scientific digests. Timing files are observational."""

    root = _absolute_without_follow(experiment_dir)
    if not root.is_dir():
        raise FileNotFoundError(f"experiment directory does not exist: {root}")
    if root.is_symlink():
        raise ValueError(f"experiment directory must not be a symlink: {root}")
    inventory = _require_contained_path(
        root,
        root / ARTIFACT_INVENTORY_NAME,
        "artifact inventory",
    )
    if not inventory.is_file():
        raise FileNotFoundError(f"artifact inventory does not exist: {inventory}")
    skipped: list[str] = []
    missing: list[str] = []
    mismatches: list[IntegrityMismatch] = []
    checked = 0
    declared: set[str] = set()
    symlink_hits = set(_symlink_violations(root))
    undeclared_policy = _undeclared_policy_for(root)
    for relative, expected in _load_inventory_entries(inventory):
        relative_path = Path(relative)
        if is_mutable_synchronization_path(relative_path):
            skipped.append(relative)
            continue
        declared.add(relative)
        actual_path = resolve_inventory_path(root, relative)
        chain = _symlink_violation_relative(root, actual_path)
        if chain is not None:
            symlink_hits.add(chain)
            continue
        if actual_path.is_symlink():
            symlink_hits.add(relative)
            continue
        if not actual_path.is_file():
            missing.append(relative)
            mismatches.append(IntegrityMismatch(relative, expected, None))
            continue
        actual = sha256_bytes(_read_regular_file_bytes(actual_path))
        checked += 1
        if actual != expected:
            mismatches.append(IntegrityMismatch(relative, expected, actual))
    inventory_relative = _relative_to_root(root, inventory).as_posix()
    undeclared = tuple(
        sorted(
            path
            for path in _scientific_file_digests(root)
            if path not in declared and path != inventory_relative
        )
    )
    symlink_violations = tuple(sorted(symlink_hits))
    undeclared_fails = undeclared_policy == UNDECLARED_POLICY_STRICT and bool(undeclared)
    report = ArtifactIntegrityReport(
        ok=not missing and not mismatches and not symlink_violations and not undeclared_fails,
        experiment_dir=str(root),
        inventory_path=str(inventory),
        checked=checked,
        skipped_mutable=tuple(skipped),
        missing=tuple(missing),
        mismatches=tuple(mismatches),
        undeclared_scientific=undeclared,
        undeclared_policy=undeclared_policy,
        symlink_violations=symlink_violations,
    )
    if not report.ok:
        raise ArtifactIntegrityError(report)
    return report


def _artifact_ref(value: object, label: str) -> ArtifactRef:
    source = _require_mapping(value, label)
    if set(source) != _ARTIFACT_REF_KEYS:
        raise ValueError(f"{label} has missing or unknown fields")
    return ArtifactRef(
        role=_require_string(source["role"], f"{label}.role"),
        path=_require_string(source["path"], f"{label}.path"),
        sha256=_require_digest(source["sha256"], f"{label}.sha256"),
    )


def _evidence_policy(value: object) -> dict[str, bool]:
    source = _require_mapping(value, "consumed_evidence_policy")
    if set(source) != _EVIDENCE_POLICY_KEYS:
        raise ValueError("consumed_evidence_policy has missing or unknown fields")
    return {
        key: _require_bool(source[key], f"consumed_evidence_policy.{key}")
        for key in sorted(_EVIDENCE_POLICY_KEYS)
    }


def _environment(value: object) -> dict[str, str | None]:
    source = _require_mapping(value, "environment")
    if set(source) != _ENVIRONMENT_KEYS:
        raise ValueError("environment has missing or unknown fields")
    result: dict[str, str | None] = {}
    for key in sorted(_ENVIRONMENT_KEYS):
        item = source[key]
        if item is None:
            result[key] = None
            continue
        result[key] = _require_digest(item, f"environment.{key}")
    return result


def _load_v1_predeclaration(payload: Mapping[str, object]) -> ExperimentPredeclaration:
    if set(payload) != _V1_KEYS:
        raise ValueError("experiment predeclaration v1 has missing or unknown fields")
    if payload["kind"] != PREDECLARATION_KIND:
        raise ValueError("experiment predeclaration kind mismatch")
    schema_version = payload["schema_version"]
    if type(schema_version) is not int or schema_version != PREDECLARATION_SCHEMA_VERSION:
        raise TypeError("unsupported experiment predeclaration schema version")
    promotion = _require_bool(payload["promotion_claim"], "promotion_claim")
    if promotion:
        raise ExperimentReproductionError("predeclaration v1 refuses promotion claims")
    policy = _evidence_policy(payload["consumed_evidence_policy"])
    if policy["sealed_test"] or policy["real_trace_audit"]:
        raise ExperimentReproductionError("predeclaration v1 refuses sealed/audit evidence")
    if not policy["development_only_for_assessment"]:
        raise ValueError("experiment predeclaration requires development_only_for_assessment")
    if not _require_bool(
        payload["source_worktree_must_be_clean"], "source_worktree_must_be_clean"
    ):
        raise ValueError("experiment predeclaration requires source_worktree_must_be_clean")
    inputs = tuple(
        _artifact_ref(item, f"inputs[{index}]")
        for index, item in enumerate(_require_list(payload["inputs"], "inputs"))
    )
    outputs = tuple(
        _artifact_ref(item, f"outputs[{index}]")
        for index, item in enumerate(_require_list(payload["outputs"], "outputs"))
    )
    return ExperimentPredeclaration(
        kind=PREDECLARATION_KIND,
        schema_version=PREDECLARATION_SCHEMA_VERSION,
        name=_require_string(payload["name"], "name"),
        source_commit=_require_git_sha(payload["source_commit"], "source_commit"),
        source_worktree_must_be_clean=True,
        promotion_claim=False,
        consumed_evidence_policy=MappingProxyType(policy),
        inputs=inputs,
        outputs=outputs,
        environment=MappingProxyType(_environment(payload["environment"])),
    )


def _require_list(value: object, label: str) -> list[object]:
    if type(value) is not list:
        raise TypeError(f"{label} must be an array")
    return cast(list[object], value)


def _parse_experiment_predeclaration(path: Path) -> tuple[ExperimentPredeclaration, bytes]:
    content = _read_regular_file_bytes(path)
    payload = json.loads(content)
    source = _require_mapping(payload, "predeclaration")
    if source.get("kind") != PREDECLARATION_KIND:
        raise ValueError("unsupported experiment predeclaration")
    validated = _load_v1_predeclaration(source)
    if content != canonical_bytes(validated.to_dict()):
        raise ValueError("experiment predeclaration bytes are not canonical")
    return validated, content


def load_experiment_predeclaration(path: Path) -> ExperimentPredeclaration:
    return _parse_experiment_predeclaration(path)[0]


def _register_declared_path(seen: set[str], path_key: str) -> None:
    if path_key in seen:
        raise ExperimentReproductionError(f"duplicate artifact path: {path_key}")
    seen.add(path_key)


def _contained_inventory_relative(experiment_dir: Path, path: Path) -> str | None:
    normalized_root = _lexically_normalized(experiment_dir)
    normalized_path = _lexically_normalized(path)
    try:
        relative = normalized_path.relative_to(normalized_root)
    except ValueError:
        return None
    return normalize_inventory_relative_path(relative.as_posix())


def _artifact_identity(
    ref: ArtifactRef,
    *,
    experiment_dir: Path,
    inventory_paths: set[str],
    seen_paths: set[str],
    as_output: bool,
) -> dict[str, object]:
    path = Path(ref.path)
    if path.is_absolute():
        if as_output:
            raise ExperimentReproductionError(
                "output artifact paths must be inventory-relative contained paths"
            )
        normalized = _lexically_normalized(path)
        if path != normalized:
            raise ExperimentReproductionError(f"declared {ref.role} path is not canonical")
        _register_declared_path(seen_paths, str(normalized))
        contained_relative = _contained_inventory_relative(experiment_dir, normalized)
        if contained_relative is not None and contained_relative not in inventory_paths:
            raise ExperimentReproductionError(
                f"declared {ref.role} artifact is not in the inventory: {contained_relative}"
            )
        content = _read_regular_file_bytes(normalized)
        resolved = normalized
    else:
        relative = normalize_inventory_relative_path(ref.path)
        if relative != ref.path:
            raise ExperimentReproductionError(f"declared {ref.role} path is not canonical")
        resolved = resolve_inventory_path(experiment_dir, relative)
        _register_declared_path(seen_paths, str(_lexically_normalized(resolved)))
        if relative not in inventory_paths:
            raise ExperimentReproductionError(
                f"declared {ref.role} artifact is not in the inventory: {relative}"
            )
        content = _read_contained_regular_file_bytes(experiment_dir, relative)
    actual = sha256_bytes(content)
    if actual != ref.sha256:
        raise ExperimentReproductionError(
            f"declared {ref.role} artifact digest mismatch: {resolved}"
        )
    return json_content_identities_from_bytes(content, resolved)


def json_content_identities_from_bytes(content: bytes, path: Path) -> dict[str, object]:
    payload: dict[str, object] = {
        "path": str(path),
        "sha256": sha256_bytes(content),
    }
    try:
        loaded = json.loads(content)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return payload
    if type(loaded) is not dict:
        return payload
    source = cast(dict[str, object], loaded)
    if content != canonical_bytes(source):
        raise ValueError(f"JSON artifact bytes are not canonical: {path}")
    extracted: dict[str, object] = {}
    for key in _JSON_DIGEST_FIELDS:
        if key not in source:
            continue
        value = source[key]
        digest = _require_digest(value, key)
        if key in _SELF_DIGEST_KEYS and digest != digest_payload(source, key):
            raise ValueError(f"{key} does not match the canonical payload digest")
        extracted[key] = digest
    if extracted:
        payload["declared_digests"] = extracted
    return payload


def json_content_identities(path: Path) -> dict[str, object]:
    return json_content_identities_from_bytes(_read_regular_file_bytes(path), path)


def _empty_environment_sets() -> dict[str, set[str]]:
    return {key: set() for key in _ENVIRONMENT_KEYS}


def _observed_environment_values(
    refs: tuple[ArtifactRef, ...],
    identities: list[dict[str, object]],
) -> tuple[dict[str, set[str]], dict[str, set[str]]]:
    file_hashes = _empty_environment_sets()
    artifact_fields = _empty_environment_sets()
    if len(refs) != len(identities):
        raise ExperimentReproductionError("artifact refs and identities are misaligned")
    for ref, identity in zip(refs, identities, strict=True):
        file_key = _FILE_DIGEST_ROLES.get(ref.role)
        if file_key is not None:
            file_hashes[file_key].add(ref.sha256)
        declared = identity.get("declared_digests")
        if type(declared) is not dict:
            continue
        extracted = cast(dict[str, object], declared)
        for key, value in extracted.items():
            if type(value) is not str:
                continue
            if key in artifact_fields:
                artifact_fields[key].add(value)
            if key == "manifest_digest" and ref.role == "root_manifest":
                artifact_fields["root_manifest_digest"].add(value)
            if key == "manifest_digest" and ref.role in {"dataset_manifest", "dataset"}:
                artifact_fields["dataset_manifest_digest"].add(value)
    return file_hashes, artifact_fields


def _live_environment_observations() -> dict[str, str]:
    from .training import _digest, _runtime_identity, _source_digest

    observed: dict[str, str] = {}
    try:
        observed["source_digest"] = _source_digest()
    except (OSError, TypeError, ValueError):
        pass
    try:
        observed["runtime_identity_digest"] = _digest(_runtime_identity())
    except (OSError, TypeError, ValueError):
        pass
    return observed


def _validate_environment(
    declared: Mapping[str, str | None],
    file_hashes: Mapping[str, set[str]],
    artifact_fields: Mapping[str, set[str]],
    live_observed: Mapping[str, str],
) -> dict[str, object]:
    fields: dict[str, object] = {}
    failures: list[str] = []
    for key in sorted(_ENVIRONMENT_KEYS):
        value = declared.get(key)
        hashed = tuple(sorted(file_hashes.get(key, ())))
        attested = tuple(sorted(artifact_fields.get(key, ())))
        live = live_observed.get(key) if key in _LIVE_ENVIRONMENT_KEYS else None
        if value is None:
            status = "not_declared"
        elif key in _LIVE_ENVIRONMENT_KEYS:
            if live is None:
                status = "unobserved_live"
                failures.append(key)
            elif value == live:
                status = "matched_live"
            else:
                status = "mismatch"
                failures.append(key)
        elif value in file_hashes.get(key, ()):
            status = "independently_hashed"
        elif value in artifact_fields.get(key, ()):
            status = "artifact_attested"
        elif not hashed and not attested:
            status = "unobserved"
            failures.append(key)
        else:
            status = "mismatch"
            failures.append(key)
        fields[key] = {
            "declared": value,
            "observed_live": live,
            "observed_file_hash": list(hashed),
            "observed_artifact": list(attested),
            "status": status,
        }
    if failures:
        raise ExperimentReproductionError(
            "environment identity validation failed: " + ", ".join(failures)
        )
    return {"ok": True, "fields": fields}


def _consumed_sealed_or_audit_evidence(
    policy: Mapping[str, bool] | None,
) -> bool | None:
    if policy is None:
        return None
    known = [policy[key] for key in ("sealed_test", "real_trace_audit") if key in policy]
    if not known:
        return None
    if any(known):
        return True
    if len(known) < 2:
        return None
    return False


def reproduce_experiment(
    experiment_dir: Path,
    *,
    repository: Path,
) -> dict[str, object]:
    """Validate source and content identities. Does not retrain or repair artifacts."""

    experiment_root = _absolute_without_follow(experiment_dir)
    if experiment_root.is_symlink() or not experiment_root.is_dir():
        raise ExperimentReproductionError(
            f"experiment directory must be a real directory: {experiment_dir}"
        )
    predeclaration_path = experiment_root / _PREDECLARATION_NAME
    predeclaration, predeclaration_bytes = _parse_experiment_predeclaration(predeclaration_path)
    if predeclaration.promotion_claim:
        raise ExperimentReproductionError("reproduction refuses promotion claims")
    policy = predeclaration.consumed_evidence_policy
    if not predeclaration.source_worktree_must_be_clean:
        raise ExperimentReproductionError("reproduction requires source_worktree_must_be_clean")
    if not policy.get("development_only_for_assessment"):
        raise ExperimentReproductionError("reproduction requires development_only_for_assessment")
    if policy.get("sealed_test") or policy.get("real_trace_audit"):
        raise ExperimentReproductionError("reproduction refuses sealed/audit evidence")
    try:
        version = capture_repository_version(repository)
    except ValueError as error:
        raise ExperimentReproductionError(str(error)) from error
    if not version.clean:
        raise ExperimentReproductionError(
            "worktree is dirty; reproduction requires a clean checkout"
        )
    if version.git_sha != predeclaration.source_commit:
        raise ExperimentReproductionError(
            "source commit mismatch: "
            f"worktree={version.git_sha} declared={predeclaration.source_commit}"
        )
    inventory_path = experiment_root / ARTIFACT_INVENTORY_NAME
    if _lstat(inventory_path) is None:
        raise ExperimentReproductionError(
            f"experiment directory is missing {ARTIFACT_INVENTORY_NAME}"
        )
    inventory_entries = _load_inventory_entries(inventory_path)
    inventory_paths = {relative for relative, _digest in inventory_entries}
    expected_inventory_paths = {_PREDECLARATION_NAME}
    for ref in (*predeclaration.inputs, *predeclaration.outputs):
        path = Path(ref.path)
        relative = (
            _contained_inventory_relative(experiment_root, path)
            if path.is_absolute()
            else normalize_inventory_relative_path(ref.path)
        )
        if relative is not None:
            if relative in {_PREDECLARATION_NAME, ARTIFACT_INVENTORY_NAME}:
                raise ExperimentReproductionError(
                    f"declared artifact path is reserved: {relative}"
                )
            expected_inventory_paths.add(relative)
    if inventory_paths != expected_inventory_paths:
        missing = sorted(expected_inventory_paths - inventory_paths)
        extra = sorted(inventory_paths - expected_inventory_paths)
        raise ExperimentReproductionError(
            f"artifact inventory membership mismatch: missing={missing}, extra={extra}"
        )
    seen_paths: set[str] = set()
    input_identities = [
        _artifact_identity(
            ref,
            experiment_dir=experiment_root,
            inventory_paths=inventory_paths,
            seen_paths=seen_paths,
            as_output=False,
        )
        for ref in predeclaration.inputs
    ]
    output_identities = [
        _artifact_identity(
            ref,
            experiment_dir=experiment_root,
            inventory_paths=inventory_paths,
            seen_paths=seen_paths,
            as_output=True,
        )
        for ref in predeclaration.outputs
    ]
    file_hashes, artifact_fields = _observed_environment_values(
        (*predeclaration.inputs, *predeclaration.outputs),
        input_identities + output_identities,
    )
    need_live = any(
        predeclaration.environment.get(key) is not None for key in _LIVE_ENVIRONMENT_KEYS
    )
    environment_validation = _validate_environment(
        predeclaration.environment,
        file_hashes,
        artifact_fields,
        _live_environment_observations() if need_live else {},
    )
    integrity = verify_artifact_integrity(experiment_root).to_dict()
    report = {
        "kind": "experiment_reproduction_report",
        "report_version": 2,
        "ok": True,
        "predeclaration_path": str(predeclaration_path),
        "predeclaration_sha256": sha256_bytes(predeclaration_bytes),
        "predeclaration_kind": predeclaration.kind,
        "name": predeclaration.name,
        "source_commit": version.git_sha,
        "repository": version.to_dict(),
        "promotion_claim": False,
        "consumed_sealed_or_audit_evidence": _consumed_sealed_or_audit_evidence(policy),
        "inputs": [ref.to_dict() for ref in predeclaration.inputs],
        "outputs": [ref.to_dict() for ref in predeclaration.outputs],
        "input_identities": input_identities,
        "output_identities": output_identities,
        "environment": {
            "declared": dict(predeclaration.environment),
            "validation": environment_validation,
        },
        "artifact_integrity": integrity,
    }
    report["report_digest"] = sha256_bytes(canonical_bytes(report))
    return report
