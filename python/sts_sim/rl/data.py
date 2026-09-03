"""Deterministic legal combat roots and immutable replanning-beam datasets."""

from __future__ import annotations

import hashlib
import json
import os
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, cast

from .._native import UnknownPublicContentError, UnmodeledPublicContentError
from ..fair import FairCombatObservation
from ..run import Action, Decision, RunEnv
from .experiment import (
    _raise_if_symlink_ancestor,
    _read_contained_regular_file_bytes,
    _read_regular_file_bytes,
    resolve_inventory_path,
    write_scientific_artifact,
)
from .provenance import (
    RepositoryVersion,
    canonical_bytes,
    capture_repository_version,
    digest_payload,
    require_digest,
    sha256_bytes,
)
from .records import (
    BEAM_TEACHER_NAME,
    COMBAT_PROXY_VALUE_TARGET_NAME,
    PUCT_TEACHER_NAME,
    RECORD_VERSION,
    CombatOutcome,
    JsonValue,
    SymbolicTrainingRecord,
    action_descriptor_from_payload,
    canonical_episode_id,
    fair_observation_digest,
    fair_observation_from_payload,
    first_argmax_visits,
    parse_jsonl_records,
    validate_beam_search_config,
    validate_search_config,
)
from .rewards import COMBAT_PROXY_V1, CombatRewardConfig
from .source_epoch import (
    SOURCE_EPOCH_DIRNAME,
    SourceEpochBundle,
    copy_source_epoch_bundle,
    load_source_epoch_bundle,
    verify_loaded_native_bytes,
    write_source_epoch_bundle,
)

ROOT_MANIFEST_VERSION = 6
DATASET_MANIFEST_VERSION = 7
_SPLIT_SALT = "combat-agent-phase2-v1"
_GENERATOR_NAME = "legal_run_policy"
_GENERATOR_VERSION = "sha256_action_policy_v4"
_LINEAGE_PREFIX = "sim-seed:"
_ALLOWED_ROOT_EXCLUSION_REASONS = frozenset(
    {
        "step_limit",
        "terminal_run",
        "terminal_combat",
        "hp_zero_player_turn",
        "unmodeled_public_content",
        "duplicate_root",
        "withheld_audited_split",
        "generation_error",
    }
)
_ROOT_MANIFEST_KEYS = frozenset(
    {
        "manifest_version",
        "generator_name",
        "generator_version",
        "generator_source_digest",
        "repository",
        "ascension",
        "max_run_steps",
        "combat_depth",
        "split_salt",
        "requested_seeds",
        "cohort_digest",
        "source_epoch_bundle_digest",
        "roots",
        "exclusions",
        "manifest_digest",
    }
)
_ASSIGNED_SPLITS = {"train", "development", "sealed_test"}
_LOADABLE_SPLITS = {"train", "development"}
_AUDITED_SPLITS = {"sealed_test", "real_trace_audit"}
_SOURCE_KIND = "simulator_legal_v1"
_DATASET_ROOT_MANIFEST_PATH = "provenance/root-manifest.json"
_NATIVE_EPISODE_ERROR = "native_episode_error"


def _package_repository_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _lineage_for_seed(seed: str) -> str:
    return f"{_LINEAGE_PREFIX}{seed}"


def _require_nonempty_string(value: object, label: str) -> str:
    if type(value) is not str or not value:
        raise TypeError(f"{label} must be a nonempty string")
    return value


def _require_nonempty_strings(value: object, label: str) -> tuple[str, ...]:
    if type(value) is not list:
        raise TypeError(f"{label} must be an array")
    items: list[str] = []
    for item in cast(list[object], value):
        if type(item) is not str or not item:
            raise TypeError(f"{label} entries must be nonempty strings")
        items.append(item)
    return tuple(items)


def _atomic_write(path: Path, content: bytes) -> None:
    write_scientific_artifact(path, content)


def _canonical_requested_seeds(seeds: list[str]) -> tuple[str, ...]:
    if not seeds or len(seeds) != len(set(seeds)):
        raise ValueError("root seeds must be nonempty and unique")
    if any(type(seed) is not str or not seed for seed in seeds):
        raise TypeError("root seeds must be nonempty strings")
    return tuple(sorted(seeds))


def _cohort_contract(
    *,
    requested_seeds: tuple[str, ...],
    generator_name: str,
    generator_version: str,
    generator_source_digest: str,
    split_salt: str,
    ascension: int,
    max_run_steps: int,
    combat_depth: int,
) -> dict[str, object]:
    return {
        "requested_seeds": list(requested_seeds),
        "generator_name": generator_name,
        "generator_version": generator_version,
        "generator_source_digest": generator_source_digest,
        "split_salt": split_salt,
        "ascension": ascension,
        "max_run_steps": max_run_steps,
        "combat_depth": combat_depth,
    }


def _cohort_digest(
    *,
    requested_seeds: tuple[str, ...],
    generator_name: str,
    generator_version: str,
    generator_source_digest: str,
    split_salt: str,
    ascension: int,
    max_run_steps: int,
    combat_depth: int,
) -> str:
    return sha256_bytes(
        canonical_bytes(
            _cohort_contract(
                requested_seeds=requested_seeds,
                generator_name=generator_name,
                generator_version=generator_version,
                generator_source_digest=generator_source_digest,
                split_salt=split_salt,
                ascension=ascension,
                max_run_steps=max_run_steps,
                combat_depth=combat_depth,
            )
        )
    )


def _teacher_search_contract_digest(
    teacher_name: str, teacher_version: str, search_config: dict[str, object]
) -> str:
    return sha256_bytes(
        canonical_bytes(
            {
                "teacher_name": teacher_name,
                "teacher_version": teacher_version,
                "search_config": search_config,
            }
        )
    )


def _split_group_id(lineages: tuple[str, ...]) -> str:
    return sha256_bytes(canonical_bytes(list(lineages)))


def _split_for_lineage(lineage: str) -> str:
    digest = hashlib.sha256(f"{_SPLIT_SALT}\0{lineage}".encode()).digest()
    bucket = int.from_bytes(digest[:8], "big") % 100
    if bucket < 70:
        return "train"
    if bucket < 85:
        return "development"
    return "sealed_test"


@dataclass(frozen=True, slots=True)
class RootEntry:
    root_id: str
    split: str
    split_group_id: str
    relative_path: str
    lineages: tuple[str, ...]
    source_seeds: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class RootExclusion:
    source_seed: str
    reason: str
    detail: str


