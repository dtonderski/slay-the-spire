"""Deterministic simulator self-play trace generation and replay."""

from __future__ import annotations

from concurrent.futures import ProcessPoolExecutor
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
class CombatPolicyIterationResult:
    output_dir: Path
    report_path: Path
    train_corpus: Path
    dev_corpus: Path
    promoted_candidate: str | None


@dataclass(frozen=True)
class TraceGuidedReplayResult:
    trace_path: Path
    output_path: Path
    report_path: Path | None
    steps: int
    combat_roots: int
    stop_reason: str
    verified: bool
    mode: str = "strict"


@dataclass(frozen=True)
class StrictTraceEnvResult:
    env: Any | None
    trace_path: Path
    steps: int
    stop_reason: str
    verified: bool
    blocker: dict[str, Any] | None
    start: dict[str, Any] | None
    final_state_id: str | None
    final_phase: str | None
    latest_observed_summary: dict[str, Any] | None


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
    real_trace_potion_use_names: tuple[str, ...] = ()
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
    repair_anchors: list[dict[str, Any]] = []
    restorations: list[dict[str, Any]] = []
    for record in records[1:]:
        if record.get("type") == "anchor":
            if record.get("pre_anchor_diffs"):
                repair_anchors.append(
                    {
                        "step": record.get("step"),
                        "trace_step": record.get("trace_step"),
                        "diff_count": len(record.get("pre_anchor_diffs") or []),
                        "diffs": record.get("pre_anchor_diffs"),
                    }
                )
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
        if record.get("type") == "restoration":
            restorations.append(
                {
                    "step": record.get("step"),
                    "trace_step": record.get("trace_step"),
                    "category": record.get("category"),
                    "reason": record.get("reason"),
                }
            )
            snapshot = record.get("snapshot_json")
            if isinstance(snapshot, str):
                env = omni.OmniRunEnv.from_snapshot_json(snapshot)
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

    replay_result = {
        "replay_ok": True,
        "steps": steps,
        "final_state_id": env.snapshot_hash(),
        "final_phase": env.phase(),
        "metadata_stop_reason": metadata.get("stop_reason"),
        "repair_anchor_count": len(repair_anchors),
        "restoration_count": len(restorations),
    }
    if repair_anchors:
        return {
            **replay_result,
            "ok": False,
            "error": "trace contains state repair anchors",
            "first_repair_anchor": repair_anchors[0],
        }
    if restorations:
        return {
            **replay_result,
            "ok": False,
            "error": "trace contains restorations",
            "first_restoration": restorations[0],
        }

    return {
        **replay_result,
        "ok": True,
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
    diagnostic_continue_after_diff: bool = False,
) -> TraceGuidedReplayResult:
    """Replay a CommunicationMod trace through OmniRunEnv.

    Strict mode is the verifier path: the first observed-vs-simulated diff, unsupported command
    mapping, or step error stops the replay. Diagnostic mode preserves the old restore-and-continue
    behavior for collecting downstream clues, but it is not verification.
    """

    records = _read_jsonl(trace)
    output.parent.mkdir(parents=True, exist_ok=True)
    if report_output:
        report_output.parent.mkdir(parents=True, exist_ok=True)

    started_at = _now()
    env = None
    last_state: dict[str, Any] | None = None
    last_state_consumed = True
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
    mode = "diagnostic" if diagnostic_continue_after_diff else "strict"

    for record_index, record in enumerate(records):
        if record.get("type") == "state":
            last_state = record
            last_state_consumed = False
            continue
        if record.get("type") != "action":
            continue
        command = str(record.get("command") or "")
        if not command:
            continue
        command_parts = command.strip().split()
        verb = command_parts[0].upper() if command_parts else ""

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
            metadata["mode"] = mode
            metadata["parity"] = "diagnostic_restore_and_continue" if diagnostic_continue_after_diff else "strict"
            metadata["source_trace"] = str(trace)
            metadata["start_command"] = command
            metadata["numeric_seed"] = start_info.get("numeric_seed")
            output_records.append(metadata)
            continue

        if replayed_steps >= max_actions:
            stop_reason = "max_actions"
            break

        observed_game_state = _game_state_from_record(last_state)
        has_fresh_observed_state = not last_state_consumed
        diffs = _observed_summary_diffs(env, observed_game_state) if has_fresh_observed_state else []
        if diffs:
            if not diagnostic_continue_after_diff:
                blocker = _blocker(
                    record,
                    command,
                    "observed_state_diff",
                    "fresh observed trace state differs from simulator before applying command",
                    diffs=diffs,
                    simulator_summary=_summary(env),
                    observed_summary=_observed_summary(observed_game_state),
                )
                stop_reason = "observed_state_diff"
                break
            if _observed_summary(observed_game_state).get("phase") == "combat":
                try:
                    env = omni.OmniRunEnv.from_communication_mod_state_json(
                        json.dumps(observed_game_state)
                    )
                    env = _env_with_observed_relics(env, observed_game_state)
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
                try:
                    env = omni.OmniRunEnv.from_communication_mod_state_json(
                        json.dumps(observed_game_state)
                    )
                    env = _env_with_observed_relics(env, observed_game_state)
                except Exception:
                    skipped_noncombat_actions += 1
                    continue
                restoration = _restoration_record(
                    step=replayed_steps,
                    record=record,
                    command=command,
                    category="observed_noncombat_boundary",
                    reason="restored unsupported non-combat observed state before continuing replay",
                    simulator_phase=env.phase(),
                    simulator_decision=env.current_decision(),
                    observed_summary=_observed_summary(observed_game_state),
                    diffs=diffs,
                    snapshot_hash=env.snapshot_hash(),
                    snapshot_json=env.snapshot_json(),
                    summary=_summary(env),
                )
                restorations.append(restoration)
                output_records.append(restoration)
                restoration_count += 1
        if has_fresh_observed_state:
            last_state_consumed = True

        if _next_trace_record_is_error(records, record_index):
            continue

        if verb == "STATE":
            continue

        actions = env.exact_legal_actions()
        action = _action_for_communication_command(
            env,
            command,
            observed_game_state,
        )
        if action is None:
            if _is_observed_noop_combat_command(env, records, record_index):
                continue
            if not diagnostic_continue_after_diff:
                blocker = _blocker(
                    record,
                    command,
                    "unsupported_command_mapping",
                    "no exact OmniRunEnv action mapping for CommunicationMod command",
                    simulator_summary=_summary(env),
                    observed_summary=_observed_summary(observed_game_state),
                )
                stop_reason = "unsupported_command_mapping"
                break
            if _observed_summary(observed_game_state).get("phase") != "combat":
                skipped_noncombat_actions += 1
                continue
            if not has_fresh_observed_state:
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
            if not diagnostic_continue_after_diff:
                blocker = _blocker(
                    record,
                    command,
                    "step_error",
                    str(error),
                    simulator_summary=before_summary,
                    observed_summary=_observed_summary(observed_game_state),
                )
                stop_reason = "step_error"
                break
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
    output_records[0]["mode"] = mode
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
    verified = bool(
        verification.get("ok", False)
        and anchor_count == 0
        and restoration_count == 0
        and blocker is None
        and stop_reason == "trace_exhausted"
    )
    report = {
        "schema": 1,
        "source": "sim_trace_guided_replay_report",
        "mode": mode,
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
        "verified": verified,
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
        verified=verified,
        mode=mode,
    )


