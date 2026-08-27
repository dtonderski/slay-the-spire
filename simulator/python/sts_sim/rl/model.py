"""Small permutation-aware combat policy/value network and checkpoints."""

from __future__ import annotations

import json
import warnings
from collections.abc import Mapping
from dataclasses import asdict, dataclass
from pathlib import Path
from types import MappingProxyType
from typing import Any, Final, cast

import torch
from torch import nn

from .provenance import RepositoryVersion, file_digest
from .tensor import (
    CATEGORY_NAMESPACES,
    SCALAR_NAMES,
    BatchedCombatDecision,
    Vocabularies,
)

CHECKPOINT_FORMAT: Final = 1
_TENSORIZER_SOURCE_PATH: Final = Path(__file__).with_name("tensor.py")
CONTENT_NAMESPACE_BY_KIND: Final = {
    "global": "phase",
    "player": "entity_kind",
    "pile": "zone",
    "card": "card",
    "monster": "monster",
    "relic": "relic",
    "potion": "potion",
    "orb": "orb",
    "selection": "selection",
}


@dataclass(frozen=True, slots=True)
class CombatModelConfig:
    width: int = 96
    heads: int = 4
    layers: int = 2
    feedforward_width: int = 192
    dropout: float = 0.0

    def __post_init__(self) -> None:
        if any(
            type(value) is not int
            for value in (self.width, self.heads, self.layers, self.feedforward_width)
        ) or type(self.dropout) not in {int, float}:
            raise TypeError("model dimensions and dropout have invalid types")
        if self.width <= 0 or self.heads <= 0 or self.layers <= 0:
            raise ValueError("model dimensions must be positive")
        if self.width % self.heads != 0:
            raise ValueError("model width must be divisible by attention heads")
        if not 0.0 <= self.dropout < 1.0:
            raise ValueError("dropout must be in [0, 1)")


@dataclass(frozen=True, slots=True)
class PolicyValueOutput:
    logits: torch.Tensor
    value: torch.Tensor
    entity_states: torch.Tensor