@dataclass(frozen=True, slots=True)
class RootManifest:
    manifest_version: int
    generator_name: str
    generator_version: str
    generator_source_digest: str
    repository: RepositoryVersion
    ascension: int
    max_run_steps: int
    combat_depth: int
    split_salt: str
    requested_seeds: tuple[str, ...]
    cohort_digest: str
    source_epoch_bundle_digest: str
    roots: tuple[RootEntry, ...]
    exclusions: tuple[RootExclusion, ...]
    manifest_digest: str

    def to_dict(self) -> dict[str, object]:
        return {
            "manifest_version": self.manifest_version,
            "generator_name": self.generator_name,
            "generator_version": self.generator_version,
            "generator_source_digest": self.generator_source_digest,
            "repository": self.repository.to_dict(),
            "ascension": self.ascension,
            "max_run_steps": self.max_run_steps,
            "combat_depth": self.combat_depth,
            "split_salt": self.split_salt,
            "requested_seeds": list(self.requested_seeds),
            "cohort_digest": self.cohort_digest,
            "source_epoch_bundle_digest": self.source_epoch_bundle_digest,
            "roots": [
                {
                    **asdict(root),
                    "lineages": list(root.lineages),
                    "source_seeds": list(root.source_seeds),
                }
                for root in self.roots
            ],
            "exclusions": [asdict(exclusion) for exclusion in self.exclusions],
            "manifest_digest": self.manifest_digest,
        }

    @classmethod
    def from_dict(cls, payload: object) -> RootManifest:
        if type(payload) is not dict:
            raise TypeError("root manifest must be an object")
        source = cast(dict[str, object], payload)
        if set(source) != _ROOT_MANIFEST_KEYS:
            raise ValueError("unsupported or malformed root manifest")
        version = source.get("manifest_version")
        if type(version) is not int or version != ROOT_MANIFEST_VERSION:
            raise ValueError("unsupported or malformed root manifest")
        raw_depth = source["combat_depth"]
        if type(raw_depth) is not int or raw_depth <= 0:
            raise ValueError("root manifest combat depth is invalid")
        generator_name = source["generator_name"]
        generator_version = source["generator_version"]
        if (
            type(generator_name) is not str
            or not generator_name
            or type(generator_version) is not str
            or not generator_version
        ):
            raise TypeError("root manifest generator identity must be nonempty strings")
        if generator_name != _GENERATOR_NAME or generator_version != _GENERATOR_VERSION:
            raise ValueError("root manifest generator identity does not match schema version")
        if (
            type(source["roots"]) is not list
            or type(source["exclusions"]) is not list
            or type(source["requested_seeds"]) is not list
        ):
            raise TypeError("root manifest collections must be arrays")
        requested_seeds = tuple(cast(list[str], source["requested_seeds"]))
        roots: list[RootEntry] = []
        for raw in cast(list[object], source["roots"]):
            if type(raw) is not dict:
                raise TypeError("root entry must be an object")
            item = cast(dict[str, object], raw)
            if set(item) != set(RootEntry.__dataclass_fields__):
                raise ValueError("root entry has missing or unknown fields")
            lineages = _require_nonempty_strings(item["lineages"], "root lineages")
            source_seeds = _require_nonempty_strings(item["source_seeds"], "root source seeds")
            roots.append(
                RootEntry(
                    require_digest(item["root_id"], "root ID"),
                    _require_nonempty_string(item["split"], "root split"),
                    require_digest(item["split_group_id"], "split group ID"),
                    _require_nonempty_string(item["relative_path"], "root path"),
                    lineages,
                    source_seeds,
                )
            )
        exclusions: list[RootExclusion] = []
        for raw in cast(list[object], source["exclusions"]):
            if type(raw) is not dict or set(cast(dict[str, object], raw)) != set(
                RootExclusion.__dataclass_fields__
            ):
                raise ValueError("root exclusion is malformed")
            item = cast(dict[str, object], raw)
            exclusions.append(
                RootExclusion(
                    _require_nonempty_string(item["source_seed"], "exclusion source seed"),
                    _require_nonempty_string(item["reason"], "exclusion reason"),
                    _require_nonempty_string(item["detail"], "exclusion detail"),
                )
            )
        manifest = cls(
            ROOT_MANIFEST_VERSION,
            cast(str, source["generator_name"]),
            cast(str, source["generator_version"]),
            require_digest(source["generator_source_digest"], "generator source digest"),
            RepositoryVersion.from_dict(source["repository"]),
            cast(int, source["ascension"]),
            cast(int, source["max_run_steps"]),
            raw_depth,
            cast(str, source["split_salt"]),
            requested_seeds,
            require_digest(source["cohort_digest"], "root cohort digest"),
            require_digest(source["source_epoch_bundle_digest"], "source-epoch-bundle digest"),
            tuple(roots),
            tuple(exclusions),
            require_digest(source["manifest_digest"], "root manifest digest"),
        )
        if type(manifest.ascension) is not int or not 0 <= manifest.ascension <= 20:
            raise ValueError("root manifest ascension is invalid")
        if type(manifest.max_run_steps) is not int or manifest.max_run_steps <= 0:
            raise ValueError("root manifest step limit is invalid")
        if type(manifest.split_salt) is not str or manifest.split_salt != _SPLIT_SALT:
            raise ValueError("root manifest split salt is invalid")
        if not manifest.requested_seeds or any(
            type(seed) is not str or not seed for seed in manifest.requested_seeds
        ):
            raise TypeError("requested seeds must be nonempty strings")
        if manifest.requested_seeds != tuple(sorted(set(manifest.requested_seeds))):
            raise ValueError("requested seeds are not canonical")
        if manifest.generator_source_digest != sha256_bytes(
            canonical_bytes(manifest.repository.to_dict())
        ):
            raise ValueError("generator source digest does not match repository")
        if manifest.cohort_digest != _cohort_digest(
            requested_seeds=manifest.requested_seeds,
            generator_name=manifest.generator_name,
            generator_version=manifest.generator_version,
            generator_source_digest=manifest.generator_source_digest,
            split_salt=manifest.split_salt,
            ascension=manifest.ascension,
            max_run_steps=manifest.max_run_steps,
            combat_depth=manifest.combat_depth,
        ):
            raise ValueError("root cohort digest is invalid")
        if manifest.manifest_digest != digest_payload(manifest.to_dict(), "manifest_digest"):
            raise ValueError("root manifest digest is invalid")
        if tuple(roots) != tuple(sorted(roots, key=lambda root: root.root_id)):
            raise ValueError("root entries are not canonically ordered")
        if not manifest.repository.clean:
            raise ValueError("root manifest requires a clean repository")
        requested_set = set(manifest.requested_seeds)
        seen_root_ids: set[str] = set()
        seen_paths: set[str] = set()
        owned_seeds: set[str] = set()
        owned_lineages: set[str] = set()
        for root in roots:
            if root.root_id in seen_root_ids:
                raise ValueError("duplicate root ID")
            seen_root_ids.add(root.root_id)
            if root.relative_path in seen_paths:
                raise ValueError("duplicate root path")
            seen_paths.add(root.relative_path)
            if root.split not in _LOADABLE_SPLITS:
                raise ValueError("root manifest contains a sealed or unknown split")
            if root.relative_path != f"{root.split}/roots/{root.root_id}.json":
                raise ValueError("root path is not split-isolated and canonical")
            if len(root.source_seeds) != 1 or len(root.lineages) != 1:
                raise ValueError("root must have exactly one source seed and lineage")
            owner_seed = root.source_seeds[0]
            lineage = root.lineages[0]
            if lineage != _lineage_for_seed(owner_seed):
                raise ValueError("root lineage does not match its source seed")
            if owner_seed not in requested_set:
                raise ValueError("root source seed is outside the requested cohort")
            if owner_seed in owned_seeds:
                raise ValueError("duplicate root source seed")
            if lineage in owned_lineages:
                raise ValueError("duplicate root lineage")
            owned_seeds.add(owner_seed)
            owned_lineages.add(lineage)
            if root.split_group_id != _split_group_id(root.lineages):
                raise ValueError("split group does not match canonical lineages")
            if _split_for_lineage(lineage) != root.split:
                raise ValueError("root provenance crosses splits")
        if tuple(exclusions) != tuple(
            sorted(
                exclusions,
                key=lambda exclusion: (exclusion.source_seed, exclusion.reason, exclusion.detail),
            )
        ):
            raise ValueError("root exclusions are not canonically ordered")
        exclusion_seeds: set[str] = set()
        published_root_ids = {root.root_id for root in roots}
        owner_by_root = {root.root_id: root.source_seeds[0] for root in roots}
        for exclusion in exclusions:
            if exclusion.source_seed not in requested_set:
                raise ValueError("exclusion source seed is outside the requested cohort")
            if exclusion.source_seed in owned_seeds:
                raise ValueError("source seed has both a root and an exclusion")
            if exclusion.source_seed in exclusion_seeds:
                raise ValueError("duplicate exclusion source seed")
            if exclusion.reason not in _ALLOWED_ROOT_EXCLUSION_REASONS:
                raise ValueError("root exclusion reason is unsupported")
            exclusion_seeds.add(exclusion.source_seed)
            if exclusion.reason == "duplicate_root":
                prefix = "duplicate of "
                if not exclusion.detail.startswith(prefix):
                    raise ValueError("duplicate_root exclusion is malformed")
                named_root = exclusion.detail[len(prefix) :]
                require_digest(named_root, "duplicate root ID")
                if named_root not in published_root_ids:
                    raise ValueError("duplicate_root exclusion does not name a published root")
                if not owner_by_root[named_root] < exclusion.source_seed:
                    raise ValueError("duplicate_root owner is not the canonical first seed")
        if owned_seeds | exclusion_seeds != requested_set:
            raise ValueError("requested seed accounting is incomplete")
        return manifest


