"""Read-only CommunicationMod bridge mirror helpers."""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
import time
from typing import Any


DEFAULT_STALE_AFTER_SECONDS = 20.0


@dataclass(frozen=True)
class BridgeMirror:
    session_dir: Path
    stale_after_seconds: float = DEFAULT_STALE_AFTER_SECONDS

    @classmethod
    def default(cls) -> "BridgeMirror":
        repo_root = Path(__file__).resolve().parents[3]
        return cls(repo_root / "tools" / "communication" / "session")

    def status(self, now: float | None = None) -> dict[str, Any]:
        now = time.time() if now is None else now
        status = _read_json(self.session_dir / "status.json")
        summary = _read_json(self.session_dir / "summary.json")
        current_state = _read_json(self.session_dir / "current_state.json")
        command_path = self.session_dir / "next_command.txt"
        ages = {
            "status_age_seconds": _age_seconds(self.session_dir / "status.json", now),
            "summary_age_seconds": _age_seconds(self.session_dir / "summary.json", now),
            "current_state_age_seconds": _age_seconds(self.session_dir / "current_state.json", now),
        }
        stale = _is_stale(ages, self.stale_after_seconds)
        exited = status.get("status") == "exited" if isinstance(status, dict) else False
        connected = bool(status) and not status.get("missing", False) and not exited
        pending_command = command_path.exists()
        lifecycle = bridge_lifecycle_from_status(
            status if isinstance(status, dict) else {},
            summary if isinstance(summary, dict) else {},
            connected=connected,
            stale=stale,
            exited=exited,
            pending_command=pending_command,
        )

        return {
            "connected": connected,
            "stale": stale,
            "exited": exited,
            "bridge_lifecycle": lifecycle,
            "session_dir": str(self.session_dir),
            "pending_command": pending_command,
            "client_pid": _first(status, summary, key="client_pid"),
            "trace_path": _first(status, summary, key="trace_path"),
            "last_state_step": _first(summary, status, key="step"),
            "last_command": status.get("command") if isinstance(status, dict) else None,
            "command_sent_at": status.get("sent_at") if isinstance(status, dict) else None,
            "ready_for_command": summary.get("ready_for_command") if isinstance(summary, dict) else None,
            "available_commands": summary.get("available_commands", []) if isinstance(summary, dict) else [],
            "status": status,
            "summary": summary,
            "current_state": current_state,
            "bridge_actions": bridge_actions_from_status(
                summary if isinstance(summary, dict) else {},
                connected=connected,
                stale=stale,
                pending_command=pending_command,
            ),
            "ages": ages,
            "last_error": _first(status, summary, key="error"),
        }

    def send_command(self, command: str, now: float | None = None) -> dict[str, Any]:
        command = command.strip()
        if not command:
            raise ValueError("command is required")
        if len(command) > 200:
            raise ValueError("command is too long")

        before = self.status(now=now)
        if before["pending_command"]:
            raise ValueError("bridge command already pending")
        if before["exited"]:
            raise ValueError("bridge has exited")

        self.session_dir.mkdir(parents=True, exist_ok=True)
        command_path = self.session_dir / "next_command.txt"
        command_path.write_text(f"{command}\n", encoding="utf-8")
        after = self.status(now=now)
        return {
            "ok": True,
            "command": command,
            "bridge_status": after,
        }


def command_for_descriptor(descriptor: dict[str, Any]) -> str:
    kind = str(descriptor.get("kind", "")).strip()
    if kind == "EndTurn":
        return "END"
    if kind == "PlayHandSlot":
        hand_slot = _required_int(descriptor, "hand_slot")
        target_slot = descriptor.get("target_slot")
        return f"PLAY {hand_slot}" if target_slot is None else f"PLAY {hand_slot} {_int(target_slot, 'target_slot')}"
    if kind == "UsePotionSlot":
        potion_slot = _required_int(descriptor, "potion_slot")
        target_slot = descriptor.get("target_slot")
        return f"POTION {potion_slot}" if target_slot is None else f"POTION {potion_slot} {_int(target_slot, 'target_slot')}"
    if kind == "DiscardPotionSlot":
        return f"POTION {_required_int(descriptor, 'potion_slot')} DISCARD"
    if kind in {"ChooseVisibleOption", "ChooseMapNodeSlot", "ChooseRestOption", "ChooseShopSlot", "TakeRewardSlot"}:
        return f"CHOOSE {_required_int(descriptor, 'option_slot')}"
    if kind == "ConfirmChoice":
        return "CONFIRM"
    if kind == "CancelChoice":
        return "CANCEL"
    if kind == "SkipVisibleReward":
        return "SKIP"
    if kind == "Proceed":
        return "PROCEED"
    if kind == "LeaveScreen":
        return "LEAVE"
    if kind == "ReturnToPreviousScreen":
        return "RETURN"
    raise ValueError(f"unsupported bridge descriptor kind: {kind or '<missing>'}")


