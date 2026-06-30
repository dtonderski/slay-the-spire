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
import time
from typing import Any

from sts.bridge import BridgeMirror
from sts.guided_collector import GuidedCollector
from sts.slaythedata_index import export_guided_run_script, select_guided_collection_candidates
from sts.ui_service import SessionManager, _start_guided_live_run, _tick_live_collector


@dataclass(frozen=True)
class GuidedCollectConfig:
    run_id: int | None = None
    character: str = "IRONCLAD"
    ascension: int = 0
    min_floor: int = 45
    max_floor: int | None = 55
    max_actions: int = 500
    max_seconds: float = 3600.0
    poll_seconds: float = 0.75
    combat_policy: str | None = None
    max_depth: int = 40
    require_tcp_control: bool = True


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
    preflight = bridge.preflight()
    if _preflight_blocks_collection(preflight, require_tcp_control=config.require_tcp_control):
        return _blocked_report(
            config,
            started_at=started_at,
            stop_reason="preflight_blocked",
            blocker={
                "reason": "bridge_preflight",
                "problems": preflight.get("problems", []),
                "warnings": preflight.get("warnings", []),
                "tcp_control_available": preflight.get("tcp_control_available"),
            },
            bridge_status=bridge.status(),
            preflight=preflight,
        )
    run_id = config.run_id if config.run_id is not None else _select_run_id(config)
    script = export_guided_run_script(run_id)
    collector.start({"script": script})
    start_result = _start_guided_live_run(
        collector,
        bridge,
        require_tcp_control=config.require_tcp_control,
    )

    history: list[dict[str, Any]] = [
        {
            "event": "start",
            "run_id": run_id,
            "command": start_result.get("command"),
            "send_result": start_result.get("send_result"),
        }
    ]
    actions_sent = 0
    stop_reason = "unknown"
    blocker: dict[str, Any] | None = None

    while actions_sent < config.max_actions and time.time() - started_at < config.max_seconds:
        status = bridge.status()
        if status.get("pending_command") or status.get("ready_for_command") is not True:
            sleep(config.poll_seconds)
            continue

        payload: dict[str, Any] = {
            "send": True,
            "max_depth": config.max_depth,
        }
        if config.combat_policy:
            payload["candidate"] = config.combat_policy

        tick = _tick_live_collector(
            collector,
            manager,
            bridge,
            payload,
            require_tcp_control=config.require_tcp_control,
        )
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
    return {
        "ok": stop_reason not in {"blocked", "timeout"},
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
        "history_tail": history[-25:],
    }


def _preflight_blocks_collection(preflight: dict[str, Any], *, require_tcp_control: bool) -> bool:
    if preflight.get("problems"):
        return True
    if require_tcp_control and preflight.get("tcp_control_available") is not True:
        return True
    return False


def _blocked_report(
    config: GuidedCollectConfig,
    *,
    started_at: float,
    stop_reason: str,
    blocker: dict[str, Any],
    bridge_status: dict[str, Any],
    preflight: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "ok": False,
        "run_id": config.run_id,
        "seed": None,
        "stop_reason": stop_reason,
        "blocker": blocker,
        "actions_sent": 0,
        "elapsed_seconds": time.time() - started_at,
        "trace_path": bridge_status.get("trace_path"),
        "bridge_step": bridge_status.get("last_state_step"),
        "bridge_state_id": bridge_status.get("state_id"),
        "tcp_control_available": bool(bridge_status.get("control")),
        "preflight": preflight,
        "history_tail": [
            {
                "event": "preflight",
                "blocker": blocker,
            }
        ],
    }


def _select_run_id(config: GuidedCollectConfig) -> int:
    candidates = select_guided_collection_candidates(
        character=config.character,
        ascension=config.ascension,
        min_floor_reached=config.min_floor,
        max_floor_reached=config.max_floor,
        min_path_length=config.min_floor,
        min_card_choices=8,
        min_event_choices=1,
        min_shop_purchases=1,
        require_guided_safe_neow=True,
        limit=1,
        ranked=False,
    )
    if not candidates:
        raise RuntimeError("no SlayTheData guided candidate run matched the default filters")
    return int(candidates[0]["id"])


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


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Run one headless SlayTheData-guided live collection attempt.")
    parser.add_argument("--run-id", type=int, default=None)
    parser.add_argument("--character", default="IRONCLAD")
    parser.add_argument("--ascension", type=int, default=0)
    parser.add_argument("--min-floor", type=int, default=45)
    parser.add_argument("--max-floor", type=int, default=55)
    parser.add_argument("--max-actions", type=int, default=500)
    parser.add_argument("--max-seconds", type=float, default=3600.0)
    parser.add_argument("--poll-seconds", type=float, default=0.75)
    parser.add_argument("--combat-policy", default=None)
    parser.add_argument("--max-depth", type=int, default=40)
    parser.add_argument("--allow-file-bridge", action="store_true")
    parser.add_argument("--report-output", type=Path, default=None)
    args = parser.parse_args(argv)

    report = collect_one_run(
        GuidedCollectConfig(
            run_id=args.run_id,
            character=args.character,
            ascension=args.ascension,
            min_floor=args.min_floor,
            max_floor=args.max_floor,
            max_actions=args.max_actions,
            max_seconds=args.max_seconds,
            poll_seconds=args.poll_seconds,
            combat_policy=args.combat_policy,
            max_depth=args.max_depth,
            require_tcp_control=not args.allow_file_bridge,
        )
    )
    encoded = json.dumps(report, indent=2, sort_keys=True)
    if args.report_output:
        args.report_output.parent.mkdir(parents=True, exist_ok=True)
        args.report_output.write_text(f"{encoded}\n", encoding="utf-8")
    print(encoded)


if __name__ == "__main__":
    main()