def parse_root_manifest(content: bytes) -> RootManifest:
    """Parse canonical root-manifest bytes without reading the path again."""

    try:
        payload = json.loads(content)
    except json.JSONDecodeError as error:
        raise ValueError("root manifest is not JSON") from error
    manifest = RootManifest.from_dict(payload)
    if content != canonical_bytes(manifest.to_dict()):
        raise ValueError("root manifest is not canonical")
    return manifest


def _require_canonical_root_snapshot(content: bytes, root_id: str) -> bytes:
    try:
        snapshot = json.loads(content)
    except json.JSONDecodeError as error:
        raise ValueError(f"root {root_id} is not JSON") from error
    canonical = canonical_bytes(snapshot)
    if content != canonical or sha256_bytes(canonical) != root_id:
        raise ValueError(f"root {root_id} is not canonical")
    return canonical


def _restore_labeled_root(container: Path, root: RootEntry) -> RunEnv:
    snapshot_bytes = _require_canonical_root_snapshot(
        _read_contained_regular_file_bytes(container, root.relative_path),
        root.root_id,
    )
    return RunEnv.from_snapshot(snapshot_bytes.decode())


def _dataset_declared_relative_files(
    manifest_name: str,
    manifest: DatasetManifest,
    named_root: RootManifest,
    bundle: SourceEpochBundle,
) -> frozenset[str]:
    provenance = Path(manifest.root_manifest_path).parent.as_posix()
    declared = {
        manifest_name,
        manifest.shard_path,
        manifest.root_manifest_path,
    }
    for root in named_root.roots:
        declared.add(f"{provenance}/{root.relative_path}")
    bundle_dir = f"{provenance}/{SOURCE_EPOCH_DIRNAME}"
    declared.update(f"{bundle_dir}/{relative}" for relative in bundle.relative_members())
    return frozenset(declared)


def _reject_undeclared_dataset_inputs(dataset_root: Path, declared_files: frozenset[str]) -> None:
    if dataset_root.is_symlink():
        raise ValueError("dataset directory must not be a symlink")
    allowed_directories = {""}
    for relative in declared_files:
        parent = Path(relative).parent
        while parent.as_posix() not in {".", ""}:
            allowed_directories.add(parent.as_posix())
            parent = parent.parent
    for dirpath, dirnames, filenames in os.walk(dataset_root, followlinks=False):
        current = Path(dirpath)
        if current.is_symlink():
            raise ValueError("dataset directory contains a symlink")
        relative_dir = current.relative_to(dataset_root).as_posix()
        if relative_dir == ".":
            relative_dir = ""
        for name in dirnames:
            child = current / name
            relative = name if relative_dir == "" else f"{relative_dir}/{name}"
            if child.is_symlink():
                raise ValueError(f"dataset directory contains a symlink: {relative}")
            if relative not in allowed_directories:
                raise ValueError(f"undeclared dataset input: {relative}")
        for name in filenames:
            child = current / name
            relative = name if relative_dir == "" else f"{relative_dir}/{name}"
            if child.is_symlink():
                raise ValueError(f"dataset directory contains a symlink: {relative}")
            if relative not in declared_files:
                raise ValueError(f"undeclared dataset input: {relative}")


def _verify_source_epoch_bundle(manifest: RootManifest, bundle_dir: Path) -> None:
    if not manifest.repository.clean:
        raise ValueError("root manifest requires a clean repository")
    bundle = load_source_epoch_bundle(bundle_dir)
    if bundle.bundle_digest != manifest.source_epoch_bundle_digest:
        raise ValueError("source-epoch-bundle digest does not match the root manifest")
    if bundle.git_sha != manifest.repository.git_sha:
        raise ValueError("root manifest repository does not match source-epoch-bundle git identity")
    if bundle.clean != manifest.repository.clean:
        raise ValueError("root manifest repository cleanliness does not match source-epoch-bundle")
    verify_loaded_native_bytes(bundle)


def _verify_loaded_root_manifest(manifest: RootManifest, parent: Path) -> None:
    _verify_source_epoch_bundle(manifest, resolve_inventory_path(parent, SOURCE_EPOCH_DIRNAME))
    for root in manifest.roots:
        canonical = _require_canonical_root_snapshot(
            _read_contained_regular_file_bytes(parent, root.relative_path),
            root.root_id,
        )
        restored = RunEnv.from_snapshot(canonical.decode())
        decision = restored.decision()
        if not _is_capturable_combat_decision(decision):
            raise ValueError(f"root {root.root_id} is not an actionable ongoing combat decision")
    _verify_duplicate_root_exclusions(manifest)