def bridge_actions_from_status(
    summary: dict[str, Any],
    *,
    connected: bool = True,
    stale: bool = False,
    pending_command: bool = False,
) -> list[dict[str, Any]]:
    available = {str(command).lower() for command in summary.get("available_commands", [])}
    disabled_reason = _bridge_disabled_reason(
        summary,
        connected=connected,
        stale=stale,
        pending_command=pending_command,
    )
    actions: list[dict[str, Any]] = []

    if "play" in available:
        combat = summary.get("combat") or {}
        monsters = [monster for monster in combat.get("monsters", []) if not monster.get("gone")]
        for card in combat.get("hand", []):
            if not card.get("playable", True):
                continue
            hand_slot = card.get("index")
            if hand_slot is None:
                continue
            label = f"Play {card.get('name') or card.get('id') or hand_slot}"
            if card.get("has_target", False):
                for monster in monsters:
                    target_slot = monster.get("index")
                    if target_slot is None:
                        continue
                    monster_label = monster.get("name") or monster.get("id") or target_slot
                    actions.append(
                        _bridge_action(
                            f"play-{hand_slot}-{target_slot}",
                            f"{label} -> {monster_label}",
                            {
                                "kind": "PlayHandSlot",
                                "hand_slot": hand_slot,
                                "target_slot": target_slot,
                            },
                            disabled_reason,
                        )
                    )
            else:
                actions.append(
                    _bridge_action(
                        f"play-{hand_slot}",
                        label,
                        {"kind": "PlayHandSlot", "hand_slot": hand_slot},
                        disabled_reason,
                    )
                )

    if "potion" in available:
        combat = summary.get("combat") or {}
        monsters = [monster for monster in combat.get("monsters", []) if not monster.get("gone")]
        for potion in summary.get("potions", []):
            potion_slot = potion.get("index")
            if potion_slot is None:
                continue
            label = f"Use {potion.get('name') or potion.get('id') or potion_slot}"
            if potion.get("can_use"):
                requires_target = bool(potion.get("requires_target"))
                if requires_target and monsters:
                    for monster in monsters:
                        target_slot = monster.get("index")
                        if target_slot is None:
                            continue
                        actions.append(
                            _bridge_action(
                                f"potion-{potion_slot}-{target_slot}",
                                f"{label} -> {monster.get('name') or monster.get('id') or target_slot}",
                                {
                                    "kind": "UsePotionSlot",
                                    "potion_slot": potion_slot,
                                    "target_slot": target_slot,
                                },
                                disabled_reason,
                            )
                        )
                else:
                    actions.append(
                        _bridge_action(
                            f"potion-{potion_slot}",
                            label,
                            {"kind": "UsePotionSlot", "potion_slot": potion_slot},
                            disabled_reason,
                        )
                    )
            if potion.get("can_discard"):
                actions.append(
                    _bridge_action(
                        f"discard-potion-{potion_slot}",
                        f"Discard {potion.get('name') or potion.get('id') or potion_slot}",
                        {"kind": "DiscardPotionSlot", "potion_slot": potion_slot},
                        disabled_reason,
                    )
                )

    if "choose" in available:
        choices = summary.get("choices") or []
        for index, choice in enumerate(choices):
            actions.append(
                _bridge_action(
                    f"choose-{index}",
                    str(choice),
                    {"kind": "ChooseVisibleOption", "option_slot": index},
                    disabled_reason,
                )
            )

    simple_commands = [
        ("end", "End turn", {"kind": "EndTurn"}),
        ("confirm", "Confirm", {"kind": "ConfirmChoice"}),
        ("cancel", "Cancel", {"kind": "CancelChoice"}),
        ("skip", "Skip", {"kind": "SkipVisibleReward"}),
        ("proceed", "Proceed", {"kind": "Proceed"}),
        ("leave", "Leave", {"kind": "LeaveScreen"}),
        ("return", "Return", {"kind": "ReturnToPreviousScreen"}),
    ]
    for command, label, descriptor in simple_commands:
        if command in available:
            actions.append(_bridge_action(command, label, descriptor, disabled_reason))

    return actions


