"""Bounded A0 runs: macro policy outside combat, frozen sampled combat inside.

Errors and collection limits never become death targets. Journals contain debug
seed/revision metadata, but only MacroInput or public combat tables reach models.
"""

import gzip
import json
from collections.abc import Callable, Generator, Iterable
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import NotRequired, TypedDict, Unpack

import numpy as np
import torch
from encoders.numeric import (
    ACTION_KIND,
    ACTION_LEGAL_INDEX,
    ACTION_OWNER,
    ACTION_REVISION,
    ACTION_TARGET,
    NumericBatch,
)
from model import CombatValueModel
from sts_sim import Decision, State

from run_training.contracts import PolicyAction, controller, outcome
from run_training.diagnostics import decision_diagnostic
from run_training.environment import journal_environment_seed
from run_training.keys import key_decision
from run_training.metrics import BehaviorStats
from run_training.model import MacroInput, MacroModel, encode
from run_training.progress import RunProgress
from run_training.restrictions import (
    AVOID_PRISMATIC_PROTOCOL,
    NAVIGATION_PROTOCOL,
    checked_eligibility,
    restrict_batch_logits,
    restrict_logits,
)
from run_training.rewards import Objective, RewardParts, reward_protocol, terminal_parts


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
    progress: RunProgress | None = None
    termination_reason: str | None = None
    game_outcome: str | None = None
    action_limit: int | None = None

    def reward_parts(self) -> RewardParts:
        if self.status == "action_limit" and (
            self.objective != "heart_keys_budget"
            or self.termination_reason != "action_limit"
            or self.game_outcome != "ongoing"
            or self.action_limit is None
            or self.action_limit < 1
            or self.accepted != self.action_limit
        ):
            raise ValueError(
                "Action-limit target lacks a genuine exhausted-budget boundary"
            )
        return terminal_parts(
            self.objective,
            self.status,
            self.furthest_act1_floor,
            progress=self.progress,
        )


class CollectionFailure(RuntimeError):
    """Simulator error, invalid boundary or budget limit: no learning target."""

    accepted: int = 0
    floor: int = 0
    furthest_act1_floor: int = 0
    behavior: BehaviorStats = BehaviorStats()
    progress: RunProgress | None = None


class SimulatorFailure(CollectionFailure):
    """A native simulator call failed, distinct from model/infrastructure errors."""


def simulator_call[T](operation: Callable[[], T]) -> T:
    try:
        return operation()
    except (ValueError, RuntimeError) as error:
        raise SimulatorFailure(f"{type(error).__name__}: {error}") from error


def sample(
    logits: torch.Tensor,
    generator: torch.Generator,
    *,
    eligibility: tuple[bool, ...] = (),
) -> int:
    if logits.ndim != 1 or not len(logits) or not torch.isfinite(logits).all():
        raise ValueError("Expected finite logits for every complete legal candidate")
    effective = restrict_logits(logits, eligibility)
    return int(torch.multinomial(effective.softmax(0), 1, generator=generator).item())


def sample_many(
    logits: list[torch.Tensor],
    generators: list[torch.Generator],
    *,
    eligibility: list[tuple[bool, ...]] | None = None,
) -> list[int]:
    """Keep each run's RNG and true candidate length; synchronize choices once."""
    if not logits or any(row.ndim != 1 or not len(row) for row in logits):
        raise ValueError("Expected nonempty complete candidate rows")
    if not torch.isfinite(torch.cat(logits)).all():
        raise ValueError("Nonfinite policy logits")
    eligibility = eligibility if eligibility is not None else [()] * len(logits)
    choices = [
        torch.multinomial(restrict_logits(row, allowed).softmax(0), 1, generator=rng)
        for row, rng, allowed in zip(logits, generators, eligibility, strict=True)
    ]
    return torch.cat(choices).cpu().tolist()


@dataclass(frozen=True)
class MacroRequest:
    inputs: MacroInput
    generator: torch.Generator
    heal_indices: tuple[int, ...]


@dataclass(frozen=True)
class CombatRequest:
    state: State
    decision: Decision
    generator: torch.Generator


@dataclass(frozen=True)
class MacroPrediction:
    index: int
    logits: tuple[float, ...]
    value: float
    heal_probability: float | None