def _verify_duplicate_root_exclusions(manifest: RootManifest) -> None:
    for exclusion in manifest.exclusions:
        if exclusion.reason != "duplicate_root":
            continue
        named_root = exclusion.detail[len("duplicate of ") :]
        try:
            env = RunEnv.new_ironclad(exclusion.source_seed, manifest.ascension)
            snapshot, capture_exclusion = _capture_combat_root(
                env,
                exclusion.source_seed,
                combat_depth=manifest.combat_depth,
                max_run_steps=manifest.max_run_steps,
            )
        except (
            UnmodeledPublicContentError,
            UnknownPublicContentError,
            RuntimeError,
            TypeError,
            ValueError,
        ) as error:
            raise ValueError("duplicate_root exclusion did not reproduce the named root") from error
        if capture_exclusion is not None or snapshot is None:
            raise ValueError("duplicate_root exclusion did not reproduce the named root")
        if sha256_bytes(canonical_bytes(snapshot)) != named_root:
            raise ValueError("duplicate_root exclusion does not match named root")


def load_root_manifest(path: Path) -> RootManifest:
    _raise_if_symlink_ancestor(path)
    manifest = parse_root_manifest(_read_regular_file_bytes(path))
    _verify_loaded_root_manifest(manifest, path.parent)
    return manifest


def _policy_index(seed: str, step: int, actions: tuple[Action, ...]) -> int:
    payload = canonical_bytes(
        [seed, step, [asdict(action.descriptor()) for action in actions]]
    )
    return int.from_bytes(hashlib.sha256(payload).digest()[:8], "big") % len(actions)


def _require_empty_output_dir(output_dir: Path) -> None:
    if output_dir.exists():
        if not output_dir.is_dir():
            raise ValueError("output path must be a directory")
        if any(output_dir.iterdir()):
            raise ValueError("output directory must be empty")


def _is_combat_phase(decision: Decision) -> bool:
    return decision.phase == "combat"


def _is_capturable_combat_decision(decision: Decision) -> bool:
    observation = decision.observation
    return (
        _is_combat_phase(decision)
        and isinstance(observation, FairCombatObservation)
        and observation.phase == "waiting_for_player"
        and observation.player.hp > 0
        and bool(decision.actions)
        and not all(action.kind == "proceed" for action in decision.actions)
    )


def _update_combat_boundary(
    *,
    in_combat: bool,
    combat_index: int,
    decision: Decision,
) -> tuple[bool, int, bool]:
    if _is_combat_phase(decision):
        if in_combat:
            return True, combat_index, False
        return True, combat_index + 1, True
    return False, combat_index, False


def _depth_progress_detail(combat_index: int, combat_depth: int) -> str:
    return f"reached combat {combat_index} of requested depth {combat_depth}"


def _terminal_combat_detail(combat_index: int, combat_depth: int) -> str:
    return f"combat {combat_index} of requested depth {combat_depth} is not an ongoing policy root"


def _hp_zero_player_turn_detail(combat_index: int, combat_depth: int) -> str:
    return (
        f"player HP is 0 at a waiting_for_player decision in combat "
        f"{combat_index} of requested depth {combat_depth}"
    )


_UNMODELED_PUBLIC_CONTENT_PREFIX = "public combat content is unmodeled: "


def _unmodeled_public_content_exclusion(seed: str, error: BaseException) -> RootExclusion:
    message = str(error)
    if message.startswith(_UNMODELED_PUBLIC_CONTENT_PREFIX):
        public_key = message[len(_UNMODELED_PUBLIC_CONTENT_PREFIX) :].strip()
        if public_key and not public_key.isdigit():
            return RootExclusion(seed, "unmodeled_public_content", public_key)
        return RootExclusion(seed, "unmodeled_public_content", "unmodeled public content")
    return RootExclusion(seed, "unmodeled_public_content", "unknown public identity")


def _dead_player_turn(decision: Decision) -> bool:
    observation = decision.observation
    return (
        isinstance(observation, FairCombatObservation)
        and observation.phase == "waiting_for_player"
        and observation.player.hp <= 0
    )


def _combat_entry_exclusion(
    decision: Decision, *, combat_index: int, combat_depth: int
) -> str | None:
    if _dead_player_turn(decision):
        return "hp_zero_player_turn"
    if combat_index == combat_depth:
        if not _is_capturable_combat_decision(decision):
            return "terminal_combat"
        return None
    if not decision.actions:
        return "terminal_combat"
    return None


def _capture_combat_root(
    env: RunEnv,
    seed: str,
    *,
    combat_depth: int,
    max_run_steps: int,
) -> tuple[dict[str, object] | None, RootExclusion | None]:
    combat_index = 0
    in_combat = False
    for step in range(max_run_steps + 1):
        decision = env.decision()
        in_combat, combat_index, just_entered = _update_combat_boundary(
            in_combat=in_combat, combat_index=combat_index, decision=decision
        )
        if just_entered:
            exclusion_reason = _combat_entry_exclusion(
                decision, combat_index=combat_index, combat_depth=combat_depth
            )
            if exclusion_reason == "terminal_combat":
                return None, RootExclusion(
                    seed,
                    "terminal_combat",
                    _terminal_combat_detail(combat_index, combat_depth),
                )
            if exclusion_reason == "hp_zero_player_turn":
                return None, RootExclusion(
                    seed,
                    "hp_zero_player_turn",
                    _hp_zero_player_turn_detail(combat_index, combat_depth),
                )
            if combat_index == combat_depth:
                return json.loads(env.snapshot().json), None
        if step == max_run_steps:
            return None, RootExclusion(
                seed, "step_limit", _depth_progress_detail(combat_index, combat_depth)
            )
        if not decision.actions:
            return None, RootExclusion(
                seed, "terminal_run", _depth_progress_detail(combat_index, combat_depth)
            )
        env.step(decision.actions[_policy_index(seed, step, decision.actions)])
    raise RuntimeError("combat root capture did not terminate")


