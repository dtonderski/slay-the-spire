"""Local simulator-only UI service for the omniscient Python API."""

from __future__ import annotations

from dataclasses import dataclass, replace
import json
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import urlparse
from uuid import uuid4

from sts import omni
from sts.bridge import BridgeMirror, command_for_descriptor
from sts.guided_collector import GuidedCollector
from sts.parity import combat_parity
from sts.search import CombatSearchConfig, search_combat
from sts.search_lab import SELECTED_COMBAT_AUTOPILOT_CANDIDATE, trace_autopilot_candidate_by_name
from sts.self_play import _action_for_communication_command
from sts.slaythedata_index import (
    export_guided_run_script,
    select_guided_collection_candidates,
    slaythedata_index_status,
)
from sts.self_play import strict_replay_real_trace_to_env
from sts.slaythedata_policy import build_guided_run_script, load_guided_run_script
from sts.trace_replay import TraceReplayStore


UI_STATIC_DIR = Path(__file__).with_name("ui_static")
DEFAULT_GUIDED_REPORT_PATH = Path(__file__).resolve().parents[2] / "target" / "guided-collect" / "latest.json"


@dataclass
class CombatSession:
    id: str
    mode: str
    state_kind: str
    env: Any
    last_error: str | None = None
    last_lifecycle: dict[str, Any] | None = None


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

    def create_live_session(self, bridge_status: dict[str, Any]) -> dict[str, Any]:
        replay = _strict_replay_from_bridge_trace(bridge_status)
        replay_blocker = None
        if replay and replay.verified and replay.env is not None:
            env = replay.env
            attach_fidelity = "seed_replay"
            attach_source = {
                "trace_path": str(replay.trace_path),
                "steps": replay.steps,
                "final_state_id": replay.final_state_id,
                "final_phase": replay.final_phase,
            }
        else:
            observed = _observed_state_from_bridge_status(bridge_status)
            env = omni.OmniRunEnv.from_communication_mod_state_json(json.dumps(observed))
            attach_fidelity = "observed_state"
            attach_source = {"trace_path": bridge_status.get("trace_path")}
            if replay:
                replay_blocker = {
                    "stop_reason": replay.stop_reason,
                    "blocker": replay.blocker,
                    "steps": replay.steps,
                    "final_state_id": replay.final_state_id,
                }
        session = CombatSession(
            id=uuid4().hex,
            mode="live_bridge",
            state_kind="run",
            env=env,
        )
        self._sessions[session.id] = session
        result = self.serialize_session(session)
        result["bridge_state_id"] = bridge_status.get("state_id")
        result["bridge_step"] = bridge_status.get("last_state_step")
        result["attach_fidelity"] = attach_fidelity
        result["attach_source"] = attach_source
        result["strict_replay_blocker"] = replay_blocker
        return result

    def get_session(self, session_id: str) -> dict[str, Any]:
        return self.serialize_session(self._require_session(session_id))

    def state(self, session_id: str) -> dict[str, Any]:
        return self.serialize_session(self._require_session(session_id))

    def actions(self, session_id: str) -> dict[str, Any]:
        session = self.serialize_session(self._require_session(session_id))
        return {
            "session_id": session["session_id"],
            "state_id": session["state_id"],
            "decision_substate": session["decision_substate"],
            "actions": session["actions"],
            "empty_action_reason": session["empty_action_reason"],
        }

    def pending_command(self, session_id: str) -> dict[str, Any]:
        session = self._require_session(session_id)
        return {
            "session_id": session.id,
            "state_id": session.env.snapshot_hash(),
            "command_lifecycle": session.last_lifecycle or _command_lifecycle("ready"),
        }

    def snapshot(self, session_id: str) -> dict[str, Any]:
        session = self._require_session(session_id)
        return {
            "session_id": session.id,
            "state_id": session.env.snapshot_hash(),
            "snapshot_json": session.env.snapshot_json(),
        }

    def restore(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        session = self._require_session(session_id)
        snapshot_json = payload.get("snapshot_json")
        if not isinstance(snapshot_json, str) or not snapshot_json.strip():
            raise ValueError("snapshot_json is required")

        previous_state_id = session.env.snapshot_hash()
        restored_env = _env_from_snapshot(session.state_kind, snapshot_json)
        session.env = restored_env
        session.last_error = None
        return self.serialize_session(
            session,
            command_lifecycle=_command_lifecycle(
                "restored",
                previous_state_id=previous_state_id,
                resulting_state_id=session.env.snapshot_hash(),
            ),
        )

    def predict(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        session = self._require_session(session_id)
        state_id = session.env.snapshot_hash()
        source_state_id = payload.get("source_state_id")
        if source_state_id != state_id:
            raise ValueError(f"stale prediction: expected {state_id}, received {source_state_id}")

        action = self._action_from_payload(session, payload, state_id)
        clone = session.env.clone()
        result = clone.step(action["exact_action"])
        return {
            "session_id": session.id,
            "source_state_id": state_id,
            "predicted_state_id": result.snapshot_hash,
            "predicted_snapshot_json": result.snapshot_json,
            "transition": _transition_to_json(result.transition),
            "action": _public_action(action),
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
                command_lifecycle=_command_lifecycle(
                    "stale",
                    error=session.last_error,
                    source_state_id=source_state_id,
                    expected_state_id=state_id,
                    received_state_id=source_state_id,
                ),
            )

        actions = self._actions(session.env, state_id)
        action = next((entry for entry in actions if entry["action_id"] == action_id), None)
        if action is None:
            session.last_error = "unknown action rejected"
            return self.serialize_session(
                session,
                command_lifecycle=_command_lifecycle(
                    "rejected",
                    error=session.last_error,
                    source_state_id=state_id,
                    state_unchanged=True,
                ),
            )

        try:
            result = session.env.step(action["exact_action"])
        except Exception as error:  # PyO3 raises Python exceptions for illegal actions.
            session.last_error = f"invalid action rejected: {error}"
            return self.serialize_session(
                session,
                command_lifecycle=_command_lifecycle(
                    "rejected",
                    error=session.last_error,
                    source_state_id=state_id,
                    state_unchanged=True,
                ),
            )

        session.last_error = None
        return self.serialize_session(
            session,
            command_lifecycle=_command_lifecycle(
                "applied",
                previous_state_id=state_id,
                resulting_state_id=result.snapshot_hash,
                transition=_transition_to_json(result.transition),
            ),
        )

    def search(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        session = self._require_session(session_id)
        if not _can_search_combat(session):
            raise ValueError("combat search is only available for combat sessions")
        state_id = session.env.snapshot_hash()
        source_state_id = payload.get("source_state_id")
        if source_state_id is not None and source_state_id != state_id:
            raise ValueError(f"stale search: expected {state_id}, received {source_state_id}")
        config = _combat_search_config(payload)
        recommendation = search_combat(session.env, config)
        resulting_state_id = session.env.snapshot_hash()
        if resulting_state_id != state_id:
            raise ValueError(f"search state changed while running: expected {state_id}, observed {resulting_state_id}")
        actions = self._actions(session.env, state_id)
        best_json = recommendation.best_action.json() if recommendation.best_action else None
        best_action_id = next(
            (action["action_id"] for action in actions if action["exact_action"].json() == best_json),
            None,
        )
        predicted_final_hp = _search_predicted_final_hp(recommendation.diagnostics)
        predicted_hp_loss = _search_predicted_hp_loss(session.env, predicted_final_hp)
        return {
            "session_id": session.id,
            "state_id": state_id,
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
                "predicted_final_hp": predicted_final_hp,
                "predicted_hp_loss": predicted_hp_loss,
                "diagnostics": recommendation.diagnostics,
                "terminal_reason": recommendation.terminal_reason,
                "config": config.__dict__,
            },
        }

    def send_live_combat_action(
        self,
        bridge_status: dict[str, Any],
        suggestion: dict[str, Any],
        payload: dict[str, Any],
        *,
        send_command: Any,
    ) -> dict[str, Any]:
        live_session = self.create_live_session(bridge_status)
        if live_session.get("attach_fidelity") != "seed_replay":
            raise ValueError("live combat send requires strict seed replay attachment")
        if not _can_search_combat(self._require_session(live_session["session_id"])):
            raise ValueError("live combat send requires a combat state")

        search_payload = {
            "candidate": payload.get("candidate"),
            "max_depth": int(payload.get("max_depth", 40)),
            "source_state_id": live_session["state_id"],
        }
        if "allowed_potions" in payload:
            search_payload["allowed_potions"] = payload["allowed_potions"]
        elif int(suggestion.get("potion_uses_allowed", 0) or 0) <= 0:
            search_payload["allowed_potions"] = []

        search_result = self.search(live_session["session_id"], search_payload)
        recommendation = search_result["recommendation"]
        best_action = recommendation.get("best_action")
        if not isinstance(best_action, dict):
            raise ValueError("combat search produced no sendable action")

        prediction = self.predict(
            live_session["session_id"],
            {
                "descriptor": best_action,
                "source_state_id": live_session["state_id"],
            },
        )
        bridge_action = _bridge_action_for_exact_action(best_action, bridge_status, live_session["state"])
        if bridge_action is None:
            raise ValueError("combat recommendation cannot be mapped to a current bridge command")

        source_state_id = bridge_status.get("state_id")
        send_kwargs = {"source_state_id": source_state_id}
        if payload.get("provenance") is not None:
            send_kwargs["metadata"] = payload["provenance"]
        result = send_command(bridge_action["command"], **send_kwargs)
        return {
            "session_id": live_session["session_id"],
            "source_state_id": live_session["state_id"],
            "bridge_state_id": source_state_id,
            "bridge_step": bridge_status.get("last_state_step"),
            "predicted_state_id": prediction["predicted_state_id"],
            "recommendation": recommendation,
            "bridge_action": bridge_action,
            "send_result": {
                "ok": result.get("ok"),
                "command_id": result.get("command_id"),
                "command": result.get("command"),
            },
        }

    def send_live_non_combat_action(
        self,
        bridge_status: dict[str, Any],
        suggestion: dict[str, Any],
        payload: dict[str, Any],
        *,
        send_command: Any,
    ) -> dict[str, Any]:
        live_session = self.create_live_session(bridge_status)
        if live_session.get("attach_fidelity") != "seed_replay":
            raise ValueError("live non-combat send requires strict seed replay attachment")
        if live_session.get("state_kind") != "run":
            raise ValueError("live non-combat send requires a run state")

        descriptor = suggestion.get("descriptor")
        if not isinstance(descriptor, dict):
            raise ValueError("guided non-combat suggestion has no descriptor")
        command = command_for_descriptor(descriptor)

        observed = _observed_state_from_bridge_status(bridge_status)
        session = self._require_session(live_session["session_id"])
        exact_action = _action_for_communication_command(session.env, command, observed)
        if exact_action is None:
            raise ValueError("guided non-combat command does not map to a current exact legal action")
        exact_json = exact_action.json()
        legal_action = next(
            (
                action
                for action in self._actions(session.env, live_session["state_id"])
                if action["exact_action_json"] == exact_json
            ),
            None,
        )
        if legal_action is None:
            raise ValueError("guided non-combat action is not legal in the current simulator state")

        prediction = self.predict(
            live_session["session_id"],
            {
                "action_id": legal_action["action_id"],
                "source_state_id": live_session["state_id"],
            },
        )
        source_state_id = bridge_status.get("state_id")
        send_kwargs = {"source_state_id": source_state_id}
        if payload.get("provenance") is not None:
            send_kwargs["metadata"] = payload["provenance"]
        result = send_command(command, **send_kwargs)
        return {
            "session_id": live_session["session_id"],
            "source_state_id": live_session["state_id"],
            "bridge_state_id": source_state_id,
            "bridge_step": bridge_status.get("last_state_step"),
            "predicted_state_id": prediction["predicted_state_id"],
            "command": command,
            "matched_action": _public_action(legal_action),
            "send_result": {
                "ok": result.get("ok"),
                "command_id": result.get("command_id"),
                "command": result.get("command"),
            },
        }

    def verify_live_prediction(
        self,
        prediction: dict[str, Any],
        *,
        bridge_status: dict[str, Any],
    ) -> dict[str, Any]:
        expected = prediction.get("predicted_state_id")
        if not expected:
            return {"status": "blocked", "reason": "missing_expected_state", "detail": "pending prediction has no expected state"}
        live_session = self.create_live_session(bridge_status)
        if live_session.get("attach_fidelity") != "seed_replay":
            return {
                "status": "blocked",
                "reason": "strict_replay_required",
                "detail": "pending prediction check requires strict seed replay attachment",
                "attach": live_session,
            }
        observed = live_session.get("state_id")
        if observed != expected:
            return {
                "status": "mismatch",
                "reason": "prediction_mismatch",
                "detail": f"expected {expected}, observed {observed}",
                "expected_state_id": expected,
                "observed_state_id": observed,
                "attach": live_session,
            }
        return {
            "status": "matched",
            "expected_state_id": expected,
            "observed_state_id": observed,
            "attach": live_session,
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
        if command_lifecycle is not None:
            session.last_lifecycle = command_lifecycle
        lifecycle = session.last_lifecycle or _command_lifecycle("ready")
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
            "command_lifecycle": lifecycle,
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

    def _action_from_payload(self, session: CombatSession, payload: dict[str, Any], state_id: str) -> dict[str, Any]:
        actions = self._actions(session.env, state_id)
        action_id = payload.get("action_id")
        if action_id:
            action = next((entry for entry in actions if entry["action_id"] == action_id), None)
            if action is not None:
                return action
            raise ValueError(f"unknown action_id for current state: {action_id}")

        descriptor = payload.get("descriptor")
        if isinstance(descriptor, dict):
            action = next((entry for entry in actions if entry["descriptor"] == descriptor), None)
            if action is not None:
                return action
            raise ValueError("descriptor does not match any current exact legal action")

        exact_action_json = payload.get("exact_action_json")
        if isinstance(exact_action_json, str):
            action = next((entry for entry in actions if entry["exact_action_json"] == exact_action_json), None)
            if action is not None:
                return action
            raise ValueError("exact_action_json does not match any current exact legal action")

        raise ValueError("prediction requires action_id, descriptor, or exact_action_json")


class UiRequestHandler(SimpleHTTPRequestHandler):
    manager = SessionManager()
    bridge = BridgeMirror.default()
    traces = TraceReplayStore.default()
    collector = GuidedCollector()

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
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "state":
                self._send_json(self.manager.state(parts[2]))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "actions":
                self._send_json(self.manager.actions(parts[2]))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "pending-command":
                self._send_json(self.manager.pending_command(parts[2]))
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
            if parts == ["api", "bridge", "preflight"]:
                self._send_json(self.bridge.preflight())
                return
            if parts == ["api", "bridge", "clients"]:
                self._send_json(self.bridge.clients())
                return
            if parts == ["api", "traces"]:
                self._send_json(self.traces.list_traces(_query_int(query, "limit", 50)))
                return
            if parts == ["api", "slaythedata", "candidates"]:
                self._send_json(_slaythedata_candidates_from_query(query))
                return
            if parts == ["api", "slaythedata", "status"]:
                self._send_json(_slaythedata_status_from_query(query))
                return
            if parts == ["api", "collector", "status"]:
                self._send_json(_collector_status_with_preflight(self.collector, self.bridge))
                return
            if parts == ["api", "collector", "report"]:
                self._send_json(_guided_collect_report())
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
            if parts == ["api", "live", "session"]:
                self._send_json(self.manager.create_live_session(self.bridge.status()))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "step":
                self._send_json(self.manager.step(parts[2], payload))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "predict":
                self._send_json(self.manager.predict(parts[2], payload))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "restore":
                self._send_json(self.manager.restore(parts[2], payload))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "search":
                self._send_json(self.manager.search(parts[2], payload))
                return
            if parts[:2] == ["api", "sessions"] and len(parts) == 4 and parts[3] == "parity":
                self._send_json(self.manager.parity(parts[2], self.bridge.status()))
                return
            if parts == ["api", "bridge", "command"]:
                self._send_json(
                    self.bridge.send_command(
                        str(payload.get("command", "")),
                        source_state_id=_optional_string(payload.get("source_state_id")),
                    )
                )
                return
            if parts == ["api", "bridge", "descriptor"]:
                command = command_for_descriptor(payload.get("descriptor", {}))
                self._send_json(
                    self.bridge.send_command(
                        command,
                        source_state_id=_optional_string(payload.get("source_state_id")),
                    )
                )
                return
            if parts == ["api", "bridge", "clients", "kill"]:
                self._send_json(self.bridge.kill_client(payload.get("pid")))
                return
            if parts == ["api", "bridge", "orphan-command-metadata", "clear"]:
                self._send_json(self.bridge.clear_orphan_command_metadata())
                return
            if parts == ["api", "slaythedata", "script"]:
                self._send_json(_guided_script_from_payload(payload))
                return
            if parts == ["api", "slaythedata", "export"]:
                self._send_json(_slaythedata_export_from_payload(payload))
                return
            if parts == ["api", "collector", "start"]:
                self._send_json(self.collector.start(_collector_start_payload(payload)))
                return
            if parts == ["api", "collector", "start-live-run"]:
                self._send_json(_start_guided_live_run(self.collector, self.bridge))
                return
            if parts == ["api", "collector", "tick"]:
                self._send_json(_tick_live_collector(self.collector, self.manager, self.bridge, payload))
                return
            if parts == ["api", "collector", "stop"]:
                self._send_json(self.collector.stop())
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


def _command_lifecycle(status: str, **fields: Any) -> dict[str, Any]:
    payload = {"status": status}
    if status != "ready":
        payload["command_id"] = uuid4().hex
    payload.update(fields)
    return payload


def _combat_search_config(payload: dict[str, Any]) -> CombatSearchConfig:
    candidate_name = str(payload.get("candidate") or SELECTED_COMBAT_AUTOPILOT_CANDIDATE)
    config = trace_autopilot_candidate_by_name(candidate_name).config
    if "max_depth" in payload:
        config = replace(config, max_depth=int(payload["max_depth"]))
    if "objective" in payload:
        config = replace(config, objective=str(payload["objective"]))
    if "algorithm" in payload:
        config = replace(config, algorithm=str(payload["algorithm"]))
    if "beam_width" in payload:
        config = replace(config, beam_width=int(payload["beam_width"]))
    if "allowed_potions" in payload:
        config = replace(config, allowed_potions=_parse_allowed_potions(payload["allowed_potions"]))
    return config


def _parse_allowed_potions(value: Any) -> tuple[str, ...] | None:
    if value is None:
        return None
    if isinstance(value, str):
        stripped = value.strip()
        if stripped.lower() in {"*", "all"}:
            return None
        if not stripped or stripped.lower() in {"none", "no", "false"}:
            return ()
        return tuple(part.strip() for part in stripped.split(",") if part.strip())
    if isinstance(value, list):
        return tuple(str(part).strip() for part in value if str(part).strip())
    raise ValueError("allowed_potions must be a string, list, or null")


def _strict_replay_from_bridge_trace(bridge_status: dict[str, Any]) -> Any | None:
    trace_path = bridge_status.get("trace_path")
    if not trace_path:
        return None
    try:
        path = Path(str(trace_path))
    except TypeError:
        return None
    if not path.is_file():
        return None
    try:
        return strict_replay_real_trace_to_env(trace=path)
    except Exception as error:
        return type(
            "StrictReplayFailure",
            (),
            {
                "verified": False,
                "env": None,
                "trace_path": path,
                "steps": 0,
                "stop_reason": "strict_replay_error",
                "blocker": {"category": "strict_replay_error", "reason": str(error)},
                "final_state_id": None,
                "final_phase": None,
            },
        )()


def _search_predicted_final_hp(diagnostics: dict[str, Any]) -> float | None:
    value = diagnostics.get("rust_final_hp")
    if value is None:
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _search_predicted_hp_loss(env: Any, predicted_final_hp: float | None) -> float | None:
    if predicted_final_hp is None:
        return None
    try:
        state = json.loads(env.state_json())
    except (TypeError, ValueError):
        return None
    current_hp = _state_player_hp(state)
    if current_hp is None:
        return None
    return max(0.0, float(current_hp) - predicted_final_hp)


def _state_player_hp(state: dict[str, Any]) -> float | None:
    candidates = [
        state.get("player_hp"),
        (state.get("player") or {}).get("hp") if isinstance(state.get("player"), dict) else None,
        ((state.get("combat") or {}).get("player") or {}).get("hp")
        if isinstance(state.get("combat"), dict)
        and isinstance((state.get("combat") or {}).get("player"), dict)
        else None,
    ]
    for candidate in candidates:
        if candidate is None:
            continue
        try:
            return float(candidate)
        except (TypeError, ValueError):
            continue
    return None


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


def _bridge_action_for_exact_action(
    action: dict[str, Any],
    bridge_status: dict[str, Any],
    sim_state: dict[str, Any],
) -> dict[str, Any] | None:
    bridge_actions = bridge_status.get("bridge_actions")
    if not isinstance(bridge_actions, list):
        return None

    if _is_end_turn_descriptor(action):
        return _find_bridge_action(bridge_actions, kind="EndTurn")

    play = _play_card_payload(action)
    if play is not None:
        hand_slot = _observed_hand_slot_for_card_id(play.get("card_id"), bridge_status, sim_state)
        if hand_slot is None:
            return None
        target = play.get("target")
        target_slot = _observed_monster_slot_for_target(target, bridge_status, sim_state)
        return _find_bridge_action(
            bridge_actions,
            kind="PlayHandSlot",
            hand_slot=hand_slot,
            target_slot=target_slot,
            target_required=target is not None,
        )

    potion = _use_potion_payload(action)
    if potion is not None:
        target = potion.get("target")
        target_slot = _observed_monster_slot_for_target(target, bridge_status, sim_state)
        return _find_bridge_action(
            bridge_actions,
            kind="UsePotionSlot",
            potion_slot=potion.get("slot"),
            target_slot=target_slot,
            target_required=target is not None,
        )

    return None


def _find_bridge_action(
    bridge_actions: list[Any],
    *,
    kind: str,
    hand_slot: Any = None,
    potion_slot: Any = None,
    target_slot: Any = None,
    target_required: bool = False,
) -> dict[str, Any] | None:
    for entry in bridge_actions:
        if not isinstance(entry, dict):
            continue
        descriptor = entry.get("descriptor")
        if not isinstance(descriptor, dict) or descriptor.get("kind") != kind:
            continue
        if hand_slot is not None and _parse_int_or_none(descriptor.get("hand_slot")) != _parse_int_or_none(hand_slot):
            continue
        if potion_slot is not None and _parse_int_or_none(descriptor.get("potion_slot")) != _parse_int_or_none(potion_slot):
            continue
        descriptor_target = descriptor.get("target_slot")
        if target_required:
            if _parse_int_or_none(descriptor_target) != _parse_int_or_none(target_slot):
                continue
        elif descriptor_target is not None:
            continue
        return entry
    return None


def _play_card_payload(action: dict[str, Any]) -> dict[str, Any] | None:
    if action.get("kind") == "PlayCard":
        return action
    nested = action.get("action")
    if isinstance(nested, dict) and isinstance(nested.get("PlayCard"), dict):
        return nested["PlayCard"]
    descriptor = action.get("descriptor")
    if isinstance(descriptor, dict):
        nested = descriptor.get("action")
        if isinstance(nested, dict) and isinstance(nested.get("PlayCard"), dict):
            return nested["PlayCard"]
    return None


def _use_potion_payload(action: dict[str, Any]) -> dict[str, Any] | None:
    nested = action.get("action")
    if isinstance(nested, dict) and isinstance(nested.get("UsePotion"), dict):
        return nested["UsePotion"]
    descriptor = action.get("descriptor")
    if isinstance(descriptor, dict):
        nested = descriptor.get("action")
        if isinstance(nested, dict) and isinstance(nested.get("UsePotion"), dict):
            return nested["UsePotion"]
    return None


def _is_end_turn_descriptor(action: dict[str, Any]) -> bool:
    if action.get("kind") == "EndTurn" or action.get("action_kind") == "end_turn":
        return True
    descriptor = action.get("descriptor")
    if isinstance(descriptor, dict) and (
        descriptor.get("kind") == "EndTurn" or descriptor.get("action_kind") == "end_turn"
    ):
        return True
    return action.get("action") == "EndTurn"


def _observed_hand_slot_for_card_id(
    card_id: Any,
    bridge_status: dict[str, Any],
    sim_state: dict[str, Any],
) -> int | None:
    hand = _bridge_combat_list(bridge_status, "hand")
    direct = _find_by_any_id(hand, card_id, ("id", "card_id", "cardId"))
    if direct is not None:
        return _slot_from_entry(direct, ("index", "slot", "hand_slot", "handSlot"))

    sim_hand = (((_run_state(sim_state).get("combat") or {}).get("piles") or {}).get("hand") or [])
    sim_index = _index_by_any_id(sim_hand, card_id, ("id", "card_id", "cardId"))
    if sim_index is None or sim_index >= len(hand):
        return None
    return _slot_from_entry(hand[sim_index], ("index", "slot", "hand_slot", "handSlot"))


def _observed_monster_slot_for_target(
    target: Any,
    bridge_status: dict[str, Any],
    sim_state: dict[str, Any],
) -> int | None:
    if target is None:
        return None
    monsters = _bridge_combat_list(bridge_status, "monsters")
    direct = _find_by_any_id(monsters, target, ("id", "monster_id", "monsterId", "index"))
    if direct is not None:
        return _slot_from_entry(direct, ("index", "slot", "target_slot", "targetSlot"))

    sim_monsters = ((_run_state(sim_state).get("combat") or {}).get("monsters") or [])
    sim_index = _index_by_any_id(sim_monsters, target, ("id", "monster_id", "monsterId"))
    if sim_index is None or sim_index >= len(monsters):
        return None
    return _slot_from_entry(monsters[sim_index], ("index", "slot", "target_slot", "targetSlot"))


def _bridge_combat_list(bridge_status: dict[str, Any], name: str) -> list[Any]:
    summary = bridge_status.get("summary")
    combat = summary.get("combat") if isinstance(summary, dict) else None
    values = combat.get(name) if isinstance(combat, dict) else None
    return values if isinstance(values, list) else []


def _find_by_any_id(entries: list[Any], wanted: Any, keys: tuple[str, ...]) -> dict[str, Any] | None:
    for entry in entries:
        if isinstance(entry, dict) and any(str(entry.get(key, "")) == str(wanted) for key in keys):
            return entry
    return None


def _index_by_any_id(entries: list[Any], wanted: Any, keys: tuple[str, ...]) -> int | None:
    for index, entry in enumerate(entries):
        if isinstance(entry, dict) and any(str(entry.get(key, "")) == str(wanted) for key in keys):
            return index
    return None


def _slot_from_entry(entry: dict[str, Any], keys: tuple[str, ...]) -> int | None:
    for key in keys:
        value = _parse_int_or_none(entry.get(key))
        if value is not None:
            return value
    return None


def _run_state(state: dict[str, Any]) -> dict[str, Any]:
    nested = state.get("state")
    return nested if isinstance(nested, dict) else state


def _parse_int_or_none(value: Any) -> int | None:
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _decision_substate(session: CombatSession, terminal_reason: str | None) -> str:
    if terminal_reason:
        return "Terminal"
    if session.state_kind == "run":
        return _call_optional(session.env, "current_decision") or "RunDecision"
    return "NormalCombat"


def _can_search_combat(session: CombatSession) -> bool:
    return session.state_kind == "combat" or session.env.phase() == "combat"


def _observed_state_from_bridge_status(bridge_status: dict[str, Any]) -> dict[str, Any]:
    current_state = bridge_status.get("current_state")
    if not isinstance(current_state, dict) or current_state.get("missing"):
        raise ValueError("no observed bridge state is available yet")

    message = current_state.get("message")
    if isinstance(message, dict):
        if isinstance(message.get("game_state"), dict):
            return message["game_state"]
        if _looks_like_communication_mod_state(message):
            return message

    if isinstance(current_state.get("game_state"), dict):
        return current_state["game_state"]
    if _looks_like_communication_mod_state(current_state):
        return current_state

    raise ValueError("latest bridge state is not a supported CommunicationMod game state")


def _guided_script_from_payload(payload: dict[str, Any]) -> dict[str, Any]:
    run_id = payload.get("run_id")
    if run_id is not None:
        return {"script": export_guided_run_script(int(run_id))}

    exported_run = payload.get("exported_run")
    if isinstance(exported_run, dict):
        return {"script": build_guided_run_script(exported_run)}

    path = payload.get("path")
    if isinstance(path, str) and path.strip():
        return {
            "script": load_guided_run_script(
                path,
                line_index=int(payload.get("line_index", 0)),
            )
        }

    raise ValueError("expected exported_run or path")


def _collector_start_payload(payload: dict[str, Any]) -> dict[str, Any]:
    if payload.get("run_id") is not None:
        return {"script": export_guided_run_script(int(payload["run_id"]))}
    return payload


def _tick_live_collector(
    collector: GuidedCollector,
    manager: SessionManager,
    bridge: BridgeMirror,
    payload: dict[str, Any],
    *,
    require_tcp_control: bool = True,
) -> dict[str, Any]:
    def send_guided_command(command: str, **kwargs: Any) -> dict[str, Any]:
        return bridge.send_command(command, require_tcp_control=require_tcp_control, **kwargs)

    return collector.tick(
        bridge.status(),
        payload,
        send_command=send_guided_command,
        send_non_combat=lambda **kwargs: manager.send_live_non_combat_action(
            **kwargs,
            send_command=send_guided_command,
        ),
        send_combat=lambda **kwargs: manager.send_live_combat_action(
            **kwargs,
            send_command=send_guided_command,
        ),
        verify_prediction=manager.verify_live_prediction,
    )


def _start_guided_live_run(
    collector: GuidedCollector,
    bridge: BridgeMirror,
    *,
    require_tcp_control: bool = True,
) -> dict[str, Any]:
    collector_status = collector.status()
    if not collector_status.get("active"):
        raise ValueError("start guided live run requires an active collector")
    if collector_status.get("status") == "blocked":
        blocker = collector_status.get("blocker") if isinstance(collector_status.get("blocker"), dict) else {}
        detail = blocker.get("detail") or blocker.get("reason") or "collector is blocked"
        raise ValueError(f"start guided live run blocked: {detail}")

    config = collector_status.get("config") if isinstance(collector_status.get("config"), dict) else {}
    character = _required_command_token(config.get("character") or config.get("character_chosen"), "character").upper()
    ascension = _required_ascension(
        config.get("ascension") if config.get("ascension") is not None else config.get("ascension_level")
    )
    seed = _required_command_token(config.get("seed_played") or config.get("seed"), "seed")
    bridge_status = bridge.status()
    command = f"START {character} {ascension} {seed}"
    metadata = {
        "source": "guided_collector_start",
        "collector_id": collector_status.get("collector_id"),
        "script_source": collector_status.get("source"),
        "replay_policy": collector_status.get("replay_policy"),
    }
    send_result = bridge.send_command(
        command,
        source_state_id=bridge_status.get("state_id"),
        metadata=metadata,
        require_tcp_control=require_tcp_control,
    )
    return {
        "ok": True,
        "command": command,
        "collector": collector.status(),
        "send_result": {
            "ok": send_result.get("ok"),
            "command_id": send_result.get("command_id"),
            "command": send_result.get("command"),
        },
    }


def _collector_status_with_preflight(collector: GuidedCollector, bridge: BridgeMirror) -> dict[str, Any]:
    return collector.status() | {"preflight": bridge.preflight()}


def _guided_collect_report(report_path: Path = DEFAULT_GUIDED_REPORT_PATH) -> dict[str, Any]:
    if not report_path.exists():
        return {
            "ok": False,
            "report_path": str(report_path),
            "missing": True,
            "error": "guided collection report not found",
        }
    report = json.loads(report_path.read_text(encoding="utf-8"))
    if not isinstance(report, dict):
        raise ValueError("guided collection report is not a JSON object")
    blocker = report.get("blocker") if isinstance(report.get("blocker"), dict) else None
    selection = report.get("selection") if isinstance(report.get("selection"), dict) else None
    return {
        "ok": bool(report.get("ok")),
        "report_path": str(report_path),
        "missing": False,
        "run_id": report.get("run_id"),
        "seed": report.get("seed"),
        "stop_reason": report.get("stop_reason"),
        "actions_sent": report.get("actions_sent", 0),
        "trace_path": report.get("trace_path"),
        "bridge_step": report.get("bridge_step"),
        "tcp_control_available": bool(report.get("tcp_control_available")),
        "selection": {
            "mode": selection.get("mode"),
            "selected_run_id": selection.get("selected_run_id"),
            "considered_count": selection.get("considered_count"),
            "candidate_count": selection.get("candidate_count"),
            "skipped_unsupported_count": len(selection.get("skipped_unsupported") or [])
            if isinstance(selection.get("skipped_unsupported"), list)
            else 0,
        }
        if selection
        else None,
        "blocker": {
            "reason": blocker.get("reason"),
            "problems": blocker.get("problems") or [],
            "warnings": blocker.get("warnings") or [],
            "detail": blocker.get("detail"),
        }
        if blocker
        else None,
        "history_tail_count": len(report.get("history_tail") or [])
        if isinstance(report.get("history_tail"), list)
        else 0,
    }


def _slaythedata_candidates_from_query(query: dict[str, list[str]]) -> dict[str, Any]:
    character = _query_string(query, "character", "IRONCLAD").upper()
    ascension = _query_int(query, "ascension", 0)
    min_floor = _query_int(query, "min_floor", 45)
    max_floor = _query_optional_int(query, "max_floor")
    min_path_length = _query_optional_int(query, "min_path_length")
    min_card_choices = _query_optional_int(query, "min_card_choices")
    min_event_choices = _query_optional_int(query, "min_event_choices")
    min_shop_purchases = _query_optional_int(query, "min_shop_purchases")
    min_potion_usage = _query_optional_int(query, "min_potion_usage")
    safe_neow = _query_bool(query, "safe_neow", True)
    limit = _query_int(query, "limit", 25)
    ranked = _query_bool(query, "ranked", True)
    rows = select_guided_collection_candidates(
        character=character,
        ascension=ascension,
        min_floor_reached=min_floor,
        max_floor_reached=max_floor,
        min_path_length=min_path_length,
        min_card_choices=min_card_choices,
        min_event_choices=min_event_choices,
        min_shop_purchases=min_shop_purchases,
        min_potion_usage=min_potion_usage,
        require_guided_safe_neow=safe_neow,
        limit=limit,
        ranked=ranked,
    )
    return {
        "candidates": rows,
        "filters": {
            "character": character,
            "ascension": ascension,
            "min_floor": min_floor,
            "max_floor": max_floor,
            "min_path_length": min_path_length,
            "min_card_choices": min_card_choices,
            "min_event_choices": min_event_choices,
            "min_shop_purchases": min_shop_purchases,
            "min_potion_usage": min_potion_usage,
            "safe_neow": safe_neow,
            "limit": limit,
            "ranked": ranked,
        },
    }


def _slaythedata_status_from_query(query: dict[str, list[str]]) -> dict[str, Any]:
    return slaythedata_index_status(
        character=_query_string(query, "character", "IRONCLAD").upper(),
        ascension=_query_int(query, "ascension", 0),
        min_floor_reached=_query_int(query, "min_floor", 45),
        min_path_length=_query_optional_int(query, "min_path_length") or 45,
        include_counts=_query_bool(query, "include_counts", False),
    )


def _slaythedata_export_from_payload(payload: dict[str, Any]) -> dict[str, Any]:
    run_id = payload.get("run_id")
    if run_id is None:
        raise ValueError("run_id is required")
    return {"script": export_guided_run_script(int(run_id))}


def _looks_like_communication_mod_state(value: dict[str, Any]) -> bool:
    return any(
        key in value
        for key in (
            "combat_state",
            "screen_type",
            "choice_list",
            "current_hp",
            "player_hp",
            "floor",
            "deck",
            "relics",
        )
    )


def _env_from_snapshot(state_kind: str, snapshot_json: str) -> Any:
    if state_kind == "combat":
        return omni.OmniCombatEnv.from_snapshot_json(snapshot_json)
    if state_kind == "run":
        return omni.OmniRunEnv.from_snapshot_json(snapshot_json)
    raise ValueError(f"unsupported session state kind: {state_kind}")


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


def _query_optional_int(query: dict[str, list[str]], name: str) -> int | None:
    values = query.get(name)
    if not values or values[0] == "":
        return None
    try:
        return int(values[0])
    except ValueError:
        return None


def _query_string(query: dict[str, list[str]], name: str, default: str) -> str:
    values = query.get(name)
    if not values:
        return default
    return values[0] or default


def _query_bool(query: dict[str, list[str]], name: str, default: bool) -> bool:
    values = query.get(name)
    if not values:
        return default
    value = str(values[0]).strip().lower()
    if value in {"1", "true", "yes", "on"}:
        return True
    if value in {"0", "false", "no", "off"}:
        return False
    return default


def _optional_string(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def _required_command_token(value: Any, label: str) -> str:
    text = _optional_string(value)
    if not text:
        raise ValueError(f"guided run {label} is required")
    if any(ch.isspace() for ch in text):
        raise ValueError(f"guided run {label} must not contain whitespace")
    return text


def _required_ascension(value: Any) -> int:
    try:
        ascension = int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError("guided run ascension is required") from exc
    if ascension < 0 or ascension > 20:
        raise ValueError("guided run ascension must be between 0 and 20")
    return ascension


if __name__ == "__main__":
    run()
