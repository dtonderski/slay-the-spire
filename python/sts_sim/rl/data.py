"""Deterministic legal combat roots and immutable replanning-beam datasets."""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, cast

from ..fair import FairCombatObservation
from ..run import Action, Decision, RunEnv
from .provenance import RepositoryVersion, capture_repository_version
from .records import (
    PUCT_TEACHER_NAME,
    PUCT_TEACHER_VERSION,
    PUCT_VALUE_TARGET_NAME,
    CombatOutcome,
    JsonValue,
    SymbolicTrainingRecord,
    action_descriptor_from_payload,
    fair_observation_digest,
    fair_observation_from_payload,
    fair_observation_payload,
    first_argmax_visits,
    read_jsonl,
    validate_v2_search_config,
    validate_v3_search_config,
)
from .rewards import COMBAT_PROXY_V1, CombatRewardConfig

ROOT_MANIFEST_V4 = 4
ROOT_MANIFEST_VERSION = 5
DATASET_MANIFEST_VERSION = 5
DATASET_MANIFEST_V6 = 6
_ACCEPTED_DATASET_MANIFEST_VERSIONS = {DATASET_MANIFEST_VERSION, DATASET_MANIFEST_V6}
_SPLIT_SALT = "combat-agent-phase2-v1"
_GENERATOR_NAME = "legal_run_policy"
_GENERATOR_VERSION_V4 = "sha256_action_policy_v3"
_GENERATOR_VERSION = "sha256_action_policy_v4"
_ROOT_MANIFEST_V4_KEYS = frozenset(
    {
        "manifest_version",
        "generator_name",
        "generator_version",
        "generator_source_digest",
        "repository",
        "ascension",
        "max_run_steps",
        "split_salt",
        "requested_seeds",
        "cohort_digest",
        "audited_splits_materialized",
        "roots",
        "exclusions",
        "manifest_digest",
    }
)
_ROOT_MANIFEST_V5_KEYS = _ROOT_MANIFEST_V4_KEYS | {"combat_depth"}
_ALLOWED_SPLITS = {"train", "development", "sealed_test", "real_trace_audit"}
_AUDITED_SPLITS = {"sealed_test", "real_trace_audit"}
_SOURCE_KIND = "simulator_legal_v1"
_DATASET_ROOT_MANIFEST_PATH = "provenance/root-manifest.json"
_NATIVE_EPISODE_ERROR = "native_episode_error"


