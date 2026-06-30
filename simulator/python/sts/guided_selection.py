"""SlayTheData guided-run selection helpers shared by UI and headless collection."""

from __future__ import annotations

from dataclasses import dataclass
import time
from typing import Any

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
                "blockers": blockers,
            }
        )
    detail = "; ".join(f"{entry['run_id']}: {entry['reason']}" for entry in blocked[:5])
    raise RuntimeError(f"no auto-selected SlayTheData candidates had supported guided scripts ({detail})")


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