def generate_legal_roots(
    output_dir: Path,
    seeds: list[str],
    *,
    ascension: int = 0,
    max_run_steps: int = 256,
    combat_depth: int = 1,
) -> RootManifest:
    """Advance seeded runs only through accepted public legal transitions."""

    requested_seeds = _canonical_requested_seeds(seeds)
    if not 0 <= ascension <= 20 or max_run_steps <= 0:
        raise ValueError("invalid root generation bounds")
    if type(combat_depth) is not int or combat_depth <= 0:
        raise ValueError("combat depth must be a positive integer")
    _require_empty_output_dir(output_dir)
    repository = capture_repository_version(_package_repository_root())
    source_digest = sha256_bytes(canonical_bytes(repository.to_dict()))
    output_dir.mkdir(parents=True, exist_ok=True)
    bundle = write_source_epoch_bundle(output_dir / SOURCE_EPOCH_DIRNAME, repository)
    root_payloads: dict[str, tuple[dict[str, object], str]] = {}
    exclusions: list[RootExclusion] = []
    duplicate_candidates: list[tuple[str, str]] = []
    for seed in requested_seeds:
        env = RunEnv.new_ironclad(seed, ascension)
        try:
            snapshot, exclusion = _capture_combat_root(
                env, seed, combat_depth=combat_depth, max_run_steps=max_run_steps
            )
            if exclusion is not None:
                exclusions.append(exclusion)
                continue
            assert snapshot is not None
            canonical = canonical_bytes(snapshot)
            root_id = sha256_bytes(canonical)
            if root_id in root_payloads:
                duplicate_candidates.append((seed, root_id))
                continue
            root_payloads[root_id] = (snapshot, seed)
        except UnmodeledPublicContentError as error:
            exclusions.append(_unmodeled_public_content_exclusion(seed, error))
        except UnknownPublicContentError as error:
            exclusions.append(RootExclusion(seed, "generation_error", str(error)))
        except (RuntimeError, TypeError, ValueError) as error:
            exclusions.append(RootExclusion(seed, "generation_error", str(error)))

    entries: list[RootEntry] = []
    published: dict[str, str] = {}
    for root_id, (snapshot, owner_seed) in sorted(root_payloads.items()):
        lineage = _lineage_for_seed(owner_seed)
        split = _split_for_lineage(lineage)
        if split not in _LOADABLE_SPLITS:
            exclusions.append(RootExclusion(owner_seed, "withheld_audited_split", split))
            continue
        relative_path = f"{split}/roots/{root_id}.json"
        _atomic_write(output_dir / relative_path, canonical_bytes(snapshot))
        entries.append(
            RootEntry(
                root_id,
                split,
                _split_group_id((lineage,)),
                relative_path,
                (lineage,),
                (owner_seed,),
            )
        )
        published[root_id] = owner_seed
    for seed, root_id in duplicate_candidates:
        if root_id in published:
            exclusions.append(RootExclusion(seed, "duplicate_root", f"duplicate of {root_id}"))
            continue
        owner_seed = root_payloads[root_id][1]
        owner_split = _split_for_lineage(_lineage_for_seed(owner_seed))
        exclusions.append(RootExclusion(seed, "withheld_audited_split", owner_split))
    exclusions.sort(key=lambda item: (item.source_seed, item.reason, item.detail))
    cohort_digest = _cohort_digest(
        requested_seeds=requested_seeds,
        generator_name=_GENERATOR_NAME,
        generator_version=_GENERATOR_VERSION,
        generator_source_digest=source_digest,
        split_salt=_SPLIT_SALT,
        ascension=ascension,
        max_run_steps=max_run_steps,
        combat_depth=combat_depth,
    )
    unsigned: dict[str, object] = {
        "manifest_version": ROOT_MANIFEST_VERSION,
        "generator_name": _GENERATOR_NAME,
        "generator_version": _GENERATOR_VERSION,
        "generator_source_digest": source_digest,
        "repository": repository.to_dict(),
        "ascension": ascension,
        "max_run_steps": max_run_steps,
        "combat_depth": combat_depth,
        "split_salt": _SPLIT_SALT,
        "requested_seeds": list(requested_seeds),
        "cohort_digest": cohort_digest,
        "source_epoch_bundle_digest": bundle.bundle_digest,
        "roots": [
            {
                **asdict(root),
                "lineages": list(root.lineages),
                "source_seeds": list(root.source_seeds),
            }
            for root in entries
        ],
        "exclusions": [asdict(exclusion) for exclusion in exclusions],
    }
    manifest = RootManifest(
        ROOT_MANIFEST_VERSION,
        _GENERATOR_NAME,
        _GENERATOR_VERSION,
        source_digest,
        repository,
        ascension,
        max_run_steps,
        combat_depth,
        _SPLIT_SALT,
        requested_seeds,
        cohort_digest,
        bundle.bundle_digest,
        tuple(entries),
        tuple(exclusions),
        sha256_bytes(canonical_bytes(unsigned)),
    )
    _atomic_write(output_dir / "root-manifest.json", canonical_bytes(manifest.to_dict()))
    return load_root_manifest(output_dir / "root-manifest.json")


@dataclass(frozen=True, slots=True)
class DatasetRootMembership:
    root_id: str
    split_group_id: str
    split: str
    lineages: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class DatasetExclusion:
    root_id: str
    reason: str
    detail: str


@dataclass(frozen=True, slots=True)
class DatasetManifest:
    manifest_version: int
    root_manifest_path: str
    root_manifest_file_digest: str
    root_manifest_digest: str
    cohort_digest: str
    roots: tuple[DatasetRootMembership, ...]
    exclusions: tuple[DatasetExclusion, ...]
    split: str
    reward_config: dict[str, object]
    reward_config_digest: str
    teacher_name: str
    teacher_version: str
    teacher_search_contract_digest: str
    source_kind: str
    search_config: dict[str, object]
    repository: RepositoryVersion
    shard_path: str
    shard_digest: str
    record_count: int
    record_ids: tuple[str, ...]
    manifest_digest: str

    def to_dict(self) -> dict[str, object]:
        return {
            "manifest_version": self.manifest_version,
            "root_manifest_path": self.root_manifest_path,
            "root_manifest_file_digest": self.root_manifest_file_digest,
            "root_manifest_digest": self.root_manifest_digest,
            "cohort_digest": self.cohort_digest,
            "roots": [{**asdict(root), "lineages": list(root.lineages)} for root in self.roots],
            "exclusions": [asdict(exclusion) for exclusion in self.exclusions],
            "split": self.split,
            "reward_config": self.reward_config,
            "reward_config_digest": self.reward_config_digest,
            "teacher_name": self.teacher_name,
            "teacher_version": self.teacher_version,
            "teacher_search_contract_digest": self.teacher_search_contract_digest,
            "source_kind": self.source_kind,
            "search_config": self.search_config,
            "repository": self.repository.to_dict(),
            "shard_path": self.shard_path,
            "shard_digest": self.shard_digest,
            "record_count": self.record_count,
            "record_ids": list(self.record_ids),
            "manifest_digest": self.manifest_digest,
        }


def _require_loadable_split(split: str) -> str:
    if split not in _LOADABLE_SPLITS:
        raise ValueError("unknown or sealed dataset split")
    return split


