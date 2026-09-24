"""Synthetic combat training: rollout, update, evaluation, and one training loop."""

import argparse
import hashlib
import json
import logging
import math
import multiprocessing
import random
import shutil
import time
from concurrent.futures import Future, ProcessPoolExecutor
from dataclasses import dataclass
from itertools import pairwise
from pathlib import Path
from typing import cast

import numpy as np
import torch
import wandb
from beam_search import beam_search
from combat_task import action_indices, combat_outcome, terminal_reward
from encoders.numeric import (
    ACTION_KIND,
    ACTION_LEGAL_INDEX,
    ACTION_OWNER,
    ACTION_POTION,
    ACTION_REVISION,
    ACTION_TARGET,
    CANDIDATE_OWNER,
    NUMERIC_VERSION,
    NumericBatch,
)
from encoders.potions import POTION_TO_INDEX
from loadout_sampling import LoadoutSampler
from model import CombatValueModel
from rollout_errors import SimulatorStepError
from scenarios import ScenarioConfig
from sts_sim import ACTION_KINDS, PotionKey, State
from synthetic_roots import SyntheticRoot, sample_root
from torch import Tensor
from torch.distributions import Categorical
from trajectories import DecisionRound, Trajectories, validate_gradients
from validation_set import Root, load_validation


@dataclass
class Episode:
    reward: float | None  # Terminal HP / starting max HP; None means truncation.
    won: bool | None
    hp: int
    decisions: int


@dataclass
class ReplayRound:
    """Public inputs needed to recompute one decision round after a no-grad rollout."""

    observations: NumericBatch
    candidates: np.ndarray
    owners: tuple[int, ...]
    choices: tuple[int, ...]
    counts: tuple[int, ...]


def _policy_candidates(
    batch: NumericBatch, active: list[int]
) -> tuple[np.ndarray, list[list[int]], list[int], list[int]]:
    """Filter escape potions without changing public legal indices.

    Returned candidate owners are positions in ``active``, matching model rows.
    Legal indices still address each state's full public action list.
    """
    rows = batch.action_rows
    owner_map = np.full(len(batch.table("header", 5)), -1, dtype=np.int64)
    owner_map[active] = np.arange(len(active))
    owners = owner_map[rows[:, ACTION_OWNER]]
    keep = owners >= 0
    potion_uses = keep & (rows[:, ACTION_KIND] == ACTION_KINDS.index("use_potion_slot"))
    potion_uses &= rows[:, ACTION_POTION] >= 0
    if np.any(potion_uses):
        potions = batch.table("potions", 3)
        offsets = np.concatenate(([0], np.cumsum(batch.lengths(potions))))
        codes = potions[offsets[owners[potion_uses]] + rows[potion_uses, ACTION_POTION], 1]
        keep[potion_uses] &= codes != POTION_TO_INDEX[PotionKey.SMOKE_BOMB]
    kept, owners = rows[keep], owners[keep]
    # Native rows are already grouped; stable sorting also preserves arbitrary public row order.
    if np.any(owners[1:] < owners[:-1]):
        order = np.argsort(owners, kind="stable")
        kept, owners = kept[order], owners[order]
    counts = np.bincount(owners, minlength=len(active))
    if np.any(counts == 0):
        raise RuntimeError("No allowed combat actions after disabling escape")
    boundaries = np.concatenate(([0], np.cumsum(counts)))
    revisions = kept[boundaries[:-1], ACTION_REVISION]
    if np.any(kept[:, ACTION_REVISION] != revisions[owners]):
        raise RuntimeError("Legal actions for one state have mixed revisions")
    candidates = np.empty((len(kept), 6), dtype=np.int64)
    candidates[:, 0] = owners
    candidates[:, 1:] = kept[:, ACTION_KIND : ACTION_TARGET + 1]
    indices = kept[:, ACTION_LEGAL_INDEX].tolist()
    legal = [indices[start:end] for start, end in pairwise(boundaries)]
    return candidates, legal, revisions.tolist(), counts.tolist()


