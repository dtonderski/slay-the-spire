"""Deterministic simulator self-play trace generation and replay."""

from __future__ import annotations

from dataclasses import dataclass, replace
from datetime import datetime, timezone
import argparse
import hashlib
import json
import random
import sys
from pathlib import Path
from typing import Any, Iterable, TextIO

from sts import omni
from sts.search import CombatSearchConfig, search_combat
from sts.search_lab import (
    BenchmarkRoot,
    SELECTED_COMBAT_AUTOPILOT_CANDIDATE,
    SearchCandidate,
    evaluate_candidate,
    trace_autopilot_candidate_by_name,
    trace_autopilot_candidates,
)


DEFAULT_COMBAT_POLICY_NAME = SELECTED_COMBAT_AUTOPILOT_CANDIDATE
DEFAULT_COMBAT_POLICY = trace_autopilot_candidate_by_name(DEFAULT_COMBAT_POLICY_NAME).config


TRACE_EVAL_SET_SPECS: dict[str, dict[str, Any]] = {
    "dev-fast-10": {
        "description": "Fast iteration set: first 10 combat-start roots.",
        "root_scope": "combat_start",
        "split": "all",
        "max_roots": 10,
        "held_out": False,
    },
    "dev-50": {
        "description": "Candidate-selection set: first 50 non-held-out combat-start roots.",
        "root_scope": "combat_start",
        "split": "dev",
        "max_roots": 50,
        "held_out": False,
    },
    "val-50": {
        "description": "Held-out validation set: first 50 held-out combat-start roots.",
        "root_scope": "combat_start",
        "split": "eval",
        "max_roots": 50,
        "held_out": True,
    },
    "full-323": {
        "description": "Coverage sanity set: all distinct usable roots from the long MANUAL01 replay.",
        "root_scope": "all",
        "split": "all",
        "max_roots": 323,
        "held_out": True,
    },
}


@dataclass(frozen=True)
class SelfPlayResult:
    trace_path: Path | None
    steps: int
    stop_reason: str
    final_phase: str | None
    verified: bool


@dataclass(frozen=True)
class CorpusBatchResult:
    output_dir: Path
    index_path: Path
    trace_count: int
    verified_count: int
    stop_reasons: dict[str, int]


@dataclass(frozen=True)
class TraceGuidedReplayResult:
    trace_path: Path
    output_path: Path
    report_path: Path | None
    steps: int
    combat_roots: int
    stop_reason: str
    verified: bool


@dataclass(frozen=True)
class TraceCombatRoot:
    trace_path: Path
    step: int
    state_id: str
    snapshot_json: str
    split: str
    potion_count: int
    potion_names: tuple[str, ...]
    legal_action_kinds: tuple[str, ...]
    legal_potion_names: tuple[str, ...]
    real_trace_final_hp: float | None = None
    real_trace_hp_loss: float | None = None
    real_trace_terminal_phase: str | None = None


def run_self_play(
    *,
    output: Path,
    seed: str | None = None,
    ascension: int = 0,
    start: str = "seed",
    random_seed: int = 0,
    max_steps: int = 200,
    combat_policy: CombatSearchConfig = DEFAULT_COMBAT_POLICY,
) -> SelfPlayResult:
    """Run one simulator self-play episode and write a replayable JSONL trace."""

    rng = random.Random(random_seed)
    output.parent.mkdir(parents=True, exist_ok=True)
    started_at = _now()

    try:
        env = _start_env(start=start, seed=seed, ascension=ascension)
    except Exception as error:
        _write_jsonl(
            output,
            [
                _metadata(
                    started_at=started_at,
                    seed=seed,
                    ascension=ascension,
                    start=start,
                    random_seed=random_seed,
                    max_steps=max_steps,
                    combat_policy=combat_policy,
                    stop_reason=f"unsupported_start: {error}",
                )
            ],
        )
        return SelfPlayResult(output, 0, f"unsupported_start: {error}", None, False)

    records: list[dict[str, Any]] = [
        _metadata(
            started_at=started_at,
            seed=seed,
            ascension=ascension,
            start=start,
            random_seed=random_seed,
            max_steps=max_steps,
            combat_policy=combat_policy,
            initial_snapshot_json=env.snapshot_json(),
            initial_state_id=env.snapshot_hash(),
        )
    ]
    stop_reason = "max_steps"
    steps = 0

    for step in range(max_steps):
        before_hash = env.snapshot_hash()
        actions = env.exact_legal_actions()
        if not actions:
            unsupported = env.unsupported_reason()
            stop_reason = f"unsupported_transition: {unsupported}" if unsupported else "no_legal_actions"
            break

        try:
            action, policy_name, policy_diagnostics = _choose_action(
                env,
                actions,
                rng,
                combat_policy,
            )
        except Exception as error:
            stop_reason = f"policy_error: {error}"
            break

        before_snapshot = env.snapshot_json()
        before_summary = _summary(env)
        try:
            result = env.step(action)
        except Exception as error:
            records.append(
                _step_record(
                    step=step,
                    before_hash=before_hash,
                    before_snapshot_json=before_snapshot,
                    before_summary=before_summary,
                    legal_actions=actions,
                    action=action,
                    policy_name=policy_name,
                    policy_diagnostics=policy_diagnostics,
                    error=f"step_error: {error}",
                )
            )
            stop_reason = f"step_error: {error}"
            break

        records.append(
            _step_record(
                step=step,
                before_hash=before_hash,
                before_snapshot_json=before_snapshot,
                before_summary=before_summary,
                legal_actions=actions,
                action=action,
                policy_name=policy_name,
                policy_diagnostics=policy_diagnostics,
                after_hash=result.snapshot_hash,
                after_snapshot_json=result.snapshot_json,
                after_summary=_summary(env),
                transition=result.transition,
                unsupported_reason=result.unsupported_reason,
            )
        )
        steps += 1

        if result.unsupported_reason and not env.exact_legal_actions():
            stop_reason = f"unsupported_transition: {result.unsupported_reason}"
            break
        phase = env.phase()
        if phase in {"won", "lost"}:
            stop_reason = phase
            break
    else:
        stop_reason = "max_steps"

    records[0]["ended_at"] = _now()
    records[0]["stop_reason"] = stop_reason
    records[0]["steps"] = steps
    records[0]["final_phase"] = env.phase()
    records[0]["final_state_id"] = env.snapshot_hash()
    records[0]["final_summary"] = _summary(env)
    _write_jsonl(output, records)
    verification = verify_self_play_trace(output)
    return SelfPlayResult(output, steps, stop_reason, env.phase(), verification["ok"])


def verify_self_play_trace(path: Path) -> dict[str, Any]:
    """Replay a generated self-play trace from its initial snapshot."""

    records = _read_jsonl(path)
    if not records:
        return {"ok": False, "error": "empty trace", "steps": 0}
    metadata = records[0]
    if metadata.get("type") != "metadata":
        return {"ok": False, "error": "first record is not metadata", "steps": 0}
    initial_snapshot = metadata.get("initial_snapshot_json")
    if not isinstance(initial_snapshot, str):
        return {"ok": False, "error": "metadata has no initial_snapshot_json", "steps": 0}

    env = omni.OmniRunEnv.from_snapshot_json(initial_snapshot)
    steps = 0
    for record in records[1:]:
        if record.get("type") == "anchor":
            snapshot = record.get("snapshot_json")
            if not isinstance(snapshot, str):
                return {"ok": False, "error": "anchor has no snapshot_json", "steps": steps}
            env = omni.OmniRunEnv.from_snapshot_json(snapshot)
            expected_hash = record.get("snapshot_hash")
            if expected_hash and env.snapshot_hash() != expected_hash:
                return {
                    "ok": False,
                    "error": "anchor hash mismatch",
                    "step": record.get("step"),
                    "expected": expected_hash,
                    "actual": env.snapshot_hash(),
                }
            continue
        if record.get("type") != "step":
            continue
        before_hash = record.get("before_hash")
        if env.snapshot_hash() != before_hash:
            return {
                "ok": False,
                "error": "before hash mismatch",
                "step": record.get("step"),
                "expected": before_hash,
                "actual": env.snapshot_hash(),
            }
        action_json = record.get("action_json")
        action = _find_action(env.exact_legal_actions(), action_json)
        if action is None:
            return {
                "ok": False,
                "error": "action not legal during replay",
                "step": record.get("step"),
                "action_json": action_json,
            }
        result = env.step(action)
        after_hash = record.get("after_hash")
        if after_hash and result.snapshot_hash != after_hash:
            return {
                "ok": False,
                "error": "after hash mismatch",
                "step": record.get("step"),
                "expected": after_hash,
                "actual": result.snapshot_hash,
            }
        steps += 1

    return {
        "ok": True,
        "steps": steps,
        "final_state_id": env.snapshot_hash(),
        "final_phase": env.phase(),
        "metadata_stop_reason": metadata.get("stop_reason"),
    }


