"""Experimental A0 Monte Carlo actor-critic, invoked only by train.py --task run.

Settled training-task episodes, gamma=1, one on-policy update, fresh run value head.
The opt-in keys/budget objective can label an action limit without claiming a game
loss. No PPO, privileged search, state repair, or updates from interrupted/errors.
"""

import argparse
import hashlib
import json
import math
import os
import random
import shutil
import signal
import time
from contextlib import contextmanager
from itertools import count
from pathlib import Path
from threading import Event

import torch
import wandb
from sts_sim import FAIR_RUN_OBSERVATION_SCHEMA_VERSION, State, _native
from torch.distributions import Categorical

from run_training.collector import (
    CollectionFailure,
    CollectionJob,
    FrozenCombat,
    RunEpisode,
    collect,
    collect_many,
    failed_episode,
)
from run_training.diagnostics import DIAGNOSTIC_PROTOCOL, policy_scores
from run_training.environment import (
    ENVIRONMENT_PROTOCOL,
    episode_environment_seed,
    journal_environment_seed,
)
from run_training.keys import KEY_CHOICE_PROTOCOL
from run_training.metrics import behavior_scores
from run_training.model import (
    FEATURE_VERSION,
    HEALTH_FEATURE_VERSION,
    HealthMacroModel,
    MacroModel,
)
from run_training.progress import PROGRESS_PROTOCOL, progress_scores
from run_training.restrictions import (
    AVOID_PRISMATIC_PROTOCOL,
    NAVIGATION_PROTOCOL,
    checked_eligibility,
    restrict_batch_logits,
    restrict_logits,
)
from run_training.rewards import (
    ACT1_BOSS_FLOOR,
    ACT1_CLEAR_BONUS,
    ACT_CLEAR_BONUS,
    ACTION_LIMIT_PENALTY,
    HEART_CLEAR_BONUS,
    HEART_PROGRESS_FLOOR,
    HEART_PROGRESS_SCALE,
    KEY_BONUS,
    game_completed,
    reward_protocol,
    succeeded,
)
from run_training.roots import RootBank, file_hash

PROTOCOL = "a0_macro_mc_frozen_sampled_combat_v3"


@contextmanager
def graceful_stop():
    requested = Event()
    previous = {sig: signal.getsignal(sig) for sig in (signal.SIGTERM, signal.SIGINT)}
    for sig in previous:
        signal.signal(sig, lambda _signum, _frame: requested.set())
    try:
        yield requested
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def load_warm_start(
    model: MacroModel,
    path: Path,
    settings: dict,
    *,
    adapt_legacy_map: bool = False,
    policy_only: bool = False,
) -> dict:
    """Explicit weights-only transfer, not resume. Policy-only requires a fresh target critic."""
    source = torch.load(path, map_location="cpu", weights_only=True)
    config = source["config"]
    # #93 predates the encoder option and only implements the hashed model.
    if config.get("encoder", "hashed") != settings.get("encoder", "hashed"):
        raise ValueError("Run warm-start mismatch: encoder")
    version = config.get("feature_version")
    encoder = config.get("encoder", "hashed")
    legacy_map = (
        adapt_legacy_map
        and config.get("observation_schema") == 8
        and settings.get("observation_schema") == 9
        and (
            (
                encoder == "hashed"
                and (
                    version == 2 or (version == 3 and config.get("encoder") == "hashed")
                )
                and settings.get("feature_version") == FEATURE_VERSION
            )
            or (
                encoder == "health"
                and version == 3
                and settings.get("feature_version") == HEALTH_FEATURE_VERSION
            )
        )
    )
    if adapt_legacy_map and not legacy_map:
        raise ValueError(
            "Legacy map adapter requires a supported schema-8 checkpoint and schema-9 target"
        )
    if not legacy_map and version != settings.get("feature_version"):
        raise ValueError("Run warm-start mismatch: feature_version")
    for key in (
        "protocol",
        "model_width",
        "observation_schema",
        "final_act",
        "ascension",
        "gamma",
        "combat_sha256",
    ):
        if key == "observation_schema" and legacy_map:
            continue
        if source["config"].get(key) != settings.get(key):
            raise ValueError(f"Run warm-start mismatch: {key}")
    if not policy_only:
        for key in ("objective", "reward_protocol"):
            if config.get(key) != settings.get(key):
                raise ValueError(f"Run warm-start mismatch: {key}")
    fresh_critic = {
        key: value.clone()
        for key, value in model.state_dict().items()
        if key.startswith(("value.", "health_value."))
    }
    if policy_only and any(
        torch.count_nonzero(value)
        for key, value in fresh_critic.items()
        if key.startswith(("value.", "health_value.2."))
    ):
        raise ValueError(
            "Policy-only transfer requires a fresh zero-output target critic"
        )
    model.load_state_dict(source["model"], strict=True)
    if not all(torch.isfinite(p).all() for p in model.parameters()):
        raise ValueError("Nonfinite macro checkpoint")
    if policy_only:
        weights = model.state_dict()
        weights.update(fresh_critic)
        model.load_state_dict(weights, strict=True)
    return {
        "parent_map_adapter": "schema8_map_history_to_schema9_context_weights_only"
        if legacy_map
        else None,
        "parent_objective": config.get("objective"),
        "parent_reward_protocol": config.get("reward_protocol"),
        "critic_initialization": "fresh_zero_output"
        if policy_only
        else "parent_weights",
        "parent_feature_version": config.get("feature_version"),
        "parent_encoder": config.get("encoder", "hashed"),
        "parent_iteration": source["iteration"],
        "parent_optimizer_updates": source["optimizer_updates"],
        "parent_native_sha256": source["config"]["native_sha256"],
        "parent_policy_restriction_protocol": config.get(
            "policy_restriction_protocol", "all_legal_actions"
        ),
    }


