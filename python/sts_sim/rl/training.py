"""Deterministic supervised beam-cloning training and development evaluation."""

from __future__ import annotations

import hashlib
import json
import math
import os
import platform
import random
import statistics
import sys
import tempfile
import warnings
from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, cast

import numpy as np
import torch

from .. import _native
from .data import DATASET_MANIFEST_V6, DATASET_MANIFEST_V7, DatasetManifest, load_dataset_manifest
from .model import CombatModelConfig, FairCombatPolicyValueNet, policy_value_loss
from .provenance import capture_repository_version
from .records import (
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    collate_training_examples,
    read_jsonl,
)
from .tensor import Vocabularies, VocabularyBuilder, encoder_contract_digest
from .tracking import OfflineWandbConfig, OfflineWandbSession, start_offline_wandb

TRAINING_CHECKPOINT_FORMAT_V3 = 3
TRAINING_CHECKPOINT_FORMAT = 4
_CHECKPOINT_KEYS_V3 = {
    "checkpoint_format",
    "config",
    "config_digest",
    "dataset_manifest_digest",
    "dataset_shard_digest",
    "root_manifest_digest",
    "cohort_digest",
    "teacher_search_contract_digest",
    "reward_config_digest",
    "source_digest",
    "runtime_identity",
    "runtime_identity_digest",
    "vocabularies",
    "vocabulary_fingerprint",
    "encoder_contract_digest",
    "model_config",
    "model_state",
    "optimizer_state",
    "scheduler_state",
    "global_step",
    "cursor",
    "order",
    "python_rng_state",
    "numpy_rng_state",
    "torch_rng_state",
}
_CHECKPOINT_KEYS_V4 = _CHECKPOINT_KEYS_V3 | {"training_target_statistics"}
_TRAINING_TARGET_STATISTICS_KEYS = {
    "count",
    "mean",
    "min",
    "max",
    "population_stddev",
    "digest",
}


def _canonical_bytes(payload: object) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def _digest(payload: object) -> str:
    return hashlib.sha256(_canonical_bytes(payload)).hexdigest()