def play_combats(
    roots: list[State],
    model: CombatValueModel | None,
    *,
    max_decisions: int,
    training: bool = False,
    rng: random.Random,
    trajectories: Trajectories | None = None,
    episode_rngs: list[random.Random] | None = None,
    episode_generators: list[torch.Generator] | None = None,
    replays: list[ReplayRound] | None = None,
) -> list[Episode]:
    """One numeric rollout path. Clone roots; errors never retry partially advanced states.

    ``episode_rngs`` and ``episode_generators`` are per-root evaluation streams. They are
    indexed by the original root and are not the training sampler. Generators are advanced
    in place and are never copied into the process-global RNG.
    """
    if not roots or max_decisions < 0:
        raise ValueError("Rollouts need roots and a nonnegative decision limit")
    if training and (model is None or trajectories is None):
        raise ValueError("Training requires a model and trajectory collector")
    # ReplayRound stores public inputs and chosen indices, not autograd tensors.
    if trajectories is not None and trajectories.rounds:
        raise ValueError("Use a fresh trajectory collector for each batch")
    if episode_rngs is not None and episode_generators is not None:
        raise ValueError("Pass only one per-episode RNG stream")
    if episode_rngs is not None and (model is not None or len(episode_rngs) != len(roots)):
        raise ValueError("Python episode RNGs are for a model-free rollout of the same length")
    if episode_generators is not None and (model is None or training or len(episode_generators) != len(roots)):
        raise ValueError("Torch episode generators are for evaluation rollouts of the same length")
    states = [root.clone() for root in roots]
    batch = NumericBatch(State.numeric_decisions(states))
    starting_max_hp = batch.table("header", 5)[:, 4].tolist()
    remaining = list(range(len(roots)))
    action_prefixes: list[list[int]] = [[] for _ in roots]
    episodes: list[Episode | None] = [None] * len(roots)
    for step in range(max_decisions + 1):
        active = []
        action_counts = np.bincount(batch.action_rows[:, ACTION_OWNER], minlength=len(remaining))
        for row, index in enumerate(remaining):
            kind, phase, combat_phase, hp, _ = batch.table("header", 5)[row]
            kind, phase = batch.symbols[kind], batch.symbols[phase]
            combat_phase = batch.symbols[combat_phase] if combat_phase >= 0 else None
            won = (
                True
                if phase == "reward" or combat_phase == "won" or (kind == "complete" and hp > 0)
                else (False if combat_phase == "lost" or (kind == "complete" and hp <= 0) else None)
            )
            if kind == "event":
                # Rare terminal event return (Colosseum); validate through the typed public screen.
                won = combat_outcome(states[index].decision().observation)
            if won is not None:
                hp = int(hp) if won else 0
                episodes[index] = Episode(hp / starting_max_hp[index], won, hp, step)
            elif kind != "combat":
                raise RuntimeError(f"Unexpected screen after combat: {kind}/{phase}")
            elif step == max_decisions:
                episodes[index] = Episode(None, None, int(hp), step)
            else:
                if action_counts[row] == 0:
                    raise SimulatorStepError(
                        "Unsettled combat decision or empty legal-action list",
                        step,
                        [index],
                        [list(action_prefixes[index])],
                    )
                active.append(row)
                continue
        if not active:
            assert all(episode is not None for episode in episodes)
            return [episode for episode in episodes if episode is not None]
        if active != batch.model_rows:
            raise RuntimeError("Numeric combat rows do not match settled decisions")
        candidates, legal_indices, revisions, counts = _policy_candidates(batch, active)
        if model is None:
            if episode_rngs is None:
                choices = [rng.randrange(count) for count in counts]
            else:
                choices = [
                    episode_rngs[remaining[row]].randrange(count) for row, count in zip(active, counts, strict=True)
                ]
        else:
            with torch.set_grad_enabled(training):
                logits, values, _ = model(batch, candidates)
                if episode_generators is None:
                    distribution = Categorical(logits=logits)
                    sampled = distribution.sample()
                    if trajectories is not None:
                        trajectories.rounds.append(
                            DecisionRound(
                                tuple(remaining[row] for row in active),
                                tuple(counts),
                                distribution.log_prob(sampled),
                                distribution.entropy(),
                                cast(Tensor, distribution.probs).max(dim=1).values.detach(),
                                values.squeeze(-1),
                            )
                        )
                    choices = sampled.tolist()
                else:
                    # One draw from each fight's generator. Batched logits are not bit-identical
                    # to a single-row forward; only the random stream stays independent.
                    choices = []
                    for position, row in enumerate(active):
                        choices.append(
                            sample_unpadded_action(
                                logits[position],
                                counts[position],
                                episode_generators[remaining[row]],
                            )
                        )
                if replays is not None:
                    replays.append(
                        ReplayRound(
                            batch,
                            candidates,
                            tuple(remaining[row] for row in active),
                            tuple(choices),
                            tuple(counts),
                        )
                    )
        chosen_indices = [legal_indices[position][choice] for position, choice in enumerate(choices)]
        for row, legal_index in zip(active, chosen_indices, strict=True):
            action_prefixes[remaining[row]].append(legal_index)
        remaining = [remaining[row] for row in active]
        try:
            payload = State.numeric_steps(
                [states[index] for index in remaining],
                chosen_indices,
                [revisions[position] for position in range(len(active))],
            )
        except ValueError as error:
            # Native batches are not atomic. Never retry/reapply actions to these clones.
            raise SimulatorStepError(
                str(error), step, remaining, [action_prefixes[index] for index in remaining]
            ) from error
        batch = NumericBatch(payload)
    raise AssertionError("Unreachable")


def episode_metrics(episodes: list[Episode]) -> dict[str, float]:
    completed = [episode for episode in episodes if episode.reward is not None]
    result = {
        "episodes": float(len(episodes)),
        "completed": float(len(completed)),
        "truncated": float(len(episodes) - len(completed)),
        "defeated": float(sum(episode.won is False for episode in completed)),
        "mean_decisions": sum(episode.decisions for episode in episodes) / len(episodes),
    }
    if completed:
        result.update(
            {
                "win_rate_completed": sum(episode.won is True for episode in completed) / len(completed),
                "mean_return_completed": sum(episode.reward for episode in completed if episode.reward is not None)
                / len(completed),
                "mean_hp_completed": sum(episode.hp for episode in completed) / len(completed),
            }
        )
    return result


def sample_unpadded_action(logits: Tensor, n_legal: int, generator: torch.Generator | None = None) -> int:
    """Sample one legal index. Extra ``-inf`` padding must not affect the draw or RNG."""
    if n_legal < 1 or n_legal > logits.shape[-1]:
        raise ValueError("Legal action count is outside the logit row")
    # Shape [1, n_legal] matches a single-fight forward, which has no foreign padding.
    row = logits[..., :n_legal]
    if row.ndim == 1:
        row = row.unsqueeze(0)
    if generator is None:
        return int(Categorical(logits=row).sample())
    # Categorical normalizes before drawing. softmax(raw logits) is not the same
    # rounding, so the generator must consume Categorical.probs.
    return int(torch.multinomial(cast(Tensor, Categorical(logits=row).probs), 1, generator=generator))


def _policy_seed(index: int, repeats: int, repeat: int) -> int:
    return 90000 + index * repeats + repeat


def _policy_generator(seed: int, device: torch.device) -> torch.Generator:
    """Per-fight generator seeded like the historical global sampler."""
    generator = torch.Generator(device=device)
    generator.manual_seed(seed)
    return generator