def update(
    episodes: list[RunEpisode],
    model: MacroModel,
    optimizer: torch.optim.Optimizer,
    *,
    entropy_coef: float,
    value_coef: float,
    batch_steps: int = 0,
) -> dict[str, float]:
    """Validate a whole batch before backward; recompute from stored public inputs."""
    if not episodes or any(ep.reward is None for ep in episodes):
        raise CollectionFailure(
            "Incomplete batch: no optimizer update without settled task targets"
        )
    if any(ep.reward != ep.reward_parts().total for ep in episodes):
        raise ValueError("Terminal returns must match the declared reward protocol")
    count = sum(len(ep.steps) for ep in episodes)
    optimizer.zero_grad(set_to_none=True)
    if not count:
        return {"optimizer_step": 0.0, "macro_decisions": 0.0}
    if batch_steps < 0:
        raise ValueError("Negative learner batch size")
    if batch_steps:
        return _batched_update(
            episodes, model, optimizer, entropy_coef, value_coef, batch_steps
        )
    # Check all inference/recomputation pairs before accumulating any gradients.
    with torch.no_grad():
        for episode in episodes:
            for step in episode.steps:
                logits, value = model(step.inputs)
                checked_eligibility(
                    len(logits), step.inputs.eligibility, choice=step.choice
                )
                expected = torch.tensor(step.logits, device=logits.device)
                effective = restrict_logits(logits, step.inputs.eligibility)
                if (
                    not torch.isfinite(logits).all()
                    or not torch.isfinite(value)
                    or not torch.allclose(effective, expected, atol=1e-6, rtol=1e-5)
                    or not math.isclose(
                        float(value), step.value, abs_tol=1e-6, rel_tol=1e-5
                    )
                ):
                    raise ValueError(
                        "On-policy recomputation mismatch; refusing optimizer update"
                    )
    total_loss = 0.0
    for episode in episodes:
        for step in episode.steps:
            logits, value = model(step.inputs)
            distribution = Categorical(
                logits=restrict_logits(logits, step.inputs.eligibility)
            )
            choice = torch.tensor(step.choice, device=logits.device)
            assert episode.reward is not None
            target = episode.reward
            # Normalize policy sums by episodes: unbiased gradient of episode
            # return, rather than changing objective with variable run length.
            actor = -distribution.log_prob(choice) * (target - step.value)
            loss = (
                actor / len(episodes)
                + (
                    value_coef * (value - target).square()
                    - entropy_coef * distribution.entropy()
                )
                / count
            )
            if not torch.isfinite(loss):
                optimizer.zero_grad(set_to_none=True)
                raise ValueError("Nonfinite run loss")
            loss.backward()
            total_loss += float(loss.detach())
    gradients = [p.grad for p in model.parameters() if p.grad is not None]
    if not all(torch.isfinite(grad).all() for grad in gradients):
        optimizer.zero_grad(set_to_none=True)
        raise ValueError("Nonfinite run gradient")
    gradient_norm = torch.nn.utils.clip_grad_norm_(
        model.parameters(), 1.0, error_if_nonfinite=True
    )
    optimizer.step()
    return {
        "loss": total_loss,
        "optimizer_step": 1.0,
        "macro_decisions": float(count),
        "gradient_norm": float(gradient_norm),
        "gradient_clipped": float(gradient_norm > 1),
    }


def _batched_update(
    episodes: list[RunEpisode],
    model: MacroModel,
    optimizer: torch.optim.Optimizer,
    entropy_coef: float,
    value_coef: float,
    batch_steps: int,
) -> dict[str, float]:
    """Same MC objective/weights, bounded ragged forwards and one backward per chunk."""
    rows = []
    for episode in episodes:
        assert episode.reward is not None
        rows.extend((step, episode.reward) for step in episode.steps)
    chunks = []
    # Packing is parameter-independent and reused by verification and backward.
    # No learned embeddings/activations survive an optimizer update.
    for start in range(0, len(rows), batch_steps):
        part = rows[start : start + batch_steps]
        if any(
            len(step.logits) != len(step.inputs.candidates)
            or not 0 <= step.choice < len(step.inputs.candidates)
            for step, _ in part
        ):
            raise ValueError(
                "Invalid macro candidate record; refusing optimizer update"
            )
        for step, _ in part:
            checked_eligibility(
                len(step.inputs.candidates), step.inputs.eligibility, choice=step.choice
            )
        batch = model.pack([step.inputs for step, _ in part])
        reference = model.embedding.weight
        choices = torch.tensor(
            [step.choice for step, _ in part], device=reference.device
        )
        targets = reference.new_tensor([target for _, target in part])
        advantages = reference.new_tensor(
            [target - step.value for step, target in part]
        )
        chunks.append((part, batch, choices, targets, advantages))
    with torch.no_grad():
        for part, batch, _, _, _ in chunks:
            logits, values = model.forward_batch(batch)
            raw_actual = logits.flatten()[batch.positions]
            effective = restrict_batch_logits(
                logits, batch.lengths, [step.inputs.eligibility for step, _ in part]
            )
            actual = effective.flatten()[batch.positions]
            expected = logits.new_tensor([x for step, _ in part for x in step.logits])
            old_values = values.new_tensor([step.value for step, _ in part])
            if (
                not torch.isfinite(raw_actual).all()
                or not torch.isfinite(values).all()
                or not torch.allclose(actual, expected, atol=1e-6, rtol=1e-5)
                or not torch.allclose(values, old_values, atol=1e-6, rtol=1e-5)
            ):
                raise ValueError(
                    "On-policy recomputation mismatch; refusing optimizer update"
                )
    total = model.embedding.weight.new_zeros(())
    for part, batch, choices, targets, advantages in chunks:
        logits, values = model.forward_batch(batch)
        effective = restrict_batch_logits(
            logits, batch.lengths, [step.inputs.eligibility for step, _ in part]
        )
        distribution = Categorical(logits=effective)
        loss = (
            -distribution.log_prob(choices) * advantages / len(episodes)
            + (
                value_coef * (values - targets).square()
                - entropy_coef * distribution.entropy()
            )
            / len(rows)
        ).sum()
        if not torch.isfinite(loss):
            optimizer.zero_grad(set_to_none=True)
            raise ValueError("Nonfinite run loss")
        loss.backward()
        total += loss.detach()
    gradients = [p.grad for p in model.parameters() if p.grad is not None]
    if not all(torch.isfinite(grad).all() for grad in gradients):
        optimizer.zero_grad(set_to_none=True)
        raise ValueError("Nonfinite run gradient")
    gradient_norm = torch.nn.utils.clip_grad_norm_(
        model.parameters(), 1.0, error_if_nonfinite=True
    )
    optimizer.step()
    return {
        "loss": float(total),
        "optimizer_step": 1.0,
        "macro_decisions": float(len(rows)),
        "gradient_norm": float(gradient_norm),
        "gradient_clipped": float(gradient_norm > 1),
    }


