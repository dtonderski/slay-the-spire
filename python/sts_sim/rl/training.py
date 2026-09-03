"""Deterministic supervised beam-cloning training and development evaluation."""

from __future__ import annotations

import hashlib
import io
import math
import platform
import random
import statistics
import struct
import sys
from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, cast

import numpy as np
import torch

from .authorization import require_held_out_evaluation
from .data import DatasetManifest, RootManifest, load_dataset_manifest
from .experiment import _raise_if_symlink_ancestor, _read_regular_file_bytes, replace_file_bytes
from .model import CombatModelConfig, FairCombatPolicyValueNet, policy_value_loss
from .provenance import canonical_bytes, capture_repository_version, sha256_bytes
from .records import (
    RECORD_VERSION,
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    collate_training_examples,
)
from .source_epoch import loaded_native_bytes
from .tensor import Vocabularies, VocabularyBuilder, encoder_contract_digest

TRAINING_CHECKPOINT_FORMAT = 5
_MT19937_N = 624
_UINT32_MAX = 0xFFFFFFFF
_CHECKPOINT_KEYS = {
    "checkpoint_format",
    "config",
    "config_digest",
    "dataset_manifest_digest",
    "dataset_shard_digest",
    "root_manifest_digest",
    "cohort_digest",
    "source_epoch_bundle_digest",
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
    "training_target_statistics",
}
_TRAINING_TARGET_STATISTICS_KEYS = {
    "count",
    "mean",
    "min",
    "max",
    "population_stddev",
    "digest",
}


def _digest(payload: object) -> str:
    return sha256_bytes(canonical_bytes(payload))


def _atomic_torch_save(path: Path, payload: dict[str, object]) -> None:
    buffer = io.BytesIO()
    torch.save(payload, buffer)
    replace_file_bytes(path, buffer.getvalue())


def _require_int_sequence(values: object, label: str) -> list[int]:
    if type(values) is list:
        items: Sequence[object] = values
    elif type(values) is tuple:
        items = values
    else:
        raise TypeError(f"{label} must be a sequence of integers")
    integers: list[int] = []
    for value in items:
        if type(value) is not int:
            raise TypeError(f"{label} must be a sequence of integers")
        integers.append(value)
    return integers


def _python_rng_payload(state: object) -> list[object]:
    if type(state) is not tuple or len(state) != 3:
        raise TypeError("python RNG state must be a 3-tuple")
    version, mt_state, gauss = state
    if type(version) is not int:
        raise TypeError("python RNG state fields are invalid")
    mt_values = _require_int_sequence(mt_state, "python RNG state")
    if gauss is not None and type(gauss) not in {int, float}:
        raise TypeError("python RNG gauss field is invalid")
    return [version, mt_values, None if gauss is None else float(cast(int | float, gauss))]


def _python_rng_from_payload(payload: object) -> tuple[int, tuple[int, ...], float | None]:
    if type(payload) is not list or len(payload) != 3:
        raise TypeError("training checkpoint python RNG state is invalid")
    version, mt_state, gauss = payload
    if type(version) is not int:
        raise TypeError("training checkpoint python RNG state is invalid")
    if version != 3:
        raise ValueError("training checkpoint python RNG version is invalid")
    mt_values = tuple(_require_int_sequence(mt_state, "training checkpoint python RNG state"))
    if len(mt_values) != _MT19937_N + 1:
        raise ValueError("training checkpoint python RNG state layout is invalid")
    *words, index = mt_values
    if any(word < 0 or word > _UINT32_MAX for word in words):
        raise ValueError("training checkpoint python RNG words are out of range")
    if not 0 <= index <= _MT19937_N:
        raise ValueError("training checkpoint python RNG index is out of range")
    if gauss is not None and type(gauss) not in {int, float}:
        raise TypeError("training checkpoint python RNG state is invalid")
    gauss_value = None if gauss is None else float(cast(int | float, gauss))
    if gauss_value is not None and not math.isfinite(gauss_value):
        raise ValueError("training checkpoint python RNG gaussian is not finite")
    state = (version, mt_values, gauss_value)
    probe = random.Random()
    try:
        probe.setstate(state)
    except (OverflowError, TypeError, ValueError) as error:
        raise ValueError("training checkpoint python RNG state is not loadable") from error
    return state