def evaluate(
    roots: list[Root],
    model: CombatValueModel | None,
    repeats: int,
    max_decisions: int,
    *,
    error_path: Path | None = None,
    batch_size: int = 1,
) -> dict[str, float]:
    """Same validation sampling seeds each time; never consume training RNG state.

    ``batch_size=1`` is the historical protocol: one forward per fight. Larger batches
    keep an independent seed per fight, but the batched forward is not bit-identical
    to that single-row forward. Do not compare those policy scores as one protocol.
    Model-free evaluation stays identical at every batch size.
    """
    if batch_size < 1:
        raise ValueError("batch_size must be positive")
    episodes: list[Episode] = []
    hp_changes: list[int] = []
    failed_cases: set[int] = set()
    failures = 0
    batched_unavailable = 0
    if model is not None:
        model.eval()
    # manual_seed also seeds CUDA; preserve its state when a GPU model is evaluated.
    cuda = model is not None and next(model.parameters()).is_cuda
    devices = list(range(torch.cuda.device_count())) if cuda else []

    def record_error(
        index: int,
        root: Root,
        repeat: int,
        seed: int,
        error: SimulatorStepError,
        *,
        source: str,
        attempted_prefixes: list | None,
    ) -> None:
        nonlocal failures
        if error_path is None:
            raise error
        failures += 1
        failed_cases.add(index)
        error_path.parent.mkdir(parents=True, exist_ok=True)
        with error_path.open("a") as handle:
            handle.write(
                json.dumps(
                    {
                        "root_index": index,
                        "combat_seed": root.combat_seed,
                        "repeat": repeat,
                        "policy_seed": seed,
                        "error": str(error),
                        "step": error.step,
                        "attempted_prefixes": attempted_prefixes,
                        "source": source,
                        "fallback": "none",
                    }
                )
                + "\n"
            )
        print(f"VALIDATION SIMULATOR ERROR root={index} repeat={repeat} source={source}: {error_path}", flush=True)

    def accept(index: int, root: Root, episode: Episode) -> None:
        episodes.append(episode)
        if episode.reward is not None:
            hp_changes.append(episode.hp - root.start_hp)

    def run_one(index: int, repeat: int, root: Root, seed: int) -> None:
        if cuda:
            torch.manual_seed(seed)
        else:
            torch.random.default_generator.manual_seed(seed)
        try:
            episode = play_combats([root.state], model, max_decisions=max_decisions, rng=random.Random(seed))[0]
        except SimulatorStepError as error:
            record_error(index, root, repeat, seed, error, source="serial", attempted_prefixes=error.action_prefixes)
            return
        accept(index, root, episode)

    jobs = [
        (index, repeat, root, _policy_seed(index, repeats, repeat))
        for index, root in enumerate(roots)
        for repeat in range(repeats)
    ]
    with torch.random.fork_rng(devices=devices):
        if batch_size == 1:
            for index, repeat, root, seed in jobs:
                run_one(index, repeat, root, seed)
        else:
            for start in range(0, len(jobs), batch_size):
                chunk = jobs[start : start + batch_size]
                try:
                    if model is None:
                        played = play_combats(
                            [root.state for _, _, root, _ in chunk],
                            None,
                            max_decisions=max_decisions,
                            rng=random.Random(0),
                            episode_rngs=[random.Random(seed) for _, _, _, seed in chunk],
                        )
                    else:
                        played = play_combats(
                            [root.state for _, _, root, _ in chunk],
                            model,
                            max_decisions=max_decisions,
                            rng=random.Random(0),
                            episode_generators=[
                                _policy_generator(seed, next(model.parameters()).device) for _, _, _, seed in chunk
                            ],
                        )
                except SimulatorStepError as error:
                    # Clones inside the failed call are discarded. Do not replace them with
                    # serial forwards: those can follow a different trajectory and hide the
                    # batched-protocol failure. The whole chunk is unavailable.
                    if error_path is None:
                        raise
                    prefixes = {
                        local: prefix for local, prefix in zip(error.root_indices, error.action_prefixes, strict=True)
                    }
                    for local, (index, repeat, root, seed) in enumerate(chunk):
                        record_error(
                            index,
                            root,
                            repeat,
                            seed,
                            error,
                            source="batched_chunk",
                            attempted_prefixes=prefixes.get(local),
                        )
                        batched_unavailable += 1
                    continue
                for (index, _, root, _), episode in zip(chunk, played, strict=True):
                    accept(index, root, episode)
    result = episode_metrics(episodes) if episodes else {"episodes": 0.0, "completed": 0.0, "truncated": 0.0}
    result.update(
        attempted_episodes=float(len(roots) * repeats),
        simulator_error_episodes=float(failures),
        batched_protocol_unavailable_episodes=float(batched_unavailable),
        cases_with_errors=float(len(failed_cases)),
        episode_coverage=len(episodes) / (len(roots) * repeats) if roots and repeats else 0.0,
    )
    if hp_changes:
        result["mean_hp_change_completed"] = sum(hp_changes) / len(hp_changes)
        result["mean_hp_lost_completed"] = -result["mean_hp_change_completed"]
    return result


def replay_storage_bytes(replays: list[ReplayRound]) -> int:
    """CPU bytes retained so the update can recompute without the rollout graph."""
    total = 0
    for replay in replays:
        tables = getattr(replay.observations, "tables", {})
        total += sum(table.nbytes for table in tables.values())
    return total


_MODEL_OWNER_WIDTHS = {
    "player_powers": 3,
    "hand": 18,
    "draw": 18,
    "discard": 18,
    "exhaust": 18,
    "selection_cards": 18,
    "enemies": 18,
    "relics": 2,
    "potions": 3,
    "selection_options": 2,
    "selected_slots": 2,
}
_ALIGNED_WIDTHS = {"player": 6, "selection": 1}
_GLOBAL_OWNER_PARENT = {"enemy_powers": "enemies", "stasis": "enemies", "relic_counters": "relics"}
_GLOBAL_OWNER_WIDTHS = {"enemy_powers": 3, "stasis": 18, "relic_counters": 3}


def _max_token_width(batch: NumericBatch) -> int:
    """Upper bound used only to group similar rounds. It is not a feature."""
    if batch.size == 0:
        return 1
    widths = np.full(batch.size, 3, dtype=np.int64)  # summary, player, selection context
    for name, width in _MODEL_OWNER_WIDTHS.items():
        rows = batch.table(name, width)
        if len(rows):
            widths += np.bincount(rows[:, 0], minlength=batch.size)
    return int(widths.max())


def _shape_bucket(replay: ReplayRound) -> tuple[int, int]:
    width = _max_token_width(replay.observations)
    actions = max(replay.counts) if replay.counts else 1
    return (1 << max(width - 1, 0).bit_length(), 1 << max(actions - 1, 0).bit_length())


