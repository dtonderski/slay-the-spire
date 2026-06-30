"""Guided trace collection coordinator.

The collector coordinates SlayTheData run-level scripts with the live bridge.
This first slice is deliberately conservative: it can load a script, report
status, and suggest the next guided action, but it does not send commands.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable
from uuid import uuid4

from sts.bridge import command_for_descriptor
from sts.slaythedata_policy import (
    build_guided_run_script,
    match_visible_choice,
    potion_uses_allowed_on_floor,
)


@dataclass
class CollectorRun:
    id: str
    script: dict[str, Any]
    status: str = "ready"
    blocker: dict[str, Any] | None = None
    last_suggestion: dict[str, Any] | None = None
    history: list[dict[str, Any]] = field(default_factory=list)


class GuidedCollector:
    def __init__(self) -> None:
        self._run: CollectorRun | None = None

    def start(self, payload: dict[str, Any]) -> dict[str, Any]:
        script = payload.get("script")
        if not isinstance(script, dict):
            exported = payload.get("exported_run")
            if not isinstance(exported, dict):
                raise ValueError("collector start requires script or exported_run")
            script = build_guided_run_script(exported)
        self._run = CollectorRun(id=uuid4().hex, script=script)
        return self.status()

    def stop(self) -> dict[str, Any]:
        if self._run is not None:
            self._run.status = "stopped"
        result = self.status()
        self._run = None
        return result

    def status(self) -> dict[str, Any]:
        if self._run is None:
            return {"active": False, "status": "idle"}
        return {
            "active": True,
            "collector_id": self._run.id,
            "status": self._run.status,
            "source": self._run.script.get("source"),
            "config": self._run.script.get("config"),
            "blocker": self._run.blocker,
            "last_suggestion": self._run.last_suggestion,
            "history_count": len(self._run.history),
        }

    def tick(
        self,
        bridge_status: dict[str, Any],
        payload: dict[str, Any] | None = None,
        *,
        send_command: Callable[..., dict[str, Any]] | None = None,
    ) -> dict[str, Any]:
        if self._run is None:
            raise ValueError("collector is not active")
        payload = payload or {}
        suggestion = suggest_guided_action(
            self._run.script,
            bridge_status,
            category=payload.get("category"),
            ordinal=int(payload.get("ordinal", 0)),
        )
        if payload.get("send"):
            suggestion = send_guided_suggestion(
                suggestion,
                bridge_status,
                send_command=send_command,
            )
        self._run.last_suggestion = suggestion
        self._run.history.append(suggestion)
        if suggestion.get("status") == "blocked":
            self._run.status = "blocked"
            self._run.blocker = suggestion
        else:
            self._run.status = "ready"
            self._run.blocker = None
        return self.status() | {"suggestion": suggestion}


def suggest_guided_action(
    script: dict[str, Any],
    bridge_status: dict[str, Any],
    *,
    category: Any = None,
    ordinal: int = 0,
) -> dict[str, Any]:
    summary = bridge_status.get("summary") if isinstance(bridge_status.get("summary"), dict) else {}
    floor = _current_floor(summary, bridge_status)
    if floor is None:
        return _blocked("missing_floor", "bridge status does not expose a current floor")

    if _looks_like_combat(summary):
        return {
            "status": "combat",
            "mode": "combat_agent",
            "floor": floor,
            "potion_uses_allowed": potion_uses_allowed_on_floor(script, floor),
            "detail": "combat decisions are delegated to the combat search policy",
        }

    choices = _visible_choices(summary, bridge_status)
    if not choices:
        return _blocked("no_visible_choices", "bridge status has no visible choices to match")

    decision_category = str(category or _infer_category(summary, bridge_status))
    if decision_category == "unsupported":
        return _blocked("unsupported_screen", "could not infer a SlayTheData decision category")

    match = match_visible_choice(
        script,
        floor=floor,
        choice_labels=choices,
        category=decision_category,
        ordinal=ordinal,
    )
    return match | {
        "floor": floor,
        "visible_choices": choices,
        "category": decision_category,
        "ordinal": ordinal,
    }


def send_guided_suggestion(
    suggestion: dict[str, Any],
    bridge_status: dict[str, Any],
    *,
    send_command: Callable[..., dict[str, Any]] | None,
) -> dict[str, Any]:
    if suggestion.get("status") != "matched":
        return suggestion | _blocked("not_sendable", "only matched non-combat suggestions can be sent")
    if send_command is None:
        return suggestion | _blocked("missing_sender", "collector tick has no bridge sender")

    blocker = _bridge_send_blocker(bridge_status)
    if blocker is not None:
        return suggestion | blocker

    descriptor = suggestion.get("descriptor")
    if not isinstance(descriptor, dict):
        return suggestion | _blocked("missing_descriptor", "matched suggestion has no bridge descriptor")

    source_state_id = bridge_status.get("state_id")
    command = command_for_descriptor(descriptor)
    try:
        result = send_command(command, source_state_id=source_state_id)
    except Exception as error:
        return suggestion | _blocked("send_failed", str(error))

    return suggestion | {
        "status": "sent",
        "command": command,
        "source_state_id": source_state_id,
        "send_result": {
            "ok": result.get("ok"),
            "command_id": result.get("command_id"),
            "command": result.get("command"),
        },
    }


def _bridge_send_blocker(bridge_status: dict[str, Any]) -> dict[str, Any] | None:
    if bridge_status.get("pending_command"):
        return _blocked("pending_command", "bridge command already pending")
    if not bridge_status.get("connected"):
        return _blocked("bridge_disconnected", "bridge is disconnected")
    if bridge_status.get("exited"):
        return _blocked("bridge_exited", "bridge has exited")
    if bridge_status.get("ready_for_command") is not True:
        return _blocked("bridge_not_ready", "bridge is not ready for a command")
    if not bridge_status.get("state_id"):
        return _blocked("missing_state_id", "bridge state id is missing")
    return None


def _current_floor(summary: dict[str, Any], bridge_status: dict[str, Any]) -> int | None:
    for value in (
        summary.get("floor"),
        (summary.get("run") or {}).get("floor") if isinstance(summary.get("run"), dict) else None,
        _game_state(bridge_status).get("floor"),
    ):
        parsed = _parse_int(value)
        if parsed is not None:
            return parsed
    return None


def _visible_choices(summary: dict[str, Any], bridge_status: dict[str, Any]) -> list[str]:
    choices = summary.get("choices")
    if isinstance(choices, list):
        return [str(choice) for choice in choices]
    game_state_choices = _game_state(bridge_status).get("choice_list")
    if isinstance(game_state_choices, list):
        return [str(choice) for choice in game_state_choices]
    return []


def _infer_category(summary: dict[str, Any], bridge_status: dict[str, Any]) -> str:
    text = " ".join(
        str(value).lower()
        for value in (
            summary.get("screen_type"),
            summary.get("phase"),
            summary.get("current_decision"),
            _game_state(bridge_status).get("screen_type"),
        )
        if value is not None
    )
    if "card" in text and "reward" in text:
        return "card_reward"
    if "shop" in text:
        return "shop"
    if "rest" in text or "campfire" in text:
        return "campfire"
    if "event" in text:
        return "event"
    return "unsupported"


def _looks_like_combat(summary: dict[str, Any]) -> bool:
    combat = summary.get("combat")
    if isinstance(combat, dict):
        return True
    phase = str(summary.get("phase") or "").lower()
    return phase == "combat"


def _game_state(bridge_status: dict[str, Any]) -> dict[str, Any]:
    current = bridge_status.get("current_state")
    if not isinstance(current, dict):
        return {}
    message = current.get("message")
    if isinstance(message, dict) and isinstance(message.get("game_state"), dict):
        return message["game_state"]
    if isinstance(current.get("game_state"), dict):
        return current["game_state"]
    return {}


def _parse_int(value: Any) -> int | None:
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _blocked(reason: str, detail: str) -> dict[str, Any]:
    return {"status": "blocked", "reason": reason, "detail": detail}