class FrozenCombat:
    def __init__(
        self, checkpoint: Path, device: str, *, adapt_legacy: bool = False
    ) -> None:
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
        from run_training.combat_compat import frozen_weights

        self.model.load_state_dict(
            frozen_weights(self.model, source["model"], adapt_legacy=adapt_legacy),
            strict=True,
        )
        if not all(torch.isfinite(p).all() for p in self.model.parameters()):
            raise ValueError("Nonfinite combat checkpoint")
        self.model.requires_grad_(False)
        self.model.eval()

    @torch.inference_mode()
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

    @torch.inference_mode()
    def choose_many(self, requests: list[CombatRequest]) -> list[int]:
        try:
            batch = NumericBatch(
                State.numeric_decisions([row.state for row in requests])
            )
        except (ValueError, RuntimeError) as error:
            # A collective export failure has no trustworthy per-case attribution.
            # Do not retry exports/actions individually or silently drop a state.
            raise RuntimeError(
                "Batched numeric export failed; aborting unlocalized cohort"
            ) from error
        if batch.model_rows != list(range(len(requests))):
            raise ValueError(
                "Combat batch must represent every requested state exactly once"
            )
        rows = batch.action_rows
        if len(rows) == 0 or np.any(np.diff(rows[:, ACTION_OWNER]) < 0):
            raise ValueError("Combat rows must be nonempty and grouped by owner")
        expected_owners = np.repeat(
            np.arange(len(requests)), [len(r.decision.actions) for r in requests]
        )
        if not np.array_equal(rows[:, ACTION_OWNER], expected_owners):
            raise ValueError("Combat numeric/typed batch owner/count mismatch")
        groups = []
        start = 0
        for request in requests:
            count = len(request.decision.actions)
            group = rows[start : start + count]
            if np.any(group[:, ACTION_REVISION] != request.decision.revision) or sorted(
                group[:, ACTION_LEGAL_INDEX].tolist()
            ) != list(range(count)):
                raise ValueError(
                    "Combat numeric rows must preserve revisions and every legal candidate"
                )
            groups.append(group)
            start += count
        candidates = np.empty((len(rows), 6), dtype=np.int64)
        candidates[:, 0] = rows[:, ACTION_OWNER]
        candidates[:, 1:] = rows[:, ACTION_KIND : ACTION_TARGET + 1]
        logits, _, _ = self.model(batch, candidates)
        choices = sample_many(
            [logits[i, : len(group)] for i, group in enumerate(groups)],
            [request.generator for request in requests],
        )
        return [
            int(group[choice, ACTION_LEGAL_INDEX])
            for group, choice in zip(groups, choices, strict=True)
        ]


def task_result(
    decision: Decision,
    objective: Objective,
    furthest_act1_floor: int | None = None,
    *,
    progress: RunProgress | None = None,
) -> tuple[str, float | None]:
    if furthest_act1_floor is None:
        furthest_act1_floor = decision.observation.context.floor
    status = outcome(decision)
    if status == "invalid":
        raise CollectionFailure("Invalid/unknown run boundary, not a loss")
    if status == "death":
        return status, terminal_parts(
            objective, status, furthest_act1_floor, progress=progress
        ).total
    # Act-1 curriculum terminates at the observable transition into Act 2.
    if objective in ("act1", "act1_binary") and decision.observation.context.act >= 2:
        return "act1_clear", terminal_parts(
            objective, "act1_clear", furthest_act1_floor
        ).total
    if status != "ongoing":
        if objective in ("act1", "act1_binary"):
            raise CollectionFailure("Unexpected Act-1 terminal boundary")
        return status, terminal_parts(
            objective, status, furthest_act1_floor, progress=progress
        ).total
    return status, None


