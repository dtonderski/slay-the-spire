"""Deterministic supervised beam-cloning training and development evaluation."""

from __future__ import annotations

import hashlib
import json
import os
import random
import tempfile
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
        payload = cast(
            dict[str, Any], torch.load(checkpoint_path, map_location="cpu", weights_only=False)
        )
        expected = {
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
        if set(payload) != expected or payload["checkpoint_format"] != TRAINING_CHECKPOINT_FORMAT:
            raise ValueError("unsupported or malformed training checkpoint")
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
        vocabularies = Vocabularies.from_dict(payload["vocabularies"])
        if payload["vocabulary_fingerprint"] != vocabularies.fingerprint:
            raise ValueError("training checkpoint vocabulary mismatch")
        if payload["encoder_contract_digest"] != encoder_contract_digest(vocabularies):
            raise ValueError("training checkpoint encoder contract mismatch")
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
        if order != _training_order(len(records), config.seed):
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
    payload = cast(
        dict[str, Any], torch.load(checkpoint_path, map_location="cpu", weights_only=False)
    )
    if payload.get("source_digest") != _source_digest():
        raise ValueError("evaluation checkpoint source digest mismatch")
    if payload.get("root_manifest_digest") != manifest.root_manifest_digest:
        raise ValueError("evaluation root manifest mismatch")
    if payload.get("reward_config_digest") != manifest.reward_config_digest:
        raise ValueError("evaluation reward config mismatch")
    vocabularies = Vocabularies.from_dict(payload["vocabularies"])
    if payload.get("encoder_contract_digest") != encoder_contract_digest(vocabularies):
        raise ValueError("evaluation encoder contract mismatch")
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
                # torch.argmax returns the first row on ties, matching canonical action order.
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
        "report_version": 1,
        "split": split,
        "checkpoint_step": payload["global_step"],
        "dataset_manifest_digest": manifest.manifest_digest,
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
