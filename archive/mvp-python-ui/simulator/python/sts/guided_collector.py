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
    floor_decision,
    guided_script_support_blocker,
    identity_blocker,
    match_map_choice,
    match_visible_choice,
    neow_target_matches_live_choice,
    potion_uses_allowed_on_floor,
)


@dataclass
class CollectorRun:
    id: str
    script: dict[str, Any]
    rust_preflight: dict[str, Any] | None = None
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
        rust_preflight = payload.get("rust_preflight")
        self._run = CollectorRun(
            id=uuid4().hex,
            script=script,
            rust_preflight=rust_preflight if isinstance(rust_preflight, dict) else None,
        )
        support_blocker = guided_script_support_blocker(script)
        if support_blocker is not None:
            self._run.status = "blocked"
            self._run.blocker = support_blocker
            self._run.last_suggestion = support_blocker
            self._run.history.append(support_blocker)
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
            "rust_preflight": self._run.rust_preflight,
            "blocker": self._run.blocker,
            "last_suggestion": self._run.last_suggestion,
            "pending_prediction": self._run.pending_prediction,
            "history_count": len(self._run.history),
        }

    def clear_runtime_blocker(self, *reasons: str) -> dict[str, Any]:
        if self._run is None:
            return self.status()
        blocker = self._run.blocker if isinstance(self._run.blocker, dict) else {}
        if self._run.status == "blocked" and blocker.get("reason") in set(reasons):
            self._run.status = "ready"
            self._run.blocker = None
        return self.status()

    def reset_runtime_state(self) -> dict[str, Any]:
        if self._run is None:
            return self.status()
        self._run.status = "ready"
        self._run.blocker = None
        self._run.last_suggestion = None
        self._run.pending_prediction = None
        self._run.history.clear()
        return self.status()

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

        suggestion = rust_preflight_suggestion(
            self._run,
            bridge_status,
            category=category,
        )
        if suggestion is None:
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
                pending_prediction = _pending_prediction_from_simulator_send(send_result)
                immediate_blocker = self._verify_observed_send_update(
                    pending_prediction,
                    send_result,
                    verify_prediction=verify_prediction,
                )
                if immediate_blocker is None:
                    self._run.pending_prediction = pending_prediction
                elif immediate_blocker.get("status") == "matched":
                    self._run.pending_prediction = None
                else:
                    return self._record_suggestion(suggestion | immediate_blocker)
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
        stale_state_blocker = _pending_prediction_stale_state_blocker(
            self._run.pending_prediction,
            bridge_status,
        )
        if stale_state_blocker is not None:
            return stale_state_blocker
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

    def _verify_observed_send_update(
        self,
        pending_prediction: dict[str, Any],
        send_result: dict[str, Any],
        *,
        verify_prediction: Callable[..., dict[str, Any]] | None,
    ) -> dict[str, Any] | None:
        raw_send_result = send_result.get("send_result")
        if not isinstance(raw_send_result, dict):
            return None
        observed_update = raw_send_result.get("observed_update")
        if not isinstance(observed_update, dict):
            return None
        if observed_update.get("ok") is not True:
            return _blocked(
                "observed_update_timeout",
                str(observed_update.get("error") or "timed out waiting for observed bridge state after command"),
            ) | {"observed_update": observed_update}
        observed_bridge_status = observed_update.get("bridge_status")
        if not isinstance(observed_bridge_status, dict):
            return None
        if verify_prediction is None:
            return _blocked(
                "missing_prediction_verifier",
                "collector received an observed update but has no verifier",
            )
        try:
            verification = verify_prediction(
                pending_prediction,
                bridge_status=observed_bridge_status,
            )
        except Exception as error:
            return _blocked("prediction_check_failed", str(error))
        if verification.get("status") == "matched":
            return {"status": "matched", "verification": verification}
        return _blocked(
            "prediction_mismatch",
            str(verification.get("detail") or "observed TCP update did not match the simulator prediction"),
        ) | {"verification": verification}

    def _record_suggestion(self, suggestion: dict[str, Any]) -> dict[str, Any]:
        if self._run is None:
            raise ValueError("collector is not active")
        self._run.last_suggestion = suggestion
        self._run.history.append(suggestion)
        if suggestion.get("status") == "blocked" and not _is_soft_wait_suggestion(suggestion):
            self._run.status = "blocked"
            self._run.blocker = suggestion
        else:
            self._run.status = "ready"
            self._run.blocker = None
        return self.status() | {"suggestion": suggestion}


