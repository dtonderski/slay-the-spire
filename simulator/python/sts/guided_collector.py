"""Guided trace collection coordinator.

The collector coordinates SlayTheData run-level scripts with the live bridge.
It remains conservative: ticks may preview, send a matched non-combat choice,
or delegate a combat tick to the UI service's strict live-session machinery.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable
from uuid import uuid4

from sts.bridge import command_for_descriptor
from sts.slaythedata_policy import (
    build_guided_run_script,
    identity_blocker,
    match_map_choice,
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
    pending_prediction: dict[str, Any] | None = None
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
            "replay_policy": self._run.script.get("replay_policy"),
            "blocker": self._run.blocker,
            "last_suggestion": self._run.last_suggestion,
            "pending_prediction": self._run.pending_prediction,
            "history_count": len(self._run.history),
        }

    def tick(
        self,
        bridge_status: dict[str, Any],
        payload: dict[str, Any] | None = None,
        *,
        send_command: Callable[..., dict[str, Any]] | None = None,
        send_non_combat: Callable[..., dict[str, Any]] | None = None,
        send_combat: Callable[..., dict[str, Any]] | None = None,
        verify_prediction: Callable[..., dict[str, Any]] | None = None,
    ) -> dict[str, Any]:
        if self._run is None:
            raise ValueError("collector is not active")
        payload = payload or {}
        pending_blocker = self._verify_pending_prediction(
            bridge_status,
            verify_prediction=verify_prediction,
        )
        if pending_blocker is not None:
            return self._record_suggestion(pending_blocker)

        requested_category = payload.get("category")
        requested_ordinal = payload.get("ordinal")
        summary = bridge_status.get("summary") if isinstance(bridge_status.get("summary"), dict) else {}
        category = str(requested_category or _infer_category(summary, bridge_status))
        ordinal = (
            int(requested_ordinal)
            if requested_ordinal is not None
            else _next_script_ordinal(self._run, bridge_status, category)
        )

        suggestion = suggest_guided_action(
            self._run.script,
            bridge_status,
            category=category,
            ordinal=ordinal,
        )
        if payload.get("send"):
            send_payload = payload | {"provenance": _guided_provenance(self._run, suggestion)}
            if suggestion.get("status") == "combat":
                suggestion = send_guided_combat_suggestion(
                    suggestion,
                    bridge_status,
                    payload=send_payload,
                    send_combat=send_combat,
                )
            elif send_non_combat is not None:
                suggestion = send_guided_non_combat_suggestion(
                    suggestion,
                    bridge_status,
                    payload=send_payload,
                    send_non_combat=send_non_combat,
                )
            else:
                suggestion = send_guided_suggestion(
                    suggestion,
                    bridge_status,
                    send_command=send_command,
                    metadata=send_payload.get("provenance"),
                )
        if suggestion.get("status") in {"sent_combat", "sent_non_combat"}:
            send_result = suggestion.get("combat_send") or suggestion.get("non_combat_send")
            if isinstance(send_result, dict):
                self._run.pending_prediction = _pending_prediction_from_simulator_send(send_result)
        return self._record_suggestion(suggestion)

    def _verify_pending_prediction(
        self,
        bridge_status: dict[str, Any],
        *,
        verify_prediction: Callable[..., dict[str, Any]] | None,
    ) -> dict[str, Any] | None:
        if self._run is None or self._run.pending_prediction is None:
            return None
        if bridge_status.get("pending_command"):
            return _blocked("pending_command", "waiting for pending bridge command before verifying prediction")
        if bridge_status.get("ready_for_command") is not True:
            return _blocked("bridge_not_ready", "waiting for the next observed bridge state before verifying prediction")
        if verify_prediction is None:
            return _blocked("missing_prediction_verifier", "collector has a pending prediction but no verifier")
        try:
            verification = verify_prediction(
                self._run.pending_prediction,
                bridge_status=bridge_status,
            )
        except Exception as error:
            return _blocked("prediction_check_failed", str(error))
        if verification.get("status") == "matched":
            self._run.pending_prediction = None
            return None
        return _blocked(
            "prediction_mismatch",
            str(verification.get("detail") or "live state did not match the pending simulator prediction"),
        ) | {"verification": verification}

    def _record_suggestion(self, suggestion: dict[str, Any]) -> dict[str, Any]:
        if self._run is None:
            raise ValueError("collector is not active")
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
    blocker = identity_blocker(script, summary)
    if blocker is not None:
        return blocker
    floor = _current_floor(summary, bridge_status)
    if floor is None:
        return _blocked("missing_floor", "bridge status does not expose a current floor")
    act = _current_act(summary, bridge_status)

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

    if decision_category == "map":
        match = match_map_choice(
            script,
            floor=floor,
            choice_labels=choices,
            next_nodes=_next_map_nodes(bridge_status),
            map_nodes=_map_nodes(bridge_status),
        )
    else:
        match = match_visible_choice(
            script,
            floor=floor,
            choice_labels=choices,
            category=decision_category,
            ordinal=ordinal,
            act=act,
        )
    return match | {
        "floor": floor,
        "act": act,
        "visible_choices": choices,
        "category": decision_category,
        "ordinal": ordinal,
    }


def send_guided_suggestion(
    suggestion: dict[str, Any],
    bridge_status: dict[str, Any],
    *,
    send_command: Callable[..., dict[str, Any]] | None,
    metadata: dict[str, Any] | None = None,
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
        send_kwargs = {"source_state_id": source_state_id}
        if metadata is not None:
            send_kwargs["metadata"] = metadata
        result = send_command(command, **send_kwargs)
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


def send_guided_combat_suggestion(
    suggestion: dict[str, Any],
    bridge_status: dict[str, Any],
    *,
    payload: dict[str, Any],
    send_combat: Callable[..., dict[str, Any]] | None,
) -> dict[str, Any]:
    if suggestion.get("status") != "combat":
        return suggestion | _blocked("not_combat", "only combat suggestions can use the combat sender")
    if send_combat is None:
        return suggestion | _blocked("missing_combat_sender", "collector tick has no combat sender")

    blocker = _bridge_send_blocker(bridge_status)
    if blocker is not None:
        return suggestion | blocker

    try:
        result = send_combat(
            bridge_status=bridge_status,
            suggestion=suggestion,
            payload=payload,
        )
    except Exception as error:
        return suggestion | _blocked("combat_send_failed", str(error))

    return suggestion | {"status": "sent_combat", "combat_send": result}


def send_guided_non_combat_suggestion(
    suggestion: dict[str, Any],
    bridge_status: dict[str, Any],
    *,
    payload: dict[str, Any],
    send_non_combat: Callable[..., dict[str, Any]] | None,
) -> dict[str, Any]:
    if suggestion.get("status") != "matched":
        return suggestion | _blocked("not_sendable", "only matched non-combat suggestions can be sent")
    if send_non_combat is None:
        return suggestion | _blocked("missing_non_combat_sender", "collector tick has no non-combat sender")

    blocker = _bridge_send_blocker(bridge_status)
    if blocker is not None:
        return suggestion | blocker

    try:
        result = send_non_combat(
            bridge_status=bridge_status,
            suggestion=suggestion,
            payload=payload,
        )
    except Exception as error:
        return suggestion | _blocked("non_combat_send_failed", str(error))

    return suggestion | {"status": "sent_non_combat", "non_combat_send": result}


def _pending_prediction_from_simulator_send(send_result: dict[str, Any]) -> dict[str, Any]:
    return {
        "predicted_state_id": send_result.get("predicted_state_id"),
        "source_state_id": send_result.get("source_state_id"),
        "bridge_state_id": send_result.get("bridge_state_id"),
        "bridge_step": send_result.get("bridge_step"),
        "command": (send_result.get("send_result") or {}).get("command"),
    }


def _guided_provenance(run: CollectorRun, suggestion: dict[str, Any]) -> dict[str, Any]:
    script = run.script if isinstance(run.script, dict) else {}
    return {
        "source": "guided_collector",
        "collector_id": run.id,
        "script_source": script.get("source"),
        "replay_policy": script.get("replay_policy"),
        "suggestion": {
            key: suggestion.get(key)
            for key in (
                "status",
                "mode",
                "category",
                "floor",
                "act",
                "target",
                "ordinal",
                "potion_uses_allowed",
            )
            if key in suggestion
        },
    }


def _next_script_ordinal(run: CollectorRun, bridge_status: dict[str, Any], category: str) -> int:
    if category in {"map", "reward", "unsupported"}:
        return 0
    summary = bridge_status.get("summary") if isinstance(bridge_status.get("summary"), dict) else {}
    floor = _current_floor(summary, bridge_status)
    act = _current_act(summary, bridge_status)
    sent = 0
    for entry in run.history:
        if not isinstance(entry, dict):
            continue
        if entry.get("status") not in {"sent", "sent_non_combat"}:
            continue
        if entry.get("category") != category:
            continue
        if floor is not None and entry.get("floor") != floor:
            continue
        if category == "boss_relic" and act is not None and entry.get("act") != act:
            continue
        sent += 1
    return sent


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


def _current_act(summary: dict[str, Any], bridge_status: dict[str, Any]) -> int | None:
    for value in (
        summary.get("act"),
        (summary.get("run") or {}).get("act") if isinstance(summary.get("run"), dict) else None,
        _game_state(bridge_status).get("act"),
        _game_state(bridge_status).get("act_num"),
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


def _next_map_nodes(bridge_status: dict[str, Any]) -> list[dict[str, Any]]:
    screen_state = _game_state(bridge_status).get("screen_state")
    nodes = screen_state.get("next_nodes") if isinstance(screen_state, dict) else None
    if isinstance(nodes, list):
        return [node for node in nodes if isinstance(node, dict)]
    return []


def _map_nodes(bridge_status: dict[str, Any]) -> list[dict[str, Any]]:
    nodes = _game_state(bridge_status).get("map")
    if isinstance(nodes, list):
        return [node for node in nodes if isinstance(node, dict)]
    return []


def _infer_category(summary: dict[str, Any], bridge_status: dict[str, Any]) -> str:
    game_state = _game_state(bridge_status)
    screen_state = game_state.get("screen_state") if isinstance(game_state.get("screen_state"), dict) else {}
    text = " ".join(
        str(value).lower()
        for value in (
            summary.get("screen_type"),
            summary.get("phase"),
            summary.get("current_decision"),
            game_state.get("screen_type"),
            screen_state.get("event_name"),
            screen_state.get("event_id"),
        )
        if value is not None
    )
    if "neow" in text:
        return "neow"
    if "boss" in text and "relic" in text:
        return "boss_relic"
    if "map" in text:
        return "map"
    if "grid" in text:
        return "grid"
    if "card" in text and "reward" in text:
        return "card_reward"
    if "reward" in text:
        return "reward"
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
