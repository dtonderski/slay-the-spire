"""Deterministic legal combat roots and immutable beam-cloning datasets."""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import cast

from ..fair import FairCombatObservation
from ..run import Action, RunEnv
from .provenance import capture_repository_version
from .records import (
    CombatOutcome,
    JsonValue,
    SymbolicTrainingRecord,
    action_descriptor_from_payload,
    fair_observation_digest,
    fair_observation_from_payload,
    fair_observation_payload,
    read_jsonl,
)
from .rewards import COMBAT_PROXY_V1, CombatRewardConfig

ROOT_MANIFEST_VERSION = 1
DATASET_MANIFEST_VERSION = 1
_SPLIT_SALT = "combat-agent-phase2-v1"
_ALLOWED_SPLITS = {"train", "development", "sealed_test", "real_trace_audit"}


def _canonical_bytes(payload: object) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(
        "utf-8"
    )


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


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
    ascension: int
    max_run_steps: int
    roots: tuple[RootEntry, ...]
    exclusions: tuple[RootExclusion, ...]
    manifest_digest: str

    def to_dict(self) -> dict[str, object]:
        return {
            "manifest_version": self.manifest_version,
            "generator_name": self.generator_name,
            "generator_version": self.generator_version,
            "ascension": self.ascension,
            "max_run_steps": self.max_run_steps,
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
        expected = {
            "manifest_version",
            "generator_name",
            "generator_version",
            "ascension",
            "max_run_steps",
            "roots",
            "exclusions",
            "manifest_digest",
        }
        if set(source) != expected or source["manifest_version"] != ROOT_MANIFEST_VERSION:
            raise ValueError("unsupported or malformed root manifest")
        roots_payload = cast(list[dict[str, object]], source["roots"])
        exclusions_payload = cast(list[dict[str, object]], source["exclusions"])
        roots = tuple(
            RootEntry(
                root_id=cast(str, item["root_id"]),
                split=cast(str, item["split"]),
                split_group_id=cast(str, item["split_group_id"]),
                relative_path=cast(str, item["relative_path"]),
                lineages=tuple(cast(list[str], item["lineages"])),
                source_seeds=tuple(cast(list[str], item["source_seeds"])),
            )
            for item in roots_payload
        )
        exclusions = tuple(
            RootExclusion(
                source_seed=cast(str, item["source_seed"]),
                reason=cast(str, item["reason"]),
                detail=cast(str, item["detail"]),
            )
            for item in exclusions_payload
        )
        manifest = cls(
            manifest_version=cast(int, source["manifest_version"]),
            generator_name=cast(str, source["generator_name"]),
            generator_version=cast(str, source["generator_version"]),
            ascension=cast(int, source["ascension"]),
            max_run_steps=cast(int, source["max_run_steps"]),
            roots=roots,
            exclusions=exclusions,
            manifest_digest=cast(str, source["manifest_digest"]),
        )
        if manifest.manifest_digest != _digest_payload(manifest.to_dict(), "manifest_digest"):
            raise ValueError("root manifest digest is invalid")
        seen: dict[str, str] = {}
        for root in roots:
            if root.split not in _ALLOWED_SPLITS:
                raise ValueError("root manifest contains an unknown split")
            previous = seen.setdefault(root.root_id, root.split)
            if previous != root.split:
                raise ValueError("identical root occurs in multiple splits")
            if root.relative_path != f"roots/{root.root_id}.json":
                raise ValueError("root path is not canonical")
        return manifest


def load_root_manifest(path: Path, *, verify_roots: bool = True) -> RootManifest:
    manifest = RootManifest.from_dict(json.loads(path.read_text(encoding="utf-8")))
    if verify_roots:
        for root in manifest.roots:
            root_path = path.parent / root.relative_path
            content = root_path.read_bytes()
            try:
                snapshot = json.loads(content)
            except json.JSONDecodeError as error:
                raise ValueError(f"root {root.root_id} is not JSON") from error
            canonical = _canonical_bytes(snapshot)
            if content != canonical:
                raise ValueError(f"root {root.root_id} bytes are not canonical")
            if _sha256_bytes(canonical) != root.root_id:
                raise ValueError(f"root {root.root_id} hash is invalid")
            restored = RunEnv.from_snapshot(canonical.decode("utf-8"))
            decision = restored.decision()
            if not isinstance(decision.observation, FairCombatObservation) or not decision.actions:
                raise ValueError(f"root {root.root_id} is not an actionable combat decision")
    return manifest


def _policy_index(seed: str, step: int, actions: tuple[Action, ...]) -> int:
    descriptors = [action.descriptor() for action in actions]
    payload = json.dumps(
        [seed, step, [asdict(descriptor) for descriptor in descriptors]],
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return int.from_bytes(hashlib.sha256(payload).digest()[:8], "big") % len(actions)


def generate_legal_roots(
    output_dir: Path,
    seeds: list[str],
    *,
    ascension: int = 0,
    max_run_steps: int = 256,
) -> RootManifest:
    """Advance seeded runs only through accepted public legal transitions."""

    if not seeds or len(seeds) != len(set(seeds)):
        raise ValueError("root seeds must be nonempty and unique")
    if not 0 <= ascension <= 20 or max_run_steps <= 0:
        raise ValueError("invalid root generation bounds")
    root_payloads: dict[str, tuple[dict[str, object], list[str], list[str], str]] = {}
    exclusions: list[RootExclusion] = []
    for seed in seeds:
        if type(seed) is not str or not seed:
            raise TypeError("root seeds must be nonempty strings")
        env = RunEnv.new_ironclad(seed, ascension)
        try:
            for step in range(max_run_steps + 1):
                decision = env.decision()
                if isinstance(decision.observation, FairCombatObservation):
                    if not decision.actions:
                        raise ValueError("combat root has no public actions")
                    snapshot = json.loads(env.snapshot().json)
                    canonical = _canonical_bytes(snapshot)
                    root_id = _sha256_bytes(canonical)
                    lineage = f"sim-seed:{seed}"
                    split = _split_for_lineage(lineage)
                    existing = root_payloads.get(root_id)
                    if existing is None:
                        root_payloads[root_id] = (snapshot, [lineage], [seed], split)
                    else:
                        existing[1].append(lineage)
                        existing[2].append(seed)
                        exclusions.append(
                            RootExclusion(seed, "duplicate_root", f"duplicate of {root_id}")
                        )
                    break
                if step == max_run_steps:
                    exclusions.append(RootExclusion(seed, "step_limit", "no combat reached"))
                    break
                if not decision.actions:
                    exclusions.append(RootExclusion(seed, "terminal_run", "no combat reached"))
                    break
                index = _policy_index(seed, step, decision.actions)
                env.step(decision.actions[index])
        except (RuntimeError, TypeError, ValueError) as error:
            exclusions.append(RootExclusion(seed, "generation_error", str(error)))

    entries: list[RootEntry] = []
    for root_id, (snapshot, lineages, source_seeds, split) in sorted(root_payloads.items()):
        relative_path = f"roots/{root_id}.json"
        _atomic_write(output_dir / relative_path, _canonical_bytes(snapshot))
        entries.append(
            RootEntry(
                root_id,
                split,
                _sha256_bytes("\0".join(sorted(set(lineages))).encode()),
                relative_path,
                tuple(sorted(set(lineages))),
                tuple(sorted(set(source_seeds))),
            )
        )
    unsigned: dict[str, object] = {
        "manifest_version": ROOT_MANIFEST_VERSION,
        "generator_name": "legal_run_policy",
        "generator_version": "sha256_action_policy_v1",
        "ascension": ascension,
        "max_run_steps": max_run_steps,
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
    digest = _sha256_bytes(_canonical_bytes(unsigned))
    manifest = RootManifest(
        ROOT_MANIFEST_VERSION,
        "legal_run_policy",
        "sha256_action_policy_v1",
        ascension,
        max_run_steps,
        tuple(entries),
        tuple(exclusions),
        digest,
    )
    _atomic_write(output_dir / "root-manifest.json", _canonical_bytes(manifest.to_dict()))
    return load_root_manifest(output_dir / "root-manifest.json")


@dataclass(frozen=True, slots=True)
class DatasetManifest:
    manifest_version: int
    root_manifest_digest: str
    split: str
    reward_config_digest: str
    teacher_name: str
    teacher_version: str
    shard_path: str
    shard_digest: str
    record_count: int
    record_ids: tuple[str, ...]
    manifest_digest: str

    def to_dict(self) -> dict[str, object]:
        return {
            **asdict(self),
            "record_ids": list(self.record_ids),
        }


def load_dataset_manifest(
    path: Path,
    *,
    requested_split: str,
    allow_audited_split: bool = False,
) -> DatasetManifest:
    source = cast(dict[str, object], json.loads(path.read_text(encoding="utf-8")))
    expected = set(DatasetManifest.__dataclass_fields__)
    if set(source) != expected or source["manifest_version"] != DATASET_MANIFEST_VERSION:
        raise ValueError("unsupported or malformed dataset manifest")
    manifest = DatasetManifest(
        manifest_version=cast(int, source["manifest_version"]),
        root_manifest_digest=cast(str, source["root_manifest_digest"]),
        split=cast(str, source["split"]),
        reward_config_digest=cast(str, source["reward_config_digest"]),
        teacher_name=cast(str, source["teacher_name"]),
        teacher_version=cast(str, source["teacher_version"]),
        shard_path=cast(str, source["shard_path"]),
        shard_digest=cast(str, source["shard_digest"]),
        record_count=cast(int, source["record_count"]),
        record_ids=tuple(cast(list[str], source["record_ids"])),
        manifest_digest=cast(str, source["manifest_digest"]),
    )
    if manifest.split != requested_split:
        raise ValueError("dataset split does not match requested split")
    if requested_split in {"sealed_test", "real_trace_audit"} and not allow_audited_split:
        raise PermissionError("sealed and audit splits require explicit audited access")
    if manifest.manifest_digest != _digest_payload(manifest.to_dict(), "manifest_digest"):
        raise ValueError("dataset manifest digest is invalid")
    shard = path.parent / manifest.shard_path
    if _sha256_bytes(shard.read_bytes()) != manifest.shard_digest:
        raise ValueError("dataset shard digest is invalid")
    records = tuple(read_jsonl(shard))
    if (
        len(records) != manifest.record_count
        or tuple(cast(str, record.record_id) for record in records) != manifest.record_ids
    ):
        raise ValueError("dataset record order or count is invalid")
    if any(record.root_manifest_digest != manifest.root_manifest_digest for record in records):
        raise ValueError("dataset record root manifest mismatch")
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
    reward_config: CombatRewardConfig = COMBAT_PROXY_V1,
    repository_root: Path | None = None,
) -> DatasetManifest:
    if split not in _ALLOWED_SPLITS:
        raise ValueError("unknown dataset split")
    if split in {"sealed_test", "real_trace_audit"} and not allow_audited_split:
        raise PermissionError("sealed and audit splits require explicit audited access")
    root_manifest = load_root_manifest(root_manifest_path)
    roots = [root for root in root_manifest.roots if root.split == split]
    if not roots:
        raise ValueError(f"root manifest contains no {split} roots")
    if repository_root is None:
        repository_root = Path(__file__).resolve().parents[4]
    repository = capture_repository_version(repository_root, allow_dirty=True)
    records: list[SymbolicTrainingRecord] = []
    search_config: dict[str, object] = {
        "depth": depth,
        "width": width,
        "transition_budget": transition_budget,
        "max_decisions": max_decisions,
        "max_player_turns": max_player_turns,
        "deadline": None,
        "replan": "every_public_decision",
    }
    for root in roots:
        env = RunEnv.from_snapshot((root_manifest_path.parent / root.relative_path).read_text())
        payload = env.beam_clone_episode_payload(
            depth=depth,
            width=width,
            transition_budget=transition_budget,
            max_decisions=max_decisions,
            max_player_turns=max_player_turns,
        )
        if payload.get("schema_version") != 1:
            raise ValueError("unsupported native beam episode schema")
        outcome = CombatOutcome.from_dict(payload["outcome"])
        target = reward_config.value(outcome)
        episode_id = _sha256_bytes(
            _canonical_bytes([root.root_id, search_config, reward_config.digest])
        )
        steps = cast(list[dict[str, object]], payload["steps"])
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
            records.append(
                SymbolicTrainingRecord(
                    observation=observation,
                    actions=actions,
                    chosen_action_index=selected,
                    chosen_action=actions[selected],
                    teacher_visit_counts=counts,
                    target_value=target,
                    value_target_name=reward_config.name,
                    outcome=outcome,
                    planner_name=cast(str, payload["teacher_name"]),
                    planner_version=cast(str, payload["teacher_version"]),
                    search_config=cast(dict[str, JsonValue], search_config),
                    root_id=root.root_id,
                    split_group_id=root.split_group_id,
                    teacher_pair_id=None,
                    repository=repository,
                    observation_digest=fair_observation_digest(observation),
                    record_version=2,
                    root_manifest_digest=root_manifest.manifest_digest,
                    reward_config_digest=reward_config.digest,
                    source_kind="simulator_legal_v1",
                    episode_id=episode_id,
                    decision_index=decision_index,
                    value_target_mask=target is not None,
                )
            )
    lines = b"".join(_canonical_bytes(record.to_dict()) + b"\n" for record in records)
    shard_name = f"{split}.jsonl"
    _atomic_write(output_dir / shard_name, lines)
    shard_digest = _sha256_bytes(lines)
    unsigned: dict[str, object] = {
        "manifest_version": DATASET_MANIFEST_VERSION,
        "root_manifest_digest": root_manifest.manifest_digest,
        "split": split,
        "reward_config_digest": reward_config.digest,
        "teacher_name": "sts_live_incumbent_beam",
        "teacher_version": "beam_clone_v1",
        "shard_path": shard_name,
        "shard_digest": shard_digest,
        "record_count": len(records),
        "record_ids": [record.record_id for record in records],
    }
    manifest = DatasetManifest(
        DATASET_MANIFEST_VERSION,
        root_manifest.manifest_digest,
        split,
        reward_config.digest,
        "sts_live_incumbent_beam",
        "beam_clone_v1",
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
