"""Bounded A0 runs: macro policy outside combat, frozen sampled combat inside.

Errors and collection limits never become death targets. Journals contain debug
seed/revision metadata, but only MacroInput or public combat tables reach models.
"""

import gzip
import json
from collections.abc import Callable
from dataclasses import asdict, dataclass, field
from pathlib import Path

import numpy as np
import torch
from encoders.numeric import (
    ACTION_KIND,
    ACTION_LEGAL_INDEX,
    ACTION_REVISION,
    ACTION_TARGET,
    NumericBatch,
)
from model import CombatValueModel
from sts_sim import Decision, State

from run_training.contracts import PolicyAction, controller, outcome
from run_training.metrics import BehaviorStats
from run_training.model import MacroInput, MacroModel, encode
from run_training.rewards import Objective, terminal_parts


@dataclass(frozen=True)
class MacroStep:
    inputs: MacroInput
    choice: int
    logits: tuple[float, ...]
    value: float


@dataclass(frozen=True)
class RunEpisode:
    status: str
    reward: float | None
    steps: tuple[MacroStep, ...]
    accepted: int
    floor: int
    error: str | None = None
    objective: Objective = "act1"
    furthest_act1_floor: int = 0
    behavior: BehaviorStats = field(default_factory=BehaviorStats)


class CollectionFailure(RuntimeError):
    """Simulator error, invalid boundary or budget limit: no learning target."""

    accepted: int = 0
    floor: int = 0
    furthest_act1_floor: int = 0
    behavior: BehaviorStats = BehaviorStats()


class SimulatorFailure(CollectionFailure):
    """A native simulator call failed, distinct from model/infrastructure errors."""


def simulator_call[T](operation: Callable[[], T]) -> T:
    try:
        return operation()
    except (ValueError, RuntimeError) as error:
        raise SimulatorFailure(f"{type(error).__name__}: {error}") from error


def sample(logits: torch.Tensor, generator: torch.Generator) -> int:
    if logits.ndim != 1 or not len(logits) or not torch.isfinite(logits).all():
        raise ValueError("Expected finite logits for every complete legal candidate")
    return int(torch.multinomial(logits.softmax(0), 1, generator=generator).item())


class FrozenCombat:
    def __init__(self, checkpoint: Path, device: str) -> None:
        # Checkpoints must be trusted. This is weight-only integration, not resume
        # of the original combat optimizer, precision protocol, or simulator.
        source = torch.load(checkpoint, map_location="cpu", weights_only=True)
        config = source["config"]
        width = config.get("model_width", 64)
        self.model = CombatValueModel(
            d_model=width,
            action_dim=width,
            n_layers=config.get("model_layers", 2),
            precision="fp32",
        ).to(device)
        self.model.load_state_dict(source["model"], strict=True)
        if not all(torch.isfinite(p).all() for p in self.model.parameters()):
            raise ValueError("Nonfinite combat checkpoint")
        self.model.requires_grad_(False)
        self.model.eval()

    @torch.no_grad()
    def choose(
        self, state: State, decision: Decision, generator: torch.Generator
    ) -> int:
        batch = NumericBatch(simulator_call(lambda: State.numeric_decisions([state])))
        rows = batch.action_rows
        if len(rows) != len(decision.actions) or np.any(
            rows[:, ACTION_REVISION] != decision.revision
        ):
            raise ValueError(
                "Combat numeric/typed candidate count or revision mismatch"
            )
        if sorted(rows[:, ACTION_LEGAL_INDEX].tolist()) != list(
            range(len(decision.actions))
        ):
            raise ValueError(
                "Combat numeric rows must cover every legal candidate exactly once"
            )
        # Unlike synthetic combat training, full-run play keeps escape potions.
        # Every advertised legal candidate is scored; no retry/filter fallback.
        candidates = np.zeros((len(rows), 6), dtype=np.int64)
        candidates[:, 1:] = rows[:, ACTION_KIND : ACTION_TARGET + 1]
        logits, _, _ = self.model(batch, candidates)
        return int(rows[sample(logits[0, : len(rows)], generator), ACTION_LEGAL_INDEX])


def task_result(
    decision: Decision, objective: Objective, furthest_act1_floor: int | None = None
) -> tuple[str, float | None]:
    if furthest_act1_floor is None:
        furthest_act1_floor = decision.observation.context.floor
    status = outcome(decision)
    if status == "invalid":
        raise CollectionFailure("Invalid/unknown run boundary, not a loss")
    if status == "death":
        return status, terminal_parts(objective, status, furthest_act1_floor).total
    # Act-1 curriculum terminates at the observable transition into Act 2.
    if objective == "act1" and decision.observation.context.act >= 2:
        return "act1_clear", terminal_parts(
            objective, "act1_clear", furthest_act1_floor
        ).total
    if status != "ongoing":
        if objective == "act1":
            raise CollectionFailure("Unexpected Act-1 terminal boundary")
        return status, terminal_parts(objective, status, furthest_act1_floor).total
    return status, None