def strict_replay_real_trace_to_env(*, trace: Path, max_actions: int = 10_000) -> StrictTraceEnvResult:
    """Replay a CommunicationMod trace from START and return the exact final env."""

    records = _read_jsonl(trace)
    env = None
    last_state: dict[str, Any] | None = None
    last_state_consumed = True
    replayed_steps = 0
    blocker: dict[str, Any] | None = None
    stop_reason = "trace_exhausted"
    start_info: dict[str, Any] | None = None

    for record_index, record in enumerate(records):
        if record.get("type") == "state":
            last_state = record
            last_state_consumed = False
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
            continue

        if replayed_steps >= max_actions:
            stop_reason = "max_actions"
            break

        observed_game_state = _game_state_from_record(last_state)
        has_fresh_observed_state = not last_state_consumed
        if has_fresh_observed_state:
            diffs = _observed_summary_diffs(env, observed_game_state)
            if diffs:
                blocker = _blocker(
                    record,
                    command,
                    "observed_state_diff",
                    "fresh observed trace state differs from simulator before applying command",
                    diffs=diffs,
                    simulator_summary=_summary(env),
                    observed_summary=_observed_summary(observed_game_state),
                )
                stop_reason = "observed_state_diff"
                break
            last_state_consumed = True

        if _next_trace_record_is_error(records, record_index):
            continue

        action = _action_for_communication_command(env, command, observed_game_state)
        if action is None:
            if _is_observed_noop_combat_command(env, records, record_index):
                continue
            blocker = _blocker(
                record,
                command,
                "unsupported_command_mapping",
                "no exact OmniRunEnv action mapping for CommunicationMod command",
                simulator_summary=_summary(env),
                observed_summary=_observed_summary(observed_game_state),
            )
            stop_reason = "unsupported_command_mapping"
            break

        try:
            env.step(action)
        except Exception as error:
            blocker = _blocker(
                record,
                command,
                "step_error",
                str(error),
                simulator_summary=_summary(env),
                observed_summary=_observed_summary(observed_game_state),
            )
            stop_reason = "step_error"
            break
        replayed_steps += 1

    latest_observed_summary = None
    if blocker is None and env is not None and last_state is not None and not last_state_consumed:
        observed_game_state = _game_state_from_record(last_state)
        latest_observed_summary = _observed_summary(observed_game_state)
        diffs = _observed_summary_diffs(env, observed_game_state)
        if diffs:
            blocker = _blocker(
                last_state,
                "",
                "final_observed_state_diff",
                "latest observed trace state differs from simulator after replay",
                diffs=diffs,
                simulator_summary=_summary(env),
                observed_summary=latest_observed_summary,
            )
            stop_reason = "final_observed_state_diff"

    verified = bool(env is not None and blocker is None and stop_reason == "trace_exhausted")
    return StrictTraceEnvResult(
        env=env if verified else None,
        trace_path=trace,
        steps=replayed_steps,
        stop_reason=stop_reason,
        verified=verified,
        blocker=blocker,
        start=start_info,
        final_state_id=env.snapshot_hash() if env is not None else None,
        final_phase=env.phase() if env is not None else None,
        latest_observed_summary=latest_observed_summary,
    )


def _is_ignored_invalid_combat_command(
    records: list[dict[str, Any]],
    record_index: int,
    command: str,
    observed_game_state: dict[str, Any],
) -> bool:
    parts = command.strip().split()
    if len(parts) < 2 or parts[0].upper() != "PLAY":
        return False
    try:
        hand_index = int(parts[1])
    except ValueError:
        return False
    hand = observed_game_state.get("hand") or []
    if hand_index <= len(hand):
        return False
    return _next_trace_record_before_state_is_action(records, record_index)


def _next_trace_record_before_state_is_action(
    records: list[dict[str, Any]], record_index: int
) -> bool:
    for next_record in records[record_index + 1 :]:
        record_type = next_record.get("type")
        if record_type == "state":
            return False
        if record_type == "action":
            return True
    return False


def _next_trace_record_is_error(records: list[dict[str, Any]], record_index: int) -> bool:
    if record_index + 1 >= len(records):
        return False
    return records[record_index + 1].get("type") == "error"


def _is_observed_noop_combat_command(
    env: Any,
    records: list[dict[str, Any]],
    record_index: int,
) -> bool:
    if _summary(env).get("phase") != "combat":
        return False
    for next_record in records[record_index + 1 :]:
        record_type = next_record.get("type")
        if record_type != "state":
            continue
        observed = _game_state_from_record(next_record)
        if _observed_summary(observed).get("phase") != "combat":
            return False
        return _observed_summary_diffs(env, observed) == []
    return False


def _next_observed_map_state_matches_current(
    env: Any,
    records: list[dict[str, Any]],
    record_index: int,
) -> bool:
    for next_record in records[record_index + 1 :]:
        if next_record.get("type") != "state":
            continue
        observed = _game_state_from_record(next_record)
        if observed.get("screen_type") != "MAP":
            return False
        return _observed_summary_diffs(env, observed) == []
    return False


def _evaluate_trace_root_task(task: dict[str, Any]) -> dict[str, Any]:
    root: TraceCombatRoot = task["root"]
    candidate: SearchCandidate = task["candidate"]
    root_allowed_potions: tuple[str, ...] | None = task["root_allowed_potions"]
    allowed_potions_mode: str = task["allowed_potions_mode"]
    max_actions: int = task["max_actions"]
    benchmark_root = BenchmarkRoot(
        name=f"{root.trace_path.stem}:step{root.step}:{root.state_id}",
        env_kind="run_combat",
        snapshot_json=root.snapshot_json,
        state_id=root.state_id,
        source_depth=root.step,
        split=root.split,
    )
    result = evaluate_candidate(benchmark_root, candidate, max_actions=max_actions)
    return result.__dict__ | {
        "trace_path": str(root.trace_path),
        "trace_step": root.step,
        "state_id": root.state_id,
        "potion_count": root.potion_count,
        "potion_names": list(root.potion_names),
        "legal_action_kinds": list(root.legal_action_kinds),
        "legal_potion_names": list(root.legal_potion_names),
        "allowed_potions": root_allowed_potions,
        "allowed_potions_mode": allowed_potions_mode,
        "real_trace_potion_use_names": list(root.real_trace_potion_use_names),
        "has_potion_actions": any("potion" in kind for kind in root.legal_action_kinds),
        "has_allowed_potion_actions": _has_allowed_potion_actions(root, root_allowed_potions),
        "real_trace_final_hp": root.real_trace_final_hp,
        "real_trace_hp_loss": root.real_trace_hp_loss,
        "real_trace_terminal_phase": root.real_trace_terminal_phase,
        "hp_loss_delta_vs_trace": (
            result.hp_loss - root.real_trace_hp_loss
            if root.real_trace_hp_loss is not None
            else None
        ),
    }


def _print_eval_progress(
    row: dict[str, Any],
    completed: int,
    total: int,
    progress_stream: TextIO,
) -> None:
    print(
        (
            f"eval progress {completed}/{total}: "
            f"{row['candidate']} {Path(row['trace_path']).name}:step{row['trace_step']} "
            f"won={row['won']} lost={row['lost']} "
            f"terminal={row['terminal_reason'] or 'nonterminal'} "
            f"seconds={row['search_seconds']:.2f}"
        ),
        file=progress_stream,
        flush=True,
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
    allowed_potions_mode: str = "global",
    root_scope: str = "all",
    failure_output: Path | None = None,
    eval_set: str | None = None,
    progress_every: int = 0,
    progress_stream: TextIO | None = None,
    jobs: int = 1,
) -> dict[str, Any]:
    """Compare search candidates from exact combat states recorded in traces."""

    if allowed_potions_mode not in {"global", "trace_used"}:
        raise ValueError(f"unsupported allowed_potions_mode: {allowed_potions_mode}")

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
    root_scope_label = _trace_eval_root_scope_label(root_scope, available_roots)
    candidates = list(candidates or trace_autopilot_candidates())
    if allowed_potions_mode == "global":
        candidates = [
            _candidate_with_allowed_potions(candidate, allowed_potions)
            for candidate in candidates
        ]

    tasks = []
    for candidate in candidates:
        for root in roots:
            root_allowed_potions = _allowed_potions_for_root(
                root,
                allowed_potions=allowed_potions,
                mode=allowed_potions_mode,
            )
            root_candidate = _candidate_with_allowed_potions(candidate, root_allowed_potions)
            tasks.append(
                {
                    "candidate": root_candidate,
                    "root": root,
                    "root_allowed_potions": root_allowed_potions,
                    "allowed_potions_mode": allowed_potions_mode,
                    "max_actions": max_actions,
                }
            )

    if jobs < 1:
        raise ValueError("jobs must be at least 1")

    episodes = []
    completed = 0
    total = len(tasks)
    if jobs == 1 or total == 0:
        row_iter = (_evaluate_trace_root_task(task) for task in tasks)
        for row in row_iter:
            episodes.append(row)
            completed += 1
            if progress_every > 0 and progress_stream is not None:
                if completed == total or completed % progress_every == 0:
                    _print_eval_progress(row, completed, total, progress_stream)
    else:
        with ProcessPoolExecutor(max_workers=jobs) as executor:
            for row in executor.map(_evaluate_trace_root_task, tasks):
                episodes.append(row)
                completed += 1
                if progress_every > 0 and progress_stream is not None:
                    if completed == total or completed % progress_every == 0:
                        _print_eval_progress(row, completed, total, progress_stream)

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
        "jobs": jobs,
        "root_scope": root_scope,
        "root_scope_label": root_scope_label,
        "eval_set": eval_set,
        "eval_set_spec": eval_set_spec,
        "held_out": bool(eval_set_spec.get("held_out")) if eval_set_spec else split == "eval",
        "allowed_potions": allowed_potions,
        "allowed_potions_mode": allowed_potions_mode,
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
            1
            for root in roots
            if _has_allowed_potion_actions(
                root,
                _allowed_potions_for_root(
                    root, allowed_potions=allowed_potions, mode=allowed_potions_mode
                ),
            )
        ),
        "root_manifest": _root_manifest(
            roots, allowed_potions, allowed_potions_mode=allowed_potions_mode
        ),
        "groups": _group_stats(
            roots, episodes, allowed_potions, allowed_potions_mode=allowed_potions_mode
        ),
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