def _numpy_mt19937_key(key: object, label: str) -> list[int]:
    if isinstance(key, np.ndarray):
        if key.dtype != np.dtype(np.uint32) or key.ndim != 1 or key.size != _MT19937_N:
            raise TypeError(f"{label} must be a 624-length uint32 vector")
        blob = np.ascontiguousarray(key, dtype=np.uint32).tobytes()
        if len(blob) != _MT19937_N * 4:
            raise TypeError(f"{label} must be a 624-length uint32 vector")
        values = list(struct.unpack(f"{_MT19937_N}I", blob))
    else:
        values = _require_int_sequence(key, label)
        if len(values) != _MT19937_N:
            raise ValueError(f"{label} must contain 624 integers")
    if any(value < 0 or value > _UINT32_MAX for value in values):
        raise ValueError(f"{label} values must fit in uint32")
    return values


def _numpy_rng_payload(state: object) -> dict[str, object]:
    if type(state) is not tuple or len(state) != 5:
        raise TypeError("numpy RNG state must be a 5-tuple")
    bit_generator, key, pos, has_gauss, cached_gaussian = cast(tuple[object, ...], state)
    if type(bit_generator) is not str or not bit_generator:
        raise TypeError("numpy RNG bit generator is invalid")
    key_values = _numpy_mt19937_key(key, "numpy RNG key")
    if type(pos) is not int or type(has_gauss) is not int:
        raise TypeError("numpy RNG counters are invalid")
    cached = float(cast(int | float, cached_gaussian))
    return {
        "bit_generator": bit_generator,
        "key": key_values,
        "pos": pos,
        "has_gauss": has_gauss,
        "cached_gaussian": cached,
    }


def _numpy_rng_from_payload(payload: object) -> tuple[str, np.ndarray, int, int, float]:
    if type(payload) is not dict:
        raise TypeError("training checkpoint numpy RNG state is invalid")
    source = cast(dict[str, object], payload)
    if set(source) != {"bit_generator", "key", "pos", "has_gauss", "cached_gaussian"}:
        raise ValueError("training checkpoint numpy RNG state has missing or unknown fields")
    bit_generator = source["bit_generator"]
    key = source["key"]
    pos = source["pos"]
    has_gauss = source["has_gauss"]
    cached_gaussian = source["cached_gaussian"]
    if type(bit_generator) is not str or not bit_generator:
        raise TypeError("training checkpoint numpy RNG bit generator is invalid")
    if type(key) is not list or type(pos) is not int or type(has_gauss) is not int:
        raise TypeError("training checkpoint numpy RNG state is invalid")
    if type(cached_gaussian) not in {int, float}:
        raise TypeError("training checkpoint numpy RNG cached gaussian is invalid")
    key_values = _numpy_mt19937_key(key, "training checkpoint numpy RNG key")
    if bit_generator != "MT19937":
        raise ValueError("training checkpoint numpy RNG must be MT19937")
    if not 0 <= pos <= _MT19937_N:
        raise ValueError("training checkpoint numpy RNG position is out of range")
    if has_gauss not in {0, 1}:
        raise ValueError("training checkpoint numpy RNG has_gauss is invalid")
    cached = float(cast(int | float, cached_gaussian))
    if not math.isfinite(cached):
        raise ValueError("training checkpoint numpy RNG cached gaussian is not finite")
    key_array = np.empty(_MT19937_N, dtype=np.uint32)
    key_array[:] = key_values
    state = (bit_generator, key_array, pos, has_gauss, cached)
    probe = np.random.RandomState()
    try:
        probe.set_state(state)
    except (TypeError, ValueError) as error:
        raise ValueError("training checkpoint numpy RNG state is not loadable") from error
    return state