def load_dataset_manifest(
    path: Path, *, requested_split: str
) -> tuple[DatasetManifest, RootManifest, tuple[SymbolicTrainingRecord, ...]]:
    _raise_if_symlink_ancestor(path)
    content = _read_regular_file_bytes(path)
    try:
        raw = json.loads(content)
    except json.JSONDecodeError as error:
        raise ValueError("dataset manifest is not JSON") from error
    if type(raw) is not dict:
        raise TypeError("dataset manifest must be an object")
    source = cast(dict[str, object], raw)
    if (
        set(source) != set(DatasetManifest.__dataclass_fields__)
        or source["manifest_version"] != DATASET_MANIFEST_VERSION
    ):
        raise ValueError("unsupported or malformed dataset manifest")
    if (
        type(source["roots"]) is not list
        or type(source["exclusions"]) is not list
        or type(source["record_ids"]) is not list
    ):
        raise TypeError("dataset manifest collections must be arrays")
    roots: list[DatasetRootMembership] = []
    for raw_root in cast(list[object], source["roots"]):
        if type(raw_root) is not dict or set(cast(dict[str, object], raw_root)) != set(
            DatasetRootMembership.__dataclass_fields__
        ):
            raise ValueError("dataset root membership is malformed")
        item = cast(dict[str, object], raw_root)
        if type(item["lineages"]) is not list:
            raise TypeError("dataset root lineages must be an array")
        roots.append(
            DatasetRootMembership(
                cast(str, item["root_id"]),
                cast(str, item["split_group_id"]),
                cast(str, item["split"]),
                tuple(cast(list[str], item["lineages"])),
            )
        )
    exclusions: list[DatasetExclusion] = []
    for raw_exclusion in cast(list[object], source["exclusions"]):
        if type(raw_exclusion) is not dict or set(cast(dict[str, object], raw_exclusion)) != set(
            DatasetExclusion.__dataclass_fields__
        ):
            raise ValueError("dataset exclusion is malformed")
        item = cast(dict[str, object], raw_exclusion)
        exclusions.append(
            DatasetExclusion(
                cast(str, item["root_id"]),
                cast(str, item["reason"]),
                cast(str, item["detail"]),
            )
        )
    if type(source["reward_config"]) is not dict or type(source["search_config"]) is not dict:
        raise TypeError("dataset configurations must be objects")
    manifest = DatasetManifest(
        DATASET_MANIFEST_VERSION,
        cast(str, source["root_manifest_path"]),
        require_digest(source["root_manifest_file_digest"], "root manifest file digest"),
        require_digest(source["root_manifest_digest"], "root manifest digest"),
        require_digest(source["cohort_digest"], "dataset cohort digest"),
        tuple(roots),
        tuple(exclusions),
        cast(str, source["split"]),
        cast(dict[str, object], source["reward_config"]),
        require_digest(source["reward_config_digest"], "reward config digest"),
        cast(str, source["teacher_name"]),
        cast(str, source["teacher_version"]),
        require_digest(source["teacher_search_contract_digest"], "teacher/search contract digest"),
        cast(str, source["source_kind"]),
        cast(dict[str, object], source["search_config"]),
        RepositoryVersion.from_dict(source["repository"]),
        cast(str, source["shard_path"]),
        require_digest(source["shard_digest"], "shard digest"),
        cast(int, source["record_count"]),
        tuple(cast(list[str], source["record_ids"])),
        require_digest(source["manifest_digest"], "dataset manifest digest"),
    )
    if manifest.split != requested_split or manifest.split not in _LOADABLE_SPLITS:
        raise ValueError("dataset split does not match requested split")
    if not manifest.repository.clean:
        raise ValueError("dataset manifest requires a clean repository")
    if manifest.source_kind != _SOURCE_KIND:
        raise ValueError("dataset source kind is unsupported")
    if (
        type(manifest.teacher_name) is not str
        or not manifest.teacher_name
        or type(manifest.teacher_version) is not str
        or not manifest.teacher_version
    ):
        raise TypeError("dataset teacher identity must be nonempty strings")
    validate_search_config(manifest.teacher_name, manifest.search_config)
    if manifest.root_manifest_path != _DATASET_ROOT_MANIFEST_PATH:
        raise ValueError("dataset root manifest path is not canonical")
    if manifest.shard_path != f"{manifest.split}/{manifest.split}.jsonl":
        raise ValueError("dataset shard path is not split-isolated and canonical")
    if (
        manifest.record_count <= 0
        or len(manifest.record_ids) != manifest.record_count
        or len(set(manifest.record_ids)) != manifest.record_count
    ):
        raise ValueError("dataset record IDs are not unique and complete")
    for record_id in manifest.record_ids:
        require_digest(record_id, "record ID")
    if tuple(roots) != tuple(sorted(roots, key=lambda root: root.root_id)) or len(
        {root.root_id for root in roots}
    ) != len(roots):
        raise ValueError("dataset root membership is not canonical")
    for root in roots:
        require_digest(root.root_id, "root ID")
        require_digest(root.split_group_id, "split group ID")
        if root.split != manifest.split:
            raise ValueError("dataset root belongs to another split")
        if not root.lineages or root.lineages != tuple(sorted(set(root.lineages))):
            raise ValueError("dataset root lineages are not canonical")
        if root.split_group_id != _split_group_id(root.lineages):
            raise ValueError("dataset root split group does not match its lineages")
    if tuple(exclusions) != tuple(
        sorted(
            exclusions,
            key=lambda exclusion: (exclusion.root_id, exclusion.reason, exclusion.detail),
        )
    ) or len({exclusion.root_id for exclusion in exclusions}) != len(exclusions):
        raise ValueError("dataset exclusions are not canonical")
    for exclusion in exclusions:
        require_digest(exclusion.root_id, "excluded root ID")
        if exclusion.reason != _NATIVE_EPISODE_ERROR:
            raise ValueError("dataset exclusion reason is unsupported")
        if type(exclusion.detail) is not str or not exclusion.detail:
            raise ValueError("dataset exclusion detail must be a nonempty public string")
    reward = CombatRewardConfig(**cast(dict[str, Any], manifest.reward_config))
    if reward.digest != manifest.reward_config_digest:
        raise ValueError("dataset reward configuration digest is invalid")
    if manifest.teacher_search_contract_digest != _teacher_search_contract_digest(
        manifest.teacher_name, manifest.teacher_version, manifest.search_config
    ):
        raise ValueError("dataset teacher/search contract digest is invalid")
    if manifest.manifest_digest != digest_payload(manifest.to_dict(), "manifest_digest"):
        raise ValueError("dataset manifest digest is invalid")
    if content != canonical_bytes(manifest.to_dict()):
        raise ValueError("dataset manifest is not canonical")
    named_root_bytes = _read_contained_regular_file_bytes(path.parent, manifest.root_manifest_path)
    if sha256_bytes(named_root_bytes) != manifest.root_manifest_file_digest:
        raise ValueError("dataset root manifest file digest is invalid")
    named_root_manifest = parse_root_manifest(named_root_bytes)
    named_root_parent = resolve_inventory_path(path.parent, manifest.root_manifest_path).parent
    _verify_loaded_root_manifest(named_root_manifest, named_root_parent)
    copied_bundle = load_source_epoch_bundle(
        resolve_inventory_path(path.parent, f"provenance/{SOURCE_EPOCH_DIRNAME}")
    )
    if copied_bundle.bundle_digest != named_root_manifest.source_epoch_bundle_digest:
        raise ValueError("copied source-epoch-bundle does not match the named root manifest")
    if copied_bundle.git_sha != manifest.repository.git_sha:
        raise ValueError("dataset repository does not match copied source-epoch-bundle git identity")
    if named_root_manifest.manifest_digest != manifest.root_manifest_digest:
        raise ValueError("dataset root manifest digest is invalid")
    if named_root_manifest.cohort_digest != manifest.cohort_digest:
        raise ValueError("dataset cohort digest is invalid")
    canonical_root_memberships = {
        root.root_id: root for root in named_root_manifest.roots if root.split == manifest.split
    }
    for membership in roots:
        root = canonical_root_memberships.get(membership.root_id)
        if root is None or (
            membership.split_group_id,
            membership.split,
            membership.lineages,
        ) != (root.split_group_id, root.split, root.lineages):
            raise ValueError("dataset root membership disagrees with named root manifest")
    successful_root_ids = {membership.root_id for membership in roots}
    excluded_root_ids = {exclusion.root_id for exclusion in exclusions}
    if successful_root_ids & excluded_root_ids:
        raise ValueError("dataset root accounting overlaps membership and exclusion")
    if excluded_root_ids - set(canonical_root_memberships):
        raise ValueError("dataset exclusion disagrees with named root manifest")
    if successful_root_ids | excluded_root_ids != set(canonical_root_memberships):
        raise ValueError("dataset root accounting is incomplete for named root manifest")
    shard_bytes = _read_contained_regular_file_bytes(path.parent, manifest.shard_path)
    if sha256_bytes(shard_bytes) != manifest.shard_digest:
        raise ValueError("dataset shard digest is invalid")
    records = parse_jsonl_records(shard_bytes)
    if (
        len(records) != manifest.record_count
        or tuple(record.record_id for record in records) != manifest.record_ids
    ):
        raise ValueError("dataset record order or count is invalid")
    memberships = {root.root_id: root for root in roots}
    seen_memberships: set[str] = set()
    for record in records:
        if record.record_version != RECORD_VERSION:
            raise ValueError("dataset record schema does not match the current record schema")
        if record.planner_name == PUCT_TEACHER_NAME:
            if record.value_target_name != COMBAT_PROXY_VALUE_TARGET_NAME:
                raise ValueError("PUCT records must use combat_proxy_v1 training targets")
            if sum(record.teacher_visit_counts) <= 0:
                raise ValueError("PUCT teacher labels must have positive visit mass")
            if record.chosen_action_index != first_argmax_visits(record.teacher_visit_counts):
                raise ValueError("PUCT chosen action is not the first visit-count argmax")
            if record.search_root_mean_value is None:
                raise ValueError("PUCT search root-mean diagnostic must be present")
        elif record.planner_name == BEAM_TEACHER_NAME:
            if (
                sum(record.teacher_visit_counts) != 1
                or record.teacher_visit_counts[record.chosen_action_index] != 1
            ):
                raise ValueError("beam-clone teacher labels must be one-hot at the chosen action")
            if record.value_target_name != reward.name:
                raise ValueError("dataset record reward contract mismatch")
            if record.search_root_mean_value is not None:
                raise ValueError("beam records must not carry a PUCT search root-mean")
        else:
            raise ValueError("dataset teacher identity is unsupported")
        expected_value = reward.value(record.outcome)
        if record.target_value != expected_value or record.value_target_mask != (
            expected_value is not None
        ):
            raise ValueError("dataset record value target does not match serialized outcome")
        expected_episode = canonical_episode_id(
            record.root_id,
            record.search_config,
            record.reward_config_digest,
        )
        if record.episode_id != expected_episode:
            raise ValueError("episode ID does not match canonical root/search/reward identity")
        membership = memberships.get(record.root_id)
        if membership is None or record.split_group_id != membership.split_group_id:
            raise ValueError("dataset record root/group membership mismatch")
        seen_memberships.add(record.root_id)
        if record.root_manifest_digest != manifest.root_manifest_digest:
            raise ValueError("dataset record root manifest mismatch")
        if (
            record.planner_name != manifest.teacher_name
            or record.planner_version != manifest.teacher_version
        ):
            raise ValueError("dataset record teacher mismatch")
        if record.reward_config_digest != manifest.reward_config_digest:
            raise ValueError("dataset record reward contract mismatch")
        if (
            record.source_kind != manifest.source_kind
            or record.search_config != manifest.search_config
        ):
            raise ValueError("dataset record source/search configuration mismatch")
        if record.repository != manifest.repository:
            raise ValueError("dataset record repository mismatch")
        for action in record.actions:
            action_descriptor_from_payload(
                {key: value for key, value in asdict(action).items() if value is not None}
            )
    first_by_episode: dict[str, SymbolicTrainingRecord] = {}
    for record in records:
        previous = first_by_episode.get(record.episode_id)
        if previous is None:
            first_by_episode[record.episode_id] = record
            continue
        if previous.outcome != record.outcome:
            raise ValueError("episode outcome is not identical across decisions")
        if (
            previous.target_value != record.target_value
            or previous.value_target_mask != record.value_target_mask
        ):
            raise ValueError("episode terminal z/mask is not identical across decisions")
    if seen_memberships != set(memberships):
        raise ValueError("dataset root membership contains no records")
    _reject_undeclared_dataset_inputs(
        path.parent,
        _dataset_declared_relative_files(
            path.name, manifest, named_root_manifest, copied_bundle
        ),
    )
    return manifest, named_root_manifest, records