def _stack_observations(batches: list[NumericBatch]) -> NumericBatch:
    """Concatenate model rows. Owner columns are shifted; hidden state is not copied in."""
    collected: dict[str, list[np.ndarray]] = {}
    model_offset = 0
    parent_offsets = {"enemies": 0, "relics": 0}
    for batch in batches:
        for name, width in _ALIGNED_WIDTHS.items():
            rows = batch.table(name, width)
            if len(rows) != batch.size:
                raise RuntimeError(f"{name} rows do not match model observations")
            collected.setdefault(name, []).append(rows)
        for name, width in _MODEL_OWNER_WIDTHS.items():
            rows = batch.table(name, width)
            if len(rows):
                rows = rows.copy()
                rows[:, 0] += model_offset
                collected.setdefault(name, []).append(rows)
        for name, parent in _GLOBAL_OWNER_PARENT.items():
            rows = batch.table(name, _GLOBAL_OWNER_WIDTHS[name])
            if len(rows):
                rows = rows.copy()
                rows[:, 0] += parent_offsets[parent]
                collected.setdefault(name, []).append(rows)
        parent_offsets["enemies"] += len(batch.table("enemies", 18))
        parent_offsets["relics"] += len(batch.table("relics", 2))
        model_offset += batch.size
    packed = {}
    for name, parts in collected.items():
        merged = np.concatenate(parts)
        packed[name] = (merged.shape[1], np.ascontiguousarray(merged).tobytes())
    return NumericBatch((NUMERIC_VERSION, [], packed, list(range(model_offset))))


def _stack_candidates(rounds: list[ReplayRound]) -> tuple[np.ndarray, list[int]]:
    pieces = []
    choices: list[int] = []
    offset = 0
    for replay in rounds:
        candidates = np.array(replay.candidates, copy=True)
        candidates[:, CANDIDATE_OWNER] += offset
        pieces.append(candidates)
        choices.extend(replay.choices)
        offset += len(replay.owners)
    return np.concatenate(pieces), choices


def _decision_rounds_from_forward(rounds: list[ReplayRound], logits: Tensor, values: Tensor) -> list[DecisionRound]:
    distribution = Categorical(logits=logits)
    choices = []
    for replay in rounds:
        choices.extend(replay.choices)
    chosen = torch.tensor(choices, dtype=torch.long, device=logits.device)
    log_probs = distribution.log_prob(chosen)
    entropies = distribution.entropy()
    max_probabilities = cast(Tensor, distribution.probs).max(dim=1).values.detach()
    flat_values = values.squeeze(-1)
    output = []
    offset = 0
    for replay in rounds:
        count = len(replay.owners)
        output.append(
            DecisionRound(
                replay.owners,
                replay.counts,
                log_probs[offset : offset + count],
                entropies[offset : offset + count],
                max_probabilities[offset : offset + count],
                flat_values[offset : offset + count],
            )
        )
        offset += count
    return output


def accumulate_replay_loss(
    model: CombatValueModel,
    replays: list[ReplayRound],
    episodes: list[Episode],
    entropy_coef: float,
    value_coef: float,
    chunk_decisions: int,
) -> tuple[Tensor, Tensor, Tensor] | None:
    """Recompute the existing objective in chunks and accumulate one gradient.

    Rounds inside a flush are grouped by token-width and action-count buckets and
    stacked into fewer forwards. A round is never split. Each group's loss is still
    divided by the completed-fight count, not by the group or chunk count. Padding,
    host repacking, and backward scheduling still matter; stacking is not free.
    Parameters stay fixed until the caller takes the optimizer step.
    """
    if chunk_decisions < 1:
        raise ValueError("chunk_decisions must be positive")
    completed = sum(episode.reward is not None for episode in episodes)
    if not completed or not replays:
        return None
    policy_total = None
    value_total = None
    loss_total = None
    chunk: list[ReplayRound] = []
    pending = 0

    def flush() -> None:
        nonlocal policy_total, value_total, loss_total, pending
        if not chunk:
            return
        grouped: dict[tuple[int, int], list[ReplayRound]] = {}
        for replay in chunk:
            grouped.setdefault(_shape_bucket(replay), []).append(replay)
        rounds = []
        for group in grouped.values():
            if len(group) == 1:
                replay = group[0]
                logits, values, _ = model(replay.observations, replay.candidates)
                rounds.extend(_decision_rounds_from_forward(group, logits, values))
            else:
                observations = _stack_observations([replay.observations for replay in group])
                candidates, _ = _stack_candidates(group)
                logits, values, _ = model(observations, candidates)
                rounds.extend(_decision_rounds_from_forward(group, logits, values))
        trajectories = Trajectories()
        trajectories.rounds = rounds
        loss, policy_loss = trajectories.losses(episodes, entropy_coef, value_coef=value_coef)
        if loss is None or policy_loss is None or trajectories.value_loss is None:
            chunk.clear()
            pending = 0
            return
        if not torch.isfinite(loss):
            raise RuntimeError("Non-finite loss")
        loss.backward()
        policy_total = policy_loss.detach() if policy_total is None else policy_total + policy_loss.detach()
        value_total = trajectories.value_loss if value_total is None else value_total + trajectories.value_loss
        loss_total = loss.detach() if loss_total is None else loss_total + loss.detach()
        chunk.clear()
        pending = 0

    for replay in replays:
        chunk.append(replay)
        pending += len(replay.owners)
        if pending >= chunk_decisions:
            flush()
    flush()
    if loss_total is None or policy_total is None or value_total is None:
        return None
    return loss_total, policy_total, value_total