def rust_preflight_blocker(preflight: dict[str, Any] | None) -> dict[str, Any] | None:
    if not isinstance(preflight, dict):
        return None
    diagnostics = [item for item in preflight.get("diagnostics") or [] if isinstance(item, dict)]
    errors = [
        item
        for item in diagnostics
        if str(item.get("severity") or "").lower() == "error"
    ]
    steps = [item for item in preflight.get("steps") or [] if isinstance(item, dict)]
    blocked_steps = [
        item
        for item in steps
        if str(item.get("status") or "").lower() == "blocked"
    ]
    if not errors and not blocked_steps:
        return None
    detail_source = (errors or blocked_steps)[0]
    return {
        "status": "blocked",
        "reason": "rust_preflight_blocked",
        "detail": detail_source.get("message")
        or detail_source.get("code")
        or "Rust SlayTheData preflight blocked replay",
        "diagnostic_errors": len(errors),
        "blocked_steps": len(blocked_steps),
        "category": "slaythedata_preflight",
    }


def rust_preflight_suggestion(
    run: CollectorRun,
    bridge_status: dict[str, Any],
    *,
    category: str,
) -> dict[str, Any] | None:
    preflight = run.rust_preflight
    if not isinstance(preflight, dict):
        return None
    summary = bridge_status.get("summary") if isinstance(bridge_status.get("summary"), dict) else {}
    floor = _current_floor(summary, bridge_status)
    choices = _visible_choices(summary, bridge_status)
    sent_step_ordinals = {
        entry.get("preflight_step_ordinal")
        for entry in run.history
        if isinstance(entry, dict)
        and entry.get("status") in {"sent", "sent_non_combat"}
        and entry.get("source") == "rust_preflight"
    }
    for step in preflight.get("steps") or []:
        if not isinstance(step, dict):
            continue
        step_ordinal = step.get("ordinal")
        if step_ordinal in sent_step_ordinals:
            continue
        hint = step.get("bridge_command") if isinstance(step.get("bridge_command"), dict) else None
        descriptor = hint.get("descriptor") if isinstance(hint, dict) and isinstance(hint.get("descriptor"), dict) else None
        if descriptor is None:
            continue
        if str(step.get("status") or "").lower() != "checked":
            continue
        if not _rust_preflight_step_matches_category(step, category, floor):
            continue
        slot = _parse_int(descriptor.get("option_slot"))
        descriptor_kind = str(descriptor.get("kind") or "")
        if descriptor_kind != "SkipVisibleReward":
            if slot is None:
                continue
            if choices and not (0 <= slot < len(choices)):
                continue
            if category == "neow" and choices and not neow_target_matches_live_choice(run.script, choices, slot):
                continue
        return {
            "status": "matched",
            "source": "rust_preflight",
            "descriptor": descriptor,
            "command": hint.get("command"),
            "target": step.get("code"),
            "matched_label": choices[slot] if slot is not None and choices and 0 <= slot < len(choices) else hint.get("command"),
            "floor": floor,
            "category": category,
            "ordinal": 0,
            "preflight_step_ordinal": step_ordinal,
            "preflight_code": step.get("code"),
            "match_evidence": "rust_preflight_checked_command",
        }
    return None