def run_self_play_batch(
    *,
    output_dir: Path,
    seeds: Iterable[str],
    ascension: int = 0,
    start: str = "seed",
    random_seed: int = 0,
    max_steps: int = 200,
    combat_policy: CombatSearchConfig = DEFAULT_COMBAT_POLICY,
) -> CorpusBatchResult:
    """Generate a verified simulator-only self-play corpus and an index file."""

    output_dir.mkdir(parents=True, exist_ok=True)
    traces_dir = output_dir / "traces"
    traces_dir.mkdir(parents=True, exist_ok=True)

    trace_summaries = []
    stop_reasons: dict[str, int] = {}
    verified_count = 0
    for index, seed in enumerate(seeds):
        episode_random_seed = random_seed + index
        trace_path = traces_dir / f"seed-{_safe_file_stem(seed)}-rng-{episode_random_seed}.jsonl"
        result = run_self_play(
            output=trace_path,
            seed=seed,
            ascension=ascension,
            start=start,
            random_seed=episode_random_seed,
            max_steps=max_steps,
            combat_policy=combat_policy,
        )
        verification = verify_self_play_trace(trace_path)
        records = _read_jsonl(trace_path)
        summary = _trace_summary(
            trace_path=trace_path,
            records=records,
            result=result,
            verification=verification,
        )
        trace_summaries.append(summary)
        if result.verified and verification.get("ok"):
            verified_count += 1
        stop_reason = result.stop_reason
        stop_reasons[stop_reason] = stop_reasons.get(stop_reason, 0) + 1

    index_doc = {
        "type": "self_play_corpus_index",
        "schema": 1,
        "source": "sim_selfplay_corpus",
        "parity": "non_parity_simulator_only",
        "created_at": _now(),
        "start": start,
        "ascension": ascension,
        "random_seed": random_seed,
        "max_steps": max_steps,
        "combat_policy": combat_policy.__dict__,
        "trace_count": len(trace_summaries),
        "verified_count": verified_count,
        "stop_reasons": stop_reasons,
        "traces": trace_summaries,
    }
    index_path = output_dir / "index.json"
    index_path.write_text(json.dumps(index_doc, indent=2, sort_keys=True), encoding="utf-8")
    return CorpusBatchResult(output_dir, index_path, len(trace_summaries), verified_count, stop_reasons)


def replay_real_trace_guided(
    *,
    trace: Path,
    output: Path,
    report_output: Path | None = None,
    max_actions: int = 10_000,
) -> TraceGuidedReplayResult:
    """Replay a CommunicationMod trace through OmniRunEnv until the first hard boundary."""

    records = _read_jsonl(trace)
    output.parent.mkdir(parents=True, exist_ok=True)
    if report_output:
        report_output.parent.mkdir(parents=True, exist_ok=True)

    started_at = _now()
    env = None
    last_state: dict[str, Any] | None = None
    output_records: list[dict[str, Any]] = []
    replayed_steps = 0
    combat_roots = 0
    combat_root_state_ids: set[str] = set()
    anchor_count = 0
    restoration_count = 0
    skipped_noncombat_actions = 0
    restorations: list[dict[str, Any]] = []
    stop_reason = "trace_exhausted"
    blocker: dict[str, Any] | None = None
    start_info: dict[str, Any] | None = None

    for record in records:
        if record.get("type") == "state":
            last_state = record
            continue
        if record.get("type") != "action":
            continue
        command = str(record.get("command") or "")
        if not command:
            continue

        if env is None:
            start_info = _parse_start_command(command)
            if start_info is None:
                blocker = _blocker(record, command, "missing_start", "first action is not START")
                stop_reason = "missing_start"
                break
            try:
                env = omni.OmniRunEnv.new_ironclad(
                    seed=start_info["external_seed"],
                    ascension=start_info["ascension"],
                )
            except Exception as error:
                blocker = _blocker(record, command, "unsupported_start", str(error))
                stop_reason = f"unsupported_start: {error}"
                break
            metadata = _metadata(
                started_at=started_at,
                seed=start_info["external_seed"],
                ascension=start_info["ascension"],
                start="communication_mod_trace_guided",
                random_seed=0,
                max_steps=max_actions,
                combat_policy=DEFAULT_COMBAT_POLICY,
                initial_snapshot_json=env.snapshot_json(),
                initial_state_id=env.snapshot_hash(),
            )
            metadata["source"] = "sim_trace_guided_replay"
            metadata["parity"] = "trace_guided_until_first_boundary"
            metadata["source_trace"] = str(trace)
            metadata["start_command"] = command
            metadata["numeric_seed"] = start_info.get("numeric_seed")
            output_records.append(metadata)
            continue

        if replayed_steps >= max_actions:
            stop_reason = "max_actions"
            break

        observed_game_state = _game_state_from_record(last_state)
        diffs = _observed_summary_diffs(env, observed_game_state)
        if diffs:
            if _observed_summary(observed_game_state).get("phase") == "combat":
                try:
                    env = omni.OmniRunEnv.from_communication_mod_state_json(
                        json.dumps(observed_game_state)
                    )
                except Exception as error:
                    blocker = _blocker(
                        record,
                        command,
                        "observed_combat_anchor_failed",
                        str(error),
                        diffs=diffs,
                        simulator_summary=_summary(env),
                        observed_summary=_observed_summary(observed_game_state),
                    )
                    stop_reason = "observed_combat_anchor_failed"
                    break
                anchor_count += 1
                output_records.append(
                    _anchor_record(
                        step=replayed_steps,
                        command=command,
                        trace_step=record.get("step"),
                        env=env,
                        observed_summary=_observed_summary(observed_game_state),
                        diffs=diffs,
                    )
                )
                anchor_hash = env.snapshot_hash()
                if anchor_hash not in combat_root_state_ids:
                    combat_root_state_ids.add(anchor_hash)
                    combat_roots += 1
                diffs = _observed_summary_diffs(env, observed_game_state)
                if diffs:
                    blocker = _blocker(
                        record,
                        command,
                        "observed_combat_anchor_divergence",
                        "observed combat anchor still differs from simulator summary",
                        diffs=diffs,
                        simulator_summary=_summary(env),
                        observed_summary=_observed_summary(observed_game_state),
                    )
                    stop_reason = "observed_combat_anchor_divergence"
                    break
            else:
                skipped_noncombat_actions += 1
                continue

        actions = env.exact_legal_actions()
        action = _action_for_communication_command(env, command, observed_game_state)
        if action is None:
            if _observed_summary(observed_game_state).get("phase") != "combat":
                skipped_noncombat_actions += 1
                continue
            restoration = _restoration_record(
                step=replayed_steps,
                record=record,
                command=command,
                category="unsupported_command_mapping",
                reason="no exact OmniRunEnv action mapping for CommunicationMod command",
                simulator_phase=env.phase(),
                simulator_decision=env.current_decision(),
                legal_actions=[_action_record(candidate) for candidate in actions],
                observed_summary=_observed_summary(observed_game_state),
            )
            restorations.append(restoration)
            output_records.append(restoration)
            restoration_count += 1
            continue

        before_hash = env.snapshot_hash()
        before_snapshot = env.snapshot_json()
        before_summary = _summary(env)
        if before_summary.get("phase") == "combat":
            if before_hash not in combat_root_state_ids:
                combat_root_state_ids.add(before_hash)
                combat_roots += 1

        try:
            result = env.step(action)
        except Exception as error:
            output_records.append(
                _step_record(
                    step=replayed_steps,
                    before_hash=before_hash,
                    before_snapshot_json=before_snapshot,
                    before_summary=before_summary,
                    legal_actions=actions,
                    action=action,
                    policy_name="communication_mod_trace_guided",
                    policy_diagnostics={"command": command, "trace_step": record.get("step")},
                    error=f"step_error: {error}",
                )
            )
            restoration = _restoration_record(
                step=replayed_steps,
                record=record,
                command=command,
                category="step_error",
                reason=str(error),
                observed_summary=_observed_summary(observed_game_state),
            )
            restorations.append(restoration)
            output_records.append(restoration)
            restoration_count += 1
            continue

        output_records.append(
            _step_record(
                step=replayed_steps,
                before_hash=before_hash,
                before_snapshot_json=before_snapshot,
                before_summary=before_summary,
                legal_actions=actions,
                action=action,
                policy_name="communication_mod_trace_guided",
                policy_diagnostics={"command": command, "trace_step": record.get("step")},
                after_hash=result.snapshot_hash,
                after_snapshot_json=result.snapshot_json,
                after_summary=_summary(env),
                transition=result.transition,
                unsupported_reason=result.unsupported_reason,
            )
        )
        replayed_steps += 1

    if not output_records:
        output_records.append(
            {
                "type": "metadata",
                "schema": 1,
                "source": "sim_trace_guided_replay",
                "started_at": started_at,
                "source_trace": str(trace),
                "stop_reason": stop_reason,
            }
        )

    output_records[0]["ended_at"] = _now()
    output_records[0]["stop_reason"] = stop_reason
    output_records[0]["steps"] = replayed_steps
    output_records[0]["combat_roots"] = combat_roots
    output_records[0]["anchor_count"] = anchor_count
    output_records[0]["restoration_count"] = restoration_count
    output_records[0]["skipped_noncombat_actions"] = skipped_noncombat_actions
    output_records[0]["final_phase"] = env.phase() if env is not None else None
    output_records[0]["final_state_id"] = env.snapshot_hash() if env is not None else None
    output_records[0]["blocker"] = blocker
    _write_jsonl(output, output_records)

    verification = (
        verify_self_play_trace(output)
        if output_records[0].get("initial_snapshot_json")
        else {"ok": False, "error": "no initial snapshot"}
    )
    report = {
        "schema": 1,
        "source": "sim_trace_guided_replay_report",
        "source_trace": str(trace),
        "output_trace": str(output),
        "report_path": str(report_output) if report_output else None,
        "start": start_info,
        "steps": replayed_steps,
        "combat_roots": combat_roots,
        "anchor_count": anchor_count,
        "restoration_count": restoration_count,
        "restorations": restorations,
        "skipped_noncombat_actions": skipped_noncombat_actions,
        "stop_reason": stop_reason,
        "blocker": blocker,
        "verified": verification.get("ok", False),
        "verification": verification,
    }
    if report_output:
        report_output.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")

    return TraceGuidedReplayResult(
        trace_path=trace,
        output_path=output,
        report_path=report_output,
        steps=replayed_steps,
        combat_roots=combat_roots,
        stop_reason=stop_reason,
        verified=bool(verification.get("ok")),
    )