def bridge_lifecycle_from_status(
    status: dict[str, Any],
    summary: dict[str, Any],
    *,
    connected: bool,
    stale: bool,
    exited: bool,
    pending_command: bool,
) -> dict[str, str | None]:
    raw_status = str(status.get("status") or "").lower()
    if exited:
        return _bridge_lifecycle("exited", "Exited", _first(status, key="reason") or _first(status, key="error"))
    if not connected:
        return _bridge_lifecycle("disconnected", "Disconnected", "No active bridge client")
    if stale:
        return _bridge_lifecycle("stale", "Stale", "Bridge files have not updated recently")
    if pending_command:
        return _bridge_lifecycle("waiting_for_command_ack", "Waiting for command ack", "next_command.txt is pending")
    if raw_status == "sent":
        command = status.get("command")
        detail = f"Last command {command}" if command else "Command sent; waiting for observed state"
        return _bridge_lifecycle("waiting_for_next_state", "Waiting for next state", detail)
    if summary.get("ready_for_command") is True or raw_status == "waiting":
        return _bridge_lifecycle("ready", "Ready", "Bridge is ready for a command")
    if raw_status == "ready":
        return _bridge_lifecycle("waiting_for_observed_state", "Waiting for observed state", "Bridge client is ready but no state is published yet")
    return _bridge_lifecycle("waiting_for_observed_state", "Waiting for observed state", raw_status or None)


def _read_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {"missing": True}
    except json.JSONDecodeError as error:
        return {"error": f"invalid JSON: {error}", "missing": False}


def _age_seconds(path: Path, now: float) -> float | None:
    try:
        return max(0.0, now - path.stat().st_mtime)
    except FileNotFoundError:
        return None


def _is_stale(ages: dict[str, float | None], threshold: float) -> bool:
    observed = [age for age in ages.values() if age is not None]
    return not observed or min(observed) > threshold


def _bridge_action(
    action_id: str,
    label: str,
    descriptor: dict[str, Any],
    disabled_reason: str | None,
) -> dict[str, Any]:
    command = command_for_descriptor(descriptor)
    return {
        "action_id": action_id,
        "label": label,
        "command": command,
        "descriptor": descriptor,
        "enabled": disabled_reason is None,
        "disabled_reason": disabled_reason,
    }


def _bridge_lifecycle(status: str, label: str, detail: Any) -> dict[str, str | None]:
    return {
        "status": status,
        "label": label,
        "detail": None if detail is None else str(detail),
    }


def _bridge_disabled_reason(
    summary: dict[str, Any],
    *,
    connected: bool,
    stale: bool,
    pending_command: bool,
) -> str | None:
    if not connected:
        return "bridge disconnected"
    if stale:
        return "bridge state is stale"
    if pending_command:
        return "bridge command already pending"
    if not summary.get("ready_for_command", False):
        return "bridge is not ready for a command"
    return None


def _first(*values: dict[str, Any], key: str) -> Any:
    for value in values:
        if isinstance(value, dict) and value.get(key) is not None:
            return value[key]
    return None


def _required_int(descriptor: dict[str, Any], key: str) -> int:
    if key not in descriptor:
        raise ValueError(f"{key} is required")
    return _int(descriptor[key], key)


def _int(value: Any, key: str) -> int:
    try:
        parsed = int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"{key} must be an integer") from exc
    if parsed < 0:
        raise ValueError(f"{key} must be non-negative")
    return parsed