class FairCombatPolicyValueNet(nn.Module):
    """Entity Transformer with a dynamic scorer over only current legal actions."""

    def __init__(
        self,
        vocabularies: Vocabularies,
        config: CombatModelConfig | None = None,
    ) -> None:
        super().__init__()
        config = CombatModelConfig() if config is None else config
        self.vocabularies = vocabularies
        self.config = config
        width = config.width
        self.kind_embedding = nn.Embedding(
            len(vocabularies.namespaces["entity_kind"].tokens), width
        )
        self.content_embeddings = nn.ModuleDict(
            {
                namespace: nn.Embedding(len(vocabularies.namespaces[namespace].tokens), width)
                for namespace in sorted(set(CONTENT_NAMESPACE_BY_KIND.values()))
            }
        )
        self.entity_kind_codes = {
            kind: vocabularies.encode("entity_kind", kind)[0] for kind in CONTENT_NAMESPACE_BY_KIND
        }
        self.zone_embedding = nn.Embedding(len(vocabularies.namespaces["zone"].tokens), width)
        self.category_embeddings = nn.ModuleList(
            nn.Embedding(len(vocabularies.namespaces[name].tokens), width)
            for name in CATEGORY_NAMESPACES
        )
        scalar_width = len(SCALAR_NAMES) * 2
        power_width = len(vocabularies.namespaces["power"].tokens) * 3
        counter_width = len(vocabularies.namespaces["counter"].tokens) * 3
        self.numeric_projection = nn.Linear(scalar_width + power_width + counter_width, width)
        self.parent_projection = nn.Linear(width, width, bias=False)
        layer = nn.TransformerEncoderLayer(
            d_model=width,
            nhead=config.heads,
            dim_feedforward=config.feedforward_width,
            dropout=config.dropout,
            activation="gelu",
            batch_first=True,
            norm_first=False,
        )
        self.encoder = nn.TransformerEncoder(
            layer,
            num_layers=config.layers,
            enable_nested_tensor=False,
        )
        self.entity_norm = nn.LayerNorm(width)
        self.action_family_embedding = nn.Embedding(
            len(vocabularies.namespaces["action_family"].tokens), width
        )
        self.action_kind_embedding = nn.Embedding(
            len(vocabularies.namespaces["action_kind"].tokens), width
        )
        self.no_source = nn.Parameter(torch.empty(width))
        self.no_target = nn.Parameter(torch.empty(width))
        self.action_scorer = nn.Sequential(
            nn.Linear(width * 5, config.feedforward_width),
            nn.GELU(),
            nn.Linear(config.feedforward_width, 1),
        )
        self.value_head = nn.Sequential(
            nn.Linear(width, config.feedforward_width),
            nn.GELU(),
            nn.Linear(config.feedforward_width, 1),
            nn.Tanh(),
        )
        nn.init.normal_(self.no_source, std=0.02)
        nn.init.normal_(self.no_target, std=0.02)

    def _content_inputs(self, batch: BatchedCombatDecision) -> torch.Tensor:
        shape = (*batch.entity_content.shape, self.config.width)
        result = torch.zeros(
            shape, dtype=self.kind_embedding.weight.dtype, device=batch.entity_content.device
        )
        for kind, namespace in CONTENT_NAMESPACE_BY_KIND.items():
            mask = batch.entity_kind == self.entity_kind_codes[kind]
            safe_content = torch.where(mask, batch.entity_content, 0)
            embedded = self.content_embeddings[namespace](safe_content)
            result = result + embedded * mask.unsqueeze(-1)
        return result

    def _entity_inputs(self, batch: BatchedCombatDecision) -> torch.Tensor:
        values = batch.entity_scalars * batch.entity_scalar_mask
        numeric = torch.cat(
            (
                values,
                batch.entity_scalar_mask.to(values.dtype),
                batch.entity_powers,
                batch.entity_power_mask.to(values.dtype),
                batch.entity_power_counts.to(values.dtype),
                batch.entity_counters,
                batch.entity_counter_mask.to(values.dtype),
                batch.entity_counter_counts.to(values.dtype),
            ),
            dim=-1,
        )
        result = (
            self.kind_embedding(batch.entity_kind)
            + self._content_inputs(batch)
            + self.zone_embedding(batch.entity_zone)
            + self.numeric_projection(numeric)
        )
        for column, embedding in enumerate(self.category_embeddings):
            result = result + embedding(batch.entity_categories[..., column])
        parent_present = batch.entity_parent >= 0
        parent_index = (
            batch.entity_parent.clamp_min(0).unsqueeze(-1).expand(-1, -1, result.shape[-1])
        )
        parent_state = result.gather(1, parent_index)
        return result + self.parent_projection(parent_state) * parent_present.unsqueeze(-1)

    def forward(self, batch: BatchedCombatDecision) -> PolicyValueOutput:
        if batch.vocabulary_fingerprint != self.vocabularies.fingerprint:
            raise ValueError("batch vocabulary does not match model vocabulary")
        global_kind = self.entity_kind_codes["global"]
        global_slots = batch.entity_kind == global_kind
        valid_global_slots = global_slots & batch.entity_mask
        if (
            batch.entity_kind.ndim != 2
            or batch.entity_kind.shape[1] == 0
            or not torch.all(batch.entity_mask[:, 0])
            or not torch.all(global_slots[:, 0])
            or not torch.all(batch.entity_parent[:, 0] == -1)
            or not torch.all(global_slots.sum(dim=1) == 1)
            or not torch.all(valid_global_slots.sum(dim=1) == 1)
        ):
            raise ValueError(
                "each batch row must contain exactly one unpadded global entity at row 0 "
                "with parent -1"
            )
        entities = self._entity_inputs(batch)
        entities = self.encoder(entities, src_key_padding_mask=~batch.entity_mask)
        entities = self.entity_norm(entities)
        state = entities[:, 0]
        width = entities.shape[-1]
        source_index = batch.action_source.clamp_min(0).unsqueeze(-1).expand(-1, -1, width)
        target_index = batch.action_target.clamp_min(0).unsqueeze(-1).expand(-1, -1, width)
        source = entities.gather(1, source_index)
        target = entities.gather(1, target_index)
        source = torch.where(batch.action_source_mask.unsqueeze(-1), source, self.no_source)
        target = torch.where(batch.action_target_mask.unsqueeze(-1), target, self.no_target)
        state_per_action = state.unsqueeze(1).expand(-1, batch.action_kind.shape[1], -1)
        action = torch.cat(
            (
                state_per_action,
                self.action_family_embedding(batch.action_family),
                self.action_kind_embedding(batch.action_kind),
                source,
                target,
            ),
            dim=-1,
        )
        logits = self.action_scorer(action).squeeze(-1)
        logits = logits.masked_fill(~batch.action_mask, float("-inf"))
        return PolicyValueOutput(logits, self.value_head(state).squeeze(-1), entities)


