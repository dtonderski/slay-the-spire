"""Content-addressed experiment provenance, integrity, and reproduction."""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import tempfile
from collections.abc import Iterator, Mapping
from dataclasses import dataclass
from pathlib import Path
from types import MappingProxyType
from typing import cast

from .provenance import capture_repository_version, file_digest
from .tracking import (
    DEFAULT_LOCAL_WANDB_BASE_URL,
    MUTABLE_SYNCHRONIZATION_DIRECTORY_NAME,
    discover_offline_wandb_runs,
    is_wandb_synchronization_path,
    sync_offline_wandb,
)

PREDECLARATION_KIND = "experiment_predeclaration_v1"
PREDECLARATION_SCHEMA_VERSION = 1
ARTIFACT_INVENTORY_NAME = "artifact-inventory.sha256"
UNDECLARED_POLICY_STRICT = "strict"
UNDECLARED_POLICY_REPORT_ONLY = "report_only"
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
_ALLOWED_MUTABLE_SYMLINK_NAMES = frozenset({"latest-run"})


def _canonical_bytes(payload: object) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


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


def _fsync_directory(directory: Path) -> None:
    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _absolute_without_follow(path: Path) -> Path:
    if path.is_absolute():
        return path
    return Path.cwd() / path


def is_mutable_synchronization_path(relative: Path) -> bool:
    """Mutable observational metadata: W&B trees, timing files, and sync sidecars."""

    if relative.is_absolute():
        raise ValueError("artifact paths must be relative to the experiment directory")
    if is_wandb_synchronization_path(relative):
        return True
    name = relative.name
    return name.endswith(".time.txt")


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
    root_abs = _absolute_without_follow(root)
    path_abs = _absolute_without_follow(path)
    try:
        path_abs.relative_to(root_abs)
    except ValueError as error:
        raise ValueError(f"{label} must reside under the experiment directory") from error
    current = path_abs
    while True:
        if current.is_symlink():
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
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    except OSError as error:
        raise ValueError(
            f"refusing to read scientific artifact through a non-regular file: {path}"
        ) from error
    try:
        info = os.fstat(descriptor)
        if not stat.S_ISREG(info.st_mode):
            raise ValueError(
                f"refusing to read scientific artifact through a non-regular file: {path}"
            )
        with os.fdopen(descriptor, "rb") as handle:
            descriptor = -1
            return handle.read()
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def write_scientific_artifact(path: Path, content: bytes) -> str:
    """Write immutable scientific bytes once. Identical content is idempotent."""

    digest = _sha256_bytes(content)
    _raise_if_symlink_ancestor(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    _fsync_directory(path.parent)
    existing_info = _lstat(path)
    if existing_info is not None:
        if path.is_symlink() or not stat.S_ISREG(existing_info.st_mode):
            raise ValueError(f"refusing to write scientific artifact through a symlink: {path}")
        existing = _read_regular_file_bytes(path)
        if existing == content:
            return _sha256_bytes(existing)
        raise ValueError(f"refusing to mutate scientific artifact: {path}")
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError:
            if path.is_symlink():
                raise ValueError(
                    f"refusing to write scientific artifact through a symlink: {path}"
                ) from None
            existing = _read_regular_file_bytes(path)
            if existing == content:
                return _sha256_bytes(existing)
            raise ValueError(f"refusing to mutate scientific artifact: {path}") from None
        _fsync_directory(path.parent)
    finally:
        if os.path.lexists(temporary):
            os.unlink(temporary)
            _fsync_directory(path.parent)
    return digest


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
    schema_version: int | None
    name: str
    source_commit: str
    source_worktree_must_be_clean: bool
    promotion_claim: bool
    consumed_evidence_policy: Mapping[str, bool] | None
    inputs: tuple[ArtifactRef, ...]
    outputs: tuple[ArtifactRef, ...]
    environment: Mapping[str, str | None]
    payload: Mapping[str, object]

    def to_dict(self) -> dict[str, object]:
        return dict(self.payload)


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


def parse_sha256sum_inventory(text: str) -> tuple[tuple[str, str], ...]:
    entries: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line_number, raw in enumerate(text.splitlines(), start=1):
        if not raw:
            continue
        matched = _INVENTORY_LINE.fullmatch(raw)
        if matched is None:
            raise ValueError(f"inventory line {line_number} is not sha256sum format")
        digest = _require_digest(matched.group(1), f"inventory line {line_number} digest")
        relative = normalize_inventory_relative_path(matched.group(2))
        if relative in seen:
            raise ValueError(f"inventory lists {relative} more than once")
        seen.add(relative)
        entries.append((relative, digest))
    return tuple(entries)


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


def _symlink_target_within_root(root: Path, path: Path) -> bool:
    raw = os.readlink(path)
    target = Path(raw)
    if not target.is_absolute():
        target = path.parent / target
    try:
        _lexically_normalized(target).relative_to(_lexically_normalized(root))
    except ValueError:
        return False
    return True


def _symlink_is_allowed(root: Path, path: Path, relative: Path) -> bool:
    if not is_mutable_synchronization_path(relative):
        return False
    if path.name not in _ALLOWED_MUTABLE_SYMLINK_NAMES:
        return False
    return _symlink_target_within_root(root, path)


def _scientific_file_digests(root: Path) -> dict[str, str]:
    mapping: dict[str, str] = {}
    for path, kind in _iter_tree_entries(root):
        if kind != "file":
            continue
        relative = _relative_to_root(root, path)
        if is_mutable_synchronization_path(relative):
            continue
        mapping[relative.as_posix()] = _sha256_bytes(_read_regular_file_bytes(path))
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
    if path.is_symlink():
        raise ValueError("predeclaration.json must not be a symlink")
    if not path.is_file():
        return UNDECLARED_POLICY_REPORT_ONLY
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("predeclaration.json is not valid JSON") from error
    source = _require_mapping(payload, "predeclaration")
    if source.get("kind") == PREDECLARATION_KIND:
        _load_v1_predeclaration(source)
        return UNDECLARED_POLICY_STRICT
    return UNDECLARED_POLICY_REPORT_ONLY


def write_artifact_inventory(
    experiment_dir: Path,
    *,
    inventory_name: str = ARTIFACT_INVENTORY_NAME,
) -> Path:
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
        if relative.as_posix() == inventory_name or is_mutable_synchronization_path(relative):
            continue
        digest = _sha256_bytes(_read_regular_file_bytes(path))
        lines.append(f"{digest}  ./{relative.as_posix()}\n")
    destination = root / inventory_name
    write_scientific_artifact(destination, "".join(lines).encode())
    return destination


def _load_inventory_entries(inventory_path: Path) -> tuple[tuple[str, str], ...]:
    if inventory_path.is_symlink():
        raise ValueError(f"artifact inventory must not be a symlink: {inventory_path}")
    return parse_sha256sum_inventory(inventory_path.read_text(encoding="utf-8"))


def verify_artifact_integrity(
    experiment_dir: Path,
    *,
    inventory_path: Path | None = None,
) -> ArtifactIntegrityReport:
    """Check declared scientific digests. W&B and timing files are observational."""

    root = _absolute_without_follow(experiment_dir)
    if not root.is_dir():
        raise FileNotFoundError(f"experiment directory does not exist: {root}")
    if root.is_symlink():
        raise ValueError(f"experiment directory must not be a symlink: {root}")
    inventory = _require_contained_path(
        root,
        inventory_path or root / ARTIFACT_INVENTORY_NAME,
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
        actual = _sha256_bytes(_read_regular_file_bytes(actual_path))
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
    if payload["schema_version"] != PREDECLARATION_SCHEMA_VERSION:
        raise ValueError("unsupported experiment predeclaration schema version")
    promotion = _require_bool(payload["promotion_claim"], "promotion_claim")
    if promotion:
        raise ExperimentReproductionError("predeclaration v1 refuses promotion claims")
    policy = _evidence_policy(payload["consumed_evidence_policy"])
    if policy["sealed_test"] or policy["real_trace_audit"]:
        raise ExperimentReproductionError("predeclaration v1 refuses sealed/audit evidence")
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
        source_worktree_must_be_clean=_require_bool(
            payload["source_worktree_must_be_clean"], "source_worktree_must_be_clean"
        ),
        promotion_claim=False,
        consumed_evidence_policy=MappingProxyType(policy),
        inputs=inputs,
        outputs=outputs,
        environment=MappingProxyType(_environment(payload["environment"])),
        payload=MappingProxyType(dict(payload)),
    )


def _require_list(value: object, label: str) -> list[object]:
    if type(value) is not list:
        raise TypeError(f"{label} must be an array")
    return cast(list[object], value)


def _load_legacy_predeclaration(payload: Mapping[str, object]) -> ExperimentPredeclaration:
    name_value = payload.get("name")
    if type(name_value) is str:
        name = _require_string(name_value, "name")
    else:
        name = "legacy_predeclaration"
    clean_value = payload.get("source_worktree_must_be_clean")
    if clean_value is None:
        clean = True
    else:
        clean = _require_bool(clean_value, "source_worktree_must_be_clean")
    promotion_value = payload.get("promotion_claim")
    promotion = (
        False if promotion_value is None else _require_bool(promotion_value, "promotion_claim")
    )
    policy_value = payload.get("consumed_evidence_policy")
    policy: dict[str, bool] | None
    if policy_value is None:
        policy = None
    else:
        source = _require_mapping(policy_value, "consumed_evidence_policy")
        policy = {}
        for key in ("sealed_test", "real_trace_audit"):
            if key in source:
                policy[key] = _require_bool(source[key], f"consumed_evidence_policy.{key}")
    return ExperimentPredeclaration(
        kind="legacy_predeclaration",
        schema_version=None,
        name=name,
        source_commit=_require_git_sha(payload.get("source_commit"), "source_commit"),
        source_worktree_must_be_clean=clean,
        promotion_claim=promotion,
        consumed_evidence_policy=None if policy is None else MappingProxyType(policy),
        inputs=(),
        outputs=(),
        environment=MappingProxyType({}),
        payload=MappingProxyType(dict(payload)),
    )


def load_experiment_predeclaration(path: Path) -> ExperimentPredeclaration:
    payload = json.loads(path.read_text(encoding="utf-8"))
    source = _require_mapping(payload, "predeclaration")
    if source.get("kind") == PREDECLARATION_KIND:
        return _load_v1_predeclaration(source)
    return _load_legacy_predeclaration(source)


def _resolve_declared_path(path_text: str, experiment_dir: Path | None) -> Path:
    path = Path(path_text)
    if path.is_absolute():
        return path
    if experiment_dir is None:
        raise ExperimentReproductionError(
            f"relative artifact path {path_text} requires an experiment directory"
        )
    return experiment_dir / path


def _verify_artifact_ref(ref: ArtifactRef, experiment_dir: Path | None) -> None:
    path = _resolve_declared_path(ref.path, experiment_dir)
    if not path.is_file():
        raise ExperimentReproductionError(f"declared {ref.role} artifact is missing: {path}")
    actual = file_digest(path)
    if actual != ref.sha256:
        raise ExperimentReproductionError(f"declared {ref.role} artifact digest mismatch: {path}")


def json_content_identities(path: Path) -> dict[str, object]:
    payload: dict[str, object] = {
        "path": str(path),
        "sha256": file_digest(path),
    }
    try:
        loaded = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return payload
    if type(loaded) is not dict:
        return payload
    source = cast(dict[str, object], loaded)
    extracted: dict[str, object] = {}
    for key in _JSON_DIGEST_FIELDS:
        value = source.get(key)
        if type(value) is str:
            extracted[key] = value
    if extracted:
        payload["declared_digests"] = extracted
    return payload


def _observed_environment_values(
    refs: tuple[ArtifactRef, ...],
    identities: list[dict[str, object]],
) -> dict[str, set[str]]:
    observed: dict[str, set[str]] = {key: set() for key in _ENVIRONMENT_KEYS}
    if len(refs) != len(identities):
        raise ExperimentReproductionError("artifact refs and identities are misaligned")
    for ref, identity in zip(refs, identities, strict=True):
        file_key = _FILE_DIGEST_ROLES.get(ref.role)
        if file_key is not None:
            observed[file_key].add(ref.sha256)
        declared = identity.get("declared_digests")
        if type(declared) is not dict:
            continue
        extracted = cast(dict[str, object], declared)
        for key, value in extracted.items():
            if type(value) is not str:
                continue
            if key in observed:
                observed[key].add(value)
            if key == "manifest_digest" and ref.role == "root_manifest":
                observed["root_manifest_digest"].add(value)
            if key == "manifest_digest" and ref.role in {"dataset_manifest", "dataset"}:
                observed["dataset_manifest_digest"].add(value)
    return observed


def _validate_environment(
    declared: Mapping[str, str | None],
    observed: Mapping[str, set[str]],
) -> dict[str, object]:
    fields: dict[str, object] = {}
    failures: list[str] = []
    for key in sorted(_ENVIRONMENT_KEYS):
        value = declared.get(key)
        found = tuple(sorted(observed.get(key, ())))
        if value is None:
            status = "not_declared"
        elif value in observed.get(key, ()):
            status = "matched"
        elif not found:
            status = "unobserved"
            failures.append(key)
        else:
            status = "mismatch"
            failures.append(key)
        fields[key] = {"declared": value, "observed": list(found), "status": status}
    if failures:
        raise ExperimentReproductionError(
            "environment identity validation failed: " + ", ".join(failures)
        )
    return {"ok": True, "fields": fields}


def reproduce_experiment(
    predeclaration_path: Path,
    *,
    repository: Path,
    experiment_dir: Path | None = None,
    allow_dirty: bool = False,
) -> dict[str, object]:
    """Validate source and content identities. Does not retrain or repair artifacts."""

    predeclaration = load_experiment_predeclaration(predeclaration_path)
    if predeclaration.promotion_claim:
        raise ExperimentReproductionError("reproduction refuses promotion claims")
    policy = predeclaration.consumed_evidence_policy
    if policy is not None and (policy.get("sealed_test") or policy.get("real_trace_audit")):
        raise ExperimentReproductionError("reproduction refuses sealed/audit evidence")
    allow_dirty_capture = allow_dirty and not predeclaration.source_worktree_must_be_clean
    try:
        version = capture_repository_version(repository, allow_dirty=allow_dirty_capture)
    except ValueError as error:
        raise ExperimentReproductionError(str(error)) from error
    if predeclaration.source_worktree_must_be_clean and not version.clean:
        raise ExperimentReproductionError(
            "worktree is dirty; reproduction requires a clean checkout"
        )
    if version.git_sha != predeclaration.source_commit:
        raise ExperimentReproductionError(
            "source commit mismatch: "
            f"worktree={version.git_sha} declared={predeclaration.source_commit}"
        )
    for ref in (*predeclaration.inputs, *predeclaration.outputs):
        _verify_artifact_ref(ref, experiment_dir)
    input_identities = [
        json_content_identities(_resolve_declared_path(ref.path, experiment_dir))
        for ref in predeclaration.inputs
    ]
    output_identities = [
        json_content_identities(_resolve_declared_path(ref.path, experiment_dir))
        for ref in predeclaration.outputs
    ]
    environment_validation = _validate_environment(
        predeclaration.environment,
        _observed_environment_values(
            (*predeclaration.inputs, *predeclaration.outputs),
            input_identities + output_identities,
        ),
    )
    integrity: dict[str, object] | None = None
    if experiment_dir is not None and (experiment_dir / ARTIFACT_INVENTORY_NAME).is_file():
        integrity = verify_artifact_integrity(experiment_dir).to_dict()
    report = {
        "kind": "experiment_reproduction_report",
        "report_version": 1,
        "ok": True,
        "predeclaration_path": str(predeclaration_path),
        "predeclaration_sha256": file_digest(predeclaration_path),
        "predeclaration_kind": predeclaration.kind,
        "name": predeclaration.name,
        "source_commit": version.git_sha,
        "repository": version.to_dict(),
        "promotion_claim": False,
        "consumed_sealed_or_audit_evidence": False,
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
    report["report_digest"] = _sha256_bytes(_canonical_bytes(report))
    return report


def sync_experiment_wandb(
    directory: Path,
    *,
    base_url: str = DEFAULT_LOCAL_WANDB_BASE_URL,
) -> dict[str, object]:
    """Sync W&B metadata only; refuse if scientific bytes change."""

    root = _absolute_without_follow(directory)
    before = _scientific_file_digests(root)
    before_symlinks = _symlink_violations(root)
    result = sync_offline_wandb(root, base_url=base_url)
    after = _scientific_file_digests(root)
    after_symlinks = _symlink_violations(root)
    if before != after or before_symlinks != after_symlinks:
        changed = sorted(set(before) | set(after))
        drifted = [path for path in changed if before.get(path) != after.get(path)]
        raise ArtifactIntegrityError(
            ArtifactIntegrityReport(
                ok=False,
                experiment_dir=str(root),
                inventory_path="",
                checked=len(before),
                skipped_mutable=tuple(
                    _relative_to_root(root, path).as_posix()
                    for path, kind in _iter_tree_entries(root)
                    if kind == "file"
                    and is_mutable_synchronization_path(_relative_to_root(root, path))
                ),
                missing=(),
                mismatches=tuple(
                    IntegrityMismatch(path, before.get(path, ""), after.get(path))
                    for path in drifted
                ),
                undeclared_scientific=(),
                undeclared_policy=UNDECLARED_POLICY_REPORT_ONLY,
                symlink_violations=after_symlinks,
            )
        )
    result = dict(result)
    result["scientific_artifacts_modified"] = False
    result["offline_run_count"] = len(discover_offline_wandb_runs(root))
    result["mutable_directory_name"] = MUTABLE_SYNCHRONIZATION_DIRECTORY_NAME
    return result