def _rust_preflight_step_matches_category(
    step: dict[str, Any],
    category: str,
    floor: int | None,
) -> bool:
    code = str(step.get("code") or "")
    step_floor = _parse_int(step.get("floor"))
    if category == "neow":
        return step_floor == 0 and code.startswith("legal_neow_")
    if category == "map" and code == "legal_map_room":
        return floor is None or step_floor in {floor, floor + 1}
    if category == "card_reward" and code == "legal_card_reward":
        return floor is None or step_floor == floor
    return False


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
        waiting = _waiting_for_run_start(summary, bridge_status)
        if waiting is not None:
            return waiting
        return _blocked("missing_floor", "bridge status does not expose a current floor")
    act = _current_act(summary, bridge_status)

    if _looks_like_combat(summary):
        potion_budget = potion_uses_allowed_on_floor(script, floor)
        return {
            "status": "combat",
            "source": "combat_agent",
            "mode": "combat_agent",
            "floor": floor,
            "potion_uses_allowed": potion_budget,
            "potion_guidance": {
                "mode": "floor_budget",
                "fidelity": "budget_only",
                "uses_allowed": potion_budget,
                "detail": "SlayTheData records potion use count by floor, not potion identity, target, or timing",
            },
            "detail": "combat decisions are delegated to the combat search policy",
        }

    decision_category = str(category or _infer_category(summary, bridge_status))
    if decision_category == "unsupported":
        return _blocked("unsupported_screen", "could not infer a SlayTheData decision category")

    choices = _visible_choices(summary, bridge_status)
    if not choices:
        proceed = _proceed_suggestion_if_available(summary, floor, act, decision_category, ordinal)
        if proceed is not None:
            return proceed
        return _blocked("no_visible_choices", "bridge status has no visible choices to match")

    if decision_category == "map":
        match = match_map_choice(
            script,
            floor=floor,
            choice_labels=choices,
            next_nodes=_next_map_nodes(bridge_status),
            map_nodes=_map_nodes(bridge_status),
        )
    else:
        if decision_category == "reward":
            live_reward = _auto_take_live_reward_choice(
                choices,
                summary,
                bridge_status,
                floor=floor,
                act=act,
                ordinal=ordinal,
            )
            if live_reward is not None:
                return live_reward
        if decision_category == "event":
            event_blocker = _observed_event_identity_blocker(script, bridge_status, floor, ordinal)
            if event_blocker is not None:
                return event_blocker
        match = match_visible_choice(
            script,
            floor=floor,
            choice_labels=choices,
            category=decision_category,
            ordinal=ordinal,
            act=act,
        )
        if decision_category == "event" and match.get("status") == "blocked":
            agent_choice = _agent_event_choice_fallback(
                floor=floor,
                act=act,
                choice_labels=choices,
                ordinal=ordinal,
                reason=str(match.get("reason") or "event_choice_unmatched"),
            )
            if agent_choice is not None:
                match = agent_choice
    return match | {
        "source": "guided_fallback",
        "fallback": True,
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
    if suggestion.get("status") == "blocked":
        return suggestion
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
        send_kwargs = {
            "source_state_id": source_state_id,
            "wait_for_state_update": False,
        }
        if metadata is not None:
            send_kwargs["metadata"] = metadata
        result = send_command(command, **send_kwargs)
    except Exception as error:
        return suggestion | _blocked("send_failed", str(error))

    observed_update = result.get("observed_update")
    if result.get("transport") == "tcp-jsonl" and "observed_update" in result:
        if not isinstance(observed_update, dict):
            return suggestion | _blocked(
                "observed_update_missing",
                "TCP command did not return an observed bridge state update",
            )
        if observed_update.get("ok") is not True:
            return suggestion | _blocked(
                "observed_update_timeout",
                str(observed_update.get("error") or "timed out waiting for observed bridge state after command"),
            ) | {"observed_update": observed_update}

    return suggestion | {
        "status": "sent",
        "command": command,
        "source_state_id": source_state_id,
        "send_result": {
            "ok": result.get("ok"),
            "command_id": result.get("command_id"),
            "command": result.get("command"),
            "transport": result.get("transport"),
            "observed_update": observed_update,
            "accepted_state_id": result.get("accepted_state_id"),
            "accepted_state_seq": result.get("accepted_state_seq"),
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
    if suggestion.get("status") == "blocked":
        return suggestion
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
    raw_send_result = send_result.get("send_result")
    if not isinstance(raw_send_result, dict):
        raw_send_result = {}
    return {
        "predicted_state_id": send_result.get("predicted_state_id"),
        "predicted_snapshot_json": send_result.get("predicted_snapshot_json"),
        "source_state_id": send_result.get("source_state_id"),
        "bridge_state_id": send_result.get("bridge_state_id"),
        "bridge_step": send_result.get("bridge_step"),
        "command": raw_send_result.get("command"),
        "accepted_state_id": raw_send_result.get("accepted_state_id"),
        "accepted_state_seq": raw_send_result.get("accepted_state_seq"),
    }


def _pending_prediction_stale_state_blocker(
    pending_prediction: dict[str, Any],
    bridge_status: dict[str, Any],
) -> dict[str, Any] | None:
    sent_bridge_state_id = pending_prediction.get("bridge_state_id")
    current_bridge_state_id = bridge_status.get("state_id")
    if not sent_bridge_state_id or current_bridge_state_id != sent_bridge_state_id:
        return None
    accepted_seq = _coerce_int(pending_prediction.get("accepted_state_seq"))
    current_seq = _bridge_state_seq(bridge_status)
    if accepted_seq is None and current_seq is None:
        return None
    if accepted_seq is not None and current_seq is not None and current_seq > accepted_seq:
        return None
    return _blocked(
        "waiting_for_observed_state",
        "waiting for a new observed bridge state before verifying the pending prediction",
    )


def _bridge_state_seq(bridge_status: dict[str, Any]) -> int | None:
    direct = _coerce_int(bridge_status.get("state_seq"))
    if direct is not None:
        return direct
    summary = bridge_status.get("summary")
    if isinstance(summary, dict):
        return _coerce_int(summary.get("state_seq"))
    return None


def _coerce_int(value: Any) -> int | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        try:
            return int(value)
        except ValueError:
            return None
    return None


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
                "potion_guidance",
            )
            if key in suggestion
        },
    }