def policy_value_loss(
    output: PolicyValueOutput,
    policy_target: torch.Tensor,
    value_target: torch.Tensor,
    action_mask: torch.Tensor,
) -> torch.Tensor:
    if output.logits.shape != policy_target.shape or output.logits.shape != action_mask.shape:
        raise ValueError("policy shapes do not match")
    if output.value.shape != value_target.shape:
        raise ValueError("value shapes do not match")
    if not torch.isfinite(policy_target).all() or torch.any(policy_target < 0):
        raise ValueError("policy targets must be finite and nonnegative")
    if torch.any(policy_target.masked_select(~action_mask) != 0):
        raise ValueError("policy targets must not place mass on padded actions")
    if not torch.isfinite(value_target).all() or torch.any(torch.abs(value_target) > 1):
        raise ValueError("value targets must be finite and in [-1, 1]")
    legal_logits = output.logits.masked_select(action_mask)
    if not torch.isfinite(legal_logits).all():
        raise ValueError("legal policy logits must be finite")
    if not torch.isfinite(output.value).all() or torch.any(torch.abs(output.value) > 1):
        raise ValueError("model values must be finite and in [-1, 1]")
    sums = policy_target.sum(dim=-1)
    if torch.any(sums <= 0):
        raise ValueError("each policy target must have positive legal mass")
    target = policy_target / sums.unsqueeze(-1)
    log_policy = torch.log_softmax(output.logits, dim=-1)
    policy_terms = torch.where(action_mask, target * log_policy, 0.0)
    policy = -policy_terms.sum(dim=-1).mean()
    value = torch.mean((output.value - value_target) ** 2)
    loss = policy + value
    if not torch.isfinite(loss):
        raise ValueError("policy/value loss is nonfinite")
    return loss


@dataclass(frozen=True, slots=True)
class LoadedCheckpoint:
    model: FairCombatPolicyValueNet
    vocabularies: Vocabularies
    repository: RepositoryVersion
    experiment_config: Mapping[str, object]
    value_target_name: str
    tensorizer_source_digest: str
    model_source_digest: str


class CheckpointSourceMismatchWarning(UserWarning):
    """A checkpoint was loaded under source bytes other than those recorded."""


def _json_copy(payload: dict[str, object]) -> dict[str, object]:
    return cast(dict[str, object], json.loads(json.dumps(payload, sort_keys=True)))


def _freeze_config(value: object) -> object:
    if isinstance(value, dict):
        return MappingProxyType({key: _freeze_config(item) for key, item in value.items()})
    if isinstance(value, list):
        return tuple(_freeze_config(item) for item in value)
    return value


def save_checkpoint(
    path: Path,
    model: FairCombatPolicyValueNet,
    repository: RepositoryVersion,
    experiment_config: dict[str, object],
    *,
    value_target_name: str,
) -> None:
    if type(value_target_name) is not str or not value_target_name:
        raise TypeError("value target name must be a nonempty string")
    experiment = _json_copy(experiment_config)
    payload: dict[str, object] = {
        "checkpoint_format": CHECKPOINT_FORMAT,
        "model_config": asdict(model.config),
        "model_state": model.state_dict(),
        "model_training": model.training,
        "vocabularies": model.vocabularies.to_dict(),
        "vocabulary_fingerprint": model.vocabularies.fingerprint,
        "repository": repository.to_dict(),
        "tensorizer_source_digest": file_digest(_TENSORIZER_SOURCE_PATH),
        "model_source_digest": file_digest(Path(__file__)),
        "experiment_config": experiment,
        "value_target_name": value_target_name,
    }
    torch.save(payload, path)


