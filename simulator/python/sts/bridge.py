"""Read-only CommunicationMod bridge mirror helpers."""

from __future__ import annotations

from dataclasses import dataclass
import csv
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time
from typing import Any
from uuid import uuid4


DEFAULT_STALE_AFTER_SECONDS = 120.0


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
        file_pending = command_path.exists()
        tcp_pending = bool(status.get("pending_command")) if isinstance(status, dict) else False
        pending_command = file_pending or tcp_pending
        command_meta = (
            _read_json(self.session_dir / "next_command.json")
            if file_pending
            else status.get("queued_command_meta", {}) if isinstance(status, dict) and tcp_pending else {}
        )
        ages = {
            "status_age_seconds": _age_seconds(self.session_dir / "status.json", now),
            "summary_age_seconds": _age_seconds(self.session_dir / "summary.json", now),
            "current_state_age_seconds": _age_seconds(self.session_dir / "current_state.json", now),
        }
        stale = _is_stale(ages, self.stale_after_seconds)
        exited = status.get("status") == "exited" if isinstance(status, dict) else False
        connected = bool(status) and not status.get("missing", False) and not exited
        state_id = _bridge_state_id(status, summary, current_state)
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
            "state_id": state_id,
            "control": _bridge_control({"status": status}),
            "controller": status.get("controller") if isinstance(status, dict) else None,
            "session_dir": str(self.session_dir),
            "pending_command": pending_command,
            "pending_command_meta": command_meta if pending_command else None,
            "command_id": command_meta.get("command_id") if pending_command else None,
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
                source_state_id=state_id,
                connected=connected,
                stale=stale,
                pending_command=pending_command,
            ),
            "ages": ages,
            "last_error": _first(status, summary, key="error"),
        }

    def preflight(self, now: float | None = None) -> dict[str, Any]:
        now = time.time() if now is None else now
        status_path = self.session_dir / "status.json"
        summary_path = self.session_dir / "summary.json"
        command_path = self.session_dir / "next_command.txt"
        command_meta_path = self.session_dir / "next_command.json"
        status = _read_json(status_path)
        summary = _read_json(summary_path)
        status_age = _age_seconds(status_path, now)
        summary_age = _age_seconds(summary_path, now)
        command_exists = command_path.exists()
        tcp_pending = bool(status.get("pending_command")) if isinstance(status, dict) else False
        command_meta_exists = command_meta_path.exists()
        command_meta = (
            _read_json(command_meta_path)
            if command_exists
            else status.get("queued_command_meta", {}) if isinstance(status, dict) and tcp_pending else {}
        )
        problems = []
        warnings = []

        if status.get("missing"):
            problems.append("missing session status.json")
        if summary.get("missing"):
            problems.append("missing session summary.json")
        if _is_stale({"status_age_seconds": status_age, "summary_age_seconds": summary_age}, self.stale_after_seconds):
            problems.append("session files are stale")
        if status.get("status") == "exited":
            problems.append(f"bridge exited: {status.get('reason') or 'unknown'}")
        if command_exists or tcp_pending:
            problems.append("bridge command already pending")
        if command_meta_exists and not command_exists:
            problems.append("next_command.json exists without next_command.txt")
        if summary.get("ready_for_command") is not True:
            warnings.append("latest summary is not ready_for_command")
        if status.get("status") == "sent" and summary.get("step") is not None and status.get("step") is not None:
            try:
                if int(status["step"]) > int(summary["step"]):
                    problems.append(f"sent command step {status['step']} is newer than summary step {summary['step']}")
            except (TypeError, ValueError):
                warnings.append("could not compare status and summary steps")
        available_commands = summary.get("available_commands") if isinstance(summary.get("available_commands"), list) else []
        if available_commands and "state" not in available_commands:
            warnings.append("available_commands does not include state")
        control = _bridge_control({"status": status})
        if control is None:
            warnings.append("TCP bridge control is not available; guided auto-collection will not send")

        return {
            "ok": not problems,
            "problems": problems,
            "warnings": warnings,
            "tcp_control_available": control is not None,
            "control": control,
            "ages": {
                "status_age_seconds": status_age,
                "summary_age_seconds": summary_age,
            },
            "pending_command": {
                "present": command_exists or tcp_pending,
                "transport": "file" if command_exists else "tcp-jsonl" if tcp_pending else None,
                "command_id": command_meta.get("command_id") if command_exists or tcp_pending else None,
                "command": command_meta.get("command") if command_exists or tcp_pending else None,
            },
            "summary": {
                "step": summary.get("step"),
                "state_seq": summary.get("state_seq"),
                "client_pid": summary.get("client_pid"),
                "screen_type": summary.get("screen_type"),
                "floor": summary.get("floor"),
                "seed": summary.get("seed"),
                "ready_for_command": summary.get("ready_for_command"),
                "available_commands": available_commands,
            }
            if not summary.get("missing")
            else None,
            "status": {
                "step": status.get("step"),
                "client_pid": status.get("client_pid"),
                "status": status.get("status"),
                "trace_path": status.get("trace_path"),
                "command": status.get("command"),
            }
            if not status.get("missing")
            else None,
        }

    def clear_orphan_command_metadata(self) -> dict[str, Any]:
        command_path = self.session_dir / "next_command.txt"
        command_meta_path = self.session_dir / "next_command.json"
        if command_path.exists():
            raise ValueError("cannot clear command metadata while next_command.txt exists")
        if not command_meta_path.exists():
            return {"ok": True, "cleared": False, "reason": "next_command.json is already absent"}
        command_meta_path.unlink()
        return {"ok": True, "cleared": True}

    def send_command(
        self,
        command: str,
        now: float | None = None,
        *,
        source_state_id: str | None = None,
        metadata: dict[str, Any] | None = None,
        require_tcp_control: bool = False,
        wait_for_state_update: bool = False,
        update_timeout_seconds: float = 5.0,
    ) -> dict[str, Any]:
        command = command.strip()
        if not command:
            raise ValueError("command is required")
        if len(command) > 200:
            raise ValueError("command is too long")

        before = self.status(now=now)
        if source_state_id is not None and source_state_id != before["state_id"]:
            raise ValueError("stale bridge action rejected")
        if before["pending_command"]:
            raise ValueError("bridge command already pending")
        if not before["connected"]:
            raise ValueError("bridge is disconnected")
        if before["exited"]:
            raise ValueError("bridge has exited")
        verb = _command_verb(command)
        available = {str(command).lower() for command in before["available_commands"]}
        stale_start_from_menu = (
            verb == "start"
            and "start" in available
            and isinstance(before.get("summary"), dict)
            and before["summary"].get("in_game") is False
        )
        if before["stale"] and verb != "state" and source_state_id is None and not stale_start_from_menu:
            raise ValueError("bridge state is stale")
        if verb != "state" and before["ready_for_command"] is not True:
            raise ValueError("bridge is not ready for a command")
        if verb != "state" and verb not in available:
            raise ValueError(f'command "{verb}" is not available')

        control = _bridge_control(before)
        if control is not None:
            return self._send_command_via_control(
                command,
                control=control,
                source_state_id=source_state_id or before["state_id"],
                source_state_seq=before.get("summary", {}).get("state_seq")
                if isinstance(before.get("summary"), dict)
                else None,
                metadata=metadata,
                now=now,
                wait_for_state_update=wait_for_state_update,
                update_timeout_seconds=update_timeout_seconds,
            )
        if require_tcp_control:
            raise ValueError("TCP bridge control is required for this command")

        self.session_dir.mkdir(parents=True, exist_ok=True)
        command_id = uuid4().hex
        command_path = self.session_dir / "next_command.txt"
        command_meta = {
            "command_id": command_id,
            "command": command,
            "source_state_id": source_state_id,
            "submitted_at": now if now is not None else time.time(),
        }
        if metadata is not None:
            command_meta["metadata"] = metadata
        (self.session_dir / "next_command.json").write_text(
            json.dumps(command_meta, sort_keys=True),
            encoding="utf-8",
        )
        command_path.write_text(f"{command}\n", encoding="utf-8")
        after = self.status(now=now)
        return {
            "ok": True,
            "command_id": command_id,
            "command": command,
            "bridge_status": after,
        }

    def _send_command_via_control(
        self,
        command: str,
        *,
        control: dict[str, Any],
        source_state_id: str | None,
        source_state_seq: Any,
        metadata: dict[str, Any] | None,
        now: float | None,
        wait_for_state_update: bool,
        update_timeout_seconds: float,
    ) -> dict[str, Any]:
        command_id = uuid4().hex
        owner = _acquire_control_owner(control)
        payload: dict[str, Any] = {
            "type": "command",
            "command": command,
            "command_id": command_id,
            "expected_state_id": source_state_id,
            "owner_token": owner.get("owner_token"),
        }
        if source_state_seq is not None:
            payload["expected_state_seq"] = source_state_seq
        if metadata is not None:
            payload["metadata"] = metadata
        if wait_for_state_update:
            payload["wait_for_state_update"] = True
            payload["update_timeout_ms"] = int(max(0.001, update_timeout_seconds) * 1000)
        response = _control_request(control, payload, timeout=max(2.0, update_timeout_seconds + 1.0))
        if not response.get("ok"):
            raise ValueError(str(response.get("error") or "bridge control command rejected"))
        after = self.status(now=now)
        return {
            "ok": True,
            "transport": "tcp-jsonl",
            "command_id": response.get("command_id") or command_id,
            "command": response.get("command") or command,
            "accepted_state_id": response.get("accepted_state_id"),
            "accepted_state_seq": response.get("accepted_state_seq"),
            "observed_update": response.get("observed_update"),
            "owner_id": owner.get("owner_id"),
            "bridge_status": after,
        }

    def clients(
        self,
        *,
        trace_dir: Path | None = None,
        now: float | None = None,
        process_info: Any | None = None,
    ) -> dict[str, Any]:
        now = time.time() if now is None else now
        process_info = process_info or _process_info
        status = self.status(now=now)
        repo_root = Path(__file__).resolve().parents[3]
        trace_dir = trace_dir or repo_root / "verification" / "corpus" / "communication_mod"
        clients: dict[int, dict[str, Any]] = {}

        def add_client(pid_value: Any, *, source: str, trace_path: Any = None, started_at: Any = None) -> None:
            try:
                pid = int(pid_value)
            except (TypeError, ValueError):
                return
            if pid <= 0:
                return
            entry = clients.setdefault(
                pid,
                {
                    "pid": pid,
                    "current": False,
                    "sources": [],
                    "trace_paths": [],
                    "started_at": None,
                    "trace_modified_at": None,
                    "trace_age_seconds": None,
                },
            )
            if source not in entry["sources"]:
                entry["sources"].append(source)
            if source == "current_status":
                entry["current"] = True
            if trace_path:
                path_text = str(trace_path)
                if path_text not in entry["trace_paths"]:
                    entry["trace_paths"].append(path_text)
                trace_stat = _path_stat(path_text, now)
                if trace_stat and (
                    entry["trace_modified_at"] is None
                    or trace_stat["modified_epoch"] > entry.get("_trace_modified_epoch", -1)
                ):
                    entry["_trace_modified_epoch"] = trace_stat["modified_epoch"]
                    entry["trace_modified_at"] = trace_stat["modified_at"]
                    entry["trace_age_seconds"] = trace_stat["age_seconds"]
            if started_at and entry["started_at"] is None:
                entry["started_at"] = started_at

        add_client(
            status.get("client_pid"),
            source="current_status",
            trace_path=status.get("trace_path"),
        )

        for trace_path in _recent_trace_paths(trace_dir):
            metadata = _trace_metadata(trace_path)
            add_client(
                metadata.get("client_pid"),
                source="trace_metadata",
                trace_path=trace_path,
                started_at=metadata.get("started_at") or metadata.get("timestamp"),
            )

        rows: list[dict[str, Any]] = []
        for pid, entry in clients.items():
            info = process_info(pid)
            entry.update(info if isinstance(info, dict) else {"alive": False})
            entry["killable"] = _is_killable_bridge_client(entry)
            entry.pop("_trace_modified_epoch", None)
            rows.append(entry)

        rows.sort(key=lambda client: (not client.get("current", False), not client.get("alive", False), client["pid"]))
        return {
            "clients": rows,
            "current_pid": status.get("client_pid"),
            "session_dir": str(self.session_dir),
            "trace_dir": str(trace_dir),
        }

    def kill_client(self, pid: Any) -> dict[str, Any]:
        try:
            parsed = int(pid)
        except (TypeError, ValueError) as exc:
            raise ValueError("pid must be an integer") from exc
        if parsed <= 0:
            raise ValueError("pid must be positive")
        if parsed == os.getpid():
            raise ValueError("refusing to kill the UI service process")

        clients = {client["pid"]: client for client in self.clients()["clients"]}
        client = clients.get(parsed)
        if client is None:
            raise ValueError("pid is not a known bridge client")
        if not client.get("alive"):
            return {"ok": True, "pid": parsed, "already_exited": True}
        if not client.get("killable"):
            raise ValueError("refusing to kill a process that does not look like an active bridge client")

        _kill_process(parsed)
        return {"ok": True, "pid": parsed, "already_exited": False}


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
        return (
            f"POTION USE {potion_slot}"
            if target_slot is None
            else f"POTION USE {potion_slot} {_int(target_slot, 'target_slot')}"
        )
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


