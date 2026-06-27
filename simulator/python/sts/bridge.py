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

        return {
            "connected": connected,
            "stale": stale,
            "exited": exited,
            "session_dir": str(self.session_dir),
            "pending_command": command_path.exists(),
            "client_pid": _first(status, summary, key="client_pid"),
            "trace_path": _first(status, summary, key="trace_path"),
            "last_state_step": _first(summary, status, key="step"),
            "ready_for_command": summary.get("ready_for_command") if isinstance(summary, dict) else None,
            "available_commands": summary.get("available_commands", []) if isinstance(summary, dict) else [],
            "status": status,
            "summary": summary,
            "current_state": current_state,
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


def _first(*values: dict[str, Any], key: str) -> Any:
    for value in values:
        if isinstance(value, dict) and value.get(key) is not None:
            return value[key]
    return None