def scores(episodes: list[RunEpisode]) -> dict[str, float]:
    targeted = [ep for ep in episodes if ep.reward is not None]
    complete = [ep for ep in targeted if game_completed(ep.status)]
    result = {
        "attempts": float(len(episodes)),
        "completed": float(len(complete)),
        "cutoffs": float(
            sum(ep.status in ("cutoff", "action_limit") for ep in episodes)
        ),
        "terminations/action_limit": float(
            sum(ep.termination_reason == "action_limit" for ep in episodes)
        ),
        "terminations/interrupted": float(
            sum(ep.termination_reason == "interrupted" for ep in episodes)
        ),
        "learning_target_episodes": float(len(targeted)),
        "errors": float(sum(ep.status == "error" for ep in episodes)),
        "successes": float(sum(succeeded(ep.objective, ep.status) for ep in complete)),
        "mean_floor": sum(ep.floor for ep in episodes) / max(1, len(episodes)),
        "accepted_actions": float(sum(ep.accepted for ep in episodes)),
        "mean_furthest_act1_floor": sum(ep.furthest_act1_floor for ep in episodes)
        / max(1, len(episodes)),
        "reward/terminal_samples": float(len(complete)),
        "reward/target_samples": float(len(targeted)),
        **behavior_scores([ep.behavior for ep in episodes]),
        **progress_scores([ep.progress for ep in episodes if ep.progress is not None]),
        **policy_scores(
            (step.logits, step.choice) for ep in episodes for step in ep.steps
        ),
        "outcome/heart_wins": float(sum(ep.status == "heart_clear" for ep in complete)),
        "outcome/deaths": float(sum(ep.status == "death" for ep in complete)),
        "outcome/act3_clear_without_heart": float(
            sum(ep.status == "act3_clear" for ep in complete)
        ),
    }
    if complete:
        result["completed_success_rate"] = sum(
            succeeded(ep.objective, ep.status) for ep in complete
        ) / len(complete)
    if targeted:
        parts = [ep.reward_parts() for ep in targeted]
        result["reward/mean"] = sum(p.total for p in parts) / len(parts)
        result["reward/progress_mean"] = sum(p.progress for p in parts) / len(parts)
        result["reward/clear_bonus_mean"] = sum(p.clear_bonus for p in parts) / len(
            parts
        )
        result["reward/heart_bonus_mean"] = sum(p.heart_bonus for p in parts) / len(
            parts
        )
        result["reward/key_bonus_mean"] = sum(p.key_bonus for p in parts) / len(parts)
        result["reward/action_limit_penalty_mean"] = sum(
            p.action_limit_penalty for p in parts
        ) / len(parts)
    targets = [
        (step.value, ep.reward)
        for ep in targeted
        if ep.reward is not None
        for step in ep.steps
    ]
    if targets:
        result["value/reward_target_mse"] = sum(
            (value - target) ** 2 for value, target in targets
        ) / len(targets)
    for act in (1, 2, 3, 4):
        result[f"outcome/deaths_act_{act}"] = float(
            sum(
                ep.status == "death"
                and ep.progress is not None
                and ep.progress.furthest_act == act
                for ep in complete
            )
        )
    # Lower bound uses all attempts. Never label cutoff as a true game loss.
    if episodes:
        result["outcome/heart_win_lower_bound"] = result["outcome/heart_wins"] / len(
            episodes
        )
        result["coverage/training_target_rate"] = len(targeted) / len(episodes)
        result["coverage/completion_rate"] = len(complete) / len(episodes)
        result["coverage/unknown_rate"] = 1 - len(complete) / len(episodes)
        result["success_lower_bound"] = sum(
            succeeded(ep.objective, ep.status) for ep in complete
        ) / len(episodes)
    return result