def _publish_dataset(
    output_dir: Path,
    *,
    split: str,
    root_manifest: RootManifest,
    root_manifest_path: Path,
    records: list[SymbolicTrainingRecord],
    used_roots: list[DatasetRootMembership],
    exclusions: list[DatasetExclusion],
    teacher: tuple[str, str],
    search_config: dict[str, object],
    reward_config: CombatRewardConfig,
    repository: RepositoryVersion,
) -> DatasetManifest:
    if repository != root_manifest.repository:
        raise ValueError(
            "package repository identity does not match the authenticated root manifest"
        )
    lines = b"".join(canonical_bytes(record.to_dict()) + b"\n" for record in records)
    shard_name = f"{split}/{split}.jsonl"
    _atomic_write(output_dir / shard_name, lines)
    shard_digest = sha256_bytes(lines)
    memberships = tuple(sorted(used_roots, key=lambda root: root.root_id))
    dataset_exclusions = tuple(
        sorted(
            exclusions,
            key=lambda exclusion: (exclusion.root_id, exclusion.reason, exclusion.detail),
        )
    )
    root_manifest_bytes = canonical_bytes(root_manifest.to_dict())
    root_manifest_file_digest = sha256_bytes(root_manifest_bytes)
    _atomic_write(output_dir / _DATASET_ROOT_MANIFEST_PATH, root_manifest_bytes)
    provenance_dir = (output_dir / _DATASET_ROOT_MANIFEST_PATH).parent
    source_root_dir = root_manifest_path.parent
    for root in root_manifest.roots:
        _atomic_write(
            provenance_dir / root.relative_path,
            _read_contained_regular_file_bytes(source_root_dir, root.relative_path),
        )
    copy_source_epoch_bundle(
        root_manifest_path.parent / SOURCE_EPOCH_DIRNAME,
        output_dir / "provenance" / SOURCE_EPOCH_DIRNAME,
    )
    teacher_search_contract_digest = _teacher_search_contract_digest(
        teacher[0], teacher[1], search_config
    )
    unsigned: dict[str, object] = {
        "manifest_version": DATASET_MANIFEST_VERSION,
        "root_manifest_path": _DATASET_ROOT_MANIFEST_PATH,
        "root_manifest_file_digest": root_manifest_file_digest,
        "root_manifest_digest": root_manifest.manifest_digest,
        "cohort_digest": root_manifest.cohort_digest,
        "roots": [{**asdict(root), "lineages": list(root.lineages)} for root in memberships],
        "exclusions": [asdict(exclusion) for exclusion in dataset_exclusions],
        "split": split,
        "reward_config": reward_config.to_dict(),
        "reward_config_digest": reward_config.digest,
        "teacher_name": teacher[0],
        "teacher_version": teacher[1],
        "teacher_search_contract_digest": teacher_search_contract_digest,
        "source_kind": _SOURCE_KIND,
        "search_config": search_config,
        "repository": repository.to_dict(),
        "shard_path": shard_name,
        "shard_digest": shard_digest,
        "record_count": len(records),
        "record_ids": [record.record_id for record in records],
    }
    manifest = DatasetManifest(
        DATASET_MANIFEST_VERSION,
        _DATASET_ROOT_MANIFEST_PATH,
        root_manifest_file_digest,
        root_manifest.manifest_digest,
        root_manifest.cohort_digest,
        memberships,
        dataset_exclusions,
        split,
        reward_config.to_dict(),
        reward_config.digest,
        teacher[0],
        teacher[1],
        teacher_search_contract_digest,
        _SOURCE_KIND,
        search_config,
        repository,
        shard_name,
        shard_digest,
        len(records),
        tuple(record.record_id for record in records),
        sha256_bytes(canonical_bytes(unsigned)),
    )
    _atomic_write(output_dir / "dataset-manifest.json", canonical_bytes(manifest.to_dict()))
    loaded, _named_root, _records = load_dataset_manifest(
        output_dir / "dataset-manifest.json", requested_split=split
    )
    return loaded