def train_batch(
    roots: list[Root],
    model: CombatValueModel,
    optimizer: torch.optim.Optimizer,
    max_decisions: int,
    entropy_coef: float = 0.0,
    *,
    value_coef: float = 0.1,
    chunk_decisions: int = 0,
) -> dict[str, float]:
    """Collect one batch, calculate the batched loss, and take one optimizer step.

    ``chunk_decisions=0`` retains every rollout graph until one backward.
    A positive value rolls out under ``no_grad`` and recomputes gradients in chunks.
    Simulator errors propagate to the experiment's diagnostic/skip handler.
    """
    if chunk_decisions < 0:
        raise ValueError("chunk_decisions must be nonnegative")
    model.train()
    optimizer.zero_grad(set_to_none=True)
    trajectories = Trajectories()
    replays: list[ReplayRound] = []
    if chunk_decisions == 0:
        episodes = play_combats(
            [root.state for root in roots],
            model,
            max_decisions=max_decisions,
            training=True,
            rng=random.Random(0),
            trajectories=trajectories,
        )
        loss, policy_loss = trajectories.losses(episodes, entropy_coef, value_coef=value_coef)
        value_loss = None if trajectories.value_loss is None else trajectories.value_loss
    else:
        with torch.no_grad():
            episodes = play_combats(
                [root.state for root in roots],
                model,
                max_decisions=max_decisions,
                training=False,
                rng=random.Random(0),
                trajectories=trajectories,
                replays=replays,
            )
        accumulated = accumulate_replay_loss(model, replays, episodes, entropy_coef, value_coef, chunk_decisions)
        if accumulated is None:
            loss, policy_loss, value_loss = None, None, None
        else:
            loss, policy_loss, value_loss = accumulated
    result = episode_metrics(episodes)
    result.update(trajectories.metrics())
    result.update(simulator_error_batches=0.0, discarded_episodes=0.0, optimizer_step=float(loss is not None))
    result["replay_storage_mib"] = replay_storage_bytes(replays) / 2**20
    hp_losses = [root.start_hp - ep.hp for root, ep in zip(roots, episodes, strict=True) if ep.reward is not None]
    if hp_losses:
        result["mean_hp_lost_completed"] = sum(hp_losses) / len(hp_losses)
    if loss is not None:
        if not torch.isfinite(loss):
            raise RuntimeError("Non-finite loss")
        if chunk_decisions == 0:
            loss.backward()
        validate_gradients(model.parameters())
        optimizer.step()
        assert policy_loss is not None and value_loss is not None
        result["loss"] = loss.detach().item() if chunk_decisions == 0 else float(loss)
        result["policy_loss"] = policy_loss.detach().item() if chunk_decisions == 0 else float(policy_loss)
        result["value_loss"] = value_loss.item() if chunk_decisions == 0 else float(value_loss)
        result["entropy_bonus"] = result["policy_loss"] + value_coef * result["value_loss"] - result["loss"]
    return result


def evaluate_beam(
    roots: list[Root], *, width: int, max_decisions: int, max_transitions: int
) -> tuple[dict, list[dict]]:
    """Privileged deterministic reference, once per root; never silently discard failures."""
    if not roots:
        raise ValueError("Beam evaluation needs nonempty roots")
    records: list[dict] = []
    start = time.monotonic()
    for index, root in enumerate(roots):
        record: dict = {
            "root_index": index,
            "seed": root.combat_seed,
            "act": root.act,
            "floor": root.floor,
            "starting_hp": root.start_hp,
        }
        try:
            result = beam_search(root.state, width=width, max_decisions=max_decisions, max_transitions=max_transitions)
            record.update(
                reward=result.reward,
                hp=result.hp,
                won=result.won,
                transitions=result.transitions,
                limit_reached=result.limit_reached,
                status="completed" if result.reward is not None else "unfinished",
            )
            # Verify the reported terminal result against the actual accepted-action path.
            replay = root.state.clone()
            indices = []
            for action in result.actions:
                decision = replay.decision()
                choice = next(i for i in action_indices(decision) if repr(decision.actions[i]) == repr(action))
                replay.step(decision.actions[choice])
                indices.append(choice)
            record["action_indices"] = indices
            if result.reward is not None:
                starting_max_hp = root.state.decision().observation.context.player_max_hp
                if terminal_reward(replay.decision().observation, starting_max_hp) != result.reward:
                    raise RuntimeError("Beam plan replay reward mismatch")
                assert result.hp is not None
                record["hp_lost"] = root.start_hp - result.hp
        except Exception as error:
            logging.getLogger(__name__).exception("Beam evaluation failed for root %s", index)
            record.update(status="error", error=f"{type(error).__name__}: {error}")
        records.append(record)
        print(f"BEAM {index + 1}/{len(roots)} status={record['status']}", flush=True)
    completed = [record for record in records if record["status"] == "completed"]
    scores = {
        "roots": len(roots),
        "completed": len(completed),
        "unfinished": sum(record["status"] == "unfinished" for record in records),
        "errors": sum(record["status"] == "error" for record in records),
        "completion_rate": len(completed) / len(roots),
        "found_win_rate_all_roots": sum(record["won"] is True for record in completed) / len(roots),
        "transitions": sum(record.get("transitions", 0) for record in records),
        "limit_reached": sum(record.get("limit_reached", False) for record in records),
        "seconds": time.monotonic() - start,
    }
    if completed:
        scores.update(
            mean_return_completed=sum(record["reward"] for record in completed) / len(completed),
            mean_hp_completed=sum(record["hp"] for record in completed) / len(completed),
            mean_hp_lost_completed=sum(record["hp_lost"] for record in completed) / len(completed),
            win_rate_completed=sum(record["won"] is True for record in completed) / len(completed),
        )
    return scores, records


def cached_references(source: Path, output: Path, settings: dict) -> dict[str, float]:
    """Reuse only references with matching fixed inputs and evaluation budgets."""
    previous = json.loads((source / "config.json").read_text())
    for key in (
        "validation_sha256",
        "validation_native_sha256",
        "evaluation_repeats",
        "evaluation_max_decisions",
        "beam_width",
        "beam_transitions",
    ):
        if previous[key] != settings[key]:
            raise ValueError(f"Reference run mismatch: {key}")
    payload = (source / "baselines.json").read_bytes()
    document = json.loads(payload)
    if "main" not in document["sets"] or document.get("beam_privileged") is not True:
        raise ValueError("Incomplete reference cache")
    (output / "baselines.json").write_bytes(payload)
    return {
        f"{prefix}_main/{key}": value
        for prefix in ("random", "privileged_beam")
        for key, value in document["sets"]["main"][prefix].items()
    }