def _trajectory(
    seed: str,
    *,
    objective: Objective,
    final_act: bool,
    max_actions: int,
    macro_rng: torch.Generator,
    combat_rng: torch.Generator,
    journal: Path,
    state_factory: Callable[..., State] = State.new,
    training_rng_seed: int | None = None,
    avoid_prismatic: bool = False,
    commit_navigation: bool = True,
    decision_diagnostics: bool = False,
    stop_requested: Callable[[], bool] | None = None,
    initial_previous: PolicyAction | None = None,
    initial_metadata: dict | None = None,
) -> Generator[MacroRequest | CombatRequest, MacroPrediction | int, RunEpisode]:
    """MC collector with explicit task boundaries, never game-state repair.

    Only heart_keys_budget gives genuine action-budget exhaustion a declared
    target. Interruptions/errors stay unknown and quarantine the whole batch.
    Legacy objectives retain reward=None at action limits.
    """
    if (
        objective
        not in (
            "act1",
            "act1_binary",
            "act3",
            "heart",
            "heart_progress",
            "heart_keys_budget",
        )
        or max_actions < 1
    ):
        raise ValueError("Invalid run collection configuration")
    if objective in ("heart_progress", "heart_keys_budget") and not final_act:
        raise ValueError("Shaped Heart objective requires final_act")
    journal_environment_seed({"training_rng_seed": training_rng_seed})
    steps: list[MacroStep] = []
    accepted = 0
    previous = initial_previous
    finished_shop: tuple[int, int] | None = None
    furthest_act1_floor = 0
    behavior = BehaviorStats()
    progress = RunProgress()
    journal.parent.mkdir(parents=True, exist_ok=True)
    with (
        gzip.open(journal, "xt", compresslevel=1)
        if journal.suffix == ".gz"
        else journal.open("x") as log
    ):

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
                "map_input_protocol": "context_current_act_map_v1",
                "reward_protocol": reward_protocol(objective),
                "decision_diagnostics": decision_diagnostics,
                "max_actions": max_actions,
                "macro_policy_seed": macro_rng.initial_seed(),
                "combat_policy_seed": combat_rng.initial_seed(),
                "training_rng_seed": training_rng_seed,
                "avoid_prismatic": avoid_prismatic,
                "policy_restriction_protocol": AVOID_PRISMATIC_PROTOCOL
                if avoid_prismatic
                else "all_legal_actions",
                "navigation_protocol": NAVIGATION_PROTOCOL
                if commit_navigation
                else "raw_navigation",
                "initial_state": initial_metadata or {"protocol": "natural_start"},
            }
        )
        try:
            profile = (
                {"training_rng_seed": training_rng_seed}
                if training_rng_seed is not None
                else {}
            )
            state = simulator_call(
                lambda: state_factory(seed, ascension=0, final_act=final_act, **profile)
            )
            decision = simulator_call(state.decision)
            while True:
                progress = progress.observed(decision.observation)
                if decision.observation.context.act == 1:
                    furthest_act1_floor = max(
                        furthest_act1_floor, decision.observation.context.floor
                    )
                status, reward = task_result(
                    decision, objective, furthest_act1_floor, progress=progress
                )
                interrupted = reward is None and bool(
                    stop_requested and stop_requested()
                )
                if reward is not None or accepted >= max_actions or interrupted:
                    game_status = outcome(decision)
                    termination_reason = (
                        "game_terminal"
                        if game_status != "ongoing"
                        else "objective_complete"
                    )
                    if reward is None:
                        termination_reason = (
                            "interrupted" if interrupted else "action_limit"
                        )
                        status = "cutoff"
                        if (
                            not interrupted
                            and accepted >= max_actions
                            and objective == "heart_keys_budget"
                        ):
                            status = "action_limit"
                            reward = terminal_parts(
                                objective,
                                status,
                                furthest_act1_floor,
                                progress=progress,
                            ).total
                    emit(
                        {
                            "type": "result",
                            "status": status,
                            "termination_reason": termination_reason,
                            "game_outcome": game_status,
                            "action_limit": max_actions,
                            "reward": reward,
                            "accepted": accepted,
                            "floor": decision.observation.context.floor,
                            "hp": decision.observation.context.player_hp,
                            "macro_decisions": len(steps),
                            "furthest_act1_floor": furthest_act1_floor,
                            "reward_parts": asdict(
                                terminal_parts(
                                    objective,
                                    status,
                                    furthest_act1_floor,
                                    progress=progress,
                                )
                            )
                            if reward is not None
                            else None,
                            "behavior": asdict(behavior),
                            "progress": asdict(progress),
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
                        progress=progress,
                        termination_reason=termination_reason,
                        game_outcome=game_status,
                        action_limit=max_actions,
                    )
                observation = decision.observation
                owner = controller(decision)
                step = None
                heal_probability = None
                heal_indices = (
                    [i for i, a in enumerate(decision.actions) if a.kind == "rest_heal"]
                    if observation.kind == "rest"
                    else []
                )
                if owner == "combat":
                    prediction = yield CombatRequest(state, decision, combat_rng)
                    if not isinstance(prediction, int):
                        raise TypeError("Expected a combat choice")
                    index = prediction
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
                        previous=previous,
                        avoid_prismatic=avoid_prismatic,
                        commit_navigation=commit_navigation,
                        shop_finished=finished_shop
                        == (observation.context.act, observation.context.floor),
                    )
                    prediction = yield MacroRequest(
                        inputs, macro_rng, tuple(heal_indices)
                    )
                    if not isinstance(prediction, MacroPrediction):
                        raise TypeError("Expected a macro prediction")
                    index = prediction.index
                    checked_eligibility(
                        len(decision.actions), inputs.eligibility, choice=index
                    )
                    heal_probability = prediction.heal_probability
                    step = MacroStep(inputs, index, prediction.logits, prediction.value)
                action = decision.actions[index]
                descriptor = PolicyAction.from_action(action)
                key_proposal = (
                    key_decision(
                        observation,
                        descriptors if step is not None else (descriptor,),
                        index if step is not None else 0,
                        eligibility=step.inputs.eligibility if step is not None else (),
                        logits=step.logits if step is not None else None,
                    )
                    if owner != "combat"
                    else None
                )
                emit(
                    {
                        "type": "attempt",
                        "step": accepted,
                        "revision": decision.revision,
                        "owner": owner,
                        "index": index,
                        "action": asdict(descriptor),
                        "policy_eligibility": list(inputs.eligibility)
                        if owner == "macro"
                        else None,
                        "rest_pre_hp": observation.context.player_hp
                        if heal_indices
                        else None,
                        "rest_pre_max_hp": observation.context.player_max_hp
                        if heal_indices
                        else None,
                        "heal_probability": heal_probability,
                        "key_choice": asdict(key_proposal)
                        if key_proposal is not None
                        else None,
                        "policy_diagnostic": decision_diagnostic(
                            observation, descriptors, step.logits, index, step.value
                        )
                        if decision_diagnostics and step is not None
                        else None,
                    }
                )
                decision = simulator_call(lambda action=action: state.step(action))
                if descriptor.kind == "leave_shop":
                    finished_shop = (observation.context.act, observation.context.floor)
                elif descriptor.kind == "choose_map_node":
                    finished_shop = None
                behavior = behavior.accepted(
                    observation, descriptor, heal_probability, key_proposal=key_proposal
                )
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
                error.progress = progress if "decision" in locals() else None
                error.furthest_act1_floor = furthest_act1_floor
                error.floor = (
                    decision.observation.context.floor if "decision" in locals() else 0
                )
            emit(
                {
                    "type": "error",
                    "termination_reason": "simulator_error"
                    if isinstance(error, CollectionFailure)
                    else "inference_or_infrastructure_error",
                    "accepted": accepted,
                    "error": f"{type(error).__name__}: {error}",
                    "furthest_act1_floor": furthest_act1_floor,
                    "behavior": asdict(behavior),
                    "progress": asdict(progress) if "decision" in locals() else None,
                }
            )
            raise