def _command_verb(command: str) -> str:
    return command.strip().split(maxsplit=1)[0].lower()


def bridge_actions_from_status(
    summary: dict[str, Any],
    *,
    source_state_id: str | None = None,
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
                            source_state_id,
                        )
                    )
            else:
                actions.append(
                    _bridge_action(
                        f"play-{hand_slot}",
                        label,
                        {"kind": "PlayHandSlot", "hand_slot": hand_slot},
                        disabled_reason,
                        source_state_id,
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
                                source_state_id,
                            )
                        )
                else:
                    actions.append(
                        _bridge_action(
                            f"potion-{potion_slot}",
                            label,
                            {"kind": "UsePotionSlot", "potion_slot": potion_slot},
                            disabled_reason,
                            source_state_id,
                        )
                    )
            if potion.get("can_discard"):
                actions.append(
                    _bridge_action(
                        f"discard-potion-{potion_slot}",
                        f"Discard {potion.get('name') or potion.get('id') or potion_slot}",
                        {"kind": "DiscardPotionSlot", "potion_slot": potion_slot},
                        disabled_reason,
                        source_state_id,
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
                    source_state_id,
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
            actions.append(_bridge_action(command, label, descriptor, disabled_reason, source_state_id))

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
    source_state_id: str | None,
) -> dict[str, Any]:
    command = command_for_descriptor(descriptor)
    return {
        "action_id": action_id,
        "source_state_id": source_state_id,
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
    if pending_command:
        return "bridge command already pending"
    if not summary.get("ready_for_command", False):
        return "bridge is not ready for a command"
    return None


def _recent_trace_paths(trace_dir: Path, limit: int = 50) -> list[Path]:
    try:
        paths = [path for path in trace_dir.glob("trace-*.jsonl") if path.is_file()]
    except OSError:
        return []
    paths.sort(key=lambda path: path.stat().st_mtime, reverse=True)
    return paths[:limit]


def _trace_metadata(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            for _ in range(8):
                line = handle.readline()
                if not line:
                    break
                try:
                    record = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if isinstance(record, dict) and record.get("type") == "metadata":
                    return record
    except OSError:
        return {}
    return {}


def _path_stat(path_text: str, now: float) -> dict[str, Any] | None:
    try:
        stat = Path(path_text).stat()
    except OSError:
        return None
    modified_epoch = stat.st_mtime
    return {
        "modified_epoch": modified_epoch,
        "modified_at": _iso_from_epoch(modified_epoch),
        "age_seconds": max(0.0, now - modified_epoch),
    }


def _iso_from_epoch(value: float) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(value))


def _process_info(pid: int) -> dict[str, Any]:
    if os.name == "nt":
        return _windows_process_info(pid)
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return {"alive": False, "name": None}
    except PermissionError:
        return {"alive": True, "name": None, "permission_limited": True}
    return {"alive": True, "name": None}


def _is_killable_bridge_client(client: dict[str, Any]) -> bool:
    if not client.get("alive"):
        return False
    if client.get("pid") == os.getpid():
        return False
    name = str(client.get("name") or "").lower()
    if name not in {"node.exe", "node"}:
        return False
    age = client.get("trace_age_seconds")
    if isinstance(age, (int, float)) and age > 12 * 60 * 60:
        return False
    return True


def _windows_process_info(pid: int) -> dict[str, Any]:
    try:
        result = subprocess.run(
            ["tasklist", "/FI", f"PID eq {pid}", "/FO", "CSV", "/NH"],
            capture_output=True,
            text=True,
            check=False,
            timeout=3,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return {"alive": False, "name": None, "process_error": str(error)}
    output = result.stdout.strip()
    if not output or "No tasks are running" in output:
        return {"alive": False, "name": None}
    try:
        row = next(csv.reader([output.splitlines()[0]]))
    except (csv.Error, StopIteration, IndexError):
        return {"alive": True, "name": None}
    return {
        "alive": True,
        "name": row[0] if row else None,
        "session_name": row[2] if len(row) > 2 else None,
        "memory": row[4] if len(row) > 4 else None,
    }


def _kill_process(pid: int) -> None:
    if os.name == "nt":
        result = subprocess.run(
            ["taskkill", "/PID", str(pid), "/F"],
            capture_output=True,
            text=True,
            check=False,
            timeout=5,
        )
        if result.returncode != 0:
            message = (result.stderr or result.stdout or "taskkill failed").strip()
            raise ValueError(message)
        return
    os.kill(pid, signal.SIGTERM)


def _bridge_state_id(status: dict[str, Any], summary: dict[str, Any], current_state: dict[str, Any]) -> str:
    for value in (summary.get("state_id"), current_state.get("state_id"), status.get("state_id")):
        if value:
            return str(value)
    payload = {
        "status": status,
        "summary": summary,
        "current_state": current_state,
    }
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()[:32]


def _bridge_control(status: dict[str, Any]) -> dict[str, Any] | None:
    raw = status.get("status") if isinstance(status, dict) else None
    control = raw.get("control") if isinstance(raw, dict) else None
    if not isinstance(control, dict):
        return None
    if control.get("protocol") != "tcp-jsonl":
        return None
    host = str(control.get("host") or "127.0.0.1")
    try:
        port = int(control.get("port"))
    except (TypeError, ValueError):
        return None
    if port <= 0 or port > 65535:
        return None
    return {"host": host, "port": port, "protocol": "tcp-jsonl"}


def _control_request(control: dict[str, Any], payload: dict[str, Any], timeout: float = 2.0) -> dict[str, Any]:
    host = str(control["host"])
    port = int(control["port"])
    encoded = (json.dumps(payload, separators=(",", ":")) + "\n").encode("utf-8")
    with socket.create_connection((host, port), timeout=timeout) as sock:
        sock.settimeout(timeout)
        sock.sendall(encoded)
        chunks: list[bytes] = []
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            chunks.append(chunk)
            if b"\n" in chunk:
                break
    data = b"".join(chunks).split(b"\n", 1)[0]
    if not data:
        raise ValueError("bridge control returned no response")
    response = json.loads(data.decode("utf-8"))
    if not isinstance(response, dict):
        raise ValueError("bridge control returned a non-object response")
    return response


def _acquire_control_owner(control: dict[str, Any]) -> dict[str, Any]:
    response = _control_request(
        control,
        {
            "type": "acquire",
            "owner_id": "sts-python-ui",
        },
    )
    if not response.get("ok"):
        raise ValueError(str(response.get("error") or "bridge control ownership rejected"))
    if not response.get("owner_token"):
        raise ValueError("bridge control did not return an owner_token")
    return response


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