def evaluate_baselines(
    roots: list[Root],
    repeats: int,
    max_decisions: int,
    output: Path,
    *,
    beam_width: int,
    beam_transitions: int,
    eval_batch_size: int = 1,
) -> dict[str, float]:
    """Persist fixed random and privileged search references; never feed them to the policy."""
    references: dict[str, float] = {}
    records: dict = {
        "beam_privileged": True,
        "beam_width": beam_width,
        "beam_max_transitions_per_root": beam_transitions,
        "max_decisions": max_decisions,
        "random_repeats": repeats,
        "sets": {},
    }
    print("Computing random baseline: main", flush=True)
    random_scores = evaluate(
        roots,
        None,
        repeats,
        max_decisions,
        error_path=output / "errors" / "random-baseline" / "main.jsonl",
        batch_size=eval_batch_size,
    )
    print("Computing privileged beam reference: main", flush=True)
    beam_scores, beam_records = evaluate_beam(
        roots,
        width=beam_width,
        max_decisions=max_decisions,
        max_transitions=beam_transitions,
    )
    references.update({f"random_main/{key}": value for key, value in random_scores.items()})
    references.update({f"privileged_beam_main/{key}": value for key, value in beam_scores.items()})
    records["sets"]["main"] = {"random": random_scores, "privileged_beam": beam_scores, "beam_roots": beam_records}
    temporary = output / "baselines.tmp"
    temporary.write_text(json.dumps(records, indent=2))
    temporary.replace(output / "baselines.json")
    return references


def fresh_batch(
    rng: random.Random,
    sampler: LoadoutSampler,
    batch_size: int,
    config: ScenarioConfig,
    excluded_seeds: frozenset[int] = frozenset(),
) -> tuple[list[Root], list[SyntheticRoot]]:
    """Generate exactly batch_size fresh roots, without a finite training dataset."""
    if batch_size < 1:
        raise ValueError("batch_size must be positive")
    roots, specifications = [], []
    for _ in range(batch_size * 2):
        sampled = sample_root(rng, sampler, config=config)
        seed = json.loads(sampled.spec_json)["seed"]
        if seed in excluded_seeds:
            continue
        roots.append(
            Root(
                sampled.state,
                str(seed),
                sampled.encounter.floor,
                sampled.state.player_hp(),
                sampled.encounter.act,
            )
        )
        specifications.append(sampled)
        if len(roots) == batch_size:
            return roots, specifications
    raise RuntimeError("Could not sample a batch disjoint from validation seeds")


# Worker-process globals, set once by the initializer. Never used in the training process.
_WORKER_SAMPLER: LoadoutSampler | None = None


def _init_root_worker(distributions: str) -> None:
    global _WORKER_SAMPLER
    torch.set_num_threads(1)
    _WORKER_SAMPLER = LoadoutSampler.load(Path(distributions))


def _sample_batch_specifications(
    rng_state: tuple, batch_size: int, config: ScenarioConfig, excluded_seeds: frozenset[int]
) -> tuple[list[tuple], tuple]:
    """Run ``fresh_batch`` from ``rng_state`` in a worker; return picklable specs and the advanced state."""
    assert _WORKER_SAMPLER is not None
    rng = random.Random()
    rng.setstate(rng_state)
    _, specifications = fresh_batch(rng, _WORKER_SAMPLER, batch_size, config, excluded_seeds)
    payload = [(root.encounter, root.loadout, root.spec_json, root.rejected_loadouts) for root in specifications]
    return payload, rng.getstate()


class RootPrefetcher:
    """Sample the next ``fresh_batch`` in one worker process while the current update runs.

    The worker starts each batch from the training RNG's current state and returns the
    state after that batch, which the training RNG then adopts. The RNG therefore passes
    through exactly the same states as serial ``fresh_batch`` calls, and a checkpoint taken
    between updates never includes the prefetched batch. Native states are rebuilt here from
    each ``spec_json``, the same constructor ``sample_root`` used in the worker.
    """

    def __init__(
        self,
        rng: random.Random,
        distributions: Path,
        batch_size: int,
        config: ScenarioConfig,
        excluded_seeds: frozenset[int] = frozenset(),
    ) -> None:
        if batch_size < 1:
            raise ValueError("batch_size must be positive")
        self._rng = rng
        self._arguments = (batch_size, config, excluded_seeds)
        # spawn: forking a process that has initialized CUDA is unsafe.
        self._executor = ProcessPoolExecutor(
            max_workers=1,
            mp_context=multiprocessing.get_context("spawn"),
            initializer=_init_root_worker,
            initargs=(str(distributions),),
        )
        self._pending: Future = self._submit()

    def _submit(self) -> Future:
        return self._executor.submit(_sample_batch_specifications, self._rng.getstate(), *self._arguments)

    def next(self) -> tuple[list[Root], list[SyntheticRoot]]:
        """Return the batch serial ``fresh_batch(rng, ...)`` would return now, and prefetch the next."""
        payload, state = self._pending.result()
        self._rng.setstate(state)
        self._pending = self._submit()
        roots, specifications = [], []
        for encounter, loadout, spec_json, rejected in payload:
            native = State.from_synthetic_spec(spec_json)
            specifications.append(SyntheticRoot(native, encounter, loadout, spec_json, rejected))
            seed = json.loads(spec_json)["seed"]
            roots.append(Root(native, str(seed), encounter.floor, native.player_hp(), encounter.act))
        return roots, specifications

    def close(self) -> None:
        """Discard any prefetched batch; the training RNG has not consumed it."""
        self._pending.cancel()
        self._executor.shutdown(wait=True, cancel_futures=True)