def run_combat_policy_iteration(
    *,
    output_dir: Path,
    train_seeds: Iterable[str],
    dev_seeds: Iterable[str],
    real_trace: Path,
    ascension: int = 0,
    max_steps: int = 220,
    max_actions: int = 120,
    candidates: Iterable[SearchCandidate] | None = None,
) -> CombatPolicyIterationResult:
    """Run a deterministic non-ML policy iteration pass.

    Simulator corpora are train/dev inputs. The strict real trace is evaluated only after
    simulator reports are complete, so it remains a held-out scoreboard.
    """

    output_dir.mkdir(parents=True, exist_ok=True)
    candidates = list(candidates or _default_iteration_candidates())
    candidate_names = [candidate.name for candidate in candidates]
    real_trace_verification = verify_self_play_trace(real_trace)

    def log(message: str) -> None:
        print(f"[iterate-combat-policy] {message}", file=sys.stderr, flush=True)

    train_corpus = output_dir / "train-sim"
    dev_corpus = output_dir / "dev-sim"
    log(f"generating train corpus: {train_corpus}")
    train_batch = run_self_play_batch(
        output_dir=train_corpus,
        seeds=train_seeds,
        ascension=ascension,
        random_seed=17_000,
        max_steps=max_steps,
        combat_policy=DEFAULT_COMBAT_POLICY,
    )
    log(f"generating dev corpus: {dev_corpus}")
    dev_batch = run_self_play_batch(
        output_dir=dev_corpus,
        seeds=dev_seeds,
        ascension=ascension,
        random_seed=23_000,
        max_steps=max_steps,
        combat_policy=DEFAULT_COMBAT_POLICY,
    )

    reports: dict[str, dict[str, Any]] = {}
    for corpus_name, corpus_path in (("train", train_corpus), ("dev", dev_corpus)):
        for root_scope in ("combat_start", "all"):
            key = f"{corpus_name}_{root_scope}"
            report_path = output_dir / f"{key}.json"
            failure_path = output_dir / f"{key}-failures.json"
            log(f"evaluating simulator {key} with {len(candidates)} candidates")
            report = evaluate_self_play_corpus(
                corpus_dir=corpus_path / "traces",
                split="all",
                max_roots=10_000,
                max_actions=max_actions,
                candidates=candidates,
                allowed_potions=None,
                allowed_potions_mode="global",
                root_scope=root_scope,
                failure_output=failure_path,
                progress_every=50,
                progress_stream=sys.stderr,
                jobs=1,
            )
            report_path.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
            reports[key] = _compact_eval_report(report, report_path)

    real_reports: dict[str, dict[str, Any]] = {}
    for root_scope in ("combat_start", "all"):
        key = f"heldout_manual01_{root_scope}"
        report_path = output_dir / f"{key}.json"
        failure_path = output_dir / f"{key}-failures.json"
        log(f"evaluating held-out {key} with trace-used potions")
        report = evaluate_self_play_corpus(
            traces=[real_trace],
            split="all",
            max_roots=10_000,
            max_actions=max_actions,
            candidates=candidates,
            allowed_potions=None,
            allowed_potions_mode="trace_used",
            root_scope=root_scope,
            failure_output=failure_path,
            progress_every=50,
            progress_stream=sys.stderr,
            jobs=1,
        )
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
        real_reports[key] = _compact_eval_report(report, report_path)

    promoted = _promoted_candidate(reports.get("dev_combat_start", {}))
    report_doc = {
        "type": "combat_policy_iteration_report",
        "schema": 1,
        "source": "sim_selfplay_policy_iteration",
        "created_at": _now(),
        "separation": {
            "train": "simulator-generated self-play corpus",
            "dev": "simulator-generated self-play corpus",
            "heldout_eval": "strict MANUAL01 real-trace replay only",
            "heldout_tuning_policy": "MANUAL01 reports are generated after simulator train/dev reports and are not used for candidate promotion.",
        },
        "candidate_names": candidate_names,
        "promoted_candidate_from_dev": promoted,
        "heldout_real_trace_verification": real_trace_verification,
        "train_batch": train_batch.__dict__ | {"output_dir": str(train_batch.output_dir), "index_path": str(train_batch.index_path)},
        "dev_batch": dev_batch.__dict__ | {"output_dir": str(dev_batch.output_dir), "index_path": str(dev_batch.index_path)},
        "sim_reports": reports,
        "heldout_manual01_reports": real_reports,
        "blockers": _iteration_blockers(
            train_batch,
            dev_batch,
            reports,
            real_reports,
            real_trace_verification,
        ),
    }
    report_path = output_dir / "iteration-report.json"
    report_path.write_text(json.dumps(report_doc, indent=2, sort_keys=True), encoding="utf-8")
    return CombatPolicyIterationResult(
        output_dir=output_dir,
        report_path=report_path,
        train_corpus=train_corpus,
        dev_corpus=dev_corpus,
        promoted_candidate=promoted,
    )


def _default_iteration_candidates() -> list[SearchCandidate]:
    names = (
        "rust_terminal_win_hp_bounded_w32_d40",
        "rust_beam_terminal_w32_d40",
        "rust_beam_terminal_w16_d40",
        "rust_greedy_tactical_d40",
    )
    return [trace_autopilot_candidate_by_name(name) for name in names]


def _compact_eval_report(report: dict[str, Any], report_path: Path) -> dict[str, Any]:
    return {
        "report_path": str(report_path),
        "root_scope": report.get("root_scope"),
        "root_scope_label": report.get("root_scope_label"),
        "roots": report.get("roots"),
        "available_roots": report.get("available_roots"),
        "allowed_potions_mode": report.get("allowed_potions_mode"),
        "blocked_traces": report.get("trace_extraction", {}).get("blocked_traces", []),
        "ranking": [
            {
                "candidate": row.get("candidate"),
                "wins": row.get("wins"),
                "losses": row.get("losses"),
                "nonterminal": row.get("nonterminal"),
                "win_rate": row.get("win_rate"),
                "mean_hp_loss": row.get("mean_hp_loss"),
                "median_hp_loss": row.get("median_hp_loss"),
                "mean_real_trace_hp_loss": row.get("mean_real_trace_hp_loss"),
                "mean_hp_loss_delta_vs_trace": row.get("mean_hp_loss_delta_vs_trace"),
                "mean_seconds_per_combat": row.get("mean_seconds_per_combat"),
                "mean_seconds_per_decision": row.get("mean_seconds_per_decision"),
                "mean_search_nodes": row.get("mean_search_nodes"),
                "total_potion_uses": row.get("total_potion_uses"),
            }
            for row in report.get("ranking", [])
        ],
        "worst_regressions": _episode_extremes(report, reverse=True),
        "best_improvements": _episode_extremes(report, reverse=False),
    }