def _canonical_bytes(payload: object) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _require_digest(value: object, label: str) -> str:
    if (
        type(value) is not str
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _atomic_write(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def _digest_payload(payload: dict[str, object], digest_key: str) -> str:
    unsigned = dict(payload)
    unsigned.pop(digest_key, None)
    return _sha256_bytes(_canonical_bytes(unsigned))


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
    combat_depth: int | None = None,
) -> dict[str, object]:
    contract: dict[str, object] = {
        "requested_seeds": list(requested_seeds),
        "generator_name": generator_name,
        "generator_version": generator_version,
        "generator_source_digest": generator_source_digest,
        "split_salt": split_salt,
        "ascension": ascension,
        "max_run_steps": max_run_steps,
    }
    if combat_depth is not None:
        contract["combat_depth"] = combat_depth
    return contract


def _cohort_digest(
    *,
    requested_seeds: tuple[str, ...],
    generator_name: str,
    generator_version: str,
    generator_source_digest: str,
    split_salt: str,
    ascension: int,
    max_run_steps: int,
    combat_depth: int | None = None,
) -> str:
    return _sha256_bytes(
        _canonical_bytes(
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
    return _sha256_bytes(
        _canonical_bytes(
            {
                "teacher_name": teacher_name,
                "teacher_version": teacher_version,
                "search_config": search_config,
            }
        )
    )


def _split_group_id(lineages: tuple[str, ...]) -> str:
    return _sha256_bytes(_canonical_bytes(list(lineages)))


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
    audited_splits_materialized: bool
    roots: tuple[RootEntry, ...]
    exclusions: tuple[RootExclusion, ...]
    manifest_digest: str

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "manifest_version": self.manifest_version,
            "generator_name": self.generator_name,
            "generator_version": self.generator_version,
            "generator_source_digest": self.generator_source_digest,
            "repository": self.repository.to_dict(),
            "ascension": self.ascension,
            "max_run_steps": self.max_run_steps,
            "split_salt": self.split_salt,
            "requested_seeds": list(self.requested_seeds),
            "cohort_digest": self.cohort_digest,
            "audited_splits_materialized": self.audited_splits_materialized,
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
        if self.manifest_version == ROOT_MANIFEST_VERSION:
            payload["combat_depth"] = self.combat_depth
        return payload

    @classmethod
    def from_dict(cls, payload: object) -> RootManifest:
        if type(payload) is not dict:
            raise TypeError("root manifest must be an object")
        source = cast(dict[str, object], payload)
        version = source.get("manifest_version")
        if type(version) is not int:
            raise ValueError("unsupported or malformed root manifest")
        if version == ROOT_MANIFEST_V4:
            if set(source) != _ROOT_MANIFEST_V4_KEYS:
                raise ValueError("unsupported or malformed root manifest")
            combat_depth = 1
            expected_generator_version = _GENERATOR_VERSION_V4
        elif version == ROOT_MANIFEST_VERSION:
            if set(source) != _ROOT_MANIFEST_V5_KEYS:
                raise ValueError("unsupported or malformed root manifest")
            raw_depth = source["combat_depth"]
            if type(raw_depth) is not int or raw_depth <= 0:
                raise ValueError("root manifest combat depth is invalid")
            combat_depth = raw_depth
            expected_generator_version = _GENERATOR_VERSION
        else:
            raise ValueError("unsupported or malformed root manifest")
        generator_name = source["generator_name"]
        generator_version = source["generator_version"]
        if (
            type(generator_name) is not str
            or not generator_name
            or type(generator_version) is not str
            or not generator_version
        ):
            raise TypeError("root manifest generator identity must be nonempty strings")
        if generator_name != _GENERATOR_NAME or generator_version != expected_generator_version:
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
            if type(item["lineages"]) is not list or type(item["source_seeds"]) is not list:
                raise TypeError("root provenance must be arrays")
            roots.append(
                RootEntry(
                    cast(str, item["root_id"]),
                    cast(str, item["split"]),
                    cast(str, item["split_group_id"]),
                    cast(str, item["relative_path"]),
                    tuple(cast(list[str], item["lineages"])),
                    tuple(cast(list[str], item["source_seeds"])),
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
                    cast(str, item["source_seed"]),
                    cast(str, item["reason"]),
                    cast(str, item["detail"]),
                )
            )
        manifest = cls(
            cast(int, source["manifest_version"]),
            cast(str, source["generator_name"]),
            cast(str, source["generator_version"]),
            _require_digest(source["generator_source_digest"], "generator source digest"),
            RepositoryVersion.from_dict(source["repository"]),
            cast(int, source["ascension"]),
            cast(int, source["max_run_steps"]),
            combat_depth,
            cast(str, source["split_salt"]),
            requested_seeds,
            _require_digest(source["cohort_digest"], "root cohort digest"),
            cast(bool, source["audited_splits_materialized"]),
            tuple(roots),
            tuple(exclusions),
            _require_digest(source["manifest_digest"], "root manifest digest"),
        )
        if type(manifest.ascension) is not int or not 0 <= manifest.ascension <= 20:
            raise ValueError("root manifest ascension is invalid")
        if type(manifest.max_run_steps) is not int or manifest.max_run_steps <= 0:
            raise ValueError("root manifest step limit is invalid")
        if type(manifest.combat_depth) is not int or manifest.combat_depth <= 0:
            raise ValueError("root manifest combat depth is invalid")
        if type(manifest.split_salt) is not str or manifest.split_salt != _SPLIT_SALT:
            raise ValueError("root manifest split salt is invalid")
        if not manifest.requested_seeds or any(
            type(seed) is not str or not seed for seed in manifest.requested_seeds
        ):
            raise TypeError("requested seeds must be nonempty strings")
        if manifest.requested_seeds != tuple(sorted(set(manifest.requested_seeds))):
            raise ValueError("requested seeds are not canonical")
        if type(manifest.audited_splits_materialized) is not bool:
            raise TypeError("audited materialization flag must be boolean")
        if manifest.generator_source_digest != _sha256_bytes(
            _canonical_bytes(manifest.repository.to_dict())
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
            combat_depth=(
                manifest.combat_depth
                if manifest.manifest_version == ROOT_MANIFEST_VERSION
                else None
            ),
        ):
            raise ValueError("root cohort digest is invalid")
        if manifest.manifest_digest != _digest_payload(manifest.to_dict(), "manifest_digest"):
            raise ValueError("root manifest digest is invalid")
        if tuple(roots) != tuple(sorted(roots, key=lambda root: root.root_id)):
            raise ValueError("root entries are not canonically ordered")
        requested_set = set(manifest.requested_seeds)
        seen: set[str] = set()
        for root in roots:
            _require_digest(root.root_id, "root ID")
            _require_digest(root.split_group_id, "split group ID")
            if root.root_id in seen:
                raise ValueError("duplicate root ID")
            seen.add(root.root_id)
            if root.split not in _ALLOWED_SPLITS:
                raise ValueError("root manifest contains an unknown split")
            if root.split in _AUDITED_SPLITS and not manifest.audited_splits_materialized:
                raise ValueError("ordinary root manifest exposes audited membership")
            if root.relative_path != f"{root.split}/roots/{root.root_id}.json":
                raise ValueError("root path is not split-isolated and canonical")
            if not root.lineages or root.lineages != tuple(sorted(set(root.lineages))):
                raise ValueError("root lineages are not canonical")
            if not root.source_seeds or root.source_seeds != tuple(sorted(set(root.source_seeds))):
                raise ValueError("root source seeds are not canonical")
            if any(seed not in requested_set for seed in root.source_seeds):
                raise ValueError("root source seed is outside the requested cohort")
            expected_group = _split_group_id(root.lineages)
            if root.split_group_id != expected_group:
                raise ValueError("split group does not match canonical lineages")
            if {_split_for_lineage(lineage) for lineage in root.lineages} != {root.split}:
                raise ValueError("root provenance crosses splits")
        accounted: set[str] = set()
        for root in roots:
            accounted.update(root.source_seeds)
        for exclusion in exclusions:
            if exclusion.source_seed not in requested_set:
                raise ValueError("exclusion source seed is outside the requested cohort")
            accounted.add(exclusion.source_seed)
        if accounted != requested_set:
            raise ValueError("requested seed accounting is incomplete")
        return manifest


def load_root_manifest(
    path: Path, *, verify_roots: bool = True, allow_audited_materialization: bool = False
) -> RootManifest:
    content = path.read_bytes()
    manifest = RootManifest.from_dict(json.loads(content))
    if content != _canonical_bytes(manifest.to_dict()):
        raise ValueError("root manifest is not canonical")
    if manifest.audited_splits_materialized and not allow_audited_materialization:
        raise PermissionError("audited root materialization requires explicit access")
    if verify_roots:
        for root in manifest.roots:
            root_path = path.parent / root.relative_path
            content = root_path.read_bytes()
            try:
                snapshot = json.loads(content)
            except json.JSONDecodeError as error:
                raise ValueError(f"root {root.root_id} is not JSON") from error
            canonical = _canonical_bytes(snapshot)
            if content != canonical or _sha256_bytes(canonical) != root.root_id:
                raise ValueError(f"root {root.root_id} is not canonical")
            restored = RunEnv.from_snapshot(canonical.decode())
            decision = restored.decision()
            if (
                not isinstance(decision.observation, FairCombatObservation)
                or decision.observation.phase != "waiting_for_player"
                or not decision.actions
                or all(action.kind == "proceed" for action in decision.actions)
            ):
                raise ValueError(
                    f"root {root.root_id} is not an actionable ongoing combat decision"
                )
    return manifest


def _policy_index(seed: str, step: int, actions: tuple[Action, ...]) -> int:
    payload = json.dumps(
        [seed, step, [asdict(action.descriptor()) for action in actions]],
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
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
        and bool(decision.actions)
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


def _combat_entry_exclusion(
    decision: Decision, *, combat_index: int, combat_depth: int
) -> str | None:
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
    materialize_audited_splits: bool = False,
    repository_root: Path | None = None,
) -> RootManifest:
    """Advance seeded runs only through accepted public legal transitions."""

    requested_seeds = _canonical_requested_seeds(seeds)
    if not 0 <= ascension <= 20 or max_run_steps <= 0:
        raise ValueError("invalid root generation bounds")
    if type(combat_depth) is not int or combat_depth <= 0:
        raise ValueError("combat depth must be a positive integer")
    if type(materialize_audited_splits) is not bool:
        raise TypeError("audited materialization flag must be boolean")
    _require_empty_output_dir(output_dir)
    if repository_root is None:
        repository_root = Path(__file__).resolve().parents[3]
    repository = capture_repository_version(repository_root, allow_dirty=True)
    source_digest = _sha256_bytes(_canonical_bytes(repository.to_dict()))
    root_payloads: dict[str, tuple[dict[str, object], list[str], list[str]]] = {}
    exclusions: list[RootExclusion] = []
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
            canonical = _canonical_bytes(snapshot)
            root_id = _sha256_bytes(canonical)
            lineage = f"sim-seed:{seed}"
            existing = root_payloads.get(root_id)
            if existing is None:
                root_payloads[root_id] = (snapshot, [lineage], [seed])
            else:
                existing[1].append(lineage)
                existing[2].append(seed)
                exclusions.append(RootExclusion(seed, "duplicate_root", f"duplicate of {root_id}"))
        except (RuntimeError, TypeError, ValueError) as error:
            exclusions.append(RootExclusion(seed, "generation_error", str(error)))

    entries: list[RootEntry] = []
    for root_id, (snapshot, raw_lineages, raw_seeds) in sorted(root_payloads.items()):
        lineages = tuple(sorted(set(raw_lineages)))
        source_seeds = tuple(sorted(set(raw_seeds)))
        splits = {_split_for_lineage(lineage) for lineage in lineages}
        if len(splits) != 1:
            exclusions.extend(
                RootExclusion(seed, "cross_split_provenance", root_id) for seed in source_seeds
            )
            continue
        split = next(iter(splits))
        if split in _AUDITED_SPLITS and not materialize_audited_splits:
            exclusions.extend(
                RootExclusion(seed, "withheld_audited_split", split) for seed in source_seeds
            )
            continue
        relative_path = f"{split}/roots/{root_id}.json"
        _atomic_write(output_dir / relative_path, _canonical_bytes(snapshot))
        entries.append(
            RootEntry(
                root_id,
                split,
                _split_group_id(lineages),
                relative_path,
                lineages,
                source_seeds,
            )
        )
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
        "audited_splits_materialized": materialize_audited_splits,
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
        materialize_audited_splits,
        tuple(entries),
        tuple(exclusions),
        _sha256_bytes(_canonical_bytes(unsigned)),
    )
    _atomic_write(output_dir / "root-manifest.json", _canonical_bytes(manifest.to_dict()))
    return load_root_manifest(
        output_dir / "root-manifest.json",
        allow_audited_materialization=materialize_audited_splits,
    )


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
    audited_access: bool
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
            "audited_access": self.audited_access,
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


def load_dataset_manifest(
    path: Path, *, requested_split: str, allow_audited_split: bool = False
) -> DatasetManifest:
    raw = json.loads(path.read_text(encoding="utf-8"))
    if type(raw) is not dict:
        raise TypeError("dataset manifest must be an object")
    source = cast(dict[str, object], raw)
    if (
        set(source) != set(DatasetManifest.__dataclass_fields__)
        or source["manifest_version"] not in _ACCEPTED_DATASET_MANIFEST_VERSIONS
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
        cast(int, source["manifest_version"]),
        cast(str, source["root_manifest_path"]),
        _require_digest(source["root_manifest_file_digest"], "root manifest file digest"),
        _require_digest(source["root_manifest_digest"], "root manifest digest"),
        _require_digest(source["cohort_digest"], "dataset cohort digest"),
        tuple(roots),
        tuple(exclusions),
        cast(str, source["split"]),
        cast(bool, source["audited_access"]),
        cast(dict[str, object], source["reward_config"]),
        _require_digest(source["reward_config_digest"], "reward config digest"),
        cast(str, source["teacher_name"]),
        cast(str, source["teacher_version"]),
        _require_digest(source["teacher_search_contract_digest"], "teacher/search contract digest"),
        cast(str, source["source_kind"]),
        cast(dict[str, object], source["search_config"]),
        RepositoryVersion.from_dict(source["repository"]),
        cast(str, source["shard_path"]),
        _require_digest(source["shard_digest"], "shard digest"),
        cast(int, source["record_count"]),
        tuple(cast(list[str], source["record_ids"])),
        _require_digest(source["manifest_digest"], "dataset manifest digest"),
    )
    if manifest.split != requested_split or manifest.split not in _ALLOWED_SPLITS:
        raise ValueError("dataset split does not match requested split")
    if manifest.source_kind != _SOURCE_KIND:
        raise ValueError("dataset source kind is unsupported")
    if (
        type(manifest.teacher_name) is not str
        or not manifest.teacher_name
        or type(manifest.teacher_version) is not str
        or not manifest.teacher_version
    ):
        raise TypeError("dataset teacher identity must be nonempty strings")
    if type(manifest.audited_access) is not bool or manifest.audited_access != (
        manifest.split in _AUDITED_SPLITS
    ):
        raise ValueError("dataset audited-access declaration is invalid")
    if manifest.audited_access and not allow_audited_split:
        raise PermissionError("sealed and audit splits require explicit audited access")
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
        _require_digest(record_id, "record ID")
    if tuple(roots) != tuple(sorted(roots, key=lambda root: root.root_id)) or len(
        {root.root_id for root in roots}
    ) != len(roots):
        raise ValueError("dataset root membership is not canonical")
    for root in roots:
        _require_digest(root.root_id, "root ID")
        _require_digest(root.split_group_id, "split group ID")
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
        _require_digest(exclusion.root_id, "excluded root ID")
        if exclusion.reason != _NATIVE_EPISODE_ERROR:
            raise ValueError("dataset exclusion reason is unsupported")
        if type(exclusion.detail) is not str or not exclusion.detail:
            raise ValueError("dataset exclusion detail must be a nonempty public string")
    reward = CombatRewardConfig(**cast(dict[str, Any], manifest.reward_config))
    if reward.digest != manifest.reward_config_digest:
        raise ValueError("dataset reward configuration digest is invalid")
    if manifest.manifest_version == DATASET_MANIFEST_VERSION:
        validate_v2_search_config(manifest.search_config)
        if (
            manifest.teacher_name == PUCT_TEACHER_NAME
            or manifest.teacher_version == PUCT_TEACHER_VERSION
        ):
            raise ValueError("V5 datasets cannot mix PUCT teacher identity")
    elif manifest.manifest_version == DATASET_MANIFEST_V6:
        validate_v3_search_config(manifest.search_config)
        if (
            manifest.teacher_name != PUCT_TEACHER_NAME
            or manifest.teacher_version != PUCT_TEACHER_VERSION
        ):
            raise ValueError("V6 datasets require the privileged PUCT teacher")
    else:
        raise ValueError("unsupported or malformed dataset manifest")
    if manifest.teacher_search_contract_digest != _teacher_search_contract_digest(
        manifest.teacher_name, manifest.teacher_version, manifest.search_config
    ):
        raise ValueError("dataset teacher/search contract digest is invalid")
    if manifest.manifest_digest != _digest_payload(manifest.to_dict(), "manifest_digest"):
        raise ValueError("dataset manifest digest is invalid")
    named_root_path = path.parent / manifest.root_manifest_path
    named_root_bytes = named_root_path.read_bytes()
    if _sha256_bytes(named_root_bytes) != manifest.root_manifest_file_digest:
        raise ValueError("dataset root manifest file digest is invalid")
    named_root_manifest = load_root_manifest(
        named_root_path,
        verify_roots=False,
        allow_audited_materialization=allow_audited_split,
    )
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
    shard = path.parent / manifest.shard_path
    if _sha256_bytes(shard.read_bytes()) != manifest.shard_digest:
        raise ValueError("dataset shard digest is invalid")
    records = tuple(read_jsonl(shard))
    if (
        len(records) != manifest.record_count
        or tuple(cast(str, record.record_id) for record in records) != manifest.record_ids
    ):
        raise ValueError("dataset record order or count is invalid")
    memberships = {root.root_id: root for root in roots}
    seen_memberships: set[str] = set()
    record_versions = {record.record_version for record in records}
    if len(record_versions) != 1:
        raise ValueError("dataset mixes record schema epochs")
    expected_record_version = 2 if manifest.manifest_version == DATASET_MANIFEST_VERSION else 3
    for record in records:
        if record.record_version != expected_record_version:
            raise ValueError("dataset record schema does not match the manifest epoch")
        if expected_record_version == 2:
            if (
                sum(record.teacher_visit_counts) != 1
                or record.teacher_visit_counts[record.chosen_action_index] != 1
            ):
                raise ValueError("beam-clone teacher labels must be one-hot at the chosen action")
            if record.value_target_name != reward.name:
                raise ValueError("dataset record reward contract mismatch")
            expected_value = reward.value(record.outcome)
            if record.target_value != expected_value or record.value_target_mask != (
                expected_value is not None
            ):
                raise ValueError("dataset record value target does not match serialized outcome")
        else:
            if record.value_target_name != PUCT_VALUE_TARGET_NAME:
                raise ValueError("PUCT records must use privileged_puct_root_mean_v1")
            if sum(record.teacher_visit_counts) <= 0:
                raise ValueError("PUCT teacher labels must have positive visit mass")
            if record.chosen_action_index != first_argmax_visits(record.teacher_visit_counts):
                raise ValueError("PUCT chosen action is not the first visit-count argmax")
            if record.target_value is None or not record.value_target_mask:
                raise ValueError("PUCT root-mean value targets must be present and unmasked")
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
        # Reparse descriptors to enforce the tensorizable canonical action schema.
        for action in record.actions:
            action_descriptor_from_payload(
                {key: value for key, value in asdict(action).items() if value is not None}
            )
    if seen_memberships != set(memberships):
        raise ValueError("dataset root membership contains no records")
    return manifest


def generate_beam_dataset(
    root_manifest_path: Path,
    output_dir: Path,
    *,
    split: str = "train",
    allow_audited_split: bool = False,
    depth: int = 8,
    width: int = 24,
    transition_budget: int = 5_000,
    max_decisions: int = 512,
    max_player_turns: int = 100,
    deduplicate_search_states: bool = True,
    reward_config: CombatRewardConfig = COMBAT_PROXY_V1,
    repository_root: Path | None = None,
) -> DatasetManifest:
    if split not in _ALLOWED_SPLITS:
        raise ValueError("unknown dataset split")
    if split in _AUDITED_SPLITS and not allow_audited_split:
        raise PermissionError("sealed and audit splits require explicit audited access")
    _require_empty_output_dir(output_dir)
    root_manifest = load_root_manifest(
        root_manifest_path,
        allow_audited_materialization=split in _AUDITED_SPLITS and allow_audited_split,
    )
    roots = [root for root in root_manifest.roots if root.split == split]
    if not roots:
        raise ValueError(f"root manifest contains no {split} roots")
    if repository_root is None:
        repository_root = Path(__file__).resolve().parents[3]
    repository = capture_repository_version(repository_root, allow_dirty=True)
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
    validate_v2_search_config(search_config)
    records: list[SymbolicTrainingRecord] = []
    teacher: tuple[str, str] | None = None
    used_roots: list[DatasetRootMembership] = []
    exclusions: list[DatasetExclusion] = []
    for root in roots:
        try:
            env = RunEnv.from_snapshot((root_manifest_path.parent / root.relative_path).read_text())
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
            if teacher is not None and teacher != native_teacher:
                raise ValueError("native teacher metadata changed within dataset")
            outcome = CombatOutcome.from_dict(payload["outcome"])
            target = reward_config.value(outcome)
            episode_id = _sha256_bytes(
                _canonical_bytes([root.root_id, search_config, reward_config.digest])
            )
            steps = cast(list[dict[str, object]], payload["steps"])
            if not steps:
                raise ValueError("terminal or post-combat root cannot produce training records")
            root_records: list[SymbolicTrainingRecord] = []
            for decision_index, step in enumerate(steps):
                projected = FairCombatObservation._from_payload(
                    cast(dict[str, object], step["observation"])
                )
                observation = fair_observation_from_payload(fair_observation_payload(projected))
                actions = tuple(
                    action_descriptor_from_payload(
                        {"family": "combat", **cast(dict[str, object], choice)}
                    )
                    for choice in cast(list[object], step["choices"])
                )
                selected = cast(int, step["selected_index"])
                counts = tuple(cast(list[int], step["teacher_visit_counts"]))
                root_records.append(
                    SymbolicTrainingRecord(
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
                        2,
                        root_manifest.manifest_digest,
                        reward_config.digest,
                        _SOURCE_KIND,
                        episode_id,
                        decision_index,
                        target is not None,
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
    lines = b"".join(_canonical_bytes(record.to_dict()) + b"\n" for record in records)
    shard_name = f"{split}/{split}.jsonl"
    _atomic_write(output_dir / shard_name, lines)
    shard_digest = _sha256_bytes(lines)
    memberships = tuple(sorted(used_roots, key=lambda root: root.root_id))
    dataset_exclusions = tuple(
        sorted(
            exclusions,
            key=lambda exclusion: (exclusion.root_id, exclusion.reason, exclusion.detail),
        )
    )
    root_manifest_bytes = _canonical_bytes(root_manifest.to_dict())
    root_manifest_file_digest = _sha256_bytes(root_manifest_bytes)
    _atomic_write(output_dir / _DATASET_ROOT_MANIFEST_PATH, root_manifest_bytes)
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
        "audited_access": split in _AUDITED_SPLITS,
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
        split in _AUDITED_SPLITS,
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
        tuple(cast(str, record.record_id) for record in records),
        _sha256_bytes(_canonical_bytes(unsigned)),
    )
    _atomic_write(output_dir / "dataset-manifest.json", _canonical_bytes(manifest.to_dict()))
    return load_dataset_manifest(
        output_dir / "dataset-manifest.json",
        requested_split=split,
        allow_audited_split=allow_audited_split,
    )