def validation_selection_key(
    metrics: dict[str, float],
) -> tuple[float, float, float] | None:
    """Success first (Heart wins for Heart objectives), never shaped score first."""
    scheduled = metrics.get("validation/scheduled", 0)
    if (
        not math.isfinite(scheduled)
        or scheduled <= 0
        or metrics.get("validation/attempts") != scheduled
        or metrics.get("validation/unattempted") != 0
    ):
        return None
    values = (
        metrics["validation/success_scheduled_lower_bound"],
        -(metrics["validation/errors"] + metrics["validation/cutoffs"]),
        metrics.get("validation/reward/mean", 0.0),
    )
    return values if all(math.isfinite(value) for value in values) else None


def validation_seeds(path: Path) -> list[str]:
    values = json.loads(path.read_text())
    if not isinstance(values, list) or not values:
        raise ValueError(
            "Validation seeds must be a nonempty JSON list of decimal strings"
        )
    if any(
        not isinstance(s, str)
        or not s.isascii()
        or not s.isdecimal()
        or not 0 <= int(s) < 2**63
        for s in values
    ):
        raise ValueError("Validation seeds must be decimal strings in [0, 2**63)")
    canonical = [str(int(s)) for s in values]
    if len(set(canonical)) != len(canonical):
        raise ValueError("Duplicate validation seed")
    return canonical


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__, prog="train.py --task run")
    result.add_argument("--run-id", required=True)
    result.add_argument(
        "--combat-checkpoint",
        type=Path,
        required=True,
        help="Trusted combat checkpoint; frozen weights only",
    )
    result.add_argument(
        "--validation-seeds",
        type=Path,
        required=True,
        help="Held-out JSON list of decimal run seeds",
    )
    result.add_argument(
        "--objective",
        choices=(
            "act1",
            "act1_binary",
            "act3",
            "heart",
            "heart_progress",
            "heart_keys_budget",
        ),
        default="act1",
    )
    result.add_argument(
        "--final-act", action=argparse.BooleanOptionalAction, default=True
    )
    result.add_argument(
        "--updates",
        type=int,
        default=1,
        help="Additional batch attempts; 0 runs until stopped",
    )
    result.add_argument(
        "--batch-size", type=int, default=8, help="Complete natural A0 runs per update"
    )
    result.add_argument(
        "--max-actions",
        type=int,
        default=5000,
        help="Total accepted actions per run, including combat/UI",
    )
    result.add_argument("--eval-every", type=int, default=1)
    result.add_argument(
        "--max-hours",
        type=float,
        help="Cooperative total wall-clock bound, including evaluation",
    )
    result.add_argument(
        "--continue-on-collection-failure",
        action="store_true",
        help="Explicit experimental mode: quarantine whole incomplete batches; never retry an action",
    )
    result.add_argument(
        "--max-consecutive-skips",
        type=int,
        default=8,
        help="Stop after this many consecutive incomplete batches; 0 disables this campaign limit",
    )
    result.add_argument("--compress-journals", action="store_true")
    result.add_argument(
        "--adapt-legacy-combat-checkpoint",
        action="store_true",
        help="Explicit weights-only v1 card vocabulary migration; append two zero embedding rows",
    )
    result.add_argument(
        "--collection-width",
        type=int,
        default=32,
        help="Concurrent independent episodes; 1 keeps the serial reference collector",
    )
    result.add_argument(
        "--learner-batch-size",
        type=int,
        default=128,
        help="Macro decisions per ragged learner batch; 0 selects serial reference",
    )
    result.add_argument(
        "--root-manifest",
        type=Path,
        help="Explicit synthetic pre-boss campfire curriculum; requires act1_binary",
    )
    result.add_argument(
        "--decision-diagnostics",
        choices=("none", "evaluation", "all"),
        default="none",
        help="Opt-in public context/candidate/probability audit in immutable journals; descriptive, not causal explanations",
    )
    result.add_argument("--encoder", choices=("hashed", "health"), default="hashed")
    result.add_argument("--root-hp-min", type=float, default=0.1)
    result.add_argument(
        "--root-eval-hp", type=float, nargs="+", default=[0.15, 0.5, 0.85]
    )
    result.add_argument("--model-width", type=int, default=64)
    result.add_argument(
        "--adapt-legacy-map-checkpoint",
        action="store_true",
        help="Explicit weights-only transfer from the previous map-history input protocol",
    )
    result.add_argument(
        "--raw-navigation",
        action="store_true",
        help="Ablation only: allow room returns and merchant re-entry",
    )
    result.add_argument("--seed", type=int, default=123)
    result.add_argument(
        "--avoid-prismatic",
        action="store_true",
        help="Explicit restricted-policy experiment: zero probability for visible optional Prismatic acquisitions; game legality unchanged",
    )
    result.add_argument(
        "--training-environment-seed",
        type=int,
        help="Explicit unsigned-64-bit master seed for private simulator-only libGDX inputs; omitted keeps strict replay-style inputs",
    )
    result.add_argument("--lr", type=float, default=1e-4)
    result.add_argument("--entropy-coef", type=float, default=0.01)
    result.add_argument("--value-coef", type=float, default=0.1)
    result.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    result.add_argument(
        "--collect-only",
        action="store_true",
        help="Pilot collection without any optimizer updates",
    )
    initialization = result.add_mutually_exclusive_group()
    initialization.add_argument(
        "--resume-from",
        type=Path,
        help="Trusted same-protocol checkpoint; always use a new run ID",
    )
    initialization.add_argument(
        "--warm-start",
        type=Path,
        help="Trusted compatible macro weights only; fresh optimizer/RNG and new run identity, allows new simulator/source",
    )
    initialization.add_argument(
        "--warm-start-policy-only",
        type=Path,
        help="Explicit actor/shared-encoder transfer across reward/objective changes; reset critic, optimizer and RNG in a new run",
    )
    result.add_argument("--output-root", type=Path, default=Path("wandb"))
    result.add_argument("--wandb-project", default="sts-run-a0-v1")
    result.add_argument(
        "--wandb-mode", choices=("online", "offline", "disabled"), default="disabled"
    )
    return result


