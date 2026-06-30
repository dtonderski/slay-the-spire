"""Headless guided trace collection runner.

This module wraps the existing SlayTheData guided collector and live UI service
plumbing so automated collection can run without clicking through the browser.
It intentionally does not add new game logic; all sends still go through
BridgeMirror, strict seed replay, prediction checks, and the combat policy.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import sys
import time
from typing import Any

from sts.bridge import BridgeMirror
from sts.guided_collector import GuidedCollector
from sts.slaythedata_index import export_guided_run_script, select_guided_collection_candidates
from sts.slaythedata_policy import guided_script_support_blocker
from sts.self_play import strict_replay_real_trace_to_env
from sts.ui_service import SessionManager, _start_guided_live_run, _tick_live_collector


@dataclass(frozen=True)
class GuidedCollectConfig:
    run_id: int | None = None
    character: str = "IRONCLAD"
    ascension: int = 0
    min_floor: int = 45
    max_floor: int | None = 55
    min_potion_usage: int | None = None
    max_actions: int = 500
    max_seconds: float = 3600.0
    poll_seconds: float = 0.75
    combat_policy: str | None = None
    max_depth: int = 40
    require_tcp_control: bool = True
    preflight_timeout_seconds: float = 0.0
    preflight_poll_seconds: float = 1.0


def collect_one_run(
    config: GuidedCollectConfig,
    *,
    bridge: BridgeMirror | None = None,
    manager: SessionManager | None = None,
    collector: GuidedCollector | None = None,
    sleep: Any = time.sleep,
) -> dict[str, Any]:
    """Start one guided live run and tick until blocked or a limit is reached."""

    bridge = bridge or BridgeMirror.default()
    manager = manager or SessionManager()
    collector = collector or GuidedCollector()
    started_at = time.time()
    preflight = _wait_for_preflight(
        bridge,
        config,
        started_at=started_at,
        sleep=sleep,
    )
    if _preflight_blocks_collection(preflight, require_tcp_control=config.require_tcp_control):
        preflight_blocker = _preflight_blocker(preflight, require_tcp_control=config.require_tcp_control)
        return _blocked_report(
            config,
            started_at=started_at,
            stop_reason="preflight_blocked",
            blocker=preflight_blocker,
            bridge_status=bridge.status(),
            preflight=preflight,
        )
    try:
        run_id, script, selection = _select_run_script(config)
    except Exception as error:
        return _blocked_report(
            config,
            started_at=started_at,
            stop_reason="selection_failed",
            blocker={
                "reason": "selection_failed",
                "detail": str(error),
            },
            bridge_status=bridge.status(),
            preflight=preflight,
        )
    collector_status = collector.start({"script": script})
    if collector_status.get("status") == "blocked":
        blocker = collector_status.get("blocker") if isinstance(collector_status.get("blocker"), dict) else {
            "reason": "collector_blocked",
            "detail": "collector blocked after loading guided script",
        }
        return _blocked_report(
            config,
            started_at=started_at,
            stop_reason="script_blocked",
            blocker=blocker,
            bridge_status=bridge.status(),
            run_id=run_id,
            seed=((collector_status.get("config") or {}).get("seed_played")),
            selection=selection,
        )
    try:
        start_result = _start_guided_live_run(
            collector,
            bridge,
            require_tcp_control=config.require_tcp_control,
        )
    except Exception as error:
        return _blocked_report(
            config,
            started_at=started_at,
            stop_reason="start_failed",
            blocker={
                "reason": "start_failed",
                "detail": str(error),
            },
            bridge_status=bridge.status(),
            run_id=run_id,
            seed=((collector_status.get("config") or {}).get("seed_played")),
            preflight=preflight,
            selection=selection,
        )

    history: list[dict[str, Any]] = [
        {
            "event": "start",
            "run_id": run_id,
            "command": start_result.get("command"),
            "send_result": _compact_send_result(start_result.get("send_result")),
        }
    ]
    actions_sent = 0
    stop_reason = "unknown"
    blocker: dict[str, Any] | None = None

    while actions_sent < config.max_actions and time.time() - started_at < config.max_seconds:
        status = bridge.status()
        terminal_stop = _terminal_stop_reason(status)
        if terminal_stop is not None:
            stop_reason = terminal_stop
            history.append({"event": "terminal", "stop_reason": terminal_stop})
            break
        if status.get("pending_command") or status.get("ready_for_command") is not True:
            sleep(config.poll_seconds)
            continue

        payload: dict[str, Any] = {
            "send": True,
            "max_depth": config.max_depth,
        }
        if config.combat_policy:
            payload["candidate"] = config.combat_policy

        try:
            tick = _tick_live_collector(
                collector,
                manager,
                bridge,
                payload,
                require_tcp_control=config.require_tcp_control,
            )
        except Exception as error:
            stop_reason = "tick_failed"
            blocker = {
                "reason": "tick_failed",
                "detail": str(error),
            }
            history.append({"status": "blocked", "reason": "tick_failed", "detail": str(error)})
            break
        suggestion = tick.get("suggestion") if isinstance(tick.get("suggestion"), dict) else {}
        history.append(_compact_tick(tick))

        if tick.get("status") == "blocked" or tick.get("blocker"):
            stop_reason = "blocked"
            blocker = tick.get("blocker") if isinstance(tick.get("blocker"), dict) else suggestion
            break
        if suggestion.get("status") in {"sent", "sent_combat", "sent_non_combat"}:
            actions_sent += 1
            continue
        if suggestion.get("status") == "blocked":
            stop_reason = "blocked"
            blocker = suggestion
            break
        sleep(config.poll_seconds)
    else:
        stop_reason = "max_actions" if actions_sent >= config.max_actions else "timeout"

    final_bridge = bridge.status()
    trace_validation = _validate_trace(final_bridge.get("trace_path"))
    return {
        "producer": "sts.guided_collect",
        "generated_at": _utc_now(),
        "ok": _is_clean_collection_stop(stop_reason) and bool(trace_validation.get("verified")),
        "run_id": run_id,
        "seed": ((collector.status().get("config") or {}).get("seed_played")),
        "stop_reason": stop_reason,
        "blocker": blocker,
        "actions_sent": actions_sent,
        "elapsed_seconds": time.time() - started_at,
        "trace_path": final_bridge.get("trace_path"),
        "bridge_step": final_bridge.get("last_state_step"),
        "bridge_state_id": final_bridge.get("state_id"),
        "tcp_control_available": bool(final_bridge.get("control")),
        "trace_validation": trace_validation,
        "selection": selection,
        "history_tail": history[-25:],
    }


def _preflight_blocks_collection(preflight: dict[str, Any], *, require_tcp_control: bool) -> bool:
    if preflight.get("problems"):
        return True
    if require_tcp_control and preflight.get("tcp_control_available") is not True:
        return True
    return False


def _preflight_blocker(preflight: dict[str, Any], *, require_tcp_control: bool) -> dict[str, Any]:
    problems = list(preflight.get("problems") or [])
    warnings = list(preflight.get("warnings") or [])
    if require_tcp_control and preflight.get("tcp_control_available") is not True:
        problem = "TCP bridge control is required for guided auto-collection"
        if problem not in problems:
            problems.append(problem)
    return {
        "reason": "bridge_preflight",
        "problems": problems,
        "warnings": warnings,
        "tcp_control_available": preflight.get("tcp_control_available"),
    }


def _wait_for_preflight(
    bridge: BridgeMirror,
    config: GuidedCollectConfig,
    *,
    started_at: float,
    sleep: Any,
) -> dict[str, Any]:
    timeout = max(0.0, float(config.preflight_timeout_seconds))
    poll = max(0.05, float(config.preflight_poll_seconds))
    while True:
        preflight = bridge.preflight()
        if not _preflight_blocks_collection(preflight, require_tcp_control=config.require_tcp_control):
            return preflight
        if time.time() - started_at >= timeout:
            return preflight
        sleep(min(poll, max(0.0, timeout - (time.time() - started_at))))


def _blocked_report(
    config: GuidedCollectConfig,
    *,
    started_at: float,
    stop_reason: str,
    blocker: dict[str, Any],
    bridge_status: dict[str, Any],
    preflight: dict[str, Any] | None = None,
    run_id: int | None = None,
    seed: str | None = None,
    selection: dict[str, Any] | None = None,
) -> dict[str, Any]:
    trace_validation = _validate_trace(bridge_status.get("trace_path"))
    return {
        "producer": "sts.guided_collect",
        "generated_at": _utc_now(),
        "ok": False,
        "run_id": config.run_id if run_id is None else run_id,
        "seed": seed,
        "stop_reason": stop_reason,
        "blocker": blocker,
        "actions_sent": 0,
        "elapsed_seconds": time.time() - started_at,
        "trace_path": bridge_status.get("trace_path"),
        "bridge_step": bridge_status.get("last_state_step"),
        "bridge_state_id": bridge_status.get("state_id"),
        "tcp_control_available": bool(bridge_status.get("control")),
        "trace_validation": trace_validation,
        "preflight": preflight,
        "selection": selection,
        "history_tail": [
            {
                "event": "preflight",
                "blocker": blocker,
            }
        ],
    }


def _select_run_script(config: GuidedCollectConfig) -> tuple[int, dict[str, Any], dict[str, Any]]:
    if config.run_id is not None:
        run_id = int(config.run_id)
        script = export_guided_run_script(run_id)
        return run_id, script, {
            "mode": "explicit",
            "selected_run_id": run_id,
            "considered_count": 1,
            "skipped_unsupported": [],
        }

    candidates = select_guided_collection_candidates(
        character=config.character,
        ascension=config.ascension,
        min_floor_reached=config.min_floor,
        max_floor_reached=config.max_floor,
        min_path_length=config.min_floor,
        min_card_choices=8,
        min_event_choices=1,
        min_shop_purchases=1,
        min_potion_usage=config.min_potion_usage,
        require_guided_safe_neow=True,
        limit=25,
        ranked=False,
    )
    if not candidates:
        raise RuntimeError("no SlayTheData guided candidate run matched the default filters")
    blocked: list[dict[str, Any]] = []
    considered = 0
    for candidate in candidates:
        run_id = int(candidate["id"])
        considered += 1
        script = export_guided_run_script(run_id)
        blocker = guided_script_support_blocker(script)
        if blocker is None:
            return run_id, script, {
                "mode": "auto",
                "selected_run_id": run_id,
                "considered_count": considered,
                "candidate_count": len(candidates),
                "skipped_unsupported": blocked,
            }
        blocked.append(
            {
                "run_id": run_id,
                "seed": (script.get("config") or {}).get("seed_played")
                if isinstance(script.get("config"), dict)
                else None,
                "reason": blocker.get("reason"),
                "detail": blocker.get("detail"),
            }
        )
    detail = "; ".join(f"{entry['run_id']}: {entry['reason']}" for entry in blocked[:5])
    raise RuntimeError(f"no auto-selected SlayTheData candidates had supported guided scripts ({detail})")


def _compact_tick(tick: dict[str, Any]) -> dict[str, Any]:
    suggestion = tick.get("suggestion") if isinstance(tick.get("suggestion"), dict) else {}
    send = suggestion.get("combat_send") or suggestion.get("non_combat_send") or suggestion.get("send_result")
    return {
        "status": tick.get("status"),
        "suggestion_status": suggestion.get("status"),
        "reason": suggestion.get("reason"),
        "detail": suggestion.get("detail"),
        "floor": suggestion.get("floor"),
        "category": suggestion.get("category") or suggestion.get("mode"),
        "command": suggestion.get("command")
        or ((send or {}).get("send_result") or {}).get("command")
        or (send or {}).get("command"),
        "pending_prediction": tick.get("pending_prediction"),
    }


def _compact_send_result(send_result: Any) -> dict[str, Any] | None:
    if not isinstance(send_result, dict):
        return None
    observed_update = send_result.get("observed_update")
    compact = {
        "ok": send_result.get("ok"),
        "command_id": send_result.get("command_id"),
        "command": send_result.get("command"),
        "transport": send_result.get("transport"),
    }
    if isinstance(observed_update, dict):
        compact["observed_update"] = {
            "ok": observed_update.get("ok"),
            "state_id": observed_update.get("state_id"),
            "state_seq": observed_update.get("state_seq"),
            "step": observed_update.get("step"),
            "error": observed_update.get("error"),
        }
        bridge_status = observed_update.get("bridge_status")
        if isinstance(bridge_status, dict):
            compact["observed_update"]["bridge_state_id"] = bridge_status.get("state_id")
            compact["observed_update"]["bridge_step"] = bridge_status.get("last_state_step")
    return compact


def _is_clean_collection_stop(stop_reason: str) -> bool:
    return stop_reason in {"trace_exhausted", "game_complete"}


def _terminal_stop_reason(bridge_status: dict[str, Any]) -> str | None:
    screen_type = _bridge_screen_type(bridge_status)
    if screen_type in {"GAME_OVER", "VICTORY", "CREDITS"}:
        return "game_complete"
    game_state = _bridge_game_state(bridge_status)
    if _is_dead_game_state(game_state):
        return "game_complete"
    return None


def _bridge_screen_type(bridge_status: dict[str, Any]) -> str:
    summary = bridge_status.get("summary") if isinstance(bridge_status.get("summary"), dict) else {}
    game_state = _bridge_game_state(bridge_status)
    for value in (summary.get("screen_type"), game_state.get("screen_type")):
        if value is not None:
            return str(value).upper()
    return ""


def _bridge_game_state(bridge_status: dict[str, Any]) -> dict[str, Any]:
    current_state = bridge_status.get("current_state")
    if not isinstance(current_state, dict):
        return {}
    message = current_state.get("message")
    if isinstance(message, dict) and isinstance(message.get("game_state"), dict):
        return message["game_state"]
    if isinstance(current_state.get("game_state"), dict):
        return current_state["game_state"]
    if isinstance(message, dict):
        return message
    return current_state


def _is_dead_game_state(game_state: dict[str, Any]) -> bool:
    if str(game_state.get("screen_type") or "").upper() == "GAME_OVER":
        return True
    try:
        return int(game_state.get("current_hp")) <= 0
    except (TypeError, ValueError):
        return False


def _validate_trace(trace_path: Any) -> dict[str, Any]:
    if not trace_path:
        return {"verified": False, "reason": "missing_trace_path"}
    path = Path(str(trace_path))
    if not path.exists():
        return {"verified": False, "reason": "trace_path_not_found", "trace_path": str(path)}
    protocol_counts = _trace_protocol_counts(path)
    try:
        result = strict_replay_real_trace_to_env(trace=path)
    except Exception as error:
        return {
            "verified": False,
            "reason": "validation_error",
            "detail": str(error),
            "trace_path": str(path),
        }
    return {
        "verified": result.verified,
        "stop_reason": result.stop_reason,
        "steps": result.steps,
        "final_state_id": result.final_state_id,
        "final_phase": result.final_phase,
        "blocker": result.blocker,
        "trace_path": str(path),
        **protocol_counts,
    }


def _trace_protocol_counts(path: Path) -> dict[str, int]:
    counts = {
        "command_accepts": 0,
        "command_observed_timeouts": 0,
    }
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            record = json.loads(line)
            record_type = record.get("type") if isinstance(record, dict) else None
            if record_type == "command_accept":
                counts["command_accepts"] += 1
            elif record_type == "command_observed_timeout":
                counts["command_observed_timeouts"] += 1
    return counts


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Run one headless SlayTheData-guided live collection attempt.")
    parser.add_argument("--run-id", type=int, default=None)
    parser.add_argument("--character", default="IRONCLAD")
    parser.add_argument("--ascension", type=int, default=0)
    parser.add_argument("--min-floor", type=int, default=45)
    parser.add_argument("--max-floor", type=int, default=55)
    parser.add_argument("--min-potion-usage", type=int, default=None)
    parser.add_argument("--max-actions", type=int, default=500)
    parser.add_argument("--max-seconds", type=float, default=3600.0)
    parser.add_argument("--poll-seconds", type=float, default=0.75)
    parser.add_argument("--combat-policy", default=None)
    parser.add_argument("--max-depth", type=int, default=40)
    parser.add_argument("--allow-file-bridge", action="store_true")
    parser.add_argument("--report-output", type=Path, default=None)
    parser.add_argument("--archive-report-dir", type=Path, default=None)
    parser.add_argument("--fail-on-not-ok", action="store_true")
    parser.add_argument("--preflight-timeout-seconds", type=float, default=0.0)
    parser.add_argument("--preflight-poll-seconds", type=float, default=1.0)
    args = parser.parse_args(argv)

    started_at = time.time()
    try:
        report = collect_one_run(
            GuidedCollectConfig(
                run_id=args.run_id,
                character=args.character,
                ascension=args.ascension,
                min_floor=args.min_floor,
                max_floor=args.max_floor,
                min_potion_usage=args.min_potion_usage,
                max_actions=args.max_actions,
                max_seconds=args.max_seconds,
                poll_seconds=args.poll_seconds,
                combat_policy=args.combat_policy,
                max_depth=args.max_depth,
                require_tcp_control=not args.allow_file_bridge,
                preflight_timeout_seconds=args.preflight_timeout_seconds,
                preflight_poll_seconds=args.preflight_poll_seconds,
            )
        )
    except Exception as error:
        report = _internal_error_report(error, elapsed_seconds=time.time() - started_at)
    encoded = json.dumps(report, indent=2, sort_keys=True)
    if args.report_output:
        args.report_output.parent.mkdir(parents=True, exist_ok=True)
        args.report_output.write_text(f"{encoded}\n", encoding="utf-8")
    if args.archive_report_dir:
        archive_path = _archive_report_path(args.archive_report_dir, report)
        archive_path.parent.mkdir(parents=True, exist_ok=True)
        archive_path.write_text(f"{encoded}\n", encoding="utf-8")
    print(encoded)
    if args.fail_on_not_ok and not report.get("ok"):
        sys.exit(1)


def _internal_error_report(error: Exception, *, elapsed_seconds: float) -> dict[str, Any]:
    detail = str(error)
    error_type = type(error).__name__
    return {
        "producer": "sts.guided_collect",
        "generated_at": _utc_now(),
        "ok": False,
        "run_id": None,
        "seed": None,
        "stop_reason": "internal_error",
        "blocker": {
            "reason": "internal_error",
            "type": error_type,
            "detail": detail,
        },
        "actions_sent": 0,
        "elapsed_seconds": elapsed_seconds,
        "trace_path": None,
        "bridge_step": None,
        "bridge_state_id": None,
        "tcp_control_available": False,
        "history_tail": [
            {
                "event": "internal_error",
                "type": error_type,
                "detail": detail,
            }
        ],
    }


def _archive_report_path(directory: Path, report: dict[str, Any]) -> Path:
    timestamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    run_id = _safe_file_part(report.get("run_id") or "no-run")
    reason = _safe_file_part(report.get("stop_reason") or "unknown")
    return directory / f"{timestamp}-{run_id}-{reason}.json"


def _safe_file_part(value: Any) -> str:
    text = str(value)
    cleaned = "".join(ch if ch.isalnum() or ch in {"-", "_"} else "-" for ch in text)
    return cleaned.strip("-") or "unknown"


def _utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


if __name__ == "__main__":
    main()
