"""Checkpoint loading, fair candidate scoring, and isolated sampling."""

from __future__ import annotations

import io
import math
import random
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

import numpy as np
import torch
from combat_task import action_indices
from encoders.numeric import (
    ACTION_KIND,
    ACTION_LEGAL_INDEX,
    ACTION_OWNER,
    ACTION_REVISION,
    ACTION_TARGET,
    NumericBatch,
)
from model import CombatValueModel
from sts_sim import Action, CombatObservation, Decision, State
from torch import Tensor

from combat_explorer import (
    MAX_TEMPERATURE,
    MIN_TEMPERATURE,
    POLICY_ADAPTER_VERSION,
    SAMPLING_GENERATOR,
    SAMPLING_GENERATOR_VERSION,
)
from combat_explorer.errors import ExplorerError, ModelError
from combat_explorer.jsonutil import finite_floats, parse_seed, sha256_bytes
from combat_explorer.presentation import action_descriptor, action_label

Mode = Literal["greedy", "sample"]


def validate_temperature(temperature: float) -> float:
    value = float(temperature)
    if not math.isfinite(value) or value <= 0:
        raise ExplorerError("Temperature must be finite and positive; use greedy mode instead of zero")
    if value < MIN_TEMPERATURE or value > MAX_TEMPERATURE:
        raise ExplorerError(f"Temperature must be between {MIN_TEMPERATURE} and {MAX_TEMPERATURE}")
    return value


def validate_mode(mode: str) -> Mode:
    if mode not in ("greedy", "sample"):
        raise ExplorerError("Mode must be greedy or sample")
    return mode  # type: ignore[return-value]


def softmax(logits: list[float], temperature: float = 1.0) -> list[float]:
    validate_temperature(temperature)
    scaled = [logit / temperature for logit in logits]
    peak = max(scaled)
    shifted = [math.exp(logit - peak) for logit in scaled]
    total = sum(shifted)
    if total <= 0 or not math.isfinite(total):
        raise ExplorerError("Softmax failed over candidate logits")
    probs = [value / total for value in shifted]
    return finite_floats(probs, "probability")


def greedy_index(logits: list[float]) -> int:
    if not logits:
        raise ExplorerError("No candidates to choose")
    best = max(logits)
    return next(index for index, logit in enumerate(logits) if logit == best)


def sample_index(probabilities: list[float], rng: random.Random) -> int:
    if not probabilities:
        raise ExplorerError("No candidates to sample")
    weights = finite_floats(probabilities, "probability")
    if any(weight < 0 for weight in weights):
        raise ExplorerError("Probabilities must be nonnegative")
    if sum(weights) <= 0:
        raise ExplorerError("Probabilities must sum to a positive value")
    return rng.choices(range(len(weights)), weights=weights, k=1)[0]


@dataclass
class LoadedCheckpoint:
    model: CombatValueModel
    fingerprint: str
    path: str
    architecture: str
    config: dict[str, Any] | None
    device: str
    adapter_version: str = POLICY_ADAPTER_VERSION


@dataclass
class ScoredCandidates:
    native_indices: list[int]
    descriptors: list[dict[str, Any]]
    labels: list[str]
    logits: list[float]
    checkpoint_fingerprint: str
    adapter_version: str
    mapping: str
    value: float | None = None