def _next_script_ordinal(run: CollectorRun, bridge_status: dict[str, Any], category: str) -> int:
    if category in {"map", "reward", "card_reward", "unsupported"}:
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


def _waiting_for_run_start(summary: dict[str, Any], bridge_status: dict[str, Any]) -> dict[str, Any] | None:
    available = {str(command).lower() for command in bridge_status.get("available_commands") or []}
    if not available:
        available = {str(command).lower() for command in summary.get("available_commands") or []}
    if summary.get("in_game") is False or "start" in available:
        return _blocked("waiting_for_run_start", "waiting for START to create the first in-run bridge state") | {
            "transient": True,
            "category": "run_start",
        }
    return None


def _is_soft_wait_suggestion(suggestion: dict[str, Any]) -> bool:
    return suggestion.get("reason") == "waiting_for_run_start" or suggestion.get("transient") is True


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
    reward_choices = _reward_choice_labels(bridge_status)
    if reward_choices:
        return reward_choices
    choices = summary.get("choices")
    if isinstance(choices, list):
        return [str(choice) for choice in choices]
    game_state_choices = _game_state(bridge_status).get("choice_list")
    if isinstance(game_state_choices, list):
        return [str(choice) for choice in game_state_choices]
    return []


def _reward_choice_labels(bridge_status: dict[str, Any]) -> list[str]:
    screen_state = _game_state(bridge_status).get("screen_state")
    rewards = screen_state.get("rewards") if isinstance(screen_state, dict) else None
    if not isinstance(rewards, list):
        return []
    labels = []
    for reward in rewards:
        if not isinstance(reward, dict):
            labels.append(str(reward))
            continue
        reward_type = str(reward.get("reward_type") or "").upper()
        potion = reward.get("potion") if isinstance(reward.get("potion"), dict) else None
        if potion is not None:
            labels.append(str(potion.get("name") or potion.get("id") or "potion"))
        elif reward_type == "GOLD" or reward.get("gold") is not None:
            labels.append("gold")
        elif reward_type == "CARD":
            labels.append("card")
        elif reward_type == "RELIC":
            relic = reward.get("relic") if isinstance(reward.get("relic"), dict) else None
            labels.append(str((relic or {}).get("name") or (relic or {}).get("id") or "relic"))
        else:
            labels.append(str(reward.get("name") or reward_type.lower() or reward))
    return labels


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