def _torch_rng_from_payload(value: object) -> torch.Tensor:
    if not isinstance(value, torch.Tensor):
        raise TypeError("training checkpoint torch RNG state is invalid")
    if value.device.type != "cpu":
        raise ValueError("training checkpoint torch RNG state must be on CPU")
    if value.ndim != 1:
        raise ValueError("training checkpoint torch RNG state must be 1-D")
    if not value.is_contiguous():
        raise ValueError("training checkpoint torch RNG state must be contiguous")
    if value.dtype != torch.uint8:
        raise ValueError("training checkpoint torch RNG state must be uint8")
    probe = torch.Generator(device="cpu")
    try:
        probe.set_state(value)
    except (RuntimeError, TypeError, ValueError) as error:
        raise ValueError("training checkpoint torch RNG state is not loadable") from error
    return value


def load_training_checkpoint(path: Path) -> tuple[dict[str, Any], TrainingConfig, str]:
    """Load one checkpoint from a single nofollow read with weights-only unpickling."""

    _raise_if_symlink_ancestor(path)
    content = _read_regular_file_bytes(path)
    raw = torch.load(io.BytesIO(content), map_location="cpu", weights_only=True)
    payload, stored_config = _validate_checkpoint_envelope(raw)
    return payload, stored_config, sha256_bytes(content)


def _source_digest() -> str:
    """Attest the complete checkout and the exact loaded native extension bytes."""

    repository_root = Path(__file__).resolve().parents[3]
    repository = capture_repository_version(repository_root)
    payload = {
        "repository": repository.to_dict(),
        "native_extension_digest": sha256_bytes(loaded_native_bytes()),
    }
    return _digest(payload)


