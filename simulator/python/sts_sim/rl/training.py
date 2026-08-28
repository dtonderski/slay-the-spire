"""Deterministic supervised beam-cloning training and development evaluation."""

from __future__ import annotations

import hashlib
import json
import os
import random
import tempfile
from collections.abc import Mapping
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, cast

import numpy as np
import torch

from .data import DatasetManifest, load_dataset_manifest
from .model import CombatModelConfig, FairCombatPolicyValueNet, policy_value_loss
from .records import (
    SymbolicCombatDataset,
    SymbolicTrainingRecord,
    collate_training_examples,
    read_jsonl,
)
from .tensor import Vocabularies, VocabularyBuilder, encoder_contract_digest

TRAINING_CHECKPOINT_FORMAT = 1
_CHECKPOINT_KEYS = {
    "checkpoint_format",
    "config",
    "config_digest",
    "dataset_manifest_digest",
    "dataset_shard_digest",
    "root_manifest_digest",
    "reward_config_digest",
    "source_digest",
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


def _canonical_bytes(payload: object) -> bytes:
    return json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()


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
    directory = Path(__file__).parent
    files = ("data.py", "model.py", "records.py", "rewards.py", "tensor.py", "training.py")
    payload = {name: hashlib.sha256((directory / name).read_bytes()).hexdigest() for name in files}
    return _digest(payload)


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


def _validate_checkpoint_envelope(payload: object) -> tuple[dict[str, Any], TrainingConfig]:
    if type(payload) is not dict:
        raise TypeError("training checkpoint must be an object")
    checkpoint = cast(dict[str, Any], payload)
    if (
        set(checkpoint) != _CHECKPOINT_KEYS
        or checkpoint["checkpoint_format"] != TRAINING_CHECKPOINT_FORMAT
    ):
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
    for name in (
        "dataset_manifest_digest",
        "dataset_shard_digest",
        "root_manifest_digest",
        "reward_config_digest",
        "source_digest",
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
    seed: int = 7
    batch_size: int = 32
    total_steps: int = 100
    learning_rate: float = 1e-3
    weight_decay: float = 1e-4
    torch_threads: int = 1
    model_width: int = 96
    model_heads: int = 4
    model_layers: int = 2
    feedforward_width: int = 192

    def __post_init__(self) -> None:
        if any(
            type(value) is not int or value <= 0
            for value in (
                self.batch_size,
                self.total_steps,
                self.torch_threads,
                self.model_width,
                self.model_heads,
                self.model_layers,
                self.feedforward_width,
            )
        ):
            raise ValueError("integer training configuration must be positive")
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
    manifest_path: Path, split: str
) -> tuple[DatasetManifest, tuple[SymbolicTrainingRecord, ...]]:
    manifest = load_dataset_manifest(manifest_path, requested_split=split)
    records = tuple(read_jsonl(manifest_path.parent / manifest.shard_path))
    if not records:
        raise ValueError("training dataset is empty")
    if any(record.record_version != 2 for record in records):
        raise ValueError("training requires record schema V2")
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
    manifest, records = _load_records(dataset_manifest_path, "train")
    source_digest = _source_digest()
    random.seed(config.seed)
    np.random.seed(config.seed)
    torch.manual_seed(config.seed)
    metrics: list[dict[str, float | int]] = []

    if resume:
        payload, stored_config = _validate_checkpoint_envelope(
            torch.load(checkpoint_path, map_location="cpu", weights_only=False)
        )
        checks = {
            "config_digest": config.digest,
            "dataset_manifest_digest": manifest.manifest_digest,
            "dataset_shard_digest": manifest.shard_digest,
            "root_manifest_digest": manifest.root_manifest_digest,
            "reward_config_digest": manifest.reward_config_digest,
            "source_digest": source_digest,
        }
        for name, expected_value in checks.items():
            if payload[name] != expected_value:
                raise ValueError(f"training checkpoint {name} mismatch")
        if stored_config != config:
            raise ValueError("training checkpoint config mismatch")
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
            "reward_config_digest": manifest.reward_config_digest,
            "source_digest": source_digest,
            "vocabularies": vocabularies.to_dict(),
            "vocabulary_fingerprint": vocabularies.fingerprint,
            "encoder_contract_digest": encoder_contract_digest(vocabularies),
            "model_config": asdict(config.model_config()),
            "model_state": model.state_dict(),
            "optimizer_state": optimizer.state_dict(),
            "scheduler_state": scheduler.state_dict(),
            "global_step": global_step,
            "cursor": cursor,
            "order": list(order),
            "python_rng_state": random.getstate(),
            "numpy_rng_state": np.random.get_state(),
            "torch_rng_state": torch.get_rng_state(),
        }
        _atomic_torch_save(checkpoint_path, checkpoint)

    return TrainingResult(
        checkpoint_path,
        global_step,
        tuple(metrics),
        vocabularies.fingerprint,
        encoder_contract_digest(vocabularies),
    )


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
    if payload["source_digest"] != _source_digest():
        raise ValueError("evaluation checkpoint source digest mismatch")
    if payload["root_manifest_digest"] != manifest.root_manifest_digest:
        raise ValueError("evaluation root manifest mismatch")
    if payload["reward_config_digest"] != manifest.reward_config_digest:
        raise ValueError("evaluation reward config mismatch")
    vocabularies = Vocabularies.from_dict(payload["vocabularies"])
    config = CombatModelConfig(**payload["model_config"])
    model = FairCombatPolicyValueNet(vocabularies, config)
    model.load_state_dict(payload["model_state"], strict=True)
    model.eval()
    records = tuple(read_jsonl(dataset_manifest_path.parent / manifest.shard_path))
    dataset = SymbolicCombatDataset(records, vocabularies)
    correct = 0
    errors = 0
    value_absolute_error = 0.0
    value_rows = 0
    per_record: list[dict[str, object]] = []
    with torch.inference_mode():
        for index in range(len(dataset)):
            try:
                example = dataset[index]
                batch = collate_training_examples((example,))
                output = model(batch.decision)
                logits = output.logits[0, : example.decision.action_count]
                selected = int(torch.argmax(logits).item())
                expected = records[index].chosen_action_index
                correct += int(selected == expected)
                if records[index].value_target_mask:
                    value_absolute_error += abs(
                        float(output.value[0]) - float(batch.value_target[0])
                    )
                    value_rows += 1
                per_record.append(
                    {
                        "record_id": records[index].record_id,
                        "selected_action_index": selected,
                        "teacher_action_index": expected,
                        "correct": selected == expected,
                    }
                )
            except (RuntimeError, TypeError, ValueError) as error:
                errors += 1
                per_record.append({"record_id": records[index].record_id, "error": str(error)})
    report: dict[str, object] = {
        "report_version": 2,
        "split": split,
        "checkpoint_step": payload["global_step"],
        "checkpoint_file_digest": hashlib.sha256(checkpoint_bytes).hexdigest(),
        "checkpoint_model_state_digest": _model_state_digest(payload["model_state"]),
        "checkpoint_config_digest": payload["config_digest"],
        "checkpoint_training_dataset_manifest_digest": payload["dataset_manifest_digest"],
        "checkpoint_training_dataset_shard_digest": payload["dataset_shard_digest"],
        "source_digest": payload["source_digest"],
        "vocabulary_fingerprint": payload["vocabulary_fingerprint"],
        "encoder_contract_digest": payload["encoder_contract_digest"],
        "reward_config_digest": manifest.reward_config_digest,
        "dataset_manifest_digest": manifest.manifest_digest,
        "dataset_shard_digest": manifest.shard_digest,
        "root_manifest_digest": manifest.root_manifest_digest,
        "records": len(records),
        "correct": correct,
        "errors": errors,
        "accuracy": correct / len(records),
        "value_mae": None if value_rows == 0 else value_absolute_error / value_rows,
        "per_record": per_record,
    }
    report["report_digest"] = _digest(report)
    return report