def _proceed_suggestion_if_available(
    summary: dict[str, Any],
    floor: int,
    act: int | None,
    category: str,
    ordinal: int,
) -> dict[str, Any] | None:
    if category not in {"reward", "card_reward"}:
        return None
    available = {str(command).lower() for command in summary.get("available_commands") or []}
    if "proceed" not in available:
        return None
    return {
        "status": "matched",
        "descriptor": {"kind": "Proceed"},
        "command": "PROCEED",
        "target": "proceed",
        "matched_label": "Proceed",
        "source": "guided_fallback",
        "fallback": True,
        "floor": floor,
        "act": act,
        "visible_choices": [],
        "category": category,
        "ordinal": ordinal,
        "match_evidence": "available_proceed_command",
    }


def _auto_take_live_reward_choice(
    choice_labels: list[str],
    summary: dict[str, Any],
    bridge_status: dict[str, Any],
    *,
    floor: int,
    act: int | None,
    ordinal: int,
) -> dict[str, Any] | None:
    gold_slot = _reward_slot_by_kind(choice_labels, bridge_status, "gold")
    if gold_slot is not None:
        return _live_reward_suggestion(
            "gold",
            choice_labels[gold_slot],
            gold_slot,
            floor=floor,
            act=act,
            ordinal=ordinal,
            lossy=False,
        )
    if _open_potion_slots(summary, bridge_status) <= 0:
        return None
    potion_slot = _reward_slot_by_kind(choice_labels, bridge_status, "potion")
    if potion_slot is None:
        return None
    return _live_reward_suggestion(
        "potion",
        choice_labels[potion_slot],
        potion_slot,
        floor=floor,
        act=act,
        ordinal=ordinal,
        lossy=True,
    )


def _live_reward_suggestion(
    target: str,
    matched_label: str,
    slot: int,
    *,
    floor: int,
    act: int | None,
    ordinal: int,
    lossy: bool,
) -> dict[str, Any]:
    result = {
        "status": "matched",
        "descriptor": {"kind": "ChooseVisibleOption", "option_slot": slot},
        "target": target,
        "matched_label": matched_label,
        "source": "reward_auto_take",
        "fallback": True,
        "floor": floor,
        "act": act,
        "visible_choices": [],
        "category": "reward",
        "ordinal": ordinal,
        "match_evidence": "live_reward_auto_take",
    }
    if lossy:
        result |= {
            "lossy": True,
            "lossy_reason": "Potion rewards are auto-collected from the live game state for guided combat-state collection",
        }
    return result


def _reward_slot_by_kind(
    choice_labels: list[str],
    bridge_status: dict[str, Any],
    kind: str,
) -> int | None:
    rewards = _reward_entries(bridge_status)
    if rewards:
        for index, reward in enumerate(rewards):
            if _reward_entry_kind(reward) == kind and index < len(choice_labels):
                return index
    for index, label in enumerate(choice_labels):
        if _canonical_visible_reward_kind(label) == kind:
            return index
    return None


def _reward_entries(bridge_status: dict[str, Any]) -> list[dict[str, Any]]:
    screen_state = _game_state(bridge_status).get("screen_state")
    rewards = screen_state.get("rewards") if isinstance(screen_state, dict) else None
    if not isinstance(rewards, list):
        return []
    return [reward for reward in rewards if isinstance(reward, dict)]