def main(argv: list[str] | None = None) -> None:
    cli = parser()
    args = cli.parse_args(argv)
    if (
        min(
            args.batch_size,
            args.max_actions,
            args.eval_every,
            args.model_width,
            args.collection_width,
        )
        < 1
    ):
        cli.error("Counts must be positive")
    if args.updates < 0 or args.max_consecutive_skips < 0:
        cli.error("Updates and consecutive skips must be nonnegative")
    if (
        not args.run_id
        or Path(args.run_id).name != args.run_id
        or args.run_id in (".", "..")
    ):
        cli.error("run-id must be a single directory name")
    if (
        not math.isfinite(args.lr)
        or args.lr <= 0
        or any(
            not math.isfinite(x) or x < 0 for x in (args.entropy_coef, args.value_coef)
        )
    ):
        cli.error(
            "Expected positive finite lr and nonnegative finite loss coefficients"
        )
    if args.max_hours is not None and (
        not math.isfinite(args.max_hours) or args.max_hours <= 0
    ):
        cli.error("max-hours must be finite and positive")
    if (
        args.objective in ("heart", "heart_progress", "heart_keys_budget")
        and not args.final_act
    ):
        cli.error("Heart objective requires --final-act")
    if args.device == "cuda" and not torch.cuda.is_available():
        cli.error("CUDA unavailable")
    if args.learner_batch_size < 0:
        cli.error("learner-batch-size must be nonnegative")
    if args.training_environment_seed is not None:
        if not 0 <= args.training_environment_seed < 2**64:
            cli.error("training-environment-seed must be an unsigned 64-bit integer")
        if args.root_manifest:
            cli.error(
                "Root curricula inherit journaled environmental inputs; training-environment-seed requires natural starts"
            )
    held_out = validation_seeds(args.validation_seeds)
    if not 0 < args.root_hp_min <= 1 or any(not 0 < x <= 1 for x in args.root_eval_hp):
        cli.error("Root HP fractions must be finite and in (0, 1]")
    hp_labels = [round(100 * fraction) for fraction in args.root_eval_hp]
    if len(hp_labels) != len(set(hp_labels)):
        cli.error(
            "root-eval-hp fractions must have distinct rounded-percent metric names"
        )
    bank = (
        RootBank(args.root_manifest, final_act=args.final_act)
        if args.root_manifest
        else None
    )
    if bank:
        if args.objective != "act1_binary":
            cli.error("Root curriculum requires the declared binary Act-1 objective")
        if set(bank.training) & set(held_out) or not set(bank.validation) <= set(
            held_out
        ):
            cli.error("Root splits must respect original held-out run seeds")
    evaluation_cases = (
        [(seed, hp) for seed in bank.validation for hp in args.root_eval_hp]
        if bank
        else [(seed, None) for seed in held_out]
    )
    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    rng = random.Random(args.seed)
    model_type = HealthMacroModel if args.encoder == "health" else MacroModel
    model = model_type(args.model_width).to(args.device)
    combat = FrozenCombat(
        args.combat_checkpoint,
        args.device,
        adapt_legacy=args.adapt_legacy_combat_checkpoint,
    )
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr)
    settings = {
        key: str(value) if isinstance(value, Path) else value
        for key, value in vars(args).items()
    }
    source_root = Path(__file__).resolve().parents[1]
    source_files = sorted(
        set(source_root.glob("*.py"))
        | set((source_root / "encoders").glob("*.py"))
        | set((source_root / "encoders").glob("*.json"))
        | set(Path(__file__).parent.glob("*.py"))
    )
    source_bytes = b"".join(
        str(p.relative_to(source_root)).encode() + b"\0" + p.read_bytes()
        for p in source_files
    )
    settings.update(
        initialization="policy_only_reset_critic"
        if args.warm_start_policy_only
        else "warm_start_weights_only"
        if args.warm_start
        else ("exact_resume" if args.resume_from else "fresh"),
        parent_checkpoint_sha256=digest(
            args.warm_start_policy_only or args.warm_start or args.resume_from
        )
        if (args.warm_start_policy_only or args.warm_start or args.resume_from)
        else None,
        protocol=PROTOCOL,
        execution_protocol="ragged_macro_learner_cooperative_collection_v1",
        policy_restriction_protocol=AVOID_PRISMATIC_PROTOCOL
        if args.avoid_prismatic
        else "all_legal_actions",
        navigation_protocol="raw_navigation"
        if args.raw_navigation
        else NAVIGATION_PROTOCOL,
        map_input_protocol="context_current_act_map_v1",
        initial_state_profile="synthetic_preboss_campfire_hp_only"
        if bank
        else "natural_start",
        root_manifest_sha256=file_hash(args.root_manifest) if bank else None,
        root_train_count=len(bank.training) if bank else 0,
        root_validation_count=len(bank.validation) if bank else 0,
        reward_protocol=reward_protocol(args.objective),
        heart_reward_spec={
            "floor_normalizer": HEART_PROGRESS_FLOOR,
            "floor_scale": HEART_PROGRESS_SCALE,
            "act_clear_bonus": ACT_CLEAR_BONUS,
            "heart_bonus": HEART_CLEAR_BONUS,
        }
        if args.objective in ("heart_progress", "heart_keys_budget")
        else None,
        key_bonus=KEY_BONUS if args.objective == "heart_keys_budget" else 0.0,
        action_limit_penalty=ACTION_LIMIT_PENALTY
        if args.objective == "heart_keys_budget"
        else None,
        key_choice_metric_protocol=KEY_CHOICE_PROTOCOL,
        progress_metric_protocol=PROGRESS_PROTOCOL,
        decision_diagnostic_protocol=DIAGNOSTIC_PROTOCOL,
        act1_progress_floor_normalizer=ACT1_BOSS_FLOOR,
        act1_clear_bonus=ACT1_CLEAR_BONUS,
        behavior_metric_protocol="accepted_behavior_counts_hp_map_shop_reentries_key_choices_v3",
        torch_version=str(torch.__version__),
        cuda_version=torch.version.cuda,
        combat_execution="frozen sampled FP32 weights-only; all legal candidates retained",
        combat_checkpoint_adapter="public_card_v1_to_v2_zero_extension"
        if args.adapt_legacy_combat_checkpoint
        else "strict_current_weights",
        failure_policy="quarantine whole batches; conditional supported training"
        if args.continue_on_collection_failure
        else "abort",
        feature_version=HEALTH_FEATURE_VERSION
        if args.encoder == "health"
        else FEATURE_VERSION,
        ascension=0,
        environment_protocol=(
            "root_journal_inherited_environment"
            if bank
            else ENVIRONMENT_PROTOCOL
            if args.training_environment_seed is not None
            else "strict_explicit_inputs"
        ),
        gamma=1.0,
        observation_schema=FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
        native_sha256=digest(Path(_native.__file__)),
        combat_sha256=digest(args.combat_checkpoint),
        validation_sha256=digest(args.validation_seeds),
        source_sha256=hashlib.sha256(source_bytes).hexdigest(),
    )
    iteration = 0
    optimizer_updates = 0
    skipped_batches = 0
    consecutive_skips = 0
    committed_iteration = None
    if args.adapt_legacy_map_checkpoint and not (
        args.warm_start or args.warm_start_policy_only
    ):
        cli.error("--adapt-legacy-map-checkpoint requires a weights-only warm start")
    if args.warm_start or args.warm_start_policy_only:
        settings.update(
            load_warm_start(
                model,
                args.warm_start or args.warm_start_policy_only,
                settings,
                adapt_legacy_map=args.adapt_legacy_map_checkpoint,
                policy_only=args.warm_start_policy_only is not None,
            )
        )
    if args.resume_from:
        source = torch.load(args.resume_from, map_location="cpu", weights_only=True)
        ignored = {
            "run_id",
            "updates",
            "resume_from",
            "output_root",
            "wandb_project",
            "wandb_mode",
            "combat_checkpoint",
            "validation_seeds",
            "max_hours",
            "warm_start",
            "adapt_legacy_map_checkpoint",
            "warm_start_policy_only",
            "initialization",
            "parent_checkpoint_sha256",
        }
        for key, value in settings.items():
            if key not in ignored and source["config"].get(key) != value:
                raise ValueError(f"Run checkpoint continuation mismatch: {key}")
        model.load_state_dict(source["model"], strict=True)
        if not all(torch.isfinite(p).all() for p in model.parameters()):
            raise ValueError("Nonfinite macro checkpoint")
        optimizer.load_state_dict(source["optimizer"])
        rng.setstate(source["sampling_rng"])
        torch.set_rng_state(source["torch_rng"])
        if args.device == "cuda":
            torch.cuda.set_rng_state_all(source["cuda_rng"])
        iteration = source["iteration"]
        optimizer_updates = source["optimizer_updates"]
        skipped_batches = source["skipped_batches"]
        consecutive_skips = source["consecutive_skips"]
    output = args.output_root / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    (output / "trainer.pid").write_text(str(os.getpid()) + "\n")
    (output / "config.json").write_text(json.dumps(settings, indent=2) + "\n")
    shutil.copyfile(args.validation_seeds, output / "validation-seeds.json")
    for path in source_files:
        destination = output / "source" / path.relative_to(source_root)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
    started = time.monotonic()
    initial_optimizer_updates = optimizer_updates
    deadline = (
        started + args.max_hours * 3600 if args.max_hours is not None else math.inf
    )
    suffix = ".jsonl.gz" if args.compress_journals else ".jsonl"

    def checkpoint() -> None:
        nonlocal committed_iteration
        temporary = output / "latest.tmp"
        torch.save(
            {
                "model": model.state_dict(),
                "optimizer": optimizer.state_dict(),
                "iteration": iteration,
                "optimizer_updates": optimizer_updates,
                "skipped_batches": skipped_batches,
                "consecutive_skips": consecutive_skips,
                "sampling_rng": rng.getstate(),
                "torch_rng": torch.get_rng_state(),
                "cuda_rng": torch.cuda.get_rng_state_all()
                if args.device == "cuda"
                else [],
                "config": settings,
            },
            temporary,
        )
        temporary.replace(output / "latest.pt")
        committed_iteration = iteration

    def stopped() -> bool:
        return stop_flag.is_set() or time.monotonic() >= deadline

    def make_job(
        seed: str,
        directory: Path,
        policy_seed: int,
        hp_fraction: float | None = None,
        *,
        evaluation: bool = False,
    ) -> CollectionJob:
        factory = State.new
        previous = None
        metadata = None
        environment_seed = (
            episode_environment_seed(args.training_environment_seed, seed)
            if args.training_environment_seed is not None
            else None
        )
        if bank:
            fraction = (
                rng.uniform(args.root_hp_min, 1.0)
                if hp_fraction is None
                else hp_fraction
            )
            state, _, previous, metadata = bank.initial(seed, fraction)
            environment_seed = journal_environment_seed(metadata)

            def start_root(*_args, **_kwargs) -> State:
                return state

            factory = start_root
        return CollectionJob(
            seed,
            {
                "objective": args.objective,
                "final_act": args.final_act,
                "avoid_prismatic": args.avoid_prismatic,
                "commit_navigation": not args.raw_navigation,
                "decision_diagnostics": args.decision_diagnostics == "all"
                or (evaluation and args.decision_diagnostics == "evaluation"),
                "max_actions": args.max_actions,
                "macro_rng": torch.Generator(device=args.device).manual_seed(
                    policy_seed
                ),
                "combat_rng": torch.Generator(device=args.device).manual_seed(
                    policy_seed ^ 0x5DEECE66D
                ),
                "journal": directory,
                "stop_requested": stopped,
                "state_factory": factory,
                "initial_previous": previous,
                "initial_metadata": metadata,
                "training_rng_seed": environment_seed,
            },
        )

    def record_failure(job: CollectionJob, error: CollectionFailure) -> None:
        with (output / "collection-errors.jsonl").open("a") as errors:
            errors.write(
                json.dumps(
                    {
                        "seed": job.seed,
                        "journal": str(job.options["journal"]),
                        "error": str(error),
                    }
                )
                + "\n"
            )

    def episodes(jobs) -> list[RunEpisode]:
        if args.collection_width > 1:
            return collect_many(
                jobs,
                model,
                combat,
                width=args.collection_width,
                continue_on_failure=args.continue_on_collection_failure,
                stop_requested=stopped,
                on_failure=record_failure,
            )
        # Keep a genuine serial reference path, including single-state inference.
        result = []
        source = iter(jobs)
        while not stopped():
            try:
                job = next(source)
            except StopIteration:
                break
            try:
                result.append(collect(job.seed, model, combat, **job.options))
            except CollectionFailure as error:
                record_failure(job, error)
                if not args.continue_on_collection_failure:
                    raise
                result.append(failed_episode(error, job.options["objective"]))
        return result

    with (
        graceful_stop() as stop_flag,
        wandb.init(
            project=args.wandb_project,
            id=args.run_id,
            name=args.run_id,
            config=settings,
            mode=args.wandb_mode,
        ) as run,
        (output / "metrics.jsonl").open("x") as metrics,
    ):
        run.define_metric("iteration")
        run.define_metric("train/*", step_metric="iteration")
        run.define_metric("validation/*", step_metric="iteration")
        run.define_metric("efficiency/*", step_metric="iteration")
        run.define_metric("policy/*", step_metric="iteration")
        (output / "tracking.json").write_text(
            json.dumps({"mode": args.wandb_mode, "url": run.url}, indent=2)
        )

        def log(values: dict, step: int) -> None:
            values = {
                **values,
                "policy/avoid_prismatic": float(args.avoid_prismatic),
                "elapsed_seconds": time.monotonic() - started,
                "optimizer_updates": optimizer_updates,
                "skipped_batches": skipped_batches,
                "efficiency/optimizer_updates_per_hour": 3600
                * (optimizer_updates - initial_optimizer_updates)
                / max(time.monotonic() - started, 1e-9),
            }
            metrics.write(json.dumps({"iteration": step, **values}) + "\n")
            metrics.flush()
            # Custom iteration axis lets train and validation commit separate rows
            # at the same iteration without W&B dropping a repeated internal step.
            run.log({"iteration": step, **values}, commit=True)
            print(json.dumps({"iteration": step, **values}), flush=True)

        best_key = None

        def evaluate(step: int) -> None:
            nonlocal best_key
            evaluation_started = time.monotonic()
            evaluated = episodes(
                make_job(
                    seed,
                    output / "journals" / f"eval-{step}-{i}{suffix}",
                    100000 + (i // len(args.root_eval_hp) if bank else i),
                    hp_fraction,
                    evaluation=True,
                )
                for i, (seed, hp_fraction) in enumerate(evaluation_cases)
            )
            root_scores = {}
            if bank:
                for index, fraction in enumerate(args.root_eval_hp):
                    group = evaluated[index :: len(args.root_eval_hp)]
                    prefix = f"validation/root_hp_{round(100 * fraction):02d}"
                    root_scores.update(
                        {f"{prefix}/{k}": v for k, v in scores(group).items()}
                    )
                    initial_values = [ep.steps[0].value for ep in group if ep.steps]
                    if initial_values:
                        root_scores[f"{prefix}/initial_value_mean"] = sum(
                            initial_values
                        ) / len(initial_values)
                    targets = [
                        (ep.steps[0].value, ep.reward)
                        for ep in group
                        if ep.steps and ep.reward is not None
                    ]
                    if targets:
                        root_scores[f"{prefix}/initial_value_mse"] = sum(
                            (v - r) ** 2 for v, r in targets
                        ) / len(targets)
            evaluation_metrics = {
                **root_scores,
                **{f"validation/{k}": v for k, v in scores(evaluated).items()},
                "validation/collection_seconds": time.monotonic() - evaluation_started,
                "validation/success_scheduled_lower_bound": sum(
                    succeeded(ep.objective, ep.status)
                    for ep in evaluated
                    if ep.reward is not None
                )
                / len(evaluation_cases),
                "validation/outcome/heart_win_scheduled_lower_bound": sum(
                    ep.status == "heart_clear" and ep.reward is not None
                    for ep in evaluated
                )
                / len(evaluation_cases),
                "validation/scheduled": len(evaluation_cases),
                "validation/unattempted": len(evaluation_cases) - len(evaluated),
            }
            log(evaluation_metrics, step)
            archive = output / f"checkpoint-{step:08d}.pt"
            shutil.copyfile(output / "latest.pt", archive)
            key = validation_selection_key(evaluation_metrics)
            if key is not None and (best_key is None or key > best_key):
                temporary = output / "best-validation.tmp"
                shutil.copyfile(archive, temporary)
                temporary.replace(output / "best-validation.pt")
                metadata = output / "best-validation.json.tmp"
                metadata.write_text(
                    json.dumps(
                        {
                            "iteration": step,
                            "checkpoint": str(archive),
                            "selection_key": key,
                            "objective": args.objective,
                            "policy_restriction_protocol": settings[
                                "policy_restriction_protocol"
                            ],
                            "meaning": "Best repeated validation in this invocation, not independent final-test performance",
                            "metrics": evaluation_metrics,
                        },
                        indent=2,
                    )
                    + "\n"
                )
                metadata.replace(output / "best-validation.json")
                best_key = key

        checkpoint()  # Last valid boundary remains recoverable after collection failure.
        try:
            evaluate(iteration)
            for _ in range(args.updates) if args.updates else count():
                if stopped():
                    break
                next_iteration = iteration + 1

                def training_jobs(batch_iteration=next_iteration):
                    for index in range(args.batch_size):
                        if bank:
                            seed = rng.choice(bank.training)
                        else:
                            seed = str(rng.getrandbits(63))
                            while seed in held_out:
                                seed = str(rng.getrandbits(63))
                        # Preserve serial seed -> policy RNG -> optional HP draw order.
                        yield make_job(
                            seed,
                            output
                            / "journals"
                            / f"train-{batch_iteration}-{index}{suffix}",
                            rng.getrandbits(63),
                        )

                collection_started = time.monotonic()
                batch = episodes(training_jobs())
                collection_seconds = time.monotonic() - collection_started
                update_started = time.monotonic()
                incomplete = len(batch) != args.batch_size or any(
                    ep.reward is None for ep in batch
                )
                if args.collect_only:
                    learned = {
                        "optimizer_step": 0.0,
                        "batch_discarded": float(incomplete),
                    }
                elif incomplete:
                    if not args.continue_on_collection_failure and not stopped():
                        raise CollectionFailure("Incomplete batch: optimizer unchanged")
                    skipped_batches += 1
                    consecutive_skips += 1
                    learned = {"optimizer_step": 0.0, "batch_discarded": 1.0}
                else:
                    learned = update(
                        batch,
                        model,
                        optimizer,
                        entropy_coef=args.entropy_coef,
                        value_coef=args.value_coef,
                        batch_steps=args.learner_batch_size,
                    )
                    optimizer_updates += int(learned["optimizer_step"])
                    consecutive_skips = 0
                learned.update(
                    collection_seconds=collection_seconds,
                    update_seconds=time.monotonic() - update_started,
                    episodes_per_second=len(batch) / max(collection_seconds, 1e-9),
                    actions_per_second=sum(ep.accepted for ep in batch)
                    / max(collection_seconds, 1e-9),
                    episodes_used_for_learning=float(
                        len(batch) if learned["optimizer_step"] else 0
                    ),
                    completed_episodes_discarded=float(
                        sum(
                            ep.reward is not None and game_completed(ep.status)
                            for ep in batch
                        )
                        if incomplete
                        else 0
                    ),
                    target_episodes_discarded=float(
                        sum(ep.reward is not None for ep in batch) if incomplete else 0
                    ),
                )
                iteration = next_iteration
                checkpoint()  # Commit learning before fallible telemetry publishing.
                log(
                    {f"train/{k}": v for k, v in {**scores(batch), **learned}.items()},
                    iteration,
                )
                if stopped():
                    break
                if (
                    args.max_consecutive_skips
                    and consecutive_skips >= args.max_consecutive_skips
                ):
                    raise CollectionFailure(
                        "Consecutive incomplete-batch limit reached; stopping safely"
                    )
                if iteration % args.eval_every == 0:
                    evaluate(iteration)
            if iteration % args.eval_every and not stopped():
                evaluate(iteration)
            checkpoint()
            (output / "finished.json").write_text(
                json.dumps(
                    {
                        "iteration": iteration,
                        "optimizer_updates": optimizer_updates,
                        "skipped_batches": skipped_batches,
                        "reason": "budget_or_signal"
                        if stopped()
                        else "updates_complete",
                        "elapsed_seconds": time.monotonic() - started,
                    },
                    indent=2,
                )
            )
        except Exception as error:
            (output / "failure.json").write_text(
                json.dumps(
                    {
                        "iteration": iteration,
                        "error": f"{type(error).__name__}: {error}",
                        "last_committed_iteration": committed_iteration,
                        "resume": "latest.pt is the last committed update; journals retain attempted actions",
                    },
                    indent=2,
                )
            )
            raise