def _episode_extremes(report: dict[str, Any], *, reverse: bool) -> list[dict[str, Any]]:
    episodes = [
        episode
        for episode in report.get("episodes", [])
        if episode.get("hp_loss_delta_vs_trace") is not None
    ]
    episodes.sort(key=lambda episode: float(episode["hp_loss_delta_vs_trace"]), reverse=reverse)
    return [
        {
            "candidate": episode.get("candidate"),
            "trace_step": episode.get("trace_step"),
            "state_id": episode.get("state_id"),
            "human_hp_loss": episode.get("real_trace_hp_loss"),
            "policy_hp_loss": episode.get("hp_loss"),
            "delta": episode.get("hp_loss_delta_vs_trace"),
            "won": episode.get("won"),
            "lost": episode.get("lost"),
            "terminal_reason": episode.get("terminal_reason"),
            "search_seconds": episode.get("search_seconds"),
        }
        for episode in episodes[:5]
    ]


def _promoted_candidate(compact_report: dict[str, Any]) -> str | None:
    ranking = compact_report.get("ranking") or []
    if not ranking:
        return None
    return str(ranking[0].get("candidate"))


def _iteration_blockers(
    train_batch: CorpusBatchResult,
    dev_batch: CorpusBatchResult,
    sim_reports: dict[str, dict[str, Any]],
    real_reports: dict[str, dict[str, Any]],
    real_trace_verification: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    if not real_trace_verification.get("ok"):
        blockers.append(
            {
                "scope": "heldout:manual01",
                "reason": "strict_real_trace_replay_not_clean",
                "verification": real_trace_verification,
            }
        )
    for name, batch in (("train", train_batch), ("dev", dev_batch)):
        if batch.verified_count != batch.trace_count:
            blockers.append(
                {
                    "scope": name,
                    "reason": "not_all_simulator_traces_verified",
                    "trace_count": batch.trace_count,
                    "verified_count": batch.verified_count,
                    "stop_reasons": batch.stop_reasons,
                }
            )
    for family, reports in (("sim", sim_reports), ("heldout", real_reports)):
        for name, report in reports.items():
            blocked = report.get("blocked_traces") or []
            if blocked:
                blockers.append(
                    {
                        "scope": f"{family}:{name}",
                        "reason": "blocked_trace_extraction",
                        "blocked_traces": blocked,
                    }
                )
    return blockers


def _trace_eval_root_scope_label(root_scope: str, available_roots: int) -> str:
    if root_scope == "combat_start":
        return f"combat_start_{available_roots}"
    if root_scope == "all":
        return f"all_decision_states_{available_roots}"
    return f"{root_scope}_{available_roots}"


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
                    real_trace_potion_use_names=tuple(baseline.get("potion_use_names", ())),
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
        baseline = {
            "potion_use_names": _real_trace_potion_use_names(records, index),
        }
        initial_hp = _summary_player_hp(summary)
        if initial_hp is None:
            baselines[state_id] = baseline
            continue
        final_summary = _real_trace_combat_end_summary(records, index, summary)
        if final_summary is None:
            baselines[state_id] = baseline
            continue
        final_hp = _summary_player_hp(final_summary)
        if final_hp is None:
            baselines[state_id] = baseline
            continue
        baseline |= {
            "final_hp": final_hp,
            "hp_loss": initial_hp - final_hp,
            "terminal_phase": final_summary.get("phase"),
        }
        baselines[state_id] = baseline
    return baselines


def _real_trace_potion_use_names(records: list[dict[str, Any]], start_index: int) -> tuple[str, ...]:
    names: list[str] = []
    for record in records[start_index:]:
        before_summary: dict[str, Any] | None = None
        after_summary: dict[str, Any] | None = None
        if record.get("type") == "step":
            before = record.get("before_summary")
            after = record.get("after_summary")
            if isinstance(before, dict):
                before_summary = before
            if isinstance(after, dict):
                after_summary = after
        elif record.get("type") == "anchor":
            summary = record.get("summary")
            if isinstance(summary, dict):
                before_summary = summary
        if before_summary is not None and before_summary.get("phase") != "combat":
            return tuple(names)

        if record.get("type") == "step" and record.get("action_kind") == "use_potion":
            potions = before_summary.get("potions") if isinstance(before_summary, dict) else None
            if isinstance(potions, list):
                name = _potion_name_for_step_record(
                    record, tuple(str(potion) for potion in potions)
                )
                if name is not None:
                    names.append(name)

        if after_summary is not None and after_summary.get("phase") != "combat":
            return tuple(names)
    return tuple(names)


def _potion_name_for_step_record(record: dict[str, Any], potion_names: tuple[str, ...]) -> str | None:
    action_json = record.get("action_json")
    if not isinstance(action_json, str):
        return None
    return _potion_name_for_action({"kind": "use_potion", "json": action_json}, potion_names)


def _real_trace_combat_end_summary(
    records: list[dict[str, Any]],
    start_index: int,
    start_summary: dict[str, Any],
) -> dict[str, Any] | None:
    last_combat_summary: dict[str, Any] | None = None
    start_floor = start_summary.get("floor")
    for record in records[start_index:]:
        summaries: list[tuple[dict[str, Any], bool]] = []
        if record.get("type") == "step":
            before = record.get("before_summary")
            after = record.get("after_summary")
            if isinstance(before, dict):
                summaries.append((before, False))
            if isinstance(after, dict):
                summaries.append((after, False))
        elif record.get("type") == "anchor":
            summary = record.get("summary")
            if isinstance(summary, dict):
                summaries.append((summary, True))
        for summary, is_anchor in summaries:
            if summary.get("phase") == "combat":
                if (
                    last_combat_summary is not None
                    and start_floor is not None
                    and summary.get("floor") != start_floor
                ):
                    return _terminal_combat_summary(last_combat_summary)
                if (
                    is_anchor
                    and last_combat_summary is not None
                    and _summary_player_hp(summary) != _summary_player_hp(last_combat_summary)
                ):
                    return None
                last_combat_summary = summary
                continue
            if last_combat_summary is not None:
                return summary
    return _terminal_combat_summary(last_combat_summary)


def _terminal_combat_summary(summary: dict[str, Any] | None) -> dict[str, Any] | None:
    if summary is None:
        return None
    combat = summary.get("combat")
    if not isinstance(combat, dict):
        return None
    monsters = combat.get("monsters")
    if not isinstance(monsters, list):
        return None
    if all(isinstance(monster, dict) and not monster.get("alive", False) for monster in monsters):
        return summary
    return None


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
        real_trace_baseline_roots = sum(
            1
            for episode in candidate_episodes
            if episode.get("real_trace_hp_loss") is not None
        )
        row = {
            "candidate": name,
            "episodes": count,
            "wins": wins,
            "losses": losses,
            "nonterminal": nonterminal,
            "real_trace_baseline_roots": real_trace_baseline_roots,
            "missing_real_trace_baseline_roots": count - real_trace_baseline_roots,
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
                "decision_trace": list(episode.get("decision_trace") or []),
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


def _env_with_observed_relics(env: Any, observed: dict[str, Any]) -> Any:
    snapshot = _snapshot_with_observed_relics(env.snapshot_json(), observed)
    if snapshot is None:
        return env
    return omni.OmniRunEnv.from_snapshot_json(snapshot)


def _snapshot_with_observed_relics(snapshot_json: str, observed: dict[str, Any]) -> str | None:
    relics = _observed_snapshot_relics(observed)
    if not relics:
        return None

    snapshot = json.loads(snapshot_json)
    run = snapshot.get("state") if isinstance(snapshot.get("state"), dict) else snapshot
    if not isinstance(run, dict):
        return None

    run["relics"] = relics
    combat = run.get("combat")
    if isinstance(combat, dict):
        combat["relics"] = relics
        counters = combat.get("relic_counters")
        if not isinstance(counters, dict):
            counters = {}
        counters.update(_observed_relic_counters(observed))
        combat["relic_counters"] = counters
    return json.dumps(snapshot, sort_keys=True)


def _observed_snapshot_relics(observed: dict[str, Any]) -> list[str]:
    relics = []
    for relic in observed.get("relics") or []:
        if not isinstance(relic, dict):
            continue
        snapshot_name = _snapshot_relic_name(relic.get("name") or relic.get("id"))
        if snapshot_name and snapshot_name not in relics:
            relics.append(snapshot_name)
    return relics


def _observed_relic_counters(observed: dict[str, Any]) -> dict[str, int]:
    counters = {}
    for relic in observed.get("relics") or []:
        if not isinstance(relic, dict):
            continue
        counter = relic.get("counter")
        if not isinstance(counter, int) or counter < 0:
            continue
        field = _RELIC_COUNTER_FIELDS.get(_snapshot_relic_name(relic.get("name") or relic.get("id")))
        if field:
            counters[field] = counter
    return counters


def _snapshot_relic_name(name: Any) -> str | None:
    if not isinstance(name, str):
        return None
    normalized = "".join(char for char in name.title() if char.isalnum())
    aliases = {
        "CaptainSWheel": "CaptainsWheel",
        "CharonSAshes": "CharonsAshes",
        "DollySMirror": "DollysMirror",
        "DuVuDoll": "DuVuDoll",
        "FrozenEgg2": "FrozenEgg",
        "LeeSWaffle": "LeesWaffle",
        "LeesWaffle": "LeesWaffle",
        "NeowsLament": None,
        "NLothSGift": None,
        "NlothsGift": None,
        "NlotHsGift": None,
        "PaperPhrog": "PaperPhrog",
        "PandoraSBox": "PandorasBox",
        "PandorasBox": "PandorasBox",
        "PhilosopherSStone": "PhilosophersStone",
        "SelfFormingClay": "SelfFormingClay",
        "SlaverSCollar": "SlaversCollar",
        "Stonecalendar": "StoneCalendar",
        "TheCourier": "TheCourier",
    }
    normalized = aliases.get(normalized, normalized)
    if normalized in _SNAPSHOT_RELIC_NAMES:
        return normalized
    return None


_RELIC_COUNTER_FIELDS = {
    "HappyFlower": "happy_flower_turns",
    "IncenseBurner": "incense_burner_counter",
    "InkBottle": "ink_bottle_cards_played",
    "Kunai": "kunai_attacks_this_turn",
    "LetterOpener": "letter_opener_skills_this_turn",
    "Nunchaku": "nunchaku_attacks_played",
    "PenNib": "pen_nib_attacks_played",
    "Pocketwatch": "cards_played_this_turn",
    "Shuriken": "shuriken_attacks_this_turn",
    "StoneCalendar": "player_turns_started",
}


_SNAPSHOT_RELIC_NAMES = {
    "Akabeko",
    "Anchor",
    "AncientTeaSet",
    "ArtOfWar",
    "Astrolabe",
    "BagOfMarbles",
    "BagOfPreparation",
    "BirdFacedUrn",
    "BlackBlood",
    "BlackStar",
    "BloodVial",
    "BloodyIdol",
    "BlueCandle",
    "BottledFlame",
    "BottledLightning",
    "BottledTornado",
    "BronzeScales",
    "Brimstone",
    "BurningBlood",
    "BustedCrown",
    "CallingBell",
    "Calipers",
    "CaptainsWheel",
    "Cauldron",
    "CentennialPuzzle",
    "CeramicFish",
    "ChampionBelt",
    "CharonsAshes",
    "ChemicalX",
    "Circlet",
    "ClockworkSouvenir",
    "CoffeeDripper",
    "CrackedCore",
    "CursedKey",
    "DarkstonePeriapt",
    "DeadBranch",
    "DollysMirror",
    "DreamCatcher",
    "DuVuDoll",
    "Ectoplasm",
    "EmptyCage",
    "EternalFeather",
    "FossilizedHelix",
    "FrozenCore",
    "FrozenEgg",
    "FrozenEye",
    "FusionHammer",
    "GamblingChip",
    "Ginger",
    "Girya",
    "GoldenIdol",
    "GremlinHorn",
    "HandDrill",
    "HappyFlower",
    "HolyWater",
    "HornCleat",
    "IceCream",
    "IncenseBurner",
    "InkBottle",
    "JuzuBracelet",
    "Kunai",
    "Lantern",
    "LeesWaffle",
    "LetterOpener",
    "LizardTail",
    "MagicFlower",
    "Mango",
    "MarkOfPain",
    "Matryoshka",
    "MawBank",
    "MealTicket",
    "MeatOnTheBone",
    "MedicalKit",
    "MembershipCard",
    "MercuryHourglass",
    "MoltenEgg",
    "MummifiedHand",
    "MutagenicStrength",
    "Nunchaku",
    "OddlySmoothStone",
    "OldCoin",
    "Omamori",
    "OrangePellets",
    "Orichalcum",
    "Orrery",
    "PandorasBox",
    "Pantograph",
    "PaperPhrog",
    "PeacePipe",
    "Pear",
    "PenNib",
    "PhilosophersStone",
    "Pocketwatch",
    "PotionBelt",
    "PrayerWheel",
    "PreservedInsect",
    "PrismaticShard",
    "PureWater",
    "QuestionCard",
    "RedCirclet",
    "RedSkull",
    "RegalPillow",
    "RingOfTheSerpent",
    "RingOfTheSnake",
    "RunicCube",
    "RunicDome",
    "RunicPyramid",
    "SacredBark",
    "SelfFormingClay",
    "Shovel",
    "Shuriken",
    "SingingBowl",
    "SlaversCollar",
    "SlingOfCourage",
    "SmilingMask",
    "SneckoEye",
    "Sozu",
    "StoneCalendar",
    "StrangeSpoon",
    "Strawberry",
    "StrikeDummy",
    "Sundial",
    "TheAbacus",
    "TheBoot",
    "TheCourier",
    "ThreadAndNeedle",
    "TinyChest",
    "TinyHouse",
    "Toolbox",
    "Torii",
    "ToxicEgg",
    "ToyOrnithopter",
    "TungstenRod",
    "Turnip",
    "UnceasingTop",
    "Vajra",
    "VelvetChoker",
    "WarPaint",
    "Whetstone",
    "WhiteBeastStatue",
    "WingBoots",
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
    visible_hand = _visible_combat_hand(combat)
    piles = combat.get("piles") or {}
    reward = run.get("reward") or {}
    visible_reward_choices = (
        reward.get("choices") or [] if reward.get("card_reward_active") else []
    )
    decision = env.current_decision()
    phase = decision if decision in {"grid", "map"} else env.phase()
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
        "relic_count": len(run.get("relics") or []),
        "map": _simulator_map_summary(run),
        "reward": {
            "choices": [card.get("content_id") for card in visible_reward_choices],
            "choice_count": len(visible_reward_choices),
            "gold_offer": None if reward.get("card_reward_active") else reward.get("gold_offer"),
            "stolen_gold_offer": None
            if reward.get("card_reward_active")
            else reward.get("stolen_gold_offer"),
            "potion_offer": None
            if reward.get("card_reward_active")
            else reward.get("potion_offer"),
            "relic_offer": None
            if reward.get("card_reward_active")
            else reward.get("relic_offer") or reward.get("relic_key_offer"),
            "card_reward_active": reward.get("card_reward_active"),
            "card_reward_pending": None
            if reward.get("card_reward_active")
            else reward.get("card_reward_pending"),
            "pending_card_reward_count": None
            if reward.get("card_reward_active")
            else reward.get("pending_card_reward_count"),
        }
        if reward
        else None,
        "combat": {
            "energy": player.get("energy"),
            "hand": [card.get("content_id") for card in visible_hand],
            "hand_count": len(visible_hand),
            "draw_pile": [card.get("content_id") for card in piles.get("draw_pile") or []],
            "discard_pile": [
                card.get("content_id") for card in piles.get("discard_pile") or []
            ],
            "exhaust_pile": [
                card.get("content_id") for card in piles.get("exhaust_pile") or []
            ],
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


def _visible_combat_hand(combat: dict[str, Any]) -> list[dict[str, Any]]:
    hand = list(((combat.get("piles") or {}).get("hand") or []))
    selected_indices: set[int] = set()
    exhaust_select = combat.get("exhaust_select")
    if isinstance(exhaust_select, dict):
        selected_indices.update(
            index
            for index in exhaust_select.get("selected_hand_indices") or []
            if isinstance(index, int)
        )
    hand_select = combat.get("hand_select")
    if isinstance(hand_select, dict):
        index = hand_select.get("selected_hand_index")
        if isinstance(index, int):
            selected_indices.add(index)
    if not selected_indices:
        return hand
    return [card for index, card in enumerate(hand) if index not in selected_indices]


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


def _action_for_communication_command(
    env: Any,
    command: str,
    observed: dict[str, Any],
) -> Any | None:
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
        return _choose_action_for_index(env, actions, observed, index)
    if verb == "PLAY" and len(parts) >= 2:
        try:
            hand_index = int(parts[1])
            target_index = int(parts[2]) if len(parts) >= 3 else None
        except ValueError:
            return None
        return _play_action_for_indices(
            env,
            actions,
            hand_index,
            target_index,
        )
    if verb == "POTION" and len(parts) >= 2:
        try:
            if parts[1].upper() == "USE":
                slot = int(parts[2])
                target_index = int(parts[3]) if len(parts) >= 4 else None
            else:
                slot = int(parts[1])
                target_index = int(parts[2]) if len(parts) >= 3 else None
        except ValueError:
            return None
        except IndexError:
            return None
        return _potion_action_for_indices(env, actions, slot, target_index)
    if verb in {"PROCEED", "LEAVE", "RETURN", "SKIP"}:
        return _screen_exit_action(actions)
    if verb == "CONFIRM":
        return _confirm_action(actions)
    if verb in {"STATE", "WAIT"}:
        return None
    _ = observed
    return None


def _choose_action_for_index(
    env: Any, actions: list[Any], observed: dict[str, Any], index: int
) -> Any | None:
    decision = env.current_decision()
    phase = decision if decision == "grid" or env.phase() == "idle" else env.phase()
    if phase == "event":
        for action in actions:
            data = _action_json_data(action)
            choose = data.get("Choose") if isinstance(data, dict) else None
            if isinstance(choose, dict) and choose.get("choice_index") == index:
                return action
    if phase == "combat":
        indexed_kinds = {
            "choose_combat_card_reward",
            "choose_hand_select",
            "choose_draw_select",
            "choose_discard_select",
            "choose_exhaust_select",
        }
        for action in actions:
            kind = getattr(action, "kind", lambda: "")()
            if kind not in indexed_kinds:
                continue
            effective_index = index
            if kind == "choose_exhaust_select":
                mapped = _exhaust_select_original_index(env, index)
                if mapped is None:
                    continue
                effective_index = mapped
            data = _action_json_data(action)
            payload = next((value for value in data.values() if isinstance(value, dict)), {})
            if payload.get("index") == effective_index:
                return action
    if phase == "reward":
        return _reward_choose_action(actions, observed, index)
    if phase == "grid":
        grid_actions = [
            action
            for action in actions
            if getattr(action, "kind", lambda: "")() == "select_grid_card"
        ]
        for action in actions:
            if getattr(action, "kind", lambda: "")() != "select_grid_card":
                continue
            data = _action_json_data(action)
            select = data.get("SelectGridCard") if isinstance(data, dict) else None
            if isinstance(select, dict) and select.get("index") == index:
                return action
        if index > 0 and index - 1 < len(grid_actions):
            return grid_actions[index - 1]
    if phase == "map":
        mapped = _map_choose_action_for_x(actions, observed, index)
        if mapped is not None:
            return mapped
        if 0 <= index < len(actions):
            return actions[index]
    if phase in {"shop", "rest", "treasure"} and 0 <= index < len(actions):
        return actions[index]
    return None


def _map_choose_action_for_x(
    actions: list[Any], observed: dict[str, Any], chosen_x: int
) -> Any | None:
    screen_state = observed.get("screen_state") if isinstance(observed, dict) else {}
    next_nodes = screen_state.get("next_nodes") if isinstance(screen_state, dict) else None
    desired_nodes = []
    if isinstance(next_nodes, list):
        desired_nodes = [
            node
            for node in next_nodes
            if isinstance(node, dict) and node.get("x") == chosen_x
        ]
    choices = observed.get("choice_list") if isinstance(observed, dict) else None
    if not desired_nodes and isinstance(choices, list) and f"x={chosen_x}" in choices:
        desired_nodes = [{"x": chosen_x}]

    for desired in desired_nodes:
        desired_y = desired.get("y")
        for action in actions:
            node_id = _map_action_node_id(action)
            if node_id is None or node_id <= 0:
                continue
            x = (node_id - 1) % 7
            y = (node_id - 1) // 7
            if x == chosen_x and (desired_y is None or y == desired_y):
                return action
    return None


def _map_action_node_id(action: Any) -> int | None:
    data = _action_json_data(action)
    choose = data.get("ChooseNode") if isinstance(data, dict) else None
    if not isinstance(choose, dict):
        return None
    node_id = choose.get("node_id")
    return node_id if isinstance(node_id, int) else None


def _exhaust_select_original_index(env: Any, visible_index: int) -> int | None:
    try:
        state = json.loads(env.state_json())
    except Exception:
        return visible_index
    combat = state.get("combat") if isinstance(state.get("combat"), dict) else {}
    hand = ((combat.get("piles") or {}).get("hand") or [])
    select = combat.get("exhaust_select") if isinstance(combat.get("exhaust_select"), dict) else {}
    selected = set(select.get("selected_hand_indices") or [])
    visible_to_original = [
        original_index
        for original_index in range(len(hand))
        if original_index not in selected
    ]
    if visible_index < 0 or visible_index >= len(visible_to_original):
        return None
    return visible_to_original[visible_index]


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
    hand_index = hand_index - 1
    if hand_index < 0 or hand_index >= len(hand):
        return None
    card_id = hand[hand_index].get("id")
    target_id = None
    if target_index is not None:
        target_id = _monster_id_for_visible_index(combat, target_index)
        if target_id is None:
            return None
    for action in actions:
        if getattr(action, "kind", lambda: "")() != "play_card":
            continue
        play = _action_json_data(action).get("PlayCard")
        if not isinstance(play, dict) or play.get("card_id") != card_id:
            continue
        if target_id is None:
            if play.get("target") is None:
                return action
            continue
        if play.get("target") == target_id or play.get("target") is None:
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
        target_id = _monster_id_for_visible_index({"monsters": monsters}, target_index)
        if target_id is None:
            return None
    same_slot_targetless = None
    legal_potion_actions = []
    for action in actions:
        if getattr(action, "kind", lambda: "")() != "use_potion":
            continue
        legal_potion_actions.append(action)
        use = _action_json_data(action).get("UsePotion")
        if not isinstance(use, dict) or use.get("slot") != slot:
            continue
        if target_id is None or use.get("target") == target_id:
            return action
        if use.get("target") is None:
            same_slot_targetless = action
    if same_slot_targetless is not None:
        return same_slot_targetless
    if len(legal_potion_actions) == 1:
        return legal_potion_actions[0]
    return None


def _monster_id_for_visible_index(combat: dict[str, Any], target_index: int) -> int | None:
    monsters = combat.get("monsters") or []
    if target_index < 0 or target_index >= len(monsters):
        return None
    monster_id = monsters[target_index].get("id")
    return monster_id if isinstance(monster_id, int) else None


def _first_action(actions: list[Any], *, kind: str) -> Any | None:
    return next((action for action in actions if getattr(action, "kind", lambda: "")() == kind), None)


def _screen_exit_action(actions: list[Any]) -> Any | None:
    preferred = [
        "close_card_reward",
        "skip_reward",
        "leave_shop_room",
        "leave_shop",
        "cancel_grid",
        "return",
        "rest_proceed",
        "proceed",
    ]
    for kind in preferred:
        action = _first_action(actions, kind=kind)
        if action is not None:
            return action
    if len(actions) == 1 and getattr(actions[0], "kind", lambda: "")() == "event_choose":
        return actions[0]
    return None


def _reward_choose_action(actions: list[Any], observed: dict[str, Any], index: int) -> Any | None:
    choices = observed.get("choice_list") if isinstance(observed, dict) else None
    label = None
    if isinstance(choices, list) and 0 <= index < len(choices):
        label = str(choices[index]).lower()
    if label in {"gold", "stolen_gold"}:
        return _first_action(actions, kind="take_gold_reward") or _first_action(
            actions, kind="take_stolen_gold_reward"
        )
    if label == "potion":
        take_potion = _first_action(actions, kind="take_potion_reward")
        if take_potion is not None:
            return take_potion
        if _first_action(actions, kind="take_gold_reward") is not None and index > 0:
            return _first_action(actions, kind="take_gold_reward")
        return _first_action(actions, kind="skip_potion_reward")
    if label == "relic":
        return _first_action(actions, kind="take_relic_reward")
    if label == "card":
        return _first_action(actions, kind="open_card_reward") or _first_action(
            actions, kind="take_card_reward"
        )
    card_picks = [action for action in actions if getattr(action, "kind", lambda: "")() == "take_card_reward"]
    if card_picks and 0 <= index < len(card_picks):
        return card_picks[index]
    boss_relic_picks = [
        action
        for action in actions
        if getattr(action, "kind", lambda: "")() == "choose_boss_relic_reward"
    ]
    if boss_relic_picks and 0 <= index < len(boss_relic_picks):
        return boss_relic_picks[index]
    if 0 <= index < len(actions):
        return actions[index]
    return None


def _confirm_action(actions: list[Any]) -> Any | None:
    for action in actions:
        if getattr(action, "kind", lambda: "")().startswith("confirm_"):
            return action
    return None


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
    for key in (
        "phase",
        "player_hp",
        "player_max_hp",
        "gold",
        "floor",
        "act",
        "potion_count",
        "relic_count",
    ):
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
    if observed_summary.get("potions") is not None:
        simulator_potions = [
            _normalize_potion_name(str(name)) for name in simulator.get("potions") or []
        ]
        observed_potions = [
            _normalize_potion_name(str(name)) for name in observed_summary.get("potions") or []
        ]
        if simulator_potions != observed_potions:
            diffs.append(
                {
                    "field": "potions",
                    "simulator": simulator.get("potions") or [],
                    "observed": observed_summary.get("potions") or [],
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
        observed_sim_summary = _observed_import_summary(observed)
        if observed_sim_summary:
            obs_import_combat = observed_sim_summary.get("combat") or {}
            if sim_combat.get("hand") != obs_import_combat.get("hand"):
                diffs.append(
                    {
                        "field": "combat.hand",
                        "simulator": sim_combat.get("hand"),
                        "observed": obs_import_combat.get("hand"),
                    }
                )
            for pile_name in ("draw_pile", "discard_pile", "exhaust_pile"):
                if sim_combat.get(pile_name) != obs_import_combat.get(pile_name):
                    diffs.append(
                        {
                            "field": f"combat.{pile_name}",
                            "simulator": sim_combat.get(pile_name),
                            "observed": obs_import_combat.get(pile_name),
                        }
                    )
            diffs.extend(_combat_monster_summary_diffs(sim_combat, obs_import_combat))
    if simulator.get("phase") == "reward" and observed_summary.get("phase") == "reward":
        observed_sim_summary = _observed_import_summary(observed)
        if observed_sim_summary:
            sim_reward = simulator.get("reward") or {}
            obs_reward = observed_sim_summary.get("reward") or {}
            for key in (
                "choices",
                "choice_count",
                "gold_offer",
                "stolen_gold_offer",
                "potion_offer",
                "relic_offer",
                "card_reward_active",
                "card_reward_pending",
                "pending_card_reward_count",
            ):
                if sim_reward.get(key) != obs_reward.get(key):
                    diffs.append(
                        {
                            "field": f"reward.{key}",
                            "simulator": sim_reward.get(key),
                            "observed": obs_reward.get(key),
                        }
                    )
    if simulator.get("phase") == "map" and observed_summary.get("phase") == "map":
        sim_map = simulator.get("map") or {}
        obs_map = observed_summary.get("map") or {}
        for key in ("current_x", "current_y"):
            if obs_map.get(key) is None:
                continue
            if sim_map.get(key) != obs_map.get(key):
                diffs.append(
                    {
                        "field": f"map.{key}",
                        "simulator": sim_map.get(key),
                        "observed": obs_map.get(key),
                    }
                )
    return diffs


def _observed_import_summary(observed: dict[str, Any]) -> dict[str, Any] | None:
    try:
        summary = _summary(omni.OmniRunEnv.from_communication_mod_state_json(json.dumps(observed)))
    except Exception:
        return None
    _drop_unknown_observed_monster_intents(summary, observed)
    return summary


def _drop_unknown_observed_monster_intents(
    summary: dict[str, Any], observed: dict[str, Any]
) -> None:
    raw_combat = observed.get("combat_state") if isinstance(observed.get("combat_state"), dict) else {}
    raw_monsters = raw_combat.get("monsters") or []
    summary_monsters = ((summary.get("combat") or {}).get("monsters") or [])
    for raw_monster, summary_monster in zip(raw_monsters, summary_monsters):
        raw_intent = str(raw_monster.get("intent") or "").upper()
        if raw_intent in {"DEBUG", "UNKNOWN", "DEFEND_BUFF"}:
            summary_monster["intent"] = None
        if (
            str(raw_monster.get("name") or "") == "Hexaghost"
            and raw_intent == "ATTACK_DEBUFF"
            and int(raw_monster.get("move_id") or -1) == 4
            and isinstance(summary_monster.get("intent"), dict)
            and isinstance(summary_monster["intent"].get("AddBurnToDiscard"), dict)
        ):
            summary_monster["intent"]["AddBurnToDiscard"]["count"] = 1


def _combat_monster_summary_diffs(
    simulator_combat: dict[str, Any], observed_combat: dict[str, Any]
) -> list[dict[str, Any]]:
    diffs: list[dict[str, Any]] = []
    sim_monsters = simulator_combat.get("monsters") or []
    obs_monsters = observed_combat.get("monsters") or []
    for index, (sim_monster, obs_monster) in enumerate(zip(sim_monsters, obs_monsters)):
        for key in ("alive", "hp", "block", "intent"):
            if obs_monster.get(key) is None:
                continue
            if sim_monster.get(key) != obs_monster.get(key):
                diffs.append(
                    {
                        "field": f"combat.monsters[{index}].{key}",
                        "simulator": sim_monster.get(key),
                        "observed": obs_monster.get(key),
                    }
                )
    return diffs


def _observed_summary(observed: dict[str, Any]) -> dict[str, Any]:
    combat = observed.get("combat_state") if isinstance(observed.get("combat_state"), dict) else {}
    player = combat.get("player") if isinstance(combat.get("player"), dict) else {}
    observed_potions = _observed_real_potions(observed)
    potion_names = [
        str(potion.get("name") or potion.get("id") or "unknown") for potion in observed_potions
    ]
    potion_count = len(observed_potions)
    relic_count = (
        len(_observed_snapshot_relics(observed)) if isinstance(observed.get("relics"), list) else None
    )
    return {
        "phase": _observed_phase(observed),
        "player_hp": observed.get("current_hp") or observed.get("player_hp") or player.get("current_hp"),
        "player_max_hp": observed.get("max_hp") or observed.get("player_max_hp") or player.get("max_hp"),
        "gold": observed.get("gold"),
        "floor": observed.get("floor"),
        "act": observed.get("act"),
        "potions": potion_names,
        "potion_count": potion_count,
        "relic_count": relic_count,
        "map": _observed_map_summary(observed),
        "combat": {
            "energy": player.get("energy") if player.get("energy") is not None else combat.get("energy"),
            "hand_count": len(combat.get("hand") or []),
            "monster_count": len(combat.get("monsters") or []),
        }
        if combat
        else None,
    }


def _simulator_map_summary(run: dict[str, Any]) -> dict[str, Any] | None:
    map_state = run.get("map") if isinstance(run.get("map"), dict) else None
    if not map_state:
        return None
    current_node = map_state.get("current_node")
    if not isinstance(current_node, int):
        return None
    nodes = ((map_state.get("map") or {}).get("nodes") or [])
    current = next(
        (node for node in nodes if isinstance(node, dict) and node.get("id") == current_node),
        {},
    )
    children = current.get("children") if isinstance(current, dict) else None
    return {
        "current_node": current_node,
        "current_x": _target_map_node_x(current_node),
        "current_y": _target_map_node_y(current_node),
        "next_xs": [
            _target_map_node_x(child)
            for child in children
            if isinstance(child, int) and _target_map_node_x(child) is not None
        ]
        if isinstance(children, list)
        else None,
    }


def _observed_map_summary(observed: dict[str, Any]) -> dict[str, Any] | None:
    screen_state = observed.get("screen_state") if isinstance(observed, dict) else None
    if not isinstance(screen_state, dict):
        return None
    current = screen_state.get("current_node")
    next_nodes = screen_state.get("next_nodes")
    boss_available = screen_state.get("boss_available") is True
    next_xs = [
        node.get("x")
        for node in next_nodes
        if isinstance(node, dict) and isinstance(node.get("x"), int)
    ] if isinstance(next_nodes, list) else None
    if boss_available and not next_xs:
        next_xs = [0]
    current_x = current.get("x") if isinstance(current, dict) else None
    current_y = current.get("y") if isinstance(current, dict) else None
    if current_x == -1 and current_y == 15 and next_xs:
        current_x = 0
        current_y = -1
    return {
        "current_x": current_x,
        "current_y": current_y,
        "next_xs": next_xs,
    }


def _target_map_node_x(node_id: int) -> int | None:
    if node_id == 0:
        return 0
    if node_id < 0:
        return None
    return (node_id - 1) % 7


def _target_map_node_y(node_id: int) -> int | None:
    if node_id <= 0:
        return -1
    return (node_id - 1) // 7


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
    if screen_type == "CHEST":
        return "treasure"
    if screen_type == "GRID":
        return "grid"
    if screen_type in {"CARD_REWARD", "COMBAT_REWARD", "BOSS_REWARD"}:
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


def _allowed_potions_for_root(
    root: TraceCombatRoot,
    *,
    allowed_potions: tuple[str, ...] | None,
    mode: str,
) -> tuple[str, ...] | None:
    if mode == "global":
        return allowed_potions
    if mode == "trace_used":
        return root.real_trace_potion_use_names
    raise ValueError(f"unsupported allowed_potions_mode: {mode}")


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
    *,
    allowed_potions_mode: str = "global",
) -> dict[str, Any]:
    return {
        "all": _group_stat_for_roots(roots, episodes),
        "potion": _group_stat_for_roots(
            [root for root in roots if root.potion_count > 0],
            episodes,
        ),
        "allowed_potion": _group_stat_for_roots(
            [
                root
                for root in roots
                if _has_allowed_potion_actions(
                    root,
                    _allowed_potions_for_root(
                        root, allowed_potions=allowed_potions, mode=allowed_potions_mode
                    ),
                )
            ],
            episodes,
        ),
    }


def _root_manifest(
    roots: list[TraceCombatRoot],
    allowed_potions: tuple[str, ...] | None,
    *,
    allowed_potions_mode: str = "global",
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
            "allowed_potions": _allowed_potions_for_root(
                root, allowed_potions=allowed_potions, mode=allowed_potions_mode
            ),
            "allowed_potions_mode": allowed_potions_mode,
            "has_allowed_potion_actions": _has_allowed_potion_actions(
                root,
                _allowed_potions_for_root(
                    root, allowed_potions=allowed_potions, mode=allowed_potions_mode
                ),
            ),
            "real_trace_potion_use_names": list(root.real_trace_potion_use_names),
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
    eval_parser.add_argument(
        "--allowed-potions-mode",
        choices=["global", "trace_used"],
        default="global",
    )
    eval_parser.add_argument("--root-scope", choices=["all", "combat_start"], default="all")
    eval_parser.add_argument("--eval-set", choices=sorted(TRACE_EVAL_SET_SPECS))
    eval_parser.add_argument("--candidate", dest="candidate_names", action="append")
    eval_parser.add_argument("--progress-every", type=int, default=0)
    eval_parser.add_argument("--jobs", type=int, default=1)
    eval_parser.add_argument("--output", type=Path)
    eval_parser.add_argument("--failure-output", type=Path)

    iteration_parser = subparsers.add_parser("iterate-combat-policy")
    iteration_parser.add_argument("--output-dir", type=Path, required=True)
    iteration_parser.add_argument("--train-seeds", nargs="+", required=True)
    iteration_parser.add_argument("--dev-seeds", nargs="+", required=True)
    iteration_parser.add_argument("--real-trace", type=Path, required=True)
    iteration_parser.add_argument("--ascension", type=int, default=0)
    iteration_parser.add_argument("--max-steps", type=int, default=220)
    iteration_parser.add_argument("--max-actions", type=int, default=120)
    iteration_parser.add_argument("--candidate", dest="candidate_names", action="append")

    report_parser = subparsers.add_parser("real-trace-report")
    report_parser.add_argument("--trace", dest="traces", type=Path, action="append", required=True)

    replay_parser = subparsers.add_parser("replay-real-trace")
    replay_parser.add_argument("--trace", type=Path, required=True)
    replay_parser.add_argument("--output", type=Path, required=True)
    replay_parser.add_argument("--report-output", type=Path)
    replay_parser.add_argument("--max-actions", type=int, default=10_000)
    replay_parser.add_argument(
        "--diagnostic-continue-after-diff",
        action="store_true",
        help="Restore/anchor observed states and continue after diffs. This is diagnostic, not verification.",
    )

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
            allowed_potions_mode=args.allowed_potions_mode,
            candidates=_trace_candidates_by_name(_parse_candidate_names(args.candidate_names)),
            root_scope=args.root_scope,
            failure_output=args.failure_output,
            eval_set=args.eval_set,
            progress_every=args.progress_every,
            progress_stream=sys.stderr if args.progress_every > 0 else None,
            jobs=args.jobs,
        )
        text = json.dumps(report, indent=2, sort_keys=True)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(text, encoding="utf-8")
        print(text)
    elif args.command == "iterate-combat-policy":
        result = run_combat_policy_iteration(
            output_dir=args.output_dir,
            train_seeds=_parse_seed_specs(args.train_seeds),
            dev_seeds=_parse_seed_specs(args.dev_seeds),
            real_trace=args.real_trace,
            ascension=args.ascension,
            max_steps=args.max_steps,
            max_actions=args.max_actions,
            candidates=_trace_candidates_by_name(_parse_candidate_names(args.candidate_names))
            if args.candidate_names
            else None,
        )
        print(json.dumps(result.__dict__, indent=2, sort_keys=True, default=str))
    elif args.command == "real-trace-report":
        print(json.dumps(real_trace_root_report(args.traces), indent=2, sort_keys=True))
    elif args.command == "replay-real-trace":
        result = replay_real_trace_guided(
            trace=args.trace,
            output=args.output,
            report_output=args.report_output,
            max_actions=args.max_actions,
            diagnostic_continue_after_diff=args.diagnostic_continue_after_diff,
        )
        print(json.dumps(result.__dict__, indent=2, sort_keys=True, default=str))


if __name__ == "__main__":
    main()