def _reward_entry_kind(reward: dict[str, Any]) -> str:
    reward_type = str(reward.get("reward_type") or "").lower()
    if isinstance(reward.get("potion"), dict) or reward_type == "potion":
        return "potion"
    if reward.get("gold") is not None or reward_type == "gold":
        return "gold"
    if reward_type == "card":
        return "card"
    if isinstance(reward.get("relic"), dict) or reward_type == "relic":
        return "relic"
    return reward_type


def _canonical_visible_reward_kind(label: Any) -> str:
    token = _normalized_token(label)
    if "potion" in token:
        return "potion"
    if "gold" in token:
        return "gold"
    if "card" in token:
        return "card"
    if "relic" in token:
        return "relic"
    return token


def _open_potion_slots(summary: dict[str, Any], bridge_status: dict[str, Any]) -> int:
    for value in (
        summary.get("open_potion_slots"),
        _game_state(bridge_status).get("open_potion_slots"),
    ):
        parsed = _parse_int(value)
        if parsed is not None:
            return parsed
    potions = _game_state(bridge_status).get("potions")
    if isinstance(potions, list):
        return sum(
            1
            for potion in potions
            if isinstance(potion, dict)
            and str(potion.get("name") or potion.get("id") or "").lower() == "potion slot"
        )
    return 0


def _agent_event_choice_fallback(
    *,
    floor: int,
    act: int | None,
    choice_labels: list[str],
    ordinal: int,
    reason: str,
) -> dict[str, Any] | None:
    if not choice_labels:
        return None
    normalized = [choice.strip().lower() for choice in choice_labels]
    slot = normalized.index("leave") if "leave" in normalized else 0
    return {
        "status": "matched",
        "descriptor": {"kind": "ChooseVisibleOption", "option_slot": slot},
        "target": "agent_event_choice",
        "matched_label": choice_labels[slot],
        "floor": floor,
        "act": act,
        "category": "event",
        "ordinal": ordinal,
        "fallback": True,
        "lossy": True,
        "lossy_reason": f"SlayTheData event choice could not be matched: {reason}",
        "match_evidence": "agent_event_fallback",
    }


def _observed_event_identity_blocker(
    script: dict[str, Any],
    bridge_status: dict[str, Any],
    floor: int,
    ordinal: int,
) -> dict[str, Any] | None:
    decision = floor_decision(script, floor)
    if not decision:
        return None
    events = [event for event in decision.get("events") or [] if isinstance(event, dict)]
    if ordinal >= len(events):
        return None
    expected = str(events[ordinal].get("event_name") or "").strip()
    if not expected:
        return None
    observed = _observed_event_name(bridge_status)
    if not observed:
        return None
    if _event_names_match(expected, observed):
        return None
    return _blocked(
        "event_identity_mismatch",
        f"SlayTheData expected event {expected!r} on floor {floor}, observed {observed!r}",
    ) | {
        "floor": floor,
        "ordinal": ordinal,
        "expected_event": expected,
        "observed_event": observed,
    }


def _observed_event_name(bridge_status: dict[str, Any]) -> str | None:
    game_state = _game_state(bridge_status)
    screen_state = game_state.get("screen_state") if isinstance(game_state.get("screen_state"), dict) else {}
    for value in (
        screen_state.get("event_name"),
        screen_state.get("event_id"),
        bridge_status.get("summary", {}).get("event_name")
        if isinstance(bridge_status.get("summary"), dict)
        else None,
    ):
        text = str(value or "").strip()
        if text:
            return text
    return None


def _event_names_match(expected: str, observed: str) -> bool:
    expected_token = _normalized_token(expected)
    observed_token = _normalized_token(observed)
    if expected_token == observed_token:
        return True
    aliases = {
        "goldenwing": "wingstatue",
        "thecleric": "cleric",
        "neowevent": "neow",
    }
    return aliases.get(expected_token, expected_token) == aliases.get(observed_token, observed_token)


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


def _normalized_token(value: Any) -> str:
    return "".join(ch.lower() for ch in str(value) if ch.isalnum())


def _blocked(reason: str, detail: str) -> dict[str, Any]:
    return {"status": "blocked", "reason": reason, "detail": detail}