class CollectionOptions(TypedDict):
    objective: Objective
    final_act: bool
    max_actions: int
    macro_rng: torch.Generator
    combat_rng: torch.Generator
    journal: Path
    state_factory: NotRequired[Callable[..., State]]
    training_rng_seed: NotRequired[int | None]
    avoid_prismatic: NotRequired[bool]
    commit_navigation: NotRequired[bool]
    decision_diagnostics: NotRequired[bool]
    stop_requested: NotRequired[Callable[[], bool] | None]
    initial_previous: NotRequired[PolicyAction | None]
    initial_metadata: NotRequired[dict | None]


@dataclass(frozen=True)
class CollectionJob:
    seed: str
    options: CollectionOptions


@torch.inference_mode()
def _predict_one(model: MacroModel, request: MacroRequest) -> MacroPrediction:
    logits, value = model(request.inputs)
    if not torch.isfinite(value):
        raise ValueError("Nonfinite macro value")
    index = sample(logits, request.generator, eligibility=request.inputs.eligibility)
    effective = restrict_logits(logits, request.inputs.eligibility)
    probability = (
        float(effective.softmax(0)[list(request.heal_indices)].sum())
        if request.heal_indices
        else None
    )
    return MacroPrediction(
        index, tuple(effective.cpu().tolist()), float(value), probability
    )


@torch.inference_mode()
def _predict_many(
    model: MacroModel, requests: list[MacroRequest]
) -> list[MacroPrediction]:
    batch = model.pack([r.inputs for r in requests])
    logits, values = model.forward_batch(batch)
    if not torch.isfinite(values).all():
        raise ValueError("Nonfinite macro value")
    choices = sample_many(
        [logits[i, :n] for i, n in enumerate(batch.lengths)],
        [r.generator for r in requests],
        eligibility=[r.inputs.eligibility for r in requests],
    )
    logits = restrict_batch_logits(
        logits, batch.lengths, [r.inputs.eligibility for r in requests]
    )
    probabilities = (
        logits.softmax(1).cpu().tolist()
        if any(request.heal_indices for request in requests)
        else []
    )
    cpu_logits, cpu_values = logits.cpu().tolist(), values.cpu().tolist()
    return [
        MacroPrediction(
            choice,
            tuple(row[:n]),
            value,
            sum(probabilities[i][j] for j in request.heal_indices)
            if request.heal_indices
            else None,
        )
        for i, (request, choice, row, value, n) in enumerate(
            zip(requests, choices, cpu_logits, cpu_values, batch.lengths, strict=True)
        )
    ]


