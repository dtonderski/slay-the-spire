"""Local simulator-only UI service for the omniscient Python API."""

from __future__ import annotations

from dataclasses import dataclass
import json
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import urlparse
from uuid import uuid4

from sts import omni
from sts.bridge import BridgeMirror, command_for_descriptor
from sts.parity import combat_parity
from sts.search import CombatSearchConfig, search_combat
from sts.trace_replay import TraceReplayStore


UI_STATIC_DIR = Path(__file__).with_name("ui_static")


@dataclass
class CombatSession:
    id: str
    mode: str
    state_kind: str
    env: Any
    last_error: str | None = None


class SessionManager:
    def __init__(self) -> None:
        self._sessions: dict[str, CombatSession] = {}

    def create_session(self, mode: str = "combat_fixture") -> dict[str, Any]:
        if mode == "combat_fixture":
            session = CombatSession(
                id=uuid4().hex,
                mode=mode,
                state_kind="combat",
                env=omni.OmniCombatEnv.initial_fixture(),
            )
        elif mode == "run_map_fixture":
            session = CombatSession(
                id=uuid4().hex,
                mode=mode,
                state_kind="run",
                env=omni.OmniRunEnv.map_fixture(),
            )
        elif mode == "run_combat_fixture":
            session = CombatSession(
                id=uuid4().hex,
                mode=mode,
                state_kind="run",
                env=omni.OmniRunEnv.combat_fixture(),
            )
        else:
            raise ValueError(f"unsupported session mode: {mode}")
        self._sessions[session.id] = session
        return self.serialize_session(session)

    def get_session(self, session_id: str) -> dict[str, Any]:
        return self.serialize_session(self._require_session(session_id))

    def snapshot(self, session_id: str) -> dict[str, Any]:
        session = self._require_session(session_id)
        return {
            "session_id": session.id,
            "state_id": session.env.snapshot_hash(),
            "snapshot_json": session.env.snapshot_json(),
        }

    def step(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        session = self._require_session(session_id)
        state_id = session.env.snapshot_hash()
        source_state_id = payload.get("source_state_id")
        action_id = payload.get("action_id")

        if source_state_id != state_id:
            session.last_error = "stale action rejected"
            return self.serialize_session(
                session,
                command_lifecycle={
                    "status": "stale",
                    "error": session.last_error,
                    "expected_state_id": state_id,
                    "received_state_id": source_state_id,
                },
            )

        actions = self._actions(session.env, state_id)
        action = next((entry for entry in actions if entry["action_id"] == action_id), None)
        if action is None:
            session.last_error = "unknown action rejected"
            return self.serialize_session(
                session,
                command_lifecycle={
                    "status": "rejected",
                    "error": session.last_error,
                    "source_state_id": state_id,
                },
            )

        try:
            result = session.env.step(action["exact_action"])
        except Exception as error:  # PyO3 raises Python exceptions for illegal actions.
            session.last_error = f"invalid action rejected: {error}"
            return self.serialize_session(
                session,
                command_lifecycle={
                    "status": "rejected",
                    "error": session.last_error,
                    "source_state_id": state_id,
                },
            )

        session.last_error = None
        return self.serialize_session(
            session,
            command_lifecycle={
                "status": "applied",
                "previous_state_id": state_id,
                "resulting_state_id": result.snapshot_hash,
                "transition": _transition_to_json(result.transition),
            },
        )

    def search(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        session = self._require_session(session_id)
        if session.state_kind != "combat":
            raise ValueError("combat search is only available for combat fixture sessions")
        max_depth = int(payload.get("max_depth", 1))
        recommendation = search_combat(session.env, CombatSearchConfig(max_depth=max_depth))
        actions = self._actions(session.env, session.env.snapshot_hash())
        best_json = recommendation.best_action.json() if recommendation.best_action else None
        best_action_id = next(
            (action["action_id"] for action in actions if action["exact_action"].json() == best_json),
            None,
        )
        return {
            "session_id": session.id,
            "state_id": session.env.snapshot_hash(),
            "recommendation": {
                "best_action_id": best_action_id,
                "best_action": _action_to_descriptor(recommendation.best_action)
                if recommendation.best_action
                else None,
                "principal_variation": [
                    _action_to_descriptor(action) for action in recommendation.principal_variation
                ],
                "visits": recommendation.visits,
                "value": recommendation.value,
                "win_probability": recommendation.win_probability,
                "terminal_rate": recommendation.terminal_rate,
                "diagnostics": recommendation.diagnostics,
                "terminal_reason": recommendation.terminal_reason,
            },
        }

    def parity(self, session_id: str, bridge_status: dict[str, Any] | None = None) -> dict[str, Any]:
        session = self._require_session(session_id)
        bridge_status = bridge_status or BridgeMirror.default().status()
        if session.state_kind != "combat":
            return {
                "session_id": session.id,
                "state_id": session.env.snapshot_hash(),
                "parity": {
                    "status": "unknown",
                    "reason": "combat parity is only available for combat fixture sessions",
                    "diffs": [],
                },
            }
        return {
            "session_id": session.id,
            "state_id": session.env.snapshot_hash(),
            "parity": combat_parity(json.loads(session.env.state_json()), bridge_status),
        }

    def serialize_session(
        self,
        session: CombatSession,
        command_lifecycle: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        state_id = session.env.snapshot_hash()
        state = json.loads(session.env.state_json())
        actions = self._actions(session.env, state_id)
        terminal_reason = _terminal_reason(session.env.phase())
        unsupported_reason = _call_optional(session.env, "unsupported_reason") if session.state_kind == "run" else None
        empty_reason = None
        if not actions:
            empty_reason = (
                {"kind": "terminal", "reason": terminal_reason}
                if terminal_reason
                else {
                    "kind": "unsupported",
                    "reason": unsupported_reason or f"no exact {session.state_kind} actions available",
                }
            )

        return {
            "session_id": session.id,
            "mode": session.mode,
            "state_kind": session.state_kind,
            "state_id": state_id,
            "phase": session.env.phase(),
            "current_decision": _call_optional(session.env, "current_decision"),
            "unsupported_reason": unsupported_reason,
            "terminal_reason": terminal_reason,
            "decision_substate": _decision_substate(session, terminal_reason),
            "state": state,
            "actions": [_public_action(action) for action in actions],
            "empty_action_reason": empty_reason,
            "command_lifecycle": command_lifecycle or {"status": "ready"},
            "last_error": session.last_error,
        }

    def _require_session(self, session_id: str) -> CombatSession:
        try:
            return self._sessions[session_id]
        except KeyError as exc:
            raise KeyError(f"unknown session: {session_id}") from exc

    def _actions(self, env: Any, state_id: str) -> list[dict[str, Any]]:
        actions = []
        for index, exact_action in enumerate(env.exact_legal_actions()):
            actions.append(
                {
                    "action_id": f"a{index}",
                    "source_state_id": state_id,
                    "descriptor": _action_to_descriptor(exact_action),
                    "label": _action_label(exact_action),
                    "enabled": True,
                    "disabled_reason": None,
                    "exact_action": exact_action,
                    "exact_action_json": exact_action.json(),
                }
            )
        return actions


class UiRequestHandler(SimpleHTTPRequestHandler):
    manager = SessionManager()
    bridge = BridgeMirror.default()
    traces = TraceReplayStore.default()

    def __init__(self, *args: Any, static_dir: Path | None = None, **kwargs: Any) -> None:
        self._static_dir = static_dir or UI_STATIC_DIR
        super().__init__(*args, directory=str(self._static_dir), **kwargs)

    def do_GET(self) -> None:
        try:
            parsed = urlparse(self.path)
            path = parsed.path
            parts = _path_parts(path)
            query = _query_params(parsed.query)
            if parts[:2] == ["api", "sessions"] and len(parts) == 3:
                self._send_json(self.manager.get_session(parts[2]))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "snapshot":
                self._send_json(self.manager.snapshot(parts[2]))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "parity":
                self._send_json(self.manager.parity(parts[2], self.bridge.status()))
                return
            if parts == ["api", "bridge"]:
                self._send_json(self.bridge.status())
                return
            if parts == ["api", "traces"]:
                self._send_json(self.traces.list_traces(_query_int(query, "limit", 50)))
                return
            if parts[:2] == ["api", "traces"] and len(parts) == 3:
                self._send_json(
                    self.traces.load_trace(
                        parts[2],
                        offset=_query_int(query, "offset", 0),
                        limit=_query_int(query, "limit", 200),
                    )
                )
                return
            if path == "/":
                self.path = "/index.html"
            super().do_GET()
        except Exception as error:
            self._send_error(error)

    def do_POST(self) -> None:
        try:
            path = urlparse(self.path).path
            parts = _path_parts(path)
            payload = self._read_json()
            if parts == ["api", "sessions"]:
                self._send_json(self.manager.create_session(payload.get("mode", "combat_fixture")))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "step":
                self._send_json(self.manager.step(parts[2], payload))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "search":
                self._send_json(self.manager.search(parts[2], payload))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "parity":
                self._send_json(self.manager.parity(parts[2], self.bridge.status()))
                return
            if parts == ["api", "bridge", "command"]:
                self._send_json(self.bridge.send_command(str(payload.get("command", ""))))
                return
            if parts == ["api", "bridge", "descriptor"]:
                command = command_for_descriptor(payload.get("descriptor", {}))
                self._send_json(self.bridge.send_command(command))
                return
            self.send_error(404, "not found")
        except Exception as error:
            self._send_error(error)

    def _read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length == 0:
            return {}
        body = self.rfile.read(length).decode("utf-8")
        return json.loads(body)

    def _send_json(self, payload: dict[str, Any], status: int = 200) -> None:
        encoded = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def _send_error(self, error: Exception) -> None:
        status = 404 if isinstance(error, KeyError) else 400
        self._send_json({"error": str(error)}, status=status)


def run(host: str = "127.0.0.1", port: int = 8799) -> None:
    server = ThreadingHTTPServer((host, port), UiRequestHandler)
    print(f"Serving omniscient simulator UI at http://{host}:{port}/")
    server.serve_forever()


def _public_action(action: dict[str, Any]) -> dict[str, Any]:
    return {
        "action_id": action["action_id"],
        "source_state_id": action["source_state_id"],
        "descriptor": action["descriptor"],
        "label": action["label"],
        "enabled": action["enabled"],
        "disabled_reason": action["disabled_reason"],
        "exact_action_json": action["exact_action_json"],
    }


def _action_to_descriptor(action: omni.ExactCombatAction) -> dict[str, Any]:
    if hasattr(action, "family"):
        return {
            "kind": "ExactRunAction",
            "family": action.family(),
            "action_kind": action.kind(),
            "action": _json_or_string(action.json()),
        }
    if action.kind() == "end_turn":
        return {"kind": "EndTurn"}
    return {
        "kind": "PlayCard",
        "card_id": action.card_id(),
        "target": action.target(),
    }


def _action_label(action: omni.ExactCombatAction) -> str:
    if hasattr(action, "family"):
        return _run_action_label(action)
    if action.kind() == "end_turn":
        return "End Turn"
    target = action.target()
    suffix = f" -> monster {target}" if target is not None else ""
    return f"Play card {action.card_id()}{suffix}"


def _transition_to_json(transition: Any) -> dict[str, Any]:
    return {
        "action_json": transition.action_json,
        "previous_hash": transition.previous_hash,
        "resulting_hash": transition.resulting_hash,
        "events_json": transition.events_json,
        "rng_draws_json": transition.rng_draws_json,
        "simulator_error": transition.simulator_error,
    }


def _terminal_reason(phase: str) -> str | None:
    return phase if phase in {"won", "lost"} else None


def _decision_substate(session: CombatSession, terminal_reason: str | None) -> str:
    if terminal_reason:
        return "Terminal"
    if session.state_kind == "run":
        return _call_optional(session.env, "current_decision") or "RunDecision"
    return "NormalCombat"


def _call_optional(obj: Any, name: str) -> Any:
    method = getattr(obj, name, None)
    if method is None:
        return None
    return method()


def _run_action_label(action: Any) -> str:
    kind = action.kind()
    family = action.family()
    data = _json_or_string(action.json())
    if isinstance(data, dict):
        details = ", ".join(f"{_humanize(key)} {value}" for key, value in data.items())
        return f"{_humanize(kind)} ({details})" if details else _humanize(kind)
    return f"{_humanize(family)}: {_humanize(kind)}"


def _json_or_string(value: str) -> Any:
    try:
        return json.loads(value)
    except json.JSONDecodeError:
        return value


def _humanize(value: Any) -> str:
    return str(value).replace("_", " ").replace("-", " ").title()


def _path_parts(path: str) -> list[str]:
    return [part for part in path.split("/") if part]


def _query_params(query: str) -> dict[str, list[str]]:
    from urllib.parse import parse_qs

    return parse_qs(query, keep_blank_values=True)


def _query_int(query: dict[str, list[str]], name: str, default: int) -> int:
    values = query.get(name)
    if not values:
        return default
    try:
        return int(values[0])
    except ValueError:
        return default


if __name__ == "__main__":
    run()