def evaluate_self_play_corpus(
    *,
    corpus_dir: Path | None = None,
    traces: Iterable[Path] | None = None,
    split: str = "all",
    max_roots: int = 64,
    max_actions: int = 40,
    candidates: Iterable[SearchCandidate] | None = None,
    allowed_potions: tuple[str, ...] | None = None,
    root_scope: str = "all",
    failure_output: Path | None = None,
    eval_set: str | None = None,
    progress_every: int = 0,
    progress_stream: TextIO | None = None,
) -> dict[str, Any]:
    """Compare search candidates from exact combat states recorded in traces."""

    eval_set_spec = _trace_eval_set_spec(eval_set)
    if eval_set_spec is not None:
        split = str(eval_set_spec["split"])
        max_roots = int(eval_set_spec["max_roots"])
        root_scope = str(eval_set_spec["root_scope"])

    trace_paths = _trace_paths(corpus_dir=corpus_dir, traces=traces)
    extraction_report = real_trace_root_report(trace_paths)
    roots = _trace_combat_roots(trace_paths, root_scope=root_scope)
    if split != "all":
        roots = [root for root in roots if root.split == split]
    available_roots = len(roots)
    roots = roots[:max_roots]
    candidates = [
        _candidate_with_allowed_potions(candidate, allowed_potions)
        for candidate in list(candidates or trace_autopilot_candidates())
    ]

    episodes = []
    completed = 0
    total = len(candidates) * len(roots)
    for candidate in candidates:
        for root in roots:
            benchmark_root = BenchmarkRoot(
                name=f"{root.trace_path.stem}:step{root.step}:{root.state_id}",
                env_kind="run_combat",
                snapshot_json=root.snapshot_json,
                state_id=root.state_id,
                source_depth=root.step,
                split=root.split,
            )
            result = evaluate_candidate(benchmark_root, candidate, max_actions=max_actions)
            row = result.__dict__ | {
                "trace_path": str(root.trace_path),
                "trace_step": root.step,
                "state_id": root.state_id,
                "potion_count": root.potion_count,
                "potion_names": list(root.potion_names),
                "legal_action_kinds": list(root.legal_action_kinds),
                "legal_potion_names": list(root.legal_potion_names),
                "has_potion_actions": any("potion" in kind for kind in root.legal_action_kinds),
                "has_allowed_potion_actions": _has_allowed_potion_actions(root, allowed_potions),
                "real_trace_final_hp": root.real_trace_final_hp,
                "real_trace_hp_loss": root.real_trace_hp_loss,
                "real_trace_terminal_phase": root.real_trace_terminal_phase,
                "hp_loss_delta_vs_trace": (
                    result.hp_loss - root.real_trace_hp_loss
                    if root.real_trace_hp_loss is not None
                    else None
                ),
            }
            episodes.append(row)
            completed += 1
            if progress_every > 0 and progress_stream is not None:
                if completed == total or completed % progress_every == 0:
                    print(
                        (
                            f"eval progress {completed}/{total}: "
                            f"{candidate.name} {root.trace_path.name}:step{root.step} "
                            f"won={result.won} lost={result.lost} "
                            f"terminal={result.terminal_reason or 'nonterminal'} "
                            f"seconds={result.search_seconds:.2f}"
                        ),
                        file=progress_stream,
                        flush=True,
                    )

    failures = _failure_fixtures(roots, episodes)
    if failure_output is not None:
        failure_output.parent.mkdir(parents=True, exist_ok=True)
        failure_output.write_text(json.dumps(failures, indent=2, sort_keys=True), encoding="utf-8")

    candidate_manifest = _candidate_manifest(candidates)
    ranking = _rank_episode_dicts(episodes, candidate_manifest=candidate_manifest)
    return {
        "type": "self_play_trace_eval",
        "schema": 1,
        "source": "sim_selfplay_trace_eval",
        "parity": "non_parity_simulator_only",
        "split": split,
        "trace_count": len(trace_paths),
        "trace_extraction": extraction_report,
        "available_roots": available_roots,
        "roots": len(roots),
        "max_roots": max_roots,
        "max_actions": max_actions,
        "root_scope": root_scope,
        "eval_set": eval_set,
        "eval_set_spec": eval_set_spec,
        "held_out": bool(eval_set_spec.get("held_out")) if eval_set_spec else split == "eval",
        "allowed_potions": allowed_potions,
        "root_family": "trace_combat_states",
        "failure_fixture_count": len(failures["fixtures"]),
        "failure_output": str(failure_output) if failure_output is not None else None,
        "candidate_manifest": candidate_manifest,
        "top_candidates": ranking[:3],
        "potion_roots": sum(1 for root in roots if root.potion_count > 0),
        "potion_action_roots": sum(
            1 for root in roots if any("potion" in kind for kind in root.legal_action_kinds)
        ),
        "allowed_potion_roots": sum(
            1 for root in roots if _has_allowed_potion_actions(root, allowed_potions)
        ),
        "root_manifest": _root_manifest(roots, allowed_potions),
        "groups": _group_stats(roots, episodes, allowed_potions),
        "ranking": ranking,
        "episodes": episodes,
    }


def real_trace_root_report(traces: Iterable[Path]) -> dict[str, Any]:
    """Explain which traces can provide simulator combat roots."""

    reports = [_single_trace_root_report(Path(path)) for path in traces]
    return {
        "traces": len(reports),
        "extractable_traces": sum(1 for report in reports if report["extractable_roots"] > 0),
        "extractable_roots": sum(int(report["extractable_roots"]) for report in reports),
        "observed_combat_states": sum(int(report["observed_combat_states"]) for report in reports),
        "observed_potion_combat_states": sum(
            int(report["observed_potion_combat_states"]) for report in reports
        ),
        "blocked_traces": [
            report for report in reports if report["extractable_roots"] == 0 and report["block_reason"]
        ],
        "files": reports,
    }