def _atomic_torch_save(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    os.close(descriptor)
    try:
        torch.save(payload, temporary)
        with Path(temporary).open("rb") as source:
            os.fsync(source.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def _source_digest() -> str:
    """Attest the complete checkout and the exact loaded native extension bytes."""

    repository_root = Path(__file__).resolve().parents[3]
    repository = capture_repository_version(repository_root, allow_dirty=True)
    native_path = Path(_native.__file__)
    payload = {
        "repository": repository.to_dict(),
        "native_extension_digest": hashlib.sha256(native_path.read_bytes()).hexdigest(),
    }
    return _digest(payload)


def _runtime_identity() -> dict[str, object]:
    python_root = Path(__file__).resolve().parents[2]
    artifact_digests = {
        name: hashlib.sha256((python_root / name).read_bytes()).hexdigest()
        for name in ("pyproject.toml", "uv.lock")
    }
    return {
        "python": {
            "implementation": platform.python_implementation(),
            "version": list(sys.version_info[:5]),
        },
        "numpy_version": np.__version__,
        "torch_version": str(torch.__version__),
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
        },
        "deterministic_policy": {
            "device": "cpu",
            "torch_deterministic_algorithms": torch.are_deterministic_algorithms_enabled(),
            "torch_intraop_threads": torch.get_num_threads(),
            "torch_interop_threads": torch.get_num_interop_threads(),
            "cudnn_deterministic": torch.backends.cudnn.deterministic,
            "cudnn_benchmark": torch.backends.cudnn.benchmark,
        },
        "dependency_artifact_digests": artifact_digests,
    }


def _model_state_digest(state: object) -> str:
    if not isinstance(state, Mapping):
        raise TypeError("checkpoint model state must be an object")
    digest = hashlib.sha256()
    for name, value in sorted(cast(Mapping[str, object], state).items()):
        if type(name) is not str or not isinstance(value, torch.Tensor):
            raise TypeError("checkpoint model state must contain named tensors")
        tensor = value.detach().cpu().contiguous()
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update(str(tensor.dtype).encode())
        digest.update(_canonical_bytes(list(tensor.shape)))
        digest.update(tensor.numpy().tobytes())
    return digest.hexdigest()


def _training_target_values(
    records: Sequence[SymbolicTrainingRecord],
) -> tuple[float, ...]:
    values: list[float] = []
    for record in records:
        if not record.value_target_mask:
            continue
        if record.target_value is None:
            raise ValueError("unmasked training target is missing")
        value = float(record.target_value)
        if not math.isfinite(value):
            raise ValueError("unmasked training target is not finite")
        values.append(value)
    return tuple(values)


def _compute_training_target_statistics(
    records: Sequence[SymbolicTrainingRecord],
) -> dict[str, object]:
    values = _training_target_values(records)
    if not values:
        unsigned: dict[str, object] = {
            "count": 0,
            "mean": None,
            "min": None,
            "max": None,
            "population_stddev": None,
        }
    else:
        unsigned = {
            "count": len(values),
            "mean": statistics.fmean(values),
            "min": min(values),
            "max": max(values),
            "population_stddev": statistics.pstdev(values),
        }
    payload = dict(unsigned)
    payload["digest"] = _digest(unsigned)
    return payload


def _validate_training_target_statistics(payload: object) -> dict[str, object]:
    if type(payload) is not dict:
        raise TypeError("training target statistics must be an object")
    source = cast(dict[str, object], payload)
    if set(source) != _TRAINING_TARGET_STATISTICS_KEYS:
        raise ValueError("training target statistics have missing or unknown fields")
    count = source["count"]
    if type(count) is not int or count < 0:
        raise ValueError("training target statistics count must be a nonnegative integer")
    unsigned = {
        "count": count,
        "mean": source["mean"],
        "min": source["min"],
        "max": source["max"],
        "population_stddev": source["population_stddev"],
    }
    if count == 0:
        if any(unsigned[key] is not None for key in ("mean", "min", "max", "population_stddev")):
            raise ValueError("empty training target statistics must be null")
    else:
        for key in ("mean", "min", "max", "population_stddev"):
            value = unsigned[key]
            if type(value) not in {int, float}:
                raise TypeError(f"training target statistics {key} must be numeric")
            number = float(cast(int | float, value))
            if not math.isfinite(number):
                raise ValueError(f"training target statistics {key} must be finite")
            unsigned[key] = number
        mean = float(cast(int | float, unsigned["mean"]))
        minimum = float(cast(int | float, unsigned["min"]))
        maximum = float(cast(int | float, unsigned["max"]))
        stddev = float(cast(int | float, unsigned["population_stddev"]))
        if not minimum <= mean <= maximum:
            raise ValueError("training target statistics range is inconsistent")
        if stddev < 0.0:
            raise ValueError("training target statistics stddev must be nonnegative")
    digest = source["digest"]
    if (
        type(digest) is not str
        or len(digest) != 64
        or any(character not in "0123456789abcdef" for character in digest)
    ):
        raise ValueError("training target statistics digest is not a SHA-256 digest")
    if digest != _digest(unsigned):
        raise ValueError("training target statistics digest mismatch")
    validated = dict(unsigned)
    validated["digest"] = digest
    return validated


def _validate_checkpoint_envelope(payload: object) -> tuple[dict[str, Any], TrainingConfig]:
    if type(payload) is not dict:
        raise TypeError("training checkpoint must be an object")
    checkpoint = cast(dict[str, Any], payload)
    checkpoint_format = checkpoint.get("checkpoint_format")
    if type(checkpoint_format) is not int:
        raise ValueError("unsupported or malformed training checkpoint")
    if checkpoint_format == TRAINING_CHECKPOINT_FORMAT_V3:
        if set(checkpoint) != _CHECKPOINT_KEYS_V3:
            raise ValueError("unsupported or malformed training checkpoint")
    elif checkpoint_format == TRAINING_CHECKPOINT_FORMAT:
        if set(checkpoint) != _CHECKPOINT_KEYS_V4:
            raise ValueError("unsupported or malformed training checkpoint")
        checkpoint["training_target_statistics"] = _validate_training_target_statistics(
            checkpoint["training_target_statistics"]
        )
    else:
        raise ValueError("unsupported or malformed training checkpoint")
    if type(checkpoint["config"]) is not dict or type(checkpoint["model_config"]) is not dict:
        raise TypeError("training checkpoint configurations must be objects")
    try:
        stored_config = TrainingConfig(**checkpoint["config"])
    except (TypeError, ValueError) as error:
        raise ValueError("training checkpoint config is invalid") from error
    if checkpoint["config_digest"] != stored_config.digest:
        raise ValueError("training checkpoint config digest mismatch")
    if checkpoint["model_config"] != asdict(stored_config.model_config()):
        raise ValueError("training checkpoint model config mismatch")
    if type(checkpoint["runtime_identity"]) is not dict:
        raise TypeError("training checkpoint runtime identity must be an object")
    if checkpoint["runtime_identity_digest"] != _digest(checkpoint["runtime_identity"]):
        raise ValueError("training checkpoint runtime identity digest mismatch")
    for name in (
        "dataset_manifest_digest",
        "dataset_shard_digest",
        "root_manifest_digest",
        "cohort_digest",
        "teacher_search_contract_digest",
        "reward_config_digest",
        "source_digest",
        "runtime_identity_digest",
        "vocabulary_fingerprint",
        "encoder_contract_digest",
    ):
        value = checkpoint[name]
        if (
            type(value) is not str
            or len(value) != 64
            or any(character not in "0123456789abcdef" for character in value)
        ):
            raise ValueError(f"training checkpoint {name} is not a SHA-256 digest")
    global_step = checkpoint["global_step"]
    cursor = checkpoint["cursor"]
    order = checkpoint["order"]
    if type(global_step) is not int or not 0 <= global_step <= stored_config.total_steps:
        raise ValueError("training checkpoint global step is invalid")
    if type(order) is not list or not order or any(type(index) is not int for index in order):
        raise ValueError("training checkpoint sample order is invalid")
    if sorted(order) != list(range(len(order))):
        raise ValueError("training checkpoint sample order is not a permutation")
    if type(cursor) is not int or not 0 <= cursor < len(order):
        raise ValueError("training checkpoint cursor is invalid")
    if cursor != global_step * stored_config.batch_size % len(order):
        raise ValueError("training checkpoint cursor/global step mismatch")
    vocabularies = Vocabularies.from_dict(checkpoint["vocabularies"])
    if checkpoint["vocabulary_fingerprint"] != vocabularies.fingerprint:
        raise ValueError("training checkpoint vocabulary mismatch")
    if checkpoint["encoder_contract_digest"] != encoder_contract_digest(vocabularies):
        raise ValueError("training checkpoint encoder contract mismatch")
    _model_state_digest(checkpoint["model_state"])
    if not isinstance(checkpoint["torch_rng_state"], torch.Tensor):
        raise TypeError("training checkpoint torch RNG state is invalid")
    return checkpoint, stored_config


@dataclass(frozen=True, slots=True)
class TrainingConfig:
    config_version: int = 1
    seed: int = 7
    batch_size: int = 32
    total_steps: int = 100
    learning_rate: float = 1e-3
    weight_decay: float = 1e-4
    torch_threads: int = 1
    minimum_roots: int = 225
    minimum_lineages: int = 100
    model_width: int = 96
    model_heads: int = 4
    model_layers: int = 2
    feedforward_width: int = 192

    def __post_init__(self) -> None:
        if any(
            type(value) is not int or value <= 0
            for value in (
                self.config_version,
                self.batch_size,
                self.total_steps,
                self.torch_threads,
                self.minimum_roots,
                self.minimum_lineages,
                self.model_width,
                self.model_heads,
                self.model_layers,
                self.feedforward_width,
            )
        ):
            raise ValueError("integer training configuration must be positive")
        if self.config_version != 1:
            raise ValueError("unsupported training configuration version")
        if type(self.seed) is not int:
            raise TypeError("training seed must be an integer")
        if not 0.0 < self.learning_rate < 1.0 or not 0.0 <= self.weight_decay < 1.0:
            raise ValueError("optimizer configuration is invalid")

    @property
    def digest(self) -> str:
        return _digest(asdict(self))

    def model_config(self) -> CombatModelConfig:
        return CombatModelConfig(
            width=self.model_width,
            heads=self.model_heads,
            layers=self.model_layers,
            feedforward_width=self.feedforward_width,
            dropout=0.0,
        )


@dataclass(frozen=True, slots=True)
class TrainingResult:
    checkpoint_path: Path
    global_step: int
    metrics: tuple[dict[str, float | int], ...]
    runtime_identity_digest: str
    vocabulary_fingerprint: str
    encoder_contract_digest: str


def _configure_cpu(threads: int) -> None:
    torch.set_num_threads(threads)
    try:
        torch.set_num_interop_threads(1)
    except RuntimeError:
        # PyTorch permits setting inter-op threads only before parallel work.
        # Repeated in-process tests must already have the required value.
        if torch.get_num_interop_threads() != 1:
            raise
    torch.use_deterministic_algorithms(True)


def _load_records(
    manifest_path: Path,
    split: str,
    minimum_roots: int,
    minimum_lineages: int,
) -> tuple[DatasetManifest, tuple[SymbolicTrainingRecord, ...]]:
    manifest = load_dataset_manifest(manifest_path, requested_split=split)
    records = tuple(read_jsonl(manifest_path.parent / manifest.shard_path))
    if not records:
        raise ValueError("training dataset is empty")
    versions = {record.record_version for record in records}
    if versions not in ({2}, {3}, {4}):
        raise ValueError("training requires a single record schema epoch (V2, V3, or V4)")
    root_count = len(manifest.roots)
    lineage_count = len({lineage for root in manifest.roots for lineage in root.lineages})
    if root_count < minimum_roots or lineage_count < minimum_lineages:
        raise ValueError(
            "training corpus is below configured minimums: "
            f"roots {root_count}/{minimum_roots}, lineages {lineage_count}/{minimum_lineages}"
        )
    return manifest, records


def _fit_vocabulary(records: tuple[SymbolicTrainingRecord, ...]) -> Vocabularies:
    builder = VocabularyBuilder()
    for record in records:
        builder.add(record.observation, record.actions)
    return builder.freeze()


def _training_order(length: int, seed: int) -> tuple[int, ...]:
    generator = torch.Generator(device="cpu")
    generator.manual_seed(seed)
    return tuple(int(index) for index in torch.randperm(length, generator=generator).tolist())


def _tracking_warn(action: str, error: BaseException) -> None:
    warnings.warn(
        (
            f"offline W&B {action} failed after successful init; "
            f"training and checkpoint are preserved: {error}"
        ),
        RuntimeWarning,
        stacklevel=2,
    )


def _fail_open_tracking(
    session: OfflineWandbSession | None,
    action: str,
    error: BaseException,
) -> None:
    _tracking_warn(action, error)
    if session is None:
        return
    session.disable_logging()
    try:
        session.finish()
    except Exception as finish_error:  # noqa: BLE001
        _tracking_warn("finish", finish_error)


def train_beam_clone(
    dataset_manifest_path: Path,
    checkpoint_path: Path,
    config: TrainingConfig,
    *,
    resume: bool = False,
    stop_after_steps: int | None = None,
    wandb_offline: OfflineWandbConfig | None = None,
) -> TrainingResult:
    """Train through ``config.total_steps`` and atomically checkpoint every batch."""

    if wandb_offline is not None and type(wandb_offline) is not OfflineWandbConfig:
        raise TypeError("wandb_offline must be OfflineWandbConfig or None")
    _configure_cpu(config.torch_threads)
    manifest, records = _load_records(
        dataset_manifest_path,
        "train",
        config.minimum_roots,
        config.minimum_lineages,
    )
    source_digest = _source_digest()
    runtime_identity = _runtime_identity()
    runtime_identity_digest = _digest(runtime_identity)
    random.seed(config.seed)
    np.random.seed(config.seed)
    torch.manual_seed(config.seed)
    metrics: list[dict[str, float | int]] = []
    session: OfflineWandbSession | None = None
    try:
        if resume:
            payload, stored_config = _validate_checkpoint_envelope(
                torch.load(checkpoint_path, map_location="cpu", weights_only=False)
            )
            checks = {
                "config_digest": config.digest,
                "dataset_manifest_digest": manifest.manifest_digest,
                "dataset_shard_digest": manifest.shard_digest,
                "root_manifest_digest": manifest.root_manifest_digest,
                "cohort_digest": manifest.cohort_digest,
                "teacher_search_contract_digest": manifest.teacher_search_contract_digest,
                "reward_config_digest": manifest.reward_config_digest,
                "source_digest": source_digest,
                "runtime_identity_digest": runtime_identity_digest,
            }
            for name, expected_value in checks.items():
                if payload[name] != expected_value:
                    raise ValueError(f"training checkpoint {name} mismatch")
            if stored_config != config:
                raise ValueError("training checkpoint config mismatch")
            expected_stats = _compute_training_target_statistics(records)
            if (
                payload["checkpoint_format"] == TRAINING_CHECKPOINT_FORMAT
                and payload["training_target_statistics"] != expected_stats
            ):
                raise ValueError("training checkpoint target statistics mismatch")
            vocabularies = Vocabularies.from_dict(payload["vocabularies"])
            model = FairCombatPolicyValueNet(vocabularies, config.model_config())
            model.load_state_dict(payload["model_state"], strict=True)
            optimizer = torch.optim.AdamW(
                model.parameters(), lr=config.learning_rate, weight_decay=config.weight_decay
            )
            optimizer.load_state_dict(payload["optimizer_state"])
            scheduler = torch.optim.lr_scheduler.LambdaLR(optimizer, lr_lambda=lambda _: 1.0)
            scheduler.load_state_dict(payload["scheduler_state"])
            global_step = cast(int, payload["global_step"])
            cursor = cast(int, payload["cursor"])
            order = tuple(cast(list[int], payload["order"]))
            if len(order) != len(records) or order != _training_order(len(records), config.seed):
                raise ValueError("training checkpoint sample order mismatch")
            random.setstate(payload["python_rng_state"])
            np.random.set_state(payload["numpy_rng_state"])
            torch.set_rng_state(payload["torch_rng_state"])
        else:
            vocabularies = _fit_vocabulary(records)
            model = FairCombatPolicyValueNet(vocabularies, config.model_config())
            optimizer = torch.optim.AdamW(
                model.parameters(), lr=config.learning_rate, weight_decay=config.weight_decay
            )
            scheduler = torch.optim.lr_scheduler.LambdaLR(optimizer, lr_lambda=lambda _: 1.0)
            global_step = 0
            cursor = 0
            order = _training_order(len(records), config.seed)

        dataset = SymbolicCombatDataset(records, vocabularies)
        target_step = config.total_steps
        if stop_after_steps is not None:
            if (
                type(stop_after_steps) is not int
                or not global_step < stop_after_steps <= target_step
            ):
                raise ValueError(
                    "stop_after_steps must be after the current step and within total_steps"
                )
            target_step = stop_after_steps
        if wandb_offline is not None:
            puct_targets = manifest.manifest_version in {
                DATASET_MANIFEST_V6,
                DATASET_MANIFEST_V7,
            }
            session = start_offline_wandb(
                wandb_offline,
                {
                    "trainer": ("privileged_puct_distill" if puct_targets else "beam_clone"),
                    "teacher_name": manifest.teacher_name,
                    "teacher_version": manifest.teacher_version,
                    "dataset_manifest_version": manifest.manifest_version,
                    "puct_targets_in_training": puct_targets,
                    "resume": resume,
                    "resume_starting_step": global_step,
                    "training_config": asdict(config),
                    "config_digest": config.digest,
                    "dataset_manifest_digest": manifest.manifest_digest,
                    "dataset_shard_digest": manifest.shard_digest,
                    "root_manifest_digest": manifest.root_manifest_digest,
                    "cohort_digest": manifest.cohort_digest,
                    "teacher_search_contract_digest": manifest.teacher_search_contract_digest,
                    "reward_config_digest": manifest.reward_config_digest,
                    "source_digest": source_digest,
                    "runtime_identity_digest": runtime_identity_digest,
                    "vocabulary_fingerprint": vocabularies.fingerprint,
                    "encoder_contract_digest": encoder_contract_digest(vocabularies),
                },
            )
        model.train()
        while global_step < target_step:
            indices = tuple(
                order[(cursor + offset) % len(order)] for offset in range(config.batch_size)
            )
            cursor = (cursor + config.batch_size) % len(order)
            batch = collate_training_examples(tuple(dataset[index] for index in indices))
            output = model(batch.decision)
            loss = policy_value_loss(
                output,
                batch.policy_target,
                batch.value_target,
                batch.decision.action_mask,
                batch.value_target_mask,
            )
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            optimizer.step()
            scheduler.step()
            global_step += 1
            metric: dict[str, float | int] = {
                "step": global_step,
                "loss": float(loss.detach()),
            }
            metrics.append(metric)
            checkpoint: dict[str, object] = {
                "checkpoint_format": TRAINING_CHECKPOINT_FORMAT,
                "config": asdict(config),
                "config_digest": config.digest,
                "dataset_manifest_digest": manifest.manifest_digest,
                "dataset_shard_digest": manifest.shard_digest,
                "root_manifest_digest": manifest.root_manifest_digest,
                "cohort_digest": manifest.cohort_digest,
                "teacher_search_contract_digest": manifest.teacher_search_contract_digest,
                "reward_config_digest": manifest.reward_config_digest,
                "source_digest": source_digest,
                "runtime_identity": runtime_identity,
                "runtime_identity_digest": runtime_identity_digest,
                "vocabularies": vocabularies.to_dict(),
                "vocabulary_fingerprint": vocabularies.fingerprint,
                "encoder_contract_digest": encoder_contract_digest(vocabularies),
                "model_config": asdict(config.model_config()),
                "model_state": model.state_dict(),
                "optimizer_state": optimizer.state_dict(),
                "scheduler_state": scheduler.state_dict(),
                "training_target_statistics": _compute_training_target_statistics(records),
                "global_step": global_step,
                "cursor": cursor,
                "order": list(order),
                "python_rng_state": random.getstate(),
                "numpy_rng_state": np.random.get_state(),
                "torch_rng_state": torch.get_rng_state(),
            }
            _atomic_torch_save(checkpoint_path, checkpoint)
            if session is not None and session.active:
                try:
                    session.log_scalars({"loss": metric["loss"]}, step=global_step)
                except Exception as error:  # noqa: BLE001
                    _fail_open_tracking(session, "log", error)
        if session is not None and session.active:
            try:
                session.log_summary(
                    {
                        "checkpoint_path": str(checkpoint_path),
                        "checkpoint_digest": hashlib.sha256(
                            checkpoint_path.read_bytes()
                        ).hexdigest(),
                        "model_state_digest": _model_state_digest(model.state_dict()),
                        "global_step": global_step,
                    }
                )
            except Exception as error:  # noqa: BLE001
                _fail_open_tracking(session, "summary", error)
        return TrainingResult(
            checkpoint_path,
            global_step,
            tuple(metrics),
            runtime_identity_digest,
            vocabularies.fingerprint,
            encoder_contract_digest(vocabularies),
        )
    finally:
        if session is not None:
            try:
                session.finish()
            except Exception as error:  # noqa: BLE001
                _tracking_warn("finish", error)


def evaluate_beam_clone(
    dataset_manifest_path: Path,
    checkpoint_path: Path,
    *,
    split: str = "development",
    allow_audited_split: bool = False,
) -> dict[str, object]:
    manifest = load_dataset_manifest(
        dataset_manifest_path,
        requested_split=split,
        allow_audited_split=allow_audited_split,
    )
    checkpoint_bytes = checkpoint_path.read_bytes()
    payload, stored_config = _validate_checkpoint_envelope(
        torch.load(checkpoint_path, map_location="cpu", weights_only=False)
    )
    _configure_cpu(stored_config.torch_threads)
    runtime_identity = _runtime_identity()
    runtime_identity_digest = _digest(runtime_identity)
    if payload["runtime_identity_digest"] != runtime_identity_digest:
        raise ValueError("evaluation checkpoint runtime identity mismatch")
    if payload["source_digest"] != _source_digest():
        raise ValueError("evaluation checkpoint source digest mismatch")
    training_root_manifest_digest = cast(str, payload["root_manifest_digest"])
    training_cohort_digest = cast(str, payload["cohort_digest"])
    if payload["teacher_search_contract_digest"] != manifest.teacher_search_contract_digest:
        raise ValueError("evaluation teacher/search contract mismatch")
    if training_root_manifest_digest != manifest.root_manifest_digest:
        if not (allow_audited_split and manifest.audited_access):
            raise ValueError("evaluation root manifest mismatch")
        if training_cohort_digest != manifest.cohort_digest:
            raise ValueError("evaluation disjoint cohort")
    elif training_cohort_digest != manifest.cohort_digest:
        raise ValueError("evaluation disjoint cohort")
    if payload["reward_config_digest"] != manifest.reward_config_digest:
        raise ValueError("evaluation reward config mismatch")
    vocabularies = Vocabularies.from_dict(payload["vocabularies"])
    config = CombatModelConfig(**payload["model_config"])
    model = FairCombatPolicyValueNet(vocabularies, config)
    model.load_state_dict(payload["model_state"], strict=True)
    model.eval()
    records = tuple(read_jsonl(dataset_manifest_path.parent / manifest.shard_path))
    dataset = SymbolicCombatDataset(records, vocabularies)
    exact_numerator = 0
    any_max_numerator = 0
    always_first_index_numerator = 0
    tied_visit_argmax_records = 0
    truncated_numerator = 0
    truncated_denominator = 0
    truncated_roots: set[str] = set()
    nontruncated_numerator = 0
    nontruncated_denominator = 0
    value_absolute_error = 0.0
    value_mae_rows = 0
    errors = 0
    predicted_targets: list[float] = []
    observed_targets: list[float] = []
    per_record: list[dict[str, object]] = []
    with torch.inference_mode():
        for index in range(len(dataset)):
            record = records[index]
            truncated = record.outcome.truncated
            if truncated:
                truncated_denominator += 1
                truncated_roots.add(record.root_id)
            else:
                nontruncated_denominator += 1
            max_visits = max(record.teacher_visit_counts)
            argmax_set = {
                action_index
                for action_index, visits in enumerate(record.teacher_visit_counts)
                if visits == max_visits
            }
            tied = len(argmax_set) > 1
            tied_visit_argmax_records += int(tied)
            always_first_index_numerator += int(record.chosen_action_index == 0)
            row: dict[str, object] = {
                "record_id": record.record_id,
                "root_id": record.root_id,
                "status": record.outcome.status,
                "truncated": truncated,
                "value_target_mask": record.value_target_mask,
                "target_value": record.target_value,
                "teacher_action_index": record.chosen_action_index,
                "tied_visit_argmax": tied,
            }
            try:
                example = dataset[index]
                batch = collate_training_examples((example,))
                output = model(batch.decision)
                logits = output.logits[0, : example.decision.action_count]
                selected = int(torch.argmax(logits).item())
                expected = record.chosen_action_index
                correct = selected == expected
                any_max = selected in argmax_set
                predicted_value = float(output.value[0])
                if not math.isfinite(predicted_value):
                    raise ValueError("predicted value is not finite")
                value_error: float | None = None
                if record.value_target_mask:
                    target_value = float(batch.value_target[0])
                    if not math.isfinite(target_value):
                        raise ValueError("value target is not finite")
                    value_error = abs(predicted_value - target_value)
                    predicted_targets.append(predicted_value)
                    observed_targets.append(target_value)
                exact_numerator += int(correct)
                any_max_numerator += int(any_max)
                if truncated:
                    truncated_numerator += int(correct)
                else:
                    nontruncated_numerator += int(correct)
                if value_error is not None:
                    value_absolute_error += value_error
                    value_mae_rows += 1
                row.update(
                    {
                        "selected_action_index": selected,
                        "correct": correct,
                        "any_max_correct": any_max,
                        "predicted_value": predicted_value,
                    }
                )
            except (RuntimeError, TypeError, ValueError) as error:
                errors += 1
                row["error"] = str(error)
            per_record.append(row)
    exact_denominator = len(records)
    target_count = len(observed_targets)
    if target_count == 0:
        target_min: float | None = None
        target_max: float | None = None
        target_mean: float | None = None
        target_stddev: float | None = None
        student_mae: float | None = None
        prediction_mean: float | None = None
        prediction_mean_mae: float | None = None
        pearson: float | None = None
        pearson_reason: str | None = "no_unmasked_finite_pairs"
    else:
        target_min = min(observed_targets)
        target_max = max(observed_targets)
        target_mean = statistics.fmean(observed_targets)
        target_stddev = statistics.pstdev(observed_targets)
        student_mae = value_absolute_error / value_mae_rows
        prediction_mean = statistics.fmean(predicted_targets)
        prediction_mean_mae = statistics.fmean(
            [abs(predicted - prediction_mean) for predicted in observed_targets]
        )
        if target_count < 8:
            pearson = None
            pearson_reason = "n_lt_8"
        elif statistics.pstdev(predicted_targets) == 0.0 or target_stddev == 0.0:
            pearson = None
            pearson_reason = "zero_variance"
        else:
            pearson = float(statistics.correlation(predicted_targets, observed_targets))
            pearson_reason = None
    training_stats = payload.get("training_target_statistics")
    training_mean: float | None = None
    training_mean_mae: float | None = None
    if payload["checkpoint_format"] == TRAINING_CHECKPOINT_FORMAT:
        validated_stats = _validate_training_target_statistics(training_stats)
        stored_mean = validated_stats["mean"]
        if stored_mean is not None:
            training_mean = float(cast(int | float, stored_mean))
            if observed_targets:
                training_mean_mae = statistics.fmean(
                    [abs(target - training_mean) for target in observed_targets]
                )
    root_counts = Counter(record.root_id for record in records)
    root_sizes = tuple(root_counts.values())
    kish = (sum(root_sizes) ** 2) / sum(size * size for size in root_sizes)
    report: dict[str, object] = {
        "report_version": 4,
        "split": split,
        "checkpoint_step": payload["global_step"],
        "checkpoint_file_digest": hashlib.sha256(checkpoint_bytes).hexdigest(),
        "checkpoint_model_state_digest": _model_state_digest(payload["model_state"]),
        "checkpoint_config_digest": payload["config_digest"],
        "checkpoint_training_dataset_manifest_digest": payload["dataset_manifest_digest"],
        "checkpoint_training_dataset_shard_digest": payload["dataset_shard_digest"],
        "source_digest": payload["source_digest"],
        "runtime_identity_digest": runtime_identity_digest,
        "vocabulary_fingerprint": payload["vocabulary_fingerprint"],
        "encoder_contract_digest": payload["encoder_contract_digest"],
        "reward_config_digest": manifest.reward_config_digest,
        "checkpoint_training_root_manifest_digest": training_root_manifest_digest,
        "checkpoint_training_cohort_digest": training_cohort_digest,
        "teacher_search_contract_digest": manifest.teacher_search_contract_digest,
        "dataset_manifest_digest": manifest.manifest_digest,
        "dataset_shard_digest": manifest.shard_digest,
        "root_manifest_digest": manifest.root_manifest_digest,
        "cohort_digest": manifest.cohort_digest,
        "records": exact_denominator,
        "correct": exact_numerator,
        "errors": errors,
        "accuracy": exact_numerator / exact_denominator,
        "exact_numerator": exact_numerator,
        "exact_denominator": exact_denominator,
        "any_max_numerator": any_max_numerator,
        "any_max_denominator": exact_denominator,
        "any_max_accuracy": any_max_numerator / exact_denominator,
        "tied_visit_argmax_records": tied_visit_argmax_records,
        "tied_visit_argmax_fraction": tied_visit_argmax_records / exact_denominator,
        "always_first_index_numerator": always_first_index_numerator,
        "always_first_index_denominator": exact_denominator,
        "always_first_index_accuracy": always_first_index_numerator / exact_denominator,
        "truncated_numerator": truncated_numerator,
        "truncated_denominator": truncated_denominator,
        "truncated_root_count": len(truncated_roots),
        "nontruncated_numerator": nontruncated_numerator,
        "nontruncated_denominator": nontruncated_denominator,
        "value_mae": student_mae,
        "value_mae_rows": value_mae_rows,
        "target_value_count": target_count,
        "target_value_min": target_min,
        "target_value_max": target_max,
        "target_value_mean": target_mean,
        "target_value_population_stddev": target_stddev,
        "training_target_mean": training_mean,
        "training_target_mean_mae": training_mean_mae,
        "prediction_mean": prediction_mean,
        "prediction_mean_mae": prediction_mean_mae,
        "pearson_correlation": pearson,
        "pearson_undefined_reason": pearson_reason,
        "root_count": len(root_counts),
        "kish_cluster_ess": kish,
        "per_record": per_record,
    }
    report["report_digest"] = _digest(report)
    return report