def load_checkpoint(
    path: Path,
    *,
    expected_vocabularies: Vocabularies | None = None,
    expected_config: CombatModelConfig | None = None,
    expected_value_target_name: str | None = None,
    strict_source: bool = False,
) -> LoadedCheckpoint:
    payload = cast(dict[str, Any], torch.load(path, map_location="cpu", weights_only=True))
    expected_keys = {
        "checkpoint_format",
        "model_config",
        "model_state",
        "model_training",
        "vocabularies",
        "vocabulary_fingerprint",
        "repository",
        "tensorizer_source_digest",
        "model_source_digest",
        "experiment_config",
        "value_target_name",
    }
    if set(payload) != expected_keys or payload["checkpoint_format"] != CHECKPOINT_FORMAT:
        raise ValueError("unsupported or malformed checkpoint envelope")
    if type(strict_source) is not bool:
        raise TypeError("strict_source must be boolean")
    source_digests: dict[str, str] = {}
    for field in ("tensorizer_source_digest", "model_source_digest"):
        digest = payload[field]
        if (
            type(digest) is not str
            or len(digest) != 64
            or any(character not in "0123456789abcdef" for character in digest)
        ):
            raise ValueError(f"checkpoint {field} is malformed")
        source_digests[field] = digest
    value_target_name = payload["value_target_name"]
    if type(value_target_name) is not str or not value_target_name:
        raise ValueError("checkpoint value target name is invalid")
    if expected_value_target_name is not None and value_target_name != expected_value_target_name:
        raise ValueError("checkpoint value target name does not match expected target")
    vocabularies = Vocabularies.from_dict(cast(dict[str, list[str]], payload["vocabularies"]))
    if payload["vocabulary_fingerprint"] != vocabularies.fingerprint:
        raise ValueError("checkpoint vocabulary fingerprint is invalid")
    if (
        expected_vocabularies is not None
        and vocabularies.fingerprint != expected_vocabularies.fingerprint
    ):
        raise ValueError("checkpoint vocabulary does not match expected vocabulary")
    current_source_digests = {
        "tensorizer_source_digest": file_digest(_TENSORIZER_SOURCE_PATH),
        "model_source_digest": file_digest(Path(__file__)),
    }
    mismatches = tuple(
        field
        for field, digest in source_digests.items()
        if digest != current_source_digests[field]
    )
    if mismatches:
        message = "checkpoint source bytes differ for: " + ", ".join(mismatches)
        if strict_source:
            raise ValueError(message)
        warnings.warn(message, CheckpointSourceMismatchWarning, stacklevel=2)
    raw_config = cast(dict[str, Any], payload["model_config"])
    if set(raw_config) != set(asdict(CombatModelConfig())):
        raise ValueError("checkpoint model config is malformed")
    config = CombatModelConfig(**raw_config)
    if expected_config is not None and config != expected_config:
        raise ValueError("checkpoint model config does not match expected config")
    training = payload["model_training"]
    if type(training) is not bool:
        raise TypeError("checkpoint model mode is malformed")
    model = FairCombatPolicyValueNet(vocabularies, config)
    model.load_state_dict(cast(dict[str, torch.Tensor], payload["model_state"]), strict=True)
    model.train(training)
    repository = RepositoryVersion.from_dict(cast(dict[str, object], payload["repository"]))
    raw_experiment = payload["experiment_config"]
    if not isinstance(raw_experiment, dict):
        raise TypeError("checkpoint experiment config is malformed")
    experiment = cast(
        Mapping[str, object],
        _freeze_config(_json_copy(cast(dict[str, object], raw_experiment))),
    )
    return LoadedCheckpoint(
        model,
        vocabularies,
        repository,
        experiment,
        value_target_name,
        source_digests["tensorizer_source_digest"],
        source_digests["model_source_digest"],
    )