def collect(
    seed: str,
    macro: MacroModel,
    combat: FrozenCombat,
    *,
    objective: Objective,
    final_act: bool,
    max_actions: int,
    macro_rng: torch.Generator,
    combat_rng: torch.Generator,
    journal: Path,
    state_factory: Callable[..., State] = State.new,
    stop_requested: Callable[[], bool] | None = None,
) -> RunEpisode:
    """Completed-episode MC collector. No bootstrap from arbitrary combat cutoffs.

    A cutoff returns reward=None and trainer rejects the entire update. This
    intentionally avoids selectively training only short/completed trajectories.
    """
    if objective not in ("act1", "act3", "heart") or max_actions < 1:
        raise ValueError("Invalid run collection configuration")
    steps: list[MacroStep] = []
    accepted = 0
    visible_map = None
    previous = None
    furthest_act1_floor = 0
    behavior = BehaviorStats()
    journal.parent.mkdir(parents=True, exist_ok=True)
    with (
        gzip.open(journal, "xt", compresslevel=1)
        if journal.suffix == ".gz"
        else journal.open("x")
    ) as log:

        def emit(row: dict) -> None:
            log.write(json.dumps(row) + "\n")
            log.flush()

        emit(
            {
                "type": "setup",
                "seed": seed,
                "ascension": 0,
                "final_act": final_act,
                "objective": objective,
                "max_actions": max_actions,
                "macro_policy_seed": macro_rng.initial_seed(),
                "combat_policy_seed": combat_rng.initial_seed(),
            }
        )
        try:
            state = simulator_call(
                lambda: state_factory(seed, ascension=0, final_act=final_act)
            )
            decision = simulator_call(state.decision)
            while True:
                if decision.observation.context.act == 1:
                    furthest_act1_floor = max(
                        furthest_act1_floor, decision.observation.context.floor
                    )
                status, reward = task_result(decision, objective, furthest_act1_floor)
                if (
                    reward is not None
                    or accepted >= max_actions
                    or (stop_requested and stop_requested())
                ):
                    if reward is None:
                        status = "cutoff"
                    emit(
                        {
                            "type": "result",
                            "status": status,
                            "reward": reward,
                            "accepted": accepted,
                            "floor": decision.observation.context.floor,
                            "hp": decision.observation.context.player_hp,
                            "macro_decisions": len(steps),
                            "furthest_act1_floor": furthest_act1_floor,
                            "reward_parts": asdict(
                                terminal_parts(objective, status, furthest_act1_floor)
                            )
                            if reward is not None
                            else None,
                            "behavior": asdict(behavior),
                        }
                    )
                    return RunEpisode(
                        status,
                        reward,
                        tuple(steps),
                        accepted,
                        decision.observation.context.floor,
                        objective=objective,
                        furthest_act1_floor=furthest_act1_floor,
                        behavior=behavior,
                    )
                observation = decision.observation
                if observation.kind == "map":
                    visible_map = observation
                if (
                    visible_map is not None
                    and visible_map.context.act != observation.context.act
                ):
                    visible_map = None
                owner = controller(decision)
                step = None
                heal_probability = None
                heal_indices = (
                    [i for i, a in enumerate(decision.actions) if a.kind == "rest_heal"]
                    if observation.kind == "rest"
                    else []
                )
                if owner == "combat":
                    index = combat.choose(state, decision, combat_rng)
                elif owner == "forced":
                    index = 0
                    if heal_indices:
                        heal_probability = 1.0
                else:
                    descriptors = tuple(
                        PolicyAction.from_action(a) for a in decision.actions
                    )
                    inputs = encode(
                        observation,
                        descriptors,
                        visible_map=visible_map,
                        previous=previous,
                    )
                    with torch.no_grad():
                        logits, value = macro(inputs)
                        if not torch.isfinite(value):
                            raise ValueError("Nonfinite macro value")
                        index = sample(logits, macro_rng)
                        if heal_indices:
                            heal_probability = float(
                                logits.softmax(0)[heal_indices].sum()
                            )
                    step = MacroStep(
                        inputs, index, tuple(logits.cpu().tolist()), float(value)
                    )
                action = decision.actions[index]
                descriptor = PolicyAction.from_action(action)
                emit(
                    {
                        "type": "attempt",
                        "step": accepted,
                        "revision": decision.revision,
                        "owner": owner,
                        "index": index,
                        "action": asdict(descriptor),
                        "rest_pre_hp": observation.context.player_hp
                        if heal_indices
                        else None,
                        "rest_pre_max_hp": observation.context.player_max_hp
                        if heal_indices
                        else None,
                        "heal_probability": heal_probability,
                    }
                )
                decision = simulator_call(lambda action=action: state.step(action))
                behavior = behavior.accepted(observation, descriptor, heal_probability)
                if step is not None:
                    steps.append(step)
                if observation.kind != "combat":
                    previous = descriptor
                accepted += 1
                emit(
                    {
                        "type": "accepted",
                        "step": accepted - 1,
                        "revision": decision.revision,
                    }
                )
        except Exception as error:
            if isinstance(error, CollectionFailure):
                error.accepted = accepted
                error.behavior = behavior
                error.furthest_act1_floor = furthest_act1_floor
                error.floor = (
                    decision.observation.context.floor if "decision" in locals() else 0
                )
            emit(
                {
                    "type": "error",
                    "accepted": accepted,
                    "error": f"{type(error).__name__}: {error}",
                    "furthest_act1_floor": furthest_act1_floor,
                    "behavior": asdict(behavior),
                }
            )
            raise