def generate_beam_dataset(
    root_manifest_path: Path,
    output_dir: Path,
    *,
    split: str = "train",
    depth: int = 8,
    width: int = 24,
    transition_budget: int = 5_000,
    max_decisions: int = 512,
    max_player_turns: int = 100,
    deduplicate_search_states: bool = True,
    reward_config: CombatRewardConfig = COMBAT_PROXY_V1,
) -> DatasetManifest:
    split = _require_loadable_split(split)
    _require_empty_output_dir(output_dir)
    root_manifest = load_root_manifest(root_manifest_path)
    roots = [root for root in root_manifest.roots if root.split == split]
    if not roots:
        raise ValueError(f"root manifest contains no {split} roots")
    repository = capture_repository_version(_package_repository_root())
    if repository != root_manifest.repository:
        raise ValueError(
            "package repository identity does not match the authenticated root manifest"
        )
    search_config: dict[str, object] = {
        "depth": depth,
        "width": width,
        "transition_budget": transition_budget,
        "max_decisions": max_decisions,
        "max_player_turns": max_player_turns,
        "deadline": None,
        "replan": "every_public_decision",
        "deduplicate_search_states": deduplicate_search_states,
    }
    validate_beam_search_config(search_config)
    records: list[SymbolicTrainingRecord] = []
    teacher: tuple[str, str] | None = None
    used_roots: list[DatasetRootMembership] = []
    exclusions: list[DatasetExclusion] = []
    for root in roots:
        try:
            env = _restore_labeled_root(root_manifest_path.parent, root)
            payload = env.beam_clone_episode_payload(
                depth=depth,
                width=width,
                transition_budget=transition_budget,
                max_decisions=max_decisions,
                max_player_turns=max_player_turns,
                deduplicate_search_states=deduplicate_search_states,
            )
            if payload.get("schema_version") != 1:
                raise ValueError("unsupported native beam episode schema")
            native_teacher = (
                cast(str, payload["teacher_name"]),
                cast(str, payload["teacher_version"]),
            )
            if native_teacher[0] != BEAM_TEACHER_NAME:
                raise ValueError("native beam teacher identity is unsupported")
            if teacher is not None and teacher != native_teacher:
                raise ValueError("native teacher metadata changed within dataset")
            outcome = CombatOutcome.from_dict(payload["outcome"])
            target = reward_config.value(outcome)
            episode_id = canonical_episode_id(root.root_id, search_config, reward_config.digest)
            steps = cast(list[dict[str, object]], payload["steps"])
            if not steps:
                raise ValueError("terminal or post-combat root cannot produce training records")
            root_records: list[SymbolicTrainingRecord] = []
            for decision_index, step in enumerate(steps):
                observation = fair_observation_from_payload(step["observation"])
                actions = tuple(
                    action_descriptor_from_payload(choice)
                    for choice in cast(list[object], step["choices"])
                )
                selected = cast(int, step["selected_index"])
                counts = tuple(cast(list[int], step["teacher_visit_counts"]))
                root_records.append(
                    SymbolicTrainingRecord.create(
                        observation,
                        actions,
                        selected,
                        actions[selected],
                        counts,
                        target,
                        reward_config.name,
                        outcome,
                        native_teacher[0],
                        native_teacher[1],
                        cast(dict[str, JsonValue], search_config),
                        root.root_id,
                        root.split_group_id,
                        None,
                        repository,
                        fair_observation_digest(observation),
                        RECORD_VERSION,
                        root_manifest.manifest_digest,
                        reward_config.digest,
                        _SOURCE_KIND,
                        episode_id,
                        decision_index,
                        target is not None,
                        None,
                    )
                )
        except (
            AttributeError,
            IndexError,
            KeyError,
            OverflowError,
            RuntimeError,
            TypeError,
            ValueError,
        ) as error:
            detail = str(error).strip() or type(error).__name__
            exclusions.append(DatasetExclusion(root.root_id, _NATIVE_EPISODE_ERROR, detail))
            continue
        if teacher is None:
            teacher = native_teacher
        records.extend(root_records)
        used_roots.append(
            DatasetRootMembership(root.root_id, root.split_group_id, root.split, root.lineages)
        )
    if teacher is None:
        raise RuntimeError(
            f"all {len(roots)} {split} roots failed native episode labeling; "
            "no dataset was published"
        )
    return _publish_dataset(
        output_dir,
        split=split,
        root_manifest=root_manifest,
        root_manifest_path=root_manifest_path,
        records=records,
        used_roots=used_roots,
        exclusions=exclusions,
        teacher=teacher,
        search_config=search_config,
        reward_config=reward_config,
        repository=repository,
    )