def _runtime_identity() -> dict[str, object]:
    python_root = Path(__file__).resolve().parents[2]
    artifact_digests = {
        name: sha256_bytes(_read_regular_file_bytes(python_root / name))
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
        digest.update(canonical_bytes(list(tensor.shape)))
        digest.update(tensor.numpy().tobytes())
    return digest.hexdigest()


def _bounded_fmean(values: Sequence[float]) -> float:
    if not values:
        raise ValueError("cannot bound the mean of an empty series")
    minimum = min(values)
    maximum = max(values)
    mean = statistics.fmean(values)
    if mean < minimum:
        return minimum
    if mean > maximum:
        return maximum
    return mean


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
        minimum = min(values)
        maximum = max(values)
        unsigned = {
            "count": len(values),
            "mean": _bounded_fmean(values),
            "min": minimum,
            "max": maximum,
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
        if not (minimum <= mean <= maximum) and not (
            math.isclose(mean, minimum, rel_tol=0.0, abs_tol=1e-12)
            or math.isclose(mean, maximum, rel_tol=0.0, abs_tol=1e-12)
        ):
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


def _canonical_unmasked_target(record: SymbolicTrainingRecord) -> float:
    if not record.value_target_mask or record.target_value is None:
        raise ValueError("unmasked evaluation target is missing")
    value = float(record.target_value)
    if not math.isfinite(value):
        raise ValueError("unmasked evaluation target is not finite")
    return value


def _kish_ess(sizes: Sequence[int]) -> float | None:
    if not sizes:
        return None
    total = sum(sizes)
    return (total * total) / sum(size * size for size in sizes)


def _pearson_correlation(
    predicted: Sequence[float], targets: Sequence[float]
) -> tuple[float | None, str | None]:
    if len(predicted) != len(targets):
        raise ValueError("pearson series length mismatch")
    if len(targets) < 8:
        return None, "n_lt_8"
    if statistics.pstdev(predicted) == 0.0 or statistics.pstdev(targets) == 0.0:
        return None, "zero_variance"
    return float(statistics.correlation(predicted, targets)), None


def _mean_absolute_deviation(values: Sequence[float], constant: float) -> float:
    return statistics.fmean(abs(value - constant) for value in values)


def _validate_checkpoint_envelope(payload: object) -> tuple[dict[str, Any], TrainingConfig]:
    if type(payload) is not dict:
        raise TypeError("training checkpoint must be an object")
    source = cast(dict[str, Any], payload)
    checkpoint = dict(source)
    checkpoint_format = checkpoint.get("checkpoint_format")
    if type(checkpoint_format) is not int or checkpoint_format != TRAINING_CHECKPOINT_FORMAT:
        raise ValueError("unsupported or malformed training checkpoint")
    if set(checkpoint) != _CHECKPOINT_KEYS:
        raise ValueError("unsupported or malformed training checkpoint")
    checkpoint["training_target_statistics"] = _validate_training_target_statistics(
        source["training_target_statistics"]
    )
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
        "source_epoch_bundle_digest",
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
    _python_rng_from_payload(checkpoint["python_rng_state"])
    _numpy_rng_from_payload(checkpoint["numpy_rng_state"])
    checkpoint["torch_rng_state"] = _torch_rng_from_payload(checkpoint["torch_rng_state"])
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
) -> tuple[DatasetManifest, RootManifest, tuple[SymbolicTrainingRecord, ...]]:
    manifest, named_root, records = load_dataset_manifest(
        manifest_path, requested_split=split
    )
    if named_root.manifest_digest != manifest.root_manifest_digest:
        raise ValueError("authenticated dataset root identity does not match the dataset manifest")
    if not records:
        raise ValueError("training dataset is empty")
    versions = {record.record_version for record in records}
    if versions != {RECORD_VERSION}:
        raise ValueError("training requires the current record schema")
    root_count = len(manifest.roots)
    lineage_count = len({lineage for root in manifest.roots for lineage in root.lineages})
    if root_count < minimum_roots or lineage_count < minimum_lineages:
        raise ValueError(
            "training corpus is below configured minimums: "
            f"roots {root_count}/{minimum_roots}, lineages {lineage_count}/{minimum_lineages}"
        )
    return manifest, named_root, records


def _fit_vocabulary(records: tuple[SymbolicTrainingRecord, ...]) -> Vocabularies:
    builder = VocabularyBuilder()
    for record in records:
        builder.add(record.observation, record.actions)
    return builder.freeze()


def _training_order(length: int, seed: int) -> tuple[int, ...]:
    generator = torch.Generator(device="cpu")
    generator.manual_seed(seed)
    return tuple(int(index) for index in torch.randperm(length, generator=generator).tolist())


def train_beam_clone(
    dataset_manifest_path: Path,
    checkpoint_path: Path,
    config: TrainingConfig,
    *,
    resume: bool = False,
    stop_after_steps: int | None = None,
) -> TrainingResult:
    """Train through ``config.total_steps`` and atomically checkpoint every batch."""

    _configure_cpu(config.torch_threads)
    manifest, training_root, records = _load_records(
        dataset_manifest_path,
        "train",
        config.minimum_roots,
        config.minimum_lineages,
    )
    source_epoch_bundle_digest = training_root.source_epoch_bundle_digest
    source_digest = _source_digest()
    runtime_identity = _runtime_identity()
    runtime_identity_digest = _digest(runtime_identity)
    random.seed(config.seed)
    np.random.seed(config.seed)
    torch.manual_seed(config.seed)
    metrics: list[dict[str, float | int]] = []
    if resume:
        payload, stored_config, _file_digest = load_training_checkpoint(checkpoint_path)
        checks = {
            "config_digest": config.digest,
            "dataset_manifest_digest": manifest.manifest_digest,
            "dataset_shard_digest": manifest.shard_digest,
            "root_manifest_digest": manifest.root_manifest_digest,
            "cohort_digest": manifest.cohort_digest,
            "source_epoch_bundle_digest": source_epoch_bundle_digest,
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
        if payload["training_target_statistics"] != expected_stats:
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
        random.setstate(_python_rng_from_payload(payload["python_rng_state"]))
        np.random.set_state(_numpy_rng_from_payload(payload["numpy_rng_state"]))
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
    training_target_statistics = _compute_training_target_statistics(records)
    target_step = config.total_steps
    if stop_after_steps is not None:
        if type(stop_after_steps) is not int or not global_step < stop_after_steps <= target_step:
            raise ValueError(
                "stop_after_steps must be after the current step and within total_steps"
            )
        target_step = stop_after_steps
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
            "source_epoch_bundle_digest": source_epoch_bundle_digest,
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
            "training_target_statistics": training_target_statistics,
            "global_step": global_step,
            "cursor": cursor,
            "order": list(order),
            "python_rng_state": _python_rng_payload(random.getstate()),
            "numpy_rng_state": _numpy_rng_payload(np.random.get_state()),
            "torch_rng_state": torch.get_rng_state(),
        }
        _atomic_torch_save(checkpoint_path, checkpoint)
    return TrainingResult(
        checkpoint_path,
        global_step,
        tuple(metrics),
        runtime_identity_digest,
        vocabularies.fingerprint,
        encoder_contract_digest(vocabularies),
    )


def evaluate_beam_clone(
    dataset_manifest_path: Path,
    checkpoint_path: Path,
    *,
    split: str = "development",
    evaluation_seed: int = 0,
    authorization_path: Path | None = None,
    training_root_manifest_path: Path | None = None,
) -> dict[str, object]:
    manifest, evaluation_root, records = load_dataset_manifest(
        dataset_manifest_path,
        requested_split=split,
    )
    if evaluation_root.manifest_digest != manifest.root_manifest_digest:
        raise ValueError("authenticated dataset root identity does not match the dataset manifest")
    evaluation_root_manifest_path = dataset_manifest_path.parent / manifest.root_manifest_path
    payload, stored_config, checkpoint_file_digest = load_training_checkpoint(checkpoint_path)
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
    if payload["source_epoch_bundle_digest"] != evaluation_root.source_epoch_bundle_digest:
        raise ValueError("evaluation source-epoch-bundle digest mismatch")
    require_held_out_evaluation(
        training_root_manifest_digest=training_root_manifest_digest,
        training_cohort_digest=training_cohort_digest,
        evaluation_manifest=evaluation_root,
        evaluation_root_manifest_path=evaluation_root_manifest_path,
        evaluation_split=split,
        evaluation_seed=evaluation_seed,
        requested_evaluator_names=("beam_clone",),
        authorization_path=authorization_path,
        training_root_manifest_path=training_root_manifest_path,
    )
    if payload["reward_config_digest"] != manifest.reward_config_digest:
        raise ValueError("evaluation reward config mismatch")
    vocabularies = Vocabularies.from_dict(payload["vocabularies"])
    config = CombatModelConfig(**payload["model_config"])
    model = FairCombatPolicyValueNet(vocabularies, config)
    model.load_state_dict(payload["model_state"], strict=True)
    model.eval()
    dataset = SymbolicCombatDataset(records, vocabularies)
    exact_numerator = 0
    any_max_numerator = 0
    always_first_index_numerator = 0
    always_first_in_max_visit_numerator = 0
    tied_visit_argmax_records = 0
    truncated_numerator = 0
    truncated_denominator = 0
    truncated_roots: set[str] = set()
    nontruncated_numerator = 0
    nontruncated_denominator = 0
    errors = 0
    value_pairs: list[tuple[str, float, float]] = []
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
            # Symbolic teacher baselines do not require model inference and stay
            # defined on every labeled record, including later inference failures.
            always_first_index_numerator += int(record.chosen_action_index == 0)
            always_first_in_max_visit_numerator += int(0 in argmax_set)
            row: dict[str, object] = {
                "record_id": record.record_id,
                "root_id": record.root_id,
                "status": record.outcome.status,
                "truncated": truncated,
                "value_target_mask": record.value_target_mask,
                "target_value": record.target_value,
                "teacher_action_index": record.chosen_action_index,
                "decision_index": record.decision_index,
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
                if record.value_target_mask:
                    target_value = _canonical_unmasked_target(record)
                    value_pairs.append((record.root_id, predicted_value, target_value))
                exact_numerator += int(correct)
                any_max_numerator += int(any_max)
                if truncated:
                    truncated_numerator += int(correct)
                else:
                    nontruncated_numerator += int(correct)
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
    predicted_values = [predicted for _root_id, predicted, _target in value_pairs]
    observed_targets = [target for _root_id, _predicted, target in value_pairs]
    value_mae_rows = len(value_pairs)
    if not value_pairs:
        target_min: float | None = None
        target_max: float | None = None
        target_mean: float | None = None
        target_stddev: float | None = None
        student_mae: float | None = None
        prediction_mean: float | None = None
        prediction_mean_mae: float | None = None
        target_mean_mae: float | None = None
        target_median_mae: float | None = None
        pearson: float | None = None
        pearson_reason: str | None = "no_unmasked_finite_pairs"
    else:
        target_min = min(observed_targets)
        target_max = max(observed_targets)
        target_mean = _bounded_fmean(observed_targets)
        target_stddev = statistics.pstdev(observed_targets)
        student_mae = statistics.fmean(
            [
                abs(predicted - target)
                for predicted, target in zip(predicted_values, observed_targets, strict=True)
            ]
        )
        prediction_mean = statistics.fmean(predicted_values)
        prediction_mean_mae = _mean_absolute_deviation(observed_targets, prediction_mean)
        target_mean_mae = _mean_absolute_deviation(observed_targets, target_mean)
        target_median_mae = _mean_absolute_deviation(
            observed_targets, float(statistics.median(observed_targets))
        )
        pearson, pearson_reason = _pearson_correlation(predicted_values, observed_targets)
    training_mean: float | None = None
    training_mean_mae: float | None = None
    validated_stats = _validate_training_target_statistics(payload["training_target_statistics"])
    stored_mean = validated_stats["mean"]
    if stored_mean is None:
        training_mean_reason = "zero_unmasked_targets"
    else:
        training_mean = float(cast(int | float, stored_mean))
        training_mean_reason = None
        if observed_targets:
            training_mean_mae = _mean_absolute_deviation(observed_targets, training_mean)
    root_counts = Counter(record.root_id for record in records)
    value_pair_root_counts = Counter(root_id for root_id, _predicted, _target in value_pairs)
    kish = _kish_ess(tuple(root_counts.values()))
    if kish is None:
        raise ValueError("evaluation dataset must contain records")
    report: dict[str, object] = {
        "report_version": 4,
        "split": split,
        "checkpoint_step": payload["global_step"],
        "checkpoint_file_digest": checkpoint_file_digest,
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
        "always_first_index_in_max_visit_set_numerator": always_first_in_max_visit_numerator,
        "always_first_index_in_max_visit_set_denominator": exact_denominator,
        "always_first_index_in_max_visit_set_accuracy": (
            always_first_in_max_visit_numerator / exact_denominator
        ),
        "always_first_index_denominator_note": (
            "always-first baselines are symbolic teacher statistics over every labeled "
            "record and do not require model inference; model exact/any-max accuracies "
            "keep inference errors in the denominator as misses. Under the current "
            "first-argmax schema, always_first_index_in_max_visit_set is equivalent to "
            "always_first_index because chosen_action_index is the first visit-count argmax."
        ),
        "truncated_numerator": truncated_numerator,
        "truncated_denominator": truncated_denominator,
        "truncated_root_count": len(truncated_roots),
        "nontruncated_numerator": nontruncated_numerator,
        "nontruncated_denominator": nontruncated_denominator,
        "value_mae": student_mae,
        "value_mae_rows": value_mae_rows,
        "target_value_count": value_mae_rows,
        "target_value_min": target_min,
        "target_value_max": target_max,
        "target_value_mean": target_mean,
        "target_value_population_stddev": target_stddev,
        "target_mean_mae": target_mean_mae,
        "target_median_mae": target_median_mae,
        "training_target_mean": training_mean,
        "training_target_mean_mae": training_mean_mae,
        "training_target_mean_undefined_reason": training_mean_reason,
        "prediction_mean": prediction_mean,
        "prediction_mean_mae": prediction_mean_mae,
        "pearson_correlation": pearson,
        "pearson_undefined_reason": pearson_reason,
        "root_count": len(root_counts),
        "kish_cluster_ess": kish,
        "value_pair_root_count": len(value_pair_root_counts),
        "value_pair_kish_cluster_ess": _kish_ess(tuple(value_pair_root_counts.values())),
        "per_record": per_record,
    }
    report["report_digest"] = _digest(report)
    return report