def collect(
    seed: str,
    macro: MacroModel,
    combat: FrozenCombat,
    **options: Unpack[CollectionOptions],
) -> RunEpisode:
    """Serial reference driver of the same journaled trajectory state machine."""
    trajectory = _trajectory(seed, **options)
    try:
        request = next(trajectory)
        while True:
            try:
                if isinstance(request, MacroRequest):
                    response = _predict_one(macro, request)
                else:
                    response = combat.choose(
                        request.state, request.decision, request.generator
                    )
            except Exception as error:
                trajectory.throw(
                    error
                )  # Attribute/log at the suspended decision; no retry.
                raise AssertionError(
                    "Trajectory swallowed an inference failure"
                ) from error
            request = trajectory.send(response)
    except StopIteration as done:
        return done.value
    finally:
        trajectory.close()


def failed_episode(error: CollectionFailure, objective: Objective) -> RunEpisode:
    return RunEpisode(
        "error",
        None,
        (),
        error.accepted,
        error.floor,
        str(error),
        objective=objective,
        furthest_act1_floor=error.furthest_act1_floor,
        behavior=error.behavior,
        progress=error.progress,
        termination_reason="simulator_error",
    )


def collect_many(
    jobs: Iterable[CollectionJob],
    macro: MacroModel,
    combat: FrozenCombat,
    *,
    width: int,
    continue_on_failure: bool = False,
    stop_requested: Callable[[], bool] | None = None,
    on_failure: Callable[[CollectionJob, CollectionFailure], None] | None = None,
) -> list[RunEpisode]:
    """Cooperatively batch inference, never game transitions or RNG streams.

    Each generator owns one independent state, action budget, history and journal.
    No optimizer updates occur here. A caller must quarantine the entire learner
    batch if any episode lacks a declared task target. Results retain input order, not completion
    order. On a fatal error all open journals are closed with cancellation records.
    """
    if width < 1:
        raise ValueError("Collection width must be positive")
    source = iter(jobs)
    active = {}
    pending = {}
    results = {}
    scheduled = 0
    exhausted = False

    def advance(index, response=None, *, first=False):
        job, trajectory = active[index]
        pending.pop(index, None)
        try:
            if first:
                pending[index] = next(trajectory)
            else:
                assert response is not None
                pending[index] = trajectory.send(response)
        except StopIteration as done:
            results[index] = done.value
            del active[index]
        except CollectionFailure as error:
            del active[index]
            if on_failure:
                on_failure(job, error)
            if not continue_on_failure:
                raise
            results[index] = failed_episode(error, job.options["objective"])

    try:
        while True:
            while (
                not exhausted
                and len(active) < width
                and not (stop_requested and stop_requested())
            ):
                try:
                    job = next(source)
                except StopIteration:
                    exhausted = True
                    break
                options = job.options.copy()
                if stop_requested is not None:
                    options["stop_requested"] = stop_requested
                active[scheduled] = (job, _trajectory(job.seed, **options))
                scheduled += 1
                advance(scheduled - 1, first=True)
            if not active:
                break
            wave = list(pending.items())
            macros = [(i, r) for i, r in wave if isinstance(r, MacroRequest)]
            fights = [(i, r) for i, r in wave if isinstance(r, CombatRequest)]
            responses = {}
            if macros:
                responses.update(
                    zip(
                        [i for i, _ in macros],
                        _predict_many(macro, [r for _, r in macros]),
                        strict=True,
                    )
                )
            if fights:
                responses.update(
                    zip(
                        [i for i, _ in fights],
                        combat.choose_many([r for _, r in fights]),
                        strict=True,
                    )
                )
            for index, _ in wave:
                advance(index, responses[index])
    except BaseException as fatal:
        for _, trajectory in active.values():
            cancelled = RuntimeError(
                "Collection cancelled after a peer/inference failure"
            )
            try:
                trajectory.throw(cancelled)
            except Exception as cleanup_error:  # noqa: BLE001 — preserve the original fatal exception
                if cleanup_error is not cancelled:
                    fatal.add_note(f"Journal cleanup failure: {cleanup_error!r}")
            finally:
                trajectory.close()
        raise
    return [results[i] for i in range(scheduled)]
