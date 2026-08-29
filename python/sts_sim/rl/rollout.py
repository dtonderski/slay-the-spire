"""Capped untrained-model combat rollout wiring."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass

import torch

from ..fair import FairCombatObservation
from ..run import Decision, RunEnv
from .model import FairCombatPolicyValueNet
from .tensor import Vocabularies, collate_combat_tensors, tensorize_combat


@dataclass(frozen=True, slots=True)
class CombatRolloutResult:
    status: str
    steps: int
    terminal: bool
    truncated: bool
    selected_action_indices: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class RolloutDistribution:
    runs: int
    terminal: int
    truncated: int
    status_counts: dict[str, int]
    step_counts: dict[int, int]


def _terminal_status(decision: Decision) -> str | None:
    if not isinstance(decision.observation, FairCombatObservation):
        return "won"
    if decision.observation.phase in ("won", "lost"):
        return decision.observation.phase
    if not decision.actions:
        return "lost" if decision.observation.player.hp <= 0 else "won"
    return None


def rollout_model_combat(
    env: RunEnv,
    model: FairCombatPolicyValueNet,
    vocabularies: Vocabularies,
    *,
    generator_seed: int,
    max_steps: int,
) -> CombatRolloutResult:
    """Sample legal action rows and apply the matching authoritative sidecar entry."""

    if max_steps <= 0:
        raise ValueError("max_steps must be positive")
    decision = env.decision()
    if not isinstance(decision.observation, FairCombatObservation):
        raise TypeError("combat rollout requires an initially active combat")
    initial_status = _terminal_status(decision)
    if initial_status is not None:
        return CombatRolloutResult(initial_status, 0, True, False, ())

    generator = torch.Generator(device="cpu")
    generator.manual_seed(generator_seed)
    selected: list[int] = []
    was_training = model.training
    model.eval()
    try:
        for _ in range(max_steps):
            observation = decision.observation
            assert isinstance(observation, FairCombatObservation)
            descriptors = tuple(action.descriptor() for action in decision.actions)
            tensors = tensorize_combat(observation, descriptors, vocabularies)
            batch = collate_combat_tensors((tensors,))
            with torch.no_grad():
                output = model(batch)
                probabilities = torch.softmax(output.logits[0, : len(decision.actions)], dim=-1)
                index = int(torch.multinomial(probabilities, 1, generator=generator).item())
            selected.append(index)
            # The model chooses only this row. Rust still owns construction,
            # revision validation, legality, transition execution, and terminal
            # classification.
            step = env.step(decision.actions[index])
            decision = step.decision
            status = step.combat_outcome or _terminal_status(decision)
            if status is not None:
                return CombatRolloutResult(status, len(selected), True, False, tuple(selected))
        return CombatRolloutResult("truncated", max_steps, False, True, tuple(selected))
    finally:
        model.train(was_training)


def summarize_rollouts(results: list[CombatRolloutResult]) -> RolloutDistribution:
    statuses = Counter(result.status for result in results)
    steps = Counter(result.steps for result in results)
    return RolloutDistribution(
        runs=len(results),
        terminal=sum(result.terminal for result in results),
        truncated=sum(result.truncated for result in results),
        status_counts=dict(sorted(statuses.items())),
        step_counts=dict(sorted(steps.items())),
    )