def update_with_diagnostics(
    roots: list[Root],
    specifications: list[SyntheticRoot],
    model: CombatValueModel,
    optimizer: torch.optim.Optimizer,
    max_decisions: int,
    entropy_coef: float,
    failure_path: Path,
    *,
    continue_on_error: bool = False,
    value_coef: float = 0.1,
    chunk_decisions: int = 0,
) -> dict[str, float]:
    """Never retry partially advanced states; preserve failed batch inputs for diagnosis."""
    try:
        return train_batch(
            roots,
            model,
            optimizer,
            max_decisions,
            entropy_coef,
            value_coef=value_coef,
            chunk_decisions=chunk_decisions,
        )
    except SimulatorStepError as error:
        failure_path.parent.mkdir(parents=True, exist_ok=True)
        with failure_path.open("x") as handle:
            json.dump(
                {
                    "error": str(error),
                    "step": error.step,
                    "specifications": [json.loads(root.spec_json) for root in specifications],
                    "root_indices": error.root_indices,
                    "attempted_prefixes": error.action_prefixes,
                    "note": "Last attempted action may not have been accepted. Reconstruct NEW roots; never retry clones.",
                },
                handle,
                indent=2,
            )
        logging.getLogger(__name__).critical(
            "SIMULATOR FAILURE: discarded %d episodes, no optimizer step. %s", len(roots), failure_path
        )
        if not continue_on_error:
            raise
        return {
            "episodes": float(len(roots)),
            "optimizer_step": 0.0,
            "simulator_error_batches": 1.0,
            "discarded_episodes": float(len(roots)),
        }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--distributions", type=Path, required=True)
    parser.add_argument("--validation-manifest", type=Path, required=True)
    parser.add_argument("--updates", type=int, default=10000)
    parser.add_argument("--max-hours", type=float, help="Stop after this training/validation wall-clock budget")
    parser.add_argument("--batch-size", type=int, default=2048)
    parser.add_argument("--model-width", type=int, default=64, help="Transformer and action embedding width")
    parser.add_argument("--model-layers", type=int, default=2, help="Transformer layer count (four attention heads)")
    parser.add_argument("--eval-every", type=int, default=100)
    parser.add_argument(
        "--eval-batch-size",
        type=int,
        default=64,
        help="Policy/random evaluation batch. 1 keeps the historical single-row protocol",
    )
    parser.add_argument("--value-coef", type=float, default=0.1)
    parser.add_argument(
        "--grad-chunk-decisions",
        type=int,
        default=0,
        help=(
            "Flush a backward after at least this many decisions. Rounds are not split, "
            "so one large round can exceed it. 0 retains the rollout graph"
        ),
    )
    parser.add_argument("--reference-run", type=Path)
    initialization = parser.add_mutually_exclusive_group()
    initialization.add_argument("--warm-start", type=Path, help="Trusted checkpoint: weights only; new optimizer/RNG")
    initialization.add_argument(
        "--resume-from", type=Path, help="Trusted same-protocol checkpoint: restore optimizer/RNG"
    )
    parser.add_argument("--beam-width", type=int, default=64)
    parser.add_argument("--beam-transitions", type=int, default=10000)
    parser.add_argument("--max-decisions", type=int, default=512)
    parser.add_argument("--min-floor", type=int, default=1)
    parser.add_argument("--max-floor", type=int, default=55)
    parser.add_argument("--seed", type=int, default=123)
    parser.add_argument("--lr", type=float, default=1e-4)
    parser.add_argument("--entropy-coef", type=float, default=0.01)
    parser.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    parser.add_argument("--continue-on-simulator-error", action="store_true")
    parser.add_argument("--wandb-project", default="sts-combat-v1")
    parser.add_argument("--wandb-mode", choices=("online", "offline", "disabled"), default="online")
    args = parser.parse_args()
    if (
        min(
            args.updates,
            args.batch_size,
            args.eval_every,
            args.eval_batch_size,
            args.max_decisions,
            args.beam_width,
            args.beam_transitions,
        )
        < 1
    ):
        parser.error("Counts must be positive")
    if not math.isfinite(args.lr) or args.lr <= 0 or not math.isfinite(args.entropy_coef) or args.entropy_coef < 0:
        parser.error("Expected positive finite learning rate and nonnegative finite entropy coefficient")
    if not math.isfinite(args.value_coef) or args.value_coef < 0:
        parser.error("Value coefficient must be finite and nonnegative")
    if args.model_width < 4 or args.model_width % 4 or args.model_layers < 1:
        parser.error("Model width must be a positive multiple of four; layers must be positive")
    if args.grad_chunk_decisions < 0:
        parser.error("Gradient chunk size must be nonnegative")
    if args.max_hours is not None and (not math.isfinite(args.max_hours) or args.max_hours <= 0):
        parser.error("Maximum hours must be finite and positive")
    config = ScenarioConfig(min_floor=args.min_floor, max_floor=args.max_floor)
    if args.device == "cuda" and not torch.cuda.is_available():
        parser.error("CUDA is unavailable")
    output = Path("wandb") / args.run_id
    output.mkdir(parents=True, exist_ok=False)
    validation_bytes = args.validation_manifest.read_bytes()
    document, groups = load_validation(args.validation_manifest)
    # Fail before references/validation if unusable; batches are sampled by RootPrefetcher's worker.
    LoadoutSampler.load(args.distributions)
    (output / "validation.json").write_bytes(validation_bytes)
    validation = groups["main"]
    repeats = document["evaluation"]["repeats"]
    eval_limit = document["evaluation"]["max_decisions"]
    # Keep ALL frozen seeds held out, even cases no longer evaluated by this trainer.
    excluded = frozenset(int(root.combat_seed) for roots in groups.values() for root in roots)
    settings = {key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()}
    settings.update(
        validation_sha256=hashlib.sha256(validation_bytes).hexdigest(),
        distributions_sha256=hashlib.sha256(args.distributions.read_bytes()).hexdigest(),
        validation_native_sha256=document["native_sha256"],
        evaluation_repeats=repeats,
        evaluation_max_decisions=eval_limit,
        validation_roots=len(validation),
        validation_subset="main",
        validation_acts=sorted({root.act for root in validation}),
        training_protocol="fresh_independent_A0_roots_per_update",
        validation_protocol=document["protocol"],
        evaluation_protocol=(
            "serial_per_episode_seed" if args.eval_batch_size == 1 else "batched_forward_per_episode_rng_v1"
        ),
    )
    (output / "config.json").write_text(json.dumps(settings, indent=2))
    torch.set_num_threads(1)
    torch.manual_seed(args.seed)
    rng = random.Random(args.seed)
    model = CombatValueModel(d_model=args.model_width, action_dim=args.model_width, n_layers=args.model_layers).to(
        args.device
    )
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr)
    initial_checkpoint = args.resume_from or args.warm_start
    if initial_checkpoint is not None:
        # Always use a new output directory. Cross-native warm starts load weights
        # only; optimizer/RNG continuation requires the same gameplay/training contract.
        source = torch.load(initial_checkpoint, map_location="cpu", weights_only=False)
        if args.resume_from is not None:
            for key, default in (("model_width", 64), ("model_layers", 2)):
                if source["config"].get(key, default) != settings[key]:
                    raise ValueError(f"Checkpoint continuation mismatch: {key}")
            for key in (
                "validation_sha256",
                "validation_native_sha256",
                "distributions_sha256",
                "training_protocol",
                "min_floor",
                "max_floor",
                "max_decisions",
                "lr",
                "entropy_coef",
                "value_coef",
                "device",
            ):
                if source["config"].get(key) != settings[key]:
                    raise ValueError(f"Checkpoint continuation mismatch: {key}")
        weights = source["model"]
        if not all(torch.isfinite(value).all() for value in weights.values()):
            raise ValueError("Warm-start checkpoint contains non-finite weights")
        model.load_state_dict(weights, strict=True)
        if args.resume_from is not None:
            optimizer.load_state_dict(source["optimizer"])
            rng.setstate(source["sampling_rng"])
            torch.set_rng_state(source["torch_rng"])
            if args.device == "cuda":
                torch.cuda.set_rng_state_all(source["cuda_rng"])
        settings["initialization"] = {
            "mode": "optimizer_rng_continuation" if args.resume_from else "weights_only_new_optimizer_and_rng",
            "checkpoint_sha256": hashlib.sha256(initial_checkpoint.read_bytes()).hexdigest(),
            "source_iteration": source["iteration"],
            "source_native_sha256": source["config"].get("validation_native_sha256"),
        }
        del source, weights
        (output / "config.json").write_text(json.dumps(settings, indent=2))
        print(f"Initialized from {initial_checkpoint}: {settings['initialization']['mode']}", flush=True)
    iteration = 0

    def checkpoint() -> None:
        temporary = output / "latest.tmp"
        torch.save(
            {
                "model": model.state_dict(),
                "optimizer": optimizer.state_dict(),
                "iteration": iteration,
                "sampling_rng": rng.getstate(),
                "torch_rng": torch.get_rng_state(),
                "cuda_rng": torch.cuda.get_rng_state_all() if args.device == "cuda" else [],
                "config": settings,
            },
            temporary,
        )
        temporary.replace(output / "latest.pt")
        archived = output / f"checkpoint-{iteration:08d}.pt"
        if not archived.exists():
            shutil.copyfile(output / "latest.pt", archived)

    with wandb.init(
        project=args.wandb_project, id=args.run_id, name=args.run_id, config=settings, mode=args.wandb_mode
    ) as run:
        # The training RNG is final here (after any resume); the first batch overlaps references/validation.
        prefetcher = RootPrefetcher(rng, args.distributions, args.batch_size, config, excluded)
        try:
            if args.reference_run is not None:
                references = cached_references(args.reference_run, output, settings)
                print(f"Loaded matching random/privileged references from {args.reference_run}", flush=True)
            else:
                references = evaluate_baselines(
                    validation,
                    repeats,
                    eval_limit,
                    output,
                    beam_width=args.beam_width,
                    beam_transitions=args.beam_transitions,
                    eval_batch_size=args.eval_batch_size,
                )
            run.log(references, step=0, commit=False)
            print("Starting fixed initial validation", flush=True)
            validation_scores = evaluate(
                validation,
                model,
                repeats,
                eval_limit,
                error_path=output / "errors" / "validation-00000000.jsonl",
                batch_size=args.eval_batch_size,
            )
            run.log({f"val_main/{key}": value for key, value in validation_scores.items()}, step=0)
            checkpoint()
            print("Initial validation logged; starting training updates", flush=True)
            deadline = time.monotonic() + args.max_hours * 3600 if args.max_hours is not None else math.inf
            for iteration in range(1, args.updates + 1):
                started = time.monotonic()
                roots, specs = prefetcher.next()
                sampling_seconds = time.monotonic() - started
                scores = update_with_diagnostics(
                    roots,
                    specs,
                    model,
                    optimizer,
                    args.max_decisions,
                    args.entropy_coef,
                    output / "errors" / f"update-{iteration:08d}.json",
                    continue_on_error=args.continue_on_simulator_error,
                    value_coef=args.value_coef,
                    chunk_decisions=args.grad_chunk_decisions,
                )
                if args.device == "cuda":
                    scores["peak_cuda_allocated_gib"] = torch.cuda.max_memory_allocated() / 2**30
                    scores["peak_cuda_reserved_gib"] = torch.cuda.max_memory_reserved() / 2**30
                scores.update(
                    sampling_seconds=sampling_seconds,
                    rejected_loadouts=float(sum(len(root.rejected_loadouts) for root in specs)),
                    update_seconds=time.monotonic() - started,
                )
                logs = {**references, **{f"train/{key}": value for key, value in scores.items()}}
                finished = iteration == args.updates or time.monotonic() >= deadline
                if iteration % args.eval_every == 0 or finished:
                    validation_scores = evaluate(
                        validation,
                        model,
                        repeats,
                        eval_limit,
                        error_path=output / "errors" / f"validation-{iteration:08d}.jsonl",
                        batch_size=args.eval_batch_size,
                    )
                    logs.update({f"val_main/{key}": value for key, value in validation_scores.items()})
                    checkpoint()
                run.log(logs, step=iteration)
                print(f"update={iteration} roots={len(roots)} optimizer_step={scores['optimizer_step']}", flush=True)
                if finished or time.monotonic() >= deadline:
                    print("Training budget reached; saving final checkpoint", flush=True)
                    break
        finally:
            prefetcher.close()
            checkpoint()


if __name__ == "__main__":
    main()
