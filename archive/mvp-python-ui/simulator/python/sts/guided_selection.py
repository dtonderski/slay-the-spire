"""SlayTheData guided-run selection helpers shared by UI and headless collection."""

from __future__ import annotations

from dataclasses import dataclass
import json
import time
from typing import Any

from sts import omni
from sts.slaythedata_index import export_guided_run_script, select_guided_collection_candidates
from sts.slaythedata_policy import guided_script_support_audit


@dataclass(frozen=True)
class GuidedSelectionConfig:
    run_id: int | None = None
    character: str = "IRONCLAD"
    ascension: int = 0
    min_floor: int = 45
    max_floor: int | None = 55
    min_potion_usage: int | None = None


def select_run_script(config: GuidedSelectionConfig) -> tuple[int, dict[str, Any], dict[str, Any]]:
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
        blockers = guided_script_support_audit(script)
        blocker = blockers[0] if blockers else None
        preflight = slaythedata_rust_preflight_for_script(script)
        preflight_blocker = slaythedata_rust_preflight_blocker(preflight)
        blocker = blocker or preflight_blocker
        if blocker is None:
            return run_id, script, {
                "mode": "auto",
                "selected_run_id": run_id,
                "considered_count": considered,
                "candidate_count": len(candidates),
                "skipped_unsupported": blocked,
                "rust_preflight": preflight,
            }
        blocked.append(
            {
                "run_id": run_id,
                "seed": (script.get("config") or {}).get("seed_played")
                if isinstance(script.get("config"), dict)
                else None,
                "reason": blocker.get("reason"),
                "detail": blocker.get("detail"),
                "blockers": blockers or ([preflight_blocker] if preflight_blocker is not None else []),
            }
        )
    detail = "; ".join(f"{entry['run_id']}: {entry['reason']}" for entry in blocked[:5])
    raise RuntimeError(f"no auto-selected SlayTheData candidates had supported guided scripts ({detail})")


def slaythedata_rust_preflight_for_script(script: dict[str, Any]) -> dict[str, Any]:
    content = json.dumps({"event": _event_payload_from_script(script)})
    return json.loads(omni.slaythedata_preflight_json(content, None))


def slaythedata_rust_preflight_blocker(preflight: dict[str, Any]) -> dict[str, Any] | None:
    if isinstance(preflight, dict) and preflight.get("route_fully_checked") is False:
        return {
            "reason": "route_not_fully_proven",
            "detail": "SlayTheData route was not fully checked against simulator map, monster, or event evidence",
        }
    diagnostics = preflight.get("diagnostics") if isinstance(preflight, dict) else None
    if isinstance(diagnostics, list):
        for diagnostic in diagnostics:
            if isinstance(diagnostic, dict) and str(diagnostic.get("severity") or "").lower() == "error":
                return {
                    "reason": diagnostic.get("code") or "rust_preflight_error",
                    "detail": diagnostic.get("message"),
                }
    steps = preflight.get("steps") if isinstance(preflight, dict) else None
    if isinstance(steps, list):
        for step in steps:
            if isinstance(step, dict) and str(step.get("status") or "").lower() == "blocked":
                return {
                    "reason": step.get("code") or "rust_preflight_blocked",
                    "detail": step.get("message"),
                    "floor": step.get("floor"),
                    "ordinal": step.get("ordinal"),
                }
    return None


def _event_payload_from_script(script: dict[str, Any]) -> dict[str, Any]:
    config = script.get("config") if isinstance(script.get("config"), dict) else {}
    route = script.get("route") if isinstance(script.get("route"), dict) else {}
    event: dict[str, Any] = {
        "character_chosen": config.get("character"),
        "ascension_level": config.get("ascension"),
        "seed_played": config.get("seed_played"),
        "neow_bonus": config.get("neow_bonus"),
        "neow_cost": config.get("neow_cost"),
        "path_taken": route.get("path_taken") or route.get("path_per_floor") or [],
        "path_per_floor": route.get("path_per_floor") or route.get("path_taken") or [],
    }
    floor_decisions = [
        floor for floor in script.get("floor_decisions") or [] if isinstance(floor, dict)
    ]
    event["card_choices"] = [
        {
            "floor": floor.get("floor"),
            "picked": _raw_card_text(reward.get("picked")),
            "not_picked": [_raw_card_text(card) for card in reward.get("not_picked") or []],
        }
        for floor in floor_decisions
        for reward in floor.get("card_rewards") or []
        if isinstance(reward, dict)
    ]
    event["event_choices"] = [
        {
            "floor": floor.get("floor"),
            "event_name": choice.get("event_name"),
            "player_choice": choice.get("player_choice"),
        }
        for floor in floor_decisions
        for choice in floor.get("events") or []
        if isinstance(choice, dict)
    ]
    event["items_purchased"] = [
        purchase.get("item")
        for floor in floor_decisions
        for purchase in floor.get("shop_purchases") or []
        if isinstance(purchase, dict)
    ]
    event["item_purchase_floors"] = [
        floor.get("floor")
        for floor in floor_decisions
        for purchase in floor.get("shop_purchases") or []
        if isinstance(purchase, dict)
    ]
    event["campfire_choices"] = [
        {"floor": floor.get("floor"), "key": choice.get("key"), "data": _raw_card_text(choice.get("data"))}
        for floor in floor_decisions
        for choice in floor.get("campfires") or []
        if isinstance(choice, dict)
    ]
    event["boss_relics"] = script.get("boss_relic_choices") or []
    return event


def _raw_card_text(value: Any) -> Any:
    if isinstance(value, dict):
        return value.get("raw") or value.get("base")
    return value


def select_run_audit_report(
    config: GuidedSelectionConfig,
    *,
    started_at: float | None = None,
) -> dict[str, Any]:
    started_at = time.time() if started_at is None else started_at
    try:
        run_id, script, selection = select_run_script(config)
    except Exception as error:
        return {
            "producer": "sts.guided_collect",
            "generated_at": _utc_now(),
            "ok": False,
            "run_id": None,
            "seed": None,
            "stop_reason": "selection_failed",
            "blocker": {"reason": "selection_failed", "detail": str(error)},
            "elapsed_seconds": time.time() - started_at,
            "selection": None,
            "support_blockers": [],
        }

    blockers = guided_script_support_audit(script)
    config_data = script.get("config") if isinstance(script.get("config"), dict) else {}
    return {
        "producer": "sts.guided_collect",
        "generated_at": _utc_now(),
        "ok": not blockers,
        "run_id": run_id,
        "seed": config_data.get("seed_played"),
        "stop_reason": "select_only",
        "blocker": blockers[0] if blockers else None,
        "elapsed_seconds": time.time() - started_at,
        "selection": selection,
        "support_blockers": blockers,
        "script_summary": {
            "character": config_data.get("character"),
            "ascension": config_data.get("ascension"),
            "neow_bonus": config_data.get("neow_bonus"),
            "neow_cost": config_data.get("neow_cost"),
            "floor_decision_count": len(script.get("floor_decisions") or []),
            "boss_relic_count": len(script.get("boss_relic_choices") or []),
        },
    }


def _utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