def _start_env(*, start: str, seed: str | None, ascension: int) -> Any:
    if start == "seed":
        return omni.OmniRunEnv.new_ironclad(seed=seed, ascension=ascension)
    if start == "map_fixture":
        return omni.OmniRunEnv.map_fixture()
    if start == "combat_fixture":
        return omni.OmniRunEnv.combat_fixture()
    raise ValueError(f"unsupported start mode: {start}")


def _trace_summary(
    *,
    trace_path: Path,
    records: list[dict[str, Any]],
    result: SelfPlayResult,
    verification: dict[str, Any],
) -> dict[str, Any]:
    metadata = records[0] if records else {}
    step_records = [record for record in records[1:] if record.get("type") == "step"]
    potion_action_steps = [
        record.get("step")
        for record in step_records
        if "potion" in str(record.get("action_kind", "")).lower()
    ]
    potion_state_steps = [
        record.get("step")
        for record in step_records
        if (record.get("before_summary") or {}).get("potions")
        or (record.get("after_summary") or {}).get("potions")
    ]
    combat_steps = [
        record.get("step")
        for record in step_records
        if (record.get("before_summary") or {}).get("phase") == "combat"
    ]
    return {
        "path": str(trace_path),
        "seed": metadata.get("seed"),
        "random_seed": metadata.get("random_seed"),
        "steps": result.steps,
        "verified": bool(result.verified and verification.get("ok")),
        "verification": verification,
        "stop_reason": result.stop_reason,
        "final_phase": result.final_phase,
        "final_summary": metadata.get("final_summary"),
        "combat_steps": len(combat_steps),
        "potion_state_steps": len(potion_state_steps),
        "potion_action_steps": potion_action_steps,
    }


def _trace_paths(*, corpus_dir: Path | None, traces: Iterable[Path] | None) -> list[Path]:
    if traces is not None:
        return sorted(Path(path) for path in traces)
    if corpus_dir is None:
        raise ValueError("corpus_dir or traces is required")
    traces_dir = corpus_dir / "traces"
    root = traces_dir if traces_dir.exists() else corpus_dir
    return sorted(path for path in root.glob("*.jsonl") if path.is_file())


def _single_trace_root_report(path: Path) -> dict[str, Any]:
    records = _read_jsonl(path)
    metadata = records[0] if records else {}
    extractable_root_ids: set[str] = set()
    observed_combat_states = 0
    observed_potion_combat_states = 0
    for record in records[1:]:
        if record.get("type") == "step" and (record.get("before_summary") or {}).get("phase") == "combat":
            state_id = record.get("before_hash")
            if isinstance(record.get("before_snapshot_json"), str) and isinstance(state_id, str):
                extractable_root_ids.add(state_id)
        if record.get("type") == "anchor" and (record.get("summary") or {}).get("phase") == "combat":
            state_id = record.get("snapshot_hash")
            if isinstance(record.get("snapshot_json"), str) and isinstance(state_id, str):
                extractable_root_ids.add(state_id)
        game_state = ((record.get("message") or {}).get("game_state") or {}) if isinstance(record, dict) else {}
        if _is_observed_combat_state(game_state):
            observed_combat_states += 1
            if _observed_usable_potions(game_state):
                observed_potion_combat_states += 1

    source = metadata.get("source") if isinstance(metadata, dict) else None
    extractable_roots = len(extractable_root_ids)
    block_reason = None
    if extractable_roots == 0 and observed_combat_states > 0:
        block_reason = "observed CommunicationMod combat states do not include simulator snapshots"
    elif extractable_roots == 0 and source == "communication_mod":
        block_reason = "CommunicationMod trace lacks simulator initial/before snapshots"
    elif extractable_roots == 0:
        block_reason = "no simulator combat root snapshots found"
    return {
        "path": str(path),
        "source": source,
        "records": len(records),
        "extractable_roots": extractable_roots,
        "observed_combat_states": observed_combat_states,
        "observed_potion_combat_states": observed_potion_combat_states,
        "block_reason": block_reason,
    }


def _is_observed_combat_state(game_state: dict[str, Any]) -> bool:
    if not isinstance(game_state, dict):
        return False
    if str(game_state.get("screen_type", "")).upper() == "COMBAT":
        return True
    if str(game_state.get("room_phase", "")).upper() == "COMBAT":
        return True
    return bool(game_state.get("monsters")) and bool(game_state.get("hand"))


def _observed_usable_potions(game_state: dict[str, Any]) -> list[str]:
    potions = []
    for potion in game_state.get("potions") or []:
        if not isinstance(potion, dict):
            continue
        if potion.get("can_use"):
            potions.append(str(potion.get("name") or potion.get("id") or "unknown"))
    return potions


def _trace_combat_roots(
    trace_paths: Iterable[Path],
    *,
    root_scope: str = "all",
) -> list[TraceCombatRoot]:
    if root_scope not in {"all", "combat_start"}:
        raise ValueError(f"unsupported root_scope: {root_scope}")
    roots: list[TraceCombatRoot] = []
    seen: set[str] = set()
    seen_combat_start_keys: set[str] = set()
    for trace_path in trace_paths:
        records = _read_jsonl(trace_path)
        real_trace_baselines = _real_trace_combat_baselines(records)
        for record in records[1:]:
            record_type = record.get("type")
            if record_type == "step":
                summary = record.get("before_summary") or {}
                snapshot_json = record.get("before_snapshot_json")
                state_id = record.get("before_hash")
                legal_actions = record.get("legal_actions", [])
            elif record_type == "anchor":
                summary = record.get("summary") or {}
                snapshot_json = record.get("snapshot_json")
                state_id = record.get("snapshot_hash")
                legal_actions = record.get("legal_actions", [])
            else:
                continue
            phase = summary.get("phase")
            if phase != "combat":
                continue
            if not isinstance(snapshot_json, str) or not isinstance(state_id, str):
                continue
            key = f"{trace_path}:{state_id}"
            if key in seen:
                continue
            if root_scope == "combat_start":
                encounter_key = _trace_encounter_key(trace_path, summary, record)
                if encounter_key in seen_combat_start_keys:
                    continue
                seen_combat_start_keys.add(encounter_key)
            key = f"{trace_path}:{state_id}"
            if key in seen:
                continue
            seen.add(key)
            legal_action_kinds = tuple(
                str(action.get("kind", "")).lower()
                for action in legal_actions
                if isinstance(action, dict)
            )
            potion_names = tuple(str(potion) for potion in summary.get("potions") or [])
            legal_potion_names = tuple(
                name
                for name in (
                    _potion_name_for_action(action, potion_names)
                    for action in legal_actions
                    if isinstance(action, dict)
                )
                if name is not None
            )
            baseline = real_trace_baselines.get(state_id, {})
            roots.append(
                TraceCombatRoot(
                    trace_path=trace_path,
                    step=int(record.get("step", 0)),
                    state_id=state_id,
                    snapshot_json=snapshot_json,
                    split=_split_for_state(state_id),
                    potion_count=len(potion_names),
                    potion_names=potion_names,
                    legal_action_kinds=legal_action_kinds,
                    legal_potion_names=legal_potion_names,
                    real_trace_final_hp=baseline.get("final_hp"),
                    real_trace_hp_loss=baseline.get("hp_loss"),
                    real_trace_terminal_phase=baseline.get("terminal_phase"),
                )
            )
    roots.sort(key=lambda root: (str(root.trace_path), root.step, root.state_id))
    return roots