class PolicyAdapter:
    def __init__(self) -> None:
        self.lock = __import__("threading").Lock()
        self.loaded: LoadedCheckpoint | None = None

    def load(self, path: Path, *, device: str = "cpu") -> LoadedCheckpoint:
        if device == "cuda" and not torch.cuda.is_available():
            raise ExplorerError("CUDA is unavailable")
        if device not in ("cpu", "cuda"):
            raise ExplorerError("Device must be cpu or cuda")
        data = path.read_bytes()
        fingerprint = sha256_bytes(data)
        try:
            payload = torch.load(io.BytesIO(data), map_location=device, weights_only=True)
        except Exception as error:
            raise ModelError(
                "Rejected checkpoint load with weights_only=True. "
                f"Supported format is the current synthetic trainer dict. ({error})"
            ) from error
        if not isinstance(payload, dict) or "model" not in payload:
            raise ModelError("Expected a synthetic trainer checkpoint dict containing a model state")
        state_dict = payload["model"]
        if not isinstance(state_dict, dict):
            raise ModelError("Checkpoint model state is not a dictionary")
        torch.set_num_threads(1)
        model = CombatValueModel()
        try:
            model.load_state_dict(state_dict, strict=True)
        except Exception as error:
            raise ModelError(f"Checkpoint weights are incompatible with CombatValueModel: {error}") from error
        model.to(device)
        model.eval()
        config = payload.get("config")
        if config is not None and not isinstance(config, dict):
            config = None
        loaded = LoadedCheckpoint(
            model=model,
            fingerprint=fingerprint,
            path=str(path),
            architecture="CombatValueModel",
            config=config,
            device=device,
        )
        with self.lock:
            self.loaded = loaded
        return loaded

    def require_loaded(self) -> LoadedCheckpoint:
        if self.loaded is None:
            raise ModelError("No model checkpoint is loaded")
        return self.loaded

    def score(self, state: State, loaded: LoadedCheckpoint | None = None) -> ScoredCandidates:
        loaded = loaded or self.require_loaded()
        decision = state.decision()
        observation = decision.observation
        if observation.kind != "combat":
            raise ModelError("CombatValueModel only scores combat observations")
        if not isinstance(observation, CombatObservation):
            raise ModelError("Policy scoring requires a typed CombatObservation")
        indices = action_indices(decision)
        if not indices:
            raise ModelError("No allowed combat actions after disabling Smoke Bomb use")
        candidates = tuple(decision.actions[index] for index in indices)
        batch = NumericBatch(State.numeric_decisions([state]))
        rows = batch.action_rows
        # Keep the task-filtered public legal-index order, including its mapping
        # back to native actions. Candidate features come only from public rows.
        if len(batch) != 1 or len(rows) != len(decision.actions):
            raise ModelError("Numeric action rows do not match the current decision")
        by_index = {int(row[ACTION_LEGAL_INDEX]): row for row in rows if int(row[ACTION_OWNER]) == 0}
        if len(by_index) != len(rows) or set(by_index) != set(range(len(decision.actions))):
            raise ModelError("Numeric action rows have missing or duplicate legal indices")
        selected = np.stack([by_index[index] for index in indices])
        if np.any(selected[:, ACTION_REVISION] != decision.revision):
            raise ModelError("Numeric action revision does not match the current decision")
        candidate_rows = np.empty((len(indices), 6), dtype=np.int64)
        candidate_rows[:, 0] = 0  # forward-local observation index
        candidate_rows[:, 1:] = selected[:, ACTION_KIND : ACTION_TARGET + 1]
        with self.lock, torch.inference_mode():
            logits_tensor, values, valid = loaded.model(batch, candidate_rows)
            row: Tensor = logits_tensor[0, : len(candidates)]
            mask = valid[0, : len(candidates)]
            logits = [float(value) for value in row.detach().cpu().tolist()]
            valid_flags = [bool(flag) for flag in mask.detach().cpu().tolist()]
            value = float(values.reshape(-1)[0].detach().cpu())
        if not all(valid_flags) or len(logits) != len(candidates):
            raise ModelError("Model valid-action mask does not match the filtered candidate list")
        try:
            logits = finite_floats(logits, "logit")
        except ValueError as error:
            raise ModelError(str(error)) from error
        if not math.isfinite(value):
            raise ModelError("Model value is not finite")
        return ScoredCandidates(
            native_indices=list(indices),
            descriptors=[action_descriptor(action) for action in candidates],
            labels=[action_label(observation, action) for action in candidates],
            logits=logits,
            checkpoint_fingerprint=loaded.fingerprint,
            adapter_version=loaded.adapter_version,
            mapping=TASK_MAPPING,
            value=value,
        )

    def analyze(self, state: State, *, mode: Mode, temperature: float) -> dict[str, Any]:
        scored = self.score(state)
        return analysis_from_scores(scored, mode=mode, temperature=temperature)

    def choose(
        self,
        scored: ScoredCandidates,
        *,
        mode: Mode,
        temperature: float,
        rng: random.Random | None,
    ) -> tuple[int, dict[str, Any]]:
        analysis = analysis_from_scores(scored, mode=mode, temperature=temperature)
        if mode == "greedy":
            choice = greedy_index(scored.logits)
        else:
            if rng is None:
                raise ModelError("Sampling requires a dedicated generator")
            choice = sample_index(analysis["adjusted_probabilities"], rng)
        analysis["chosen_candidate_index"] = choice
        analysis["chosen_native_index"] = scored.native_indices[choice]
        return scored.native_indices[choice], analysis


TASK_MAPPING = "combat_task.action_indices/smoke_bomb_excluded"


def analysis_from_scores(scored: ScoredCandidates, *, mode: Mode, temperature: float) -> dict[str, Any]:
    base = softmax(scored.logits, 1.0)
    adjusted = base if mode == "greedy" or temperature == 1.0 else softmax(scored.logits, temperature)
    if mode == "sample":
        validate_temperature(temperature)
    elif mode == "greedy":
        pass
    else:
        raise ExplorerError("Mode must be greedy or sample")
    return {
        "checkpoint_fingerprint": scored.checkpoint_fingerprint,
        "adapter_version": scored.adapter_version,
        "mapping": scored.mapping,
        "mode": mode,
        "temperature": None if mode == "greedy" else temperature,
        "native_indices": list(scored.native_indices),
        "descriptors": list(scored.descriptors),
        "labels": list(scored.labels),
        "logits": list(scored.logits),
        "value": scored.value,
        "base_probabilities": base,
        "adjusted_probabilities": None if mode == "greedy" else adjusted,
        "note": (
            "Greedy selects argmax over logits with earlier native-candidate-order ties. "
            "Displayed base probabilities are the model distribution, not 100% on the chosen action."
            if mode == "greedy"
            else "Probabilities are model preferences, not calibrated confidence or causal explanations."
        ),
    }


def new_sampling_rng(seed: str) -> random.Random:
    return random.Random(parse_seed(seed, field="sampling_seed"))


def sampling_metadata(seed: str) -> dict[str, str]:
    return {
        "sampling_seed": seed,
        "generator": SAMPLING_GENERATOR,
        "generator_version": SAMPLING_GENERATOR_VERSION,
    }


def allowed_actions(decision: Decision) -> list[Action]:
    return [decision.actions[index] for index in action_indices(decision)]