def _real_trace_combat_baselines(records: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    baselines: dict[str, dict[str, Any]] = {}
    indexed: list[tuple[int, str, dict[str, Any]]] = []
    for index, record in enumerate(records[1:], start=1):
        if record.get("type") == "step":
            summary = record.get("before_summary") or {}
            state_id = record.get("before_hash")
        elif record.get("type") == "anchor":
            summary = record.get("summary") or {}
            state_id = record.get("snapshot_hash")
        else:
            continue
        if summary.get("phase") != "combat" or not isinstance(state_id, str):
            continue
        indexed.append((index, state_id, summary))

    for index, state_id, summary in indexed:
        initial_hp = _summary_player_hp(summary)
        if initial_hp is None:
            continue
        final_summary = _real_trace_combat_end_summary(records, index)
        if final_summary is None:
            continue
        final_hp = _summary_player_hp(final_summary)
        if final_hp is None:
            continue
        baselines[state_id] = {
            "final_hp": final_hp,
            "hp_loss": initial_hp - final_hp,
            "terminal_phase": final_summary.get("phase"),
        }
    return baselines


def _real_trace_combat_end_summary(
    records: list[dict[str, Any]],
    start_index: int,
) -> dict[str, Any] | None:
    last_combat_summary: dict[str, Any] | None = None
    for record in records[start_index:]:
        summaries: list[dict[str, Any]] = []
        if record.get("type") == "step":
            before = record.get("before_summary")
            after = record.get("after_summary")
            if isinstance(before, dict):
                summaries.append(before)
            if isinstance(after, dict):
                summaries.append(after)
        elif record.get("type") == "anchor":
            summary = record.get("summary")
            if isinstance(summary, dict):
                summaries.append(summary)
        for summary in summaries:
            if summary.get("phase") == "combat":
                last_combat_summary = summary
                continue
            if last_combat_summary is not None:
                return summary
    return last_combat_summary


def _summary_player_hp(summary: dict[str, Any]) -> float | None:
    hp = summary.get("player_hp")
    if isinstance(hp, int | float):
        return float(hp)
    return None


def _trace_encounter_key(
    trace_path: Path,
    summary: dict[str, Any],
    record: dict[str, Any],
) -> str:
    floor = summary.get("floor")
    act = summary.get("act")
    if floor is not None:
        return f"{trace_path}:act{act}:floor{floor}"
    trace_step = record.get("trace_step")
    step = record.get("step")
    return f"{trace_path}:trace_step{trace_step}:step{step}"


def _choose_action(
    env: Any,
    actions: list[Any],
    rng: random.Random,
    combat_policy: CombatSearchConfig,
) -> tuple[Any, str, dict[str, Any]]:
    if env.phase() == "combat":
        recommendation = search_combat(env, combat_policy)
        if recommendation.best_action is not None:
            return (
                recommendation.best_action,
                "portfolio_combat",
                {
                    "score": recommendation.score,
                    "nodes": recommendation.nodes,
                    "terminal_reason": recommendation.terminal_reason,
                    **recommendation.diagnostics,
                },
            )

    action = _random_viable_noncombat_action(env, actions, rng)
    return action, "random_noncombat", {"candidate_count": len(actions)}


def _random_viable_noncombat_action(env: Any, actions: list[Any], rng: random.Random) -> Any:
    shuffled = list(actions)
    rng.shuffle(shuffled)
    fallback = shuffled[0]
    for action in shuffled:
        child = env.clone()
        try:
            result = child.step(action)
        except Exception:
            continue
        if result.unsupported_reason and not child.exact_legal_actions():
            continue
        return action
    return fallback


def _find_action(actions: Iterable[Any], action_json: Any) -> Any | None:
    for action in actions:
        if action.json() == action_json:
            return action
    return None


def _rank_episode_dicts(
    episodes: list[dict[str, Any]],
    *,
    candidate_manifest: dict[str, dict[str, Any]] | None = None,
) -> list[dict[str, Any]]:
    by_candidate: dict[str, list[dict[str, Any]]] = {}
    for episode in episodes:
        by_candidate.setdefault(str(episode["candidate"]), []).append(episode)

    ranking = []
    for name, candidate_episodes in by_candidate.items():
        count = len(candidate_episodes)
        wins = sum(1 for episode in candidate_episodes if episode["won"])
        losses = sum(1 for episode in candidate_episodes if episode["lost"])
        nonterminal = count - wins - losses
        row = {
            "candidate": name,
            "episodes": count,
            "wins": wins,
            "losses": losses,
            "nonterminal": nonterminal,
            "win_rate": wins / count if count else 0.0,
            "mean_score": _mean(float(episode["final_score"]) for episode in candidate_episodes),
            "mean_hp_loss": _mean(float(episode["hp_loss"]) for episode in candidate_episodes),
            "median_hp_loss": _percentile(
                (float(episode["hp_loss"]) for episode in candidate_episodes), 50
            ),
            "p95_hp_loss": _percentile(
                (float(episode["hp_loss"]) for episode in candidate_episodes), 95
            ),
            "mean_final_hp": _mean(float(episode["final_hp"]) for episode in candidate_episodes),
            "mean_monster_hp": _mean(float(episode["monster_hp"]) for episode in candidate_episodes),
            "mean_actions": _mean(float(episode["actions"]) for episode in candidate_episodes),
            "mean_potion_uses": _mean(
                float(episode["potion_uses"]) for episode in candidate_episodes
            ),
            "total_potion_uses": sum(int(episode["potion_uses"]) for episode in candidate_episodes),
            "potion_use_counts": _episode_potion_use_counts(candidate_episodes),
            "mean_real_trace_hp_loss": _mean(
                float(episode["real_trace_hp_loss"])
                for episode in candidate_episodes
                if episode.get("real_trace_hp_loss") is not None
            ),
            "mean_hp_loss_delta_vs_trace": _mean(
                float(episode["hp_loss_delta_vs_trace"])
                for episode in candidate_episodes
                if episode.get("hp_loss_delta_vs_trace") is not None
            ),
            "mean_seconds_per_combat": _mean(
                float(episode["search_seconds"]) for episode in candidate_episodes
            ),
            "mean_seconds_per_decision": _mean(
                float(episode["mean_seconds_per_decision"])
                for episode in candidate_episodes
            ),
            "p50_seconds_per_decision": _percentile(
                (
                    float(second)
                    for episode in candidate_episodes
                    for second in episode.get("decision_seconds", [])
                ),
                50,
            ),
            "p95_seconds_per_decision": _percentile(
                (
                    float(second)
                    for episode in candidate_episodes
                    for second in episode.get("decision_seconds", [])
                ),
                95,
            ),
            "mean_search_nodes": _mean(
                float(episode["search_nodes"]) for episode in candidate_episodes
            ),
            "p95_search_nodes": _percentile(
                (float(episode["search_nodes"]) for episode in candidate_episodes), 95
            ),
            "potion_roots": sum(1 for episode in candidate_episodes if episode["potion_count"] > 0),
            "potion_action_roots": sum(
                1 for episode in candidate_episodes if episode["has_potion_actions"]
            ),
            "allowed_potion_roots": sum(
                1 for episode in candidate_episodes if episode["has_allowed_potion_actions"]
            ),
        }
        if candidate_manifest and name in candidate_manifest:
            row |= candidate_manifest[name]
        ranking.append(row)
    return sorted(
        ranking,
        key=lambda row: (
            -float(row["win_rate"]),
            -float(row["mean_score"]),
            float(row["mean_search_nodes"]),
            row["candidate"],
        ),
    )


def _candidate_manifest(candidates: Iterable[SearchCandidate]) -> dict[str, dict[str, Any]]:
    return {candidate.name: _candidate_config_summary(candidate.config) for candidate in candidates}


def _candidate_config_summary(config: CombatSearchConfig) -> dict[str, Any]:
    return {
        "algorithm": config.algorithm,
        "objective": config.objective,
        "max_depth": config.max_depth,
        "beam_width": config.beam_width,
        "allowed_potions": config.allowed_potions,
    }


def _failure_fixtures(
    roots: list[TraceCombatRoot],
    episodes: list[dict[str, Any]],
) -> dict[str, Any]:
    roots_by_key = {
        (str(root.trace_path), root.step, root.state_id): root
        for root in roots
    }
    fixtures = []
    for episode in episodes:
        if episode.get("won"):
            continue
        key = (
            str(episode.get("trace_path")),
            int(episode.get("trace_step", 0)),
            str(episode.get("state_id")),
        )
        root = roots_by_key.get(key)
        if root is None:
            continue
        fixtures.append(
            {
                "name": _failure_fixture_name(episode),
                "trace_path": str(root.trace_path),
                "trace_step": root.step,
                "state_id": root.state_id,
                "candidate": episode.get("candidate"),
                "split": root.split,
                "terminal_reason": episode.get("terminal_reason"),
                "actions": episode.get("actions"),
                "initial_hp": episode.get("initial_hp"),
                "final_hp": episode.get("final_hp"),
                "hp_loss": episode.get("hp_loss"),
                "monster_hp": episode.get("monster_hp"),
                "potion_uses": episode.get("potion_uses"),
                "potion_use_names": list(episode.get("potion_use_names") or []),
                "search_seconds": episode.get("search_seconds"),
                "mean_seconds_per_decision": episode.get("mean_seconds_per_decision"),
                "real_trace_final_hp": root.real_trace_final_hp,
                "real_trace_hp_loss": root.real_trace_hp_loss,
                "hp_loss_delta_vs_trace": episode.get("hp_loss_delta_vs_trace"),
                "potion_names": list(root.potion_names),
                "legal_action_kinds": list(root.legal_action_kinds),
                "legal_potion_names": list(root.legal_potion_names),
                "snapshot_json": root.snapshot_json,
            }
        )
    return {
        "type": "combat_autopilot_failure_fixtures",
        "schema": 1,
        "source": "sim_selfplay_trace_eval",
        "fixture_count": len(fixtures),
        "fixtures": fixtures,
    }


def _failure_fixture_name(episode: dict[str, Any]) -> str:
    candidate = _safe_file_stem(str(episode.get("candidate") or "candidate"))
    state_id = _safe_file_stem(str(episode.get("state_id") or "state"))[:16]
    trace_step = episode.get("trace_step", 0)
    reason = _safe_file_stem(str(episode.get("terminal_reason") or "nonterminal"))
    return f"{candidate}-step{trace_step}-{state_id}-{reason}"


def _episode_potion_use_counts(episodes: Iterable[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for episode in episodes:
        for name in episode.get("potion_use_names") or []:
            potion_name = str(name)
            counts[potion_name] = counts.get(potion_name, 0) + 1
    return dict(sorted(counts.items()))


def _metadata(
    *,
    started_at: str,
    seed: str | None,
    ascension: int,
    start: str,
    random_seed: int,
    max_steps: int,
    combat_policy: CombatSearchConfig,
    initial_snapshot_json: str | None = None,
    initial_state_id: str | None = None,
    stop_reason: str | None = None,
) -> dict[str, Any]:
    return {
        "type": "metadata",
        "schema": 1,
        "source": "sim_selfplay",
        "started_at": started_at,
        "seed": seed,
        "ascension": ascension,
        "start": start,
        "random_seed": random_seed,
        "max_steps": max_steps,
        "combat_policy": combat_policy.__dict__,
        "noncombat_policy": "random_viable_v1",
        "initial_state_id": initial_state_id,
        "initial_snapshot_json": initial_snapshot_json,
        "stop_reason": stop_reason,
    }


def _step_record(
    *,
    step: int,
    before_hash: str,
    before_snapshot_json: str,
    before_summary: dict[str, Any],
    legal_actions: list[Any],
    action: Any,
    policy_name: str,
    policy_diagnostics: dict[str, Any],
    after_hash: str | None = None,
    after_snapshot_json: str | None = None,
    after_summary: dict[str, Any] | None = None,
    transition: Any | None = None,
    unsupported_reason: str | None = None,
    error: str | None = None,
) -> dict[str, Any]:
    return {
        "type": "step",
        "step": step,
        "before_hash": before_hash,
        "before_snapshot_json": before_snapshot_json,
        "before_summary": before_summary,
        "legal_actions": [_action_record(action) for action in legal_actions],
        "action_family": action.family(),
        "action_kind": action.kind(),
        "action_json": action.json(),
        "policy": policy_name,
        "policy_diagnostics": policy_diagnostics,
        "after_hash": after_hash,
        "after_snapshot_json": after_snapshot_json,
        "after_summary": after_summary,
        "transition": _transition_record(transition),
        "unsupported_reason": unsupported_reason,
        "error": error,
    }


def _action_record(action: Any) -> dict[str, Any]:
    return {"family": action.family(), "kind": action.kind(), "json": action.json()}


def _transition_record(transition: Any | None) -> dict[str, Any] | None:
    if transition is None:
        return None
    return {
        "action_json": transition.action_json,
        "previous_hash": transition.previous_hash,
        "resulting_hash": transition.resulting_hash,
        "events_json": transition.events_json,
        "rng_draws_json": transition.rng_draws_json,
        "simulator_error": transition.simulator_error,
    }


def _anchor_record(
    *,
    step: int,
    command: str,
    trace_step: Any,
    env: Any,
    observed_summary: dict[str, Any],
    diffs: list[dict[str, Any]],
) -> dict[str, Any]:
    legal_actions = env.exact_legal_actions()
    return {
        "type": "anchor",
        "step": step,
        "reason": "observed_combat_state",
        "command": command,
        "trace_step": trace_step,
        "snapshot_hash": env.snapshot_hash(),
        "snapshot_json": env.snapshot_json(),
        "summary": _summary(env),
        "legal_actions": [_action_record(action) for action in legal_actions],
        "observed_summary": observed_summary,
        "pre_anchor_diffs": diffs,
    }


def _restoration_record(
    *,
    step: int,
    record: dict[str, Any],
    command: str,
    category: str,
    reason: str,
    **extra: Any,
) -> dict[str, Any]:
    return {
        "type": "restoration",
        "step": step,
        "trace_step": record.get("step"),
        "command": command,
        "category": category,
        "reason": reason,
        **extra,
    }


def _summary(env: Any) -> dict[str, Any]:
    state = json.loads(env.state_json())
    run = state.get("state", state)
    combat = run.get("combat") or {}
    player = combat.get("player") or {}
    decision = env.current_decision()
    phase = decision if env.phase() == "idle" and decision == "map" else env.phase()
    return {
        "phase": phase,
        "decision": decision,
        "unsupported_reason": env.unsupported_reason(),
        "player_hp": run.get("player_hp") if run.get("player_hp") is not None else player.get("hp"),
        "player_max_hp": run.get("player_max_hp")
        if run.get("player_max_hp") is not None
        else player.get("max_hp"),
        "gold": run.get("gold"),
        "floor": run.get("current_floor"),
        "act": run.get("current_act"),
        "potions": run.get("potions") or [],
        "potion_count": len(run.get("potions") or []),
        "relics": run.get("relics") or [],
        "combat": {
            "energy": player.get("energy"),
            "hand": [card.get("content_id") for card in ((combat.get("piles") or {}).get("hand") or [])],
            "hand_count": len((combat.get("piles") or {}).get("hand") or []),
            "monster_count": len(combat.get("monsters", [])),
            "monsters": [
                {
                    "id": monster.get("id"),
                    "hp": monster.get("hp"),
                    "block": monster.get("block"),
                    "alive": monster.get("alive"),
                    "intent": monster.get("intent"),
                }
                for monster in combat.get("monsters", [])
            ],
        }
        if combat
        else None,
    }


def _write_jsonl(path: Path, records: Iterable[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, sort_keys=True))
            handle.write("\n")


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    with path.open("r", encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def _parse_start_command(command: str) -> dict[str, Any] | None:
    parts = command.strip().split()
    if len(parts) != 4 or parts[0].upper() != "START":
        return None
    try:
        ascension = int(parts[2])
    except ValueError:
        return None
    return {
        "character": parts[1],
        "ascension": ascension,
        "external_seed": parts[3],
        "numeric_seed": None,
    }


def _game_state_from_record(record: dict[str, Any] | None) -> dict[str, Any]:
    if not isinstance(record, dict):
        return {}
    message = record.get("message")
    if not isinstance(message, dict):
        return {}
    game_state = message.get("game_state")
    return game_state if isinstance(game_state, dict) else {}


def _action_for_communication_command(env: Any, command: str, observed: dict[str, Any]) -> Any | None:
    parts = command.strip().split()
    if not parts:
        return None
    verb = parts[0].upper()
    actions = env.exact_legal_actions()
    if verb == "END":
        return _first_action(actions, kind="end_turn")
    if verb == "CHOOSE" and len(parts) >= 2:
        try:
            index = int(parts[1])
        except ValueError:
            return None
        return _choose_action_for_index(env, actions, index)
    if verb == "PLAY" and len(parts) >= 2:
        try:
            hand_index = int(parts[1])
            target_index = int(parts[2]) if len(parts) >= 3 else None
        except ValueError:
            return None
        return _play_action_for_indices(env, actions, hand_index, target_index)
    if verb == "POTION" and len(parts) >= 2:
        try:
            slot = int(parts[1])
            target_index = int(parts[2]) if len(parts) >= 3 else None
        except ValueError:
            return None
        return _potion_action_for_indices(env, actions, slot, target_index)
    if verb in {"STATE", "WAIT"}:
        return None
    _ = observed
    return None


def _choose_action_for_index(env: Any, actions: list[Any], index: int) -> Any | None:
    phase = env.current_decision() if env.phase() == "idle" else env.phase()
    if phase == "event":
        for action in actions:
            data = _action_json_data(action)
            choose = data.get("Choose") if isinstance(data, dict) else None
            if isinstance(choose, dict) and choose.get("choice_index") == index:
                return action
    if phase in {"map", "reward", "shop", "rest"} and 0 <= index < len(actions):
        return actions[index]
    return None


def _play_action_for_indices(
    env: Any,
    actions: list[Any],
    hand_index: int,
    target_index: int | None,
) -> Any | None:
    state = json.loads(env.state_json())
    run = state.get("state", state)
    combat = run.get("combat") or {}
    hand = ((combat.get("piles") or {}).get("hand") or [])
    monsters = combat.get("monsters") or []
    hand_index = hand_index - 1
    if hand_index < 0 or hand_index >= len(hand):
        return None
    card_id = hand[hand_index].get("id")
    target_id = None
    if target_index is not None:
        target_id = target_index + 1
    for action in actions:
        if getattr(action, "kind", lambda: "")() != "play_card":
            continue
        play = _action_json_data(action).get("PlayCard")
        if not isinstance(play, dict) or play.get("card_id") != card_id:
            continue
        if target_id is None or play.get("target") == target_id:
            return action
    return None


def _potion_action_for_indices(
    env: Any,
    actions: list[Any],
    slot: int,
    target_index: int | None,
) -> Any | None:
    state = json.loads(env.state_json())
    run = state.get("state", state)
    monsters = ((run.get("combat") or {}).get("monsters") or [])
    target_id = None
    if target_index is not None:
        target_id = target_index + 1
    for action in actions:
        if getattr(action, "kind", lambda: "")() != "use_potion":
            continue
        use = _action_json_data(action).get("UsePotion")
        if not isinstance(use, dict) or use.get("slot") != slot:
            continue
        if target_id is None or use.get("target") == target_id:
            return action
    return None


def _first_action(actions: list[Any], *, kind: str) -> Any | None:
    return next((action for action in actions if getattr(action, "kind", lambda: "")() == kind), None)


def _action_json_data(action: Any) -> dict[str, Any]:
    try:
        data = json.loads(action.json())
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def _observed_summary_diffs(env: Any, observed: dict[str, Any]) -> list[dict[str, Any]]:
    if not observed:
        return []
    simulator = _summary(env)
    observed_summary = _observed_summary(observed)
    diffs = []
    for key in ("phase", "player_hp", "player_max_hp", "gold", "potion_count"):
        if observed_summary.get(key) is None:
            continue
        if simulator.get(key) != observed_summary.get(key):
            diffs.append(
                {
                    "field": key,
                    "simulator": simulator.get(key),
                    "observed": observed_summary.get(key),
                }
            )
    if simulator.get("phase") == "combat" and observed_summary.get("phase") == "combat":
        sim_combat = simulator.get("combat") or {}
        obs_combat = observed_summary.get("combat") or {}
        for key in ("energy", "hand_count", "monster_count"):
            if obs_combat.get(key) is None:
                continue
            if sim_combat.get(key) != obs_combat.get(key):
                diffs.append(
                    {
                        "field": f"combat.{key}",
                        "simulator": sim_combat.get(key),
                        "observed": obs_combat.get(key),
                    }
                )
    return diffs


def _observed_summary(observed: dict[str, Any]) -> dict[str, Any]:
    combat = observed.get("combat_state") if isinstance(observed.get("combat_state"), dict) else {}
    player = combat.get("player") if isinstance(combat.get("player"), dict) else {}
    potion_count = len(_observed_real_potions(observed))
    return {
        "phase": _observed_phase(observed),
        "player_hp": observed.get("current_hp") or observed.get("player_hp") or player.get("current_hp"),
        "player_max_hp": observed.get("max_hp") or observed.get("player_max_hp") or player.get("max_hp"),
        "gold": observed.get("gold"),
        "potion_count": potion_count,
        "combat": {
            "energy": combat.get("energy"),
            "hand_count": len(combat.get("hand") or []),
            "monster_count": len(combat.get("monsters") or []),
        }
        if combat
        else None,
    }


def _observed_phase(observed: dict[str, Any]) -> str | None:
    if not observed:
        return None
    if isinstance(observed.get("combat_state"), dict):
        return "combat"
    screen_type = str(observed.get("screen_type") or "").upper()
    if screen_type == "MAP":
        return "map"
    if screen_type == "EVENT":
        return "event"
    if screen_type in {"SHOP_SCREEN", "SHOP"}:
        return "shop"
    if screen_type in {"REST", "REST_ROOM"}:
        return "rest"
    if screen_type in {"CARD_REWARD", "COMBAT_REWARD", "BOSS_REWARD", "GRID"}:
        return "reward"
    if screen_type == "NONE" and observed.get("choice_list"):
        return "idle"
    return None


def _observed_real_potions(observed: dict[str, Any]) -> list[dict[str, Any]]:
    potions = []
    for potion in observed.get("potions") or []:
        if not isinstance(potion, dict):
            continue
        name = str(potion.get("name") or potion.get("id") or "")
        if name.lower() == "potion slot":
            continue
        potions.append(potion)
    return potions


def _blocker(
    record: dict[str, Any],
    command: str,
    category: str,
    reason: str,
    **extra: Any,
) -> dict[str, Any]:
    return {
        "trace_step": record.get("step"),
        "command": command,
        "category": category,
        "reason": reason,
        **extra,
    }


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _candidate_with_allowed_potions(
    candidate: SearchCandidate,
    allowed_potions: tuple[str, ...] | None,
) -> SearchCandidate:
    if allowed_potions is None or candidate.config.allowed_potions is not None:
        return candidate
    return SearchCandidate(
        candidate.name,
        replace(candidate.config, allowed_potions=allowed_potions),
    )


def _has_allowed_potion_actions(
    root: TraceCombatRoot,
    allowed_potions: tuple[str, ...] | None,
) -> bool:
    if allowed_potions is None:
        return bool(root.legal_potion_names)
    allowed = {_normalize_potion_name(name) for name in allowed_potions}
    return any(_normalize_potion_name(name) in allowed for name in root.legal_potion_names)


def _potion_name_for_action(action: dict[str, Any], potion_names: tuple[str, ...]) -> str | None:
    if str(action.get("kind", "")).lower() != "use_potion":
        return None
    action_json = action.get("json")
    if not isinstance(action_json, str):
        return None
    try:
        data = json.loads(action_json)
    except json.JSONDecodeError:
        return None
    use = data.get("UsePotion") if isinstance(data, dict) else None
    slot = use.get("slot") if isinstance(use, dict) else None
    if not isinstance(slot, int) or slot >= len(potion_names):
        return None
    return potion_names[slot]


def _group_stats(
    roots: list[TraceCombatRoot],
    episodes: list[dict[str, Any]],
    allowed_potions: tuple[str, ...] | None,
) -> dict[str, Any]:
    return {
        "all": _group_stat_for_roots(roots, episodes),
        "potion": _group_stat_for_roots(
            [root for root in roots if root.potion_count > 0],
            episodes,
        ),
        "allowed_potion": _group_stat_for_roots(
            [root for root in roots if _has_allowed_potion_actions(root, allowed_potions)],
            episodes,
        ),
    }


def _root_manifest(
    roots: list[TraceCombatRoot],
    allowed_potions: tuple[str, ...] | None,
) -> list[dict[str, Any]]:
    return [
        {
            "trace_path": str(root.trace_path),
            "trace_step": root.step,
            "state_id": root.state_id,
            "split": root.split,
            "potion_count": root.potion_count,
            "potion_names": list(root.potion_names),
            "legal_action_kinds": list(root.legal_action_kinds),
            "legal_potion_names": list(root.legal_potion_names),
            "has_allowed_potion_actions": _has_allowed_potion_actions(root, allowed_potions),
            "real_trace_hp_loss": root.real_trace_hp_loss,
        }
        for root in roots
    ]


def _group_stat_for_roots(
    roots: list[TraceCombatRoot],
    episodes: list[dict[str, Any]],
) -> dict[str, Any]:
    keys = {(str(root.trace_path), root.step, root.state_id) for root in roots}
    group_episodes = [
        episode
        for episode in episodes
        if (episode["trace_path"], episode["trace_step"], episode["state_id"]) in keys
    ]
    return {
        "roots": len(roots),
        "episodes": len(group_episodes),
        "ranking": _rank_episode_dicts(group_episodes),
    }


def _split_for_state(state_id: str) -> str:
    digest = hashlib.sha256(state_id.encode("utf-8")).digest()[0]
    return "eval" if digest % 5 == 0 else "dev"


def _mean(values: Iterable[float]) -> float:
    values = list(values)
    return sum(values) / len(values) if values else 0.0


def _percentile(values: Iterable[float], percentile: float) -> float:
    values = sorted(float(value) for value in values)
    if not values:
        return 0.0
    if len(values) == 1:
        return values[0]
    rank = (len(values) - 1) * percentile / 100.0
    lower = int(rank)
    upper = min(lower + 1, len(values) - 1)
    weight = rank - lower
    return values[lower] * (1.0 - weight) + values[upper] * weight


def _trace_eval_set_spec(eval_set: str | None) -> dict[str, Any] | None:
    if eval_set is None:
        return None
    if eval_set not in TRACE_EVAL_SET_SPECS:
        raise ValueError(f"unsupported eval_set: {eval_set}")
    return {"name": eval_set, **TRACE_EVAL_SET_SPECS[eval_set]}


def _normalize_potion_name(name: str) -> str:
    normalized = "".join(char.lower() for char in name if char.isalnum())
    return normalized.removesuffix("potion")


def _safe_file_stem(value: str) -> str:
    safe = "".join(char if char.isalnum() or char in {"-", "_"} else "_" for char in value)
    return safe[:80] or "empty"


def _parse_seed_specs(specs: Iterable[str]) -> list[str]:
    seeds: list[str] = []
    for spec in specs:
        for part in spec.split(","):
            part = part.strip()
            if not part:
                continue
            if ".." in part:
                start_text, end_text = part.split("..", 1)
                start = int(start_text)
                end = int(end_text)
                step = 1 if end >= start else -1
                seeds.extend(str(value) for value in range(start, end + step, step))
            else:
                seeds.append(part)
    return seeds


def _parse_allowed_potions(value: str | None) -> tuple[str, ...] | None:
    if value is None:
        return None
    if value.strip().lower() in {"*", "all"}:
        return None
    if not value.strip() or value.strip().lower() in {"none", "no", "false"}:
        return ()
    return tuple(part.strip() for part in value.split(",") if part.strip())


def _parse_candidate_names(values: Iterable[str] | None) -> tuple[str, ...]:
    if values is None:
        return ()
    names: list[str] = []
    for value in values:
        for part in value.split(","):
            part = part.strip()
            if part:
                names.append(part)
    return tuple(names)


def _trace_candidates_by_name(names: Iterable[str]) -> list[SearchCandidate]:
    requested = tuple(names)
    if not requested:
        return trace_autopilot_candidates()
    return [trace_autopilot_candidate_by_name(name) for name in requested]


def _combat_policy_from_name(
    name: str,
    allowed_potions: tuple[str, ...] | None,
) -> CombatSearchConfig:
    return replace(
        trace_autopilot_candidate_by_name(name).config,
        allowed_potions=allowed_potions,
    )


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Generate or verify simulator self-play traces.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    run_parser = subparsers.add_parser("run")
    run_parser.add_argument("--output", type=Path, required=True)
    run_parser.add_argument("--seed")
    run_parser.add_argument("--ascension", type=int, default=0)
    run_parser.add_argument("--start", choices=["seed", "map_fixture", "combat_fixture"], default="seed")
    run_parser.add_argument("--random-seed", type=int, default=0)
    run_parser.add_argument("--max-steps", type=int, default=200)
    run_parser.add_argument("--allowed-potions")
    run_parser.add_argument("--combat-policy", default=DEFAULT_COMBAT_POLICY_NAME)

    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("trace", type=Path)

    batch_parser = subparsers.add_parser("batch")
    batch_parser.add_argument("--output-dir", type=Path, required=True)
    batch_parser.add_argument("--seeds", nargs="+", required=True)
    batch_parser.add_argument("--ascension", type=int, default=0)
    batch_parser.add_argument("--start", choices=["seed", "map_fixture", "combat_fixture"], default="seed")
    batch_parser.add_argument("--random-seed", type=int, default=0)
    batch_parser.add_argument("--max-steps", type=int, default=200)
    batch_parser.add_argument("--allowed-potions")
    batch_parser.add_argument("--combat-policy", default=DEFAULT_COMBAT_POLICY_NAME)

    eval_parser = subparsers.add_parser("eval")
    eval_parser.add_argument("--corpus-dir", type=Path)
    eval_parser.add_argument("--trace", dest="traces", type=Path, action="append")
    eval_parser.add_argument("--split", choices=["dev", "eval", "all"], default="all")
    eval_parser.add_argument("--max-roots", type=int, default=64)
    eval_parser.add_argument("--max-actions", type=int, default=40)
    eval_parser.add_argument("--allowed-potions")
    eval_parser.add_argument("--root-scope", choices=["all", "combat_start"], default="all")
    eval_parser.add_argument("--eval-set", choices=sorted(TRACE_EVAL_SET_SPECS))
    eval_parser.add_argument("--candidate", dest="candidate_names", action="append")
    eval_parser.add_argument("--progress-every", type=int, default=0)
    eval_parser.add_argument("--output", type=Path)
    eval_parser.add_argument("--failure-output", type=Path)

    report_parser = subparsers.add_parser("real-trace-report")
    report_parser.add_argument("--trace", dest="traces", type=Path, action="append", required=True)

    replay_parser = subparsers.add_parser("replay-real-trace")
    replay_parser.add_argument("--trace", type=Path, required=True)
    replay_parser.add_argument("--output", type=Path, required=True)
    replay_parser.add_argument("--report-output", type=Path)
    replay_parser.add_argument("--max-actions", type=int, default=10_000)

    args = parser.parse_args(argv)
    if args.command == "run":
        result = run_self_play(
            output=args.output,
            seed=args.seed,
            ascension=args.ascension,
            start=args.start,
            random_seed=args.random_seed,
            max_steps=args.max_steps,
            combat_policy=_combat_policy_from_name(
                args.combat_policy,
                _parse_allowed_potions(args.allowed_potions),
            ),
        )
        print(json.dumps(result.__dict__ | {"trace_path": str(result.trace_path)}, indent=2))
    elif args.command == "verify":
        print(json.dumps(verify_self_play_trace(args.trace), indent=2, sort_keys=True))
    elif args.command == "batch":
        result = run_self_play_batch(
            output_dir=args.output_dir,
            seeds=_parse_seed_specs(args.seeds),
            ascension=args.ascension,
            start=args.start,
            random_seed=args.random_seed,
            max_steps=args.max_steps,
            combat_policy=_combat_policy_from_name(
                args.combat_policy,
                _parse_allowed_potions(args.allowed_potions),
            ),
        )
        print(json.dumps(result.__dict__ | {"output_dir": str(result.output_dir), "index_path": str(result.index_path)}, indent=2, sort_keys=True))
    elif args.command == "eval":
        report = evaluate_self_play_corpus(
            corpus_dir=args.corpus_dir,
            traces=args.traces,
            split=args.split,
            max_roots=args.max_roots,
            max_actions=args.max_actions,
            allowed_potions=_parse_allowed_potions(args.allowed_potions),
            candidates=_trace_candidates_by_name(_parse_candidate_names(args.candidate_names)),
            root_scope=args.root_scope,
            failure_output=args.failure_output,
            eval_set=args.eval_set,
            progress_every=args.progress_every,
            progress_stream=sys.stderr if args.progress_every > 0 else None,
        )
        text = json.dumps(report, indent=2, sort_keys=True)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(text, encoding="utf-8")
        print(text)
    elif args.command == "real-trace-report":
        print(json.dumps(real_trace_root_report(args.traces), indent=2, sort_keys=True))
    elif args.command == "replay-real-trace":
        result = replay_real_trace_guided(
            trace=args.trace,
            output=args.output,
            report_output=args.report_output,
            max_actions=args.max_actions,
        )
        print(json.dumps(result.__dict__, indent=2, sort_keys=True, default=str))


if __name__ == "__main__":
    main()
