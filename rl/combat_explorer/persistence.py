"""Versioned session documents and deterministic tree reconstruction."""

from __future__ import annotations

import json
import os
import re
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from combat_task import action_indices
from sts_sim import FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION, FAIR_RUN_OBSERVATION_SCHEMA_VERSION, State
from validation_set import native_sha256

from combat_explorer import (
    FORMAT_NAME,
    MAX_ID_LENGTH,
    MAX_JSON_DEPTH,
    MAX_NODE_CHILDREN,
    MAX_SESSION_FILE_BYTES,
    MAX_SESSION_NODES,
    SESSION_SCHEMA_VERSION,
    TASK_PROTOCOL,
)
from combat_explorer.errors import ExplorerError, NotFoundError
from combat_explorer.jsonutil import canonical_dumps, observation_sha256, require_mapping, seed_to_str
from combat_explorer.presentation import (
    action_descriptor,
    action_label,
    descriptors_match,
    fallback_markers,
    fallback_summary,
    summarize_transition,
    transition_markers,
)
from combat_explorer.sessions import Node, Session, SessionManager, _new_id, job_from_document, job_payload

FILENAME_PATTERN = re.compile(r"^[A-Za-z0-9._-]{1,128}\.json$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")


def session_document(
    session: Session,
    *,
    selected_node_id: str | None = None,
    preferred_children: dict[str, str] | None = None,
    jobs: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    with session.lock:
        nodes = []
        for node in session.nodes.values():
            nodes.append(
                {
                    "id": node.id,
                    "parent_id": node.parent_id,
                    "child_ids": list(node.child_ids),
                    "depth": node.depth,
                    "turn": node.turn,
                    "revision": node.revision,
                    "schema_version": node.schema_version,
                    "outcome": node.outcome,
                    "observation": node.observation_json,
                    "observation_sha256": node.observation_sha256,
                    "action_manifest": node.action_manifest,
                    "allowed_native_indices": list(node.allowed_native_indices),
                    "incoming": node.incoming,
                    "summary": node.summary,
                    "markers": node.markers,
                }
            )
        if selected_node_id is not None:
            selected = _require_id(selected_node_id, "selected_node_id")
        else:
            selected = session.root_id
        if selected not in session.nodes:
            raise ExplorerError("selected_node_id is not in the session tree", code="invalid_session")
        preferred = preferred_children or {}
        if not isinstance(preferred, dict):
            raise ExplorerError("preferred_children must be an object", code="invalid_session")
        _validate_preferred(preferred, session.nodes)
        saved_jobs = jobs if jobs is not None else []
        return {
            "format": FORMAT_NAME,
            "schema_version": SESSION_SCHEMA_VERSION,
            "session_id": session.id,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "spec_json": session.spec_json,
            "seed": seed_to_str(session.spec["seed"]),
            "provenance": session.provenance,
            "native_sha256": native_sha256(),
            "task_protocol": TASK_PROTOCOL,
            "observation_schemas": {
                "run": FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
                "combat": FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION,
            },
            "root_id": session.root_id,
            "nodes": nodes,
            "jobs": saved_jobs,
            "ui": {
                "selected_node_id": selected,
                "preferred_children": preferred,
            },
            "model": None
            if session.model is None
            else {
                "path": session.model.path,
                "fingerprint": session.model.fingerprint,
                "architecture": session.model.architecture,
                "adapter_version": session.model.adapter_version,
            },
        }


def jobs_for_save(session: Session, manager: SessionManager) -> list[dict[str, Any]]:
    with session.lock:
        payloads = []
        for job_id in session.job_ids:
            job = manager.jobs.get(job_id)
            if job is None:
                continue
            payload = job_payload(job)
            if payload["status"] == "running":
                payload = {**payload, "status": "interrupted", "reason": "interrupted"}
            payloads.append(payload)
        return payloads


def validate_document(document: object) -> dict[str, Any]:
    data = require_mapping(document, "session")
    _check_json_depth(data, "session")
    if data.get("format") != FORMAT_NAME:
        raise ExplorerError("Unsupported session format", code="invalid_session")
    if data.get("schema_version") != SESSION_SCHEMA_VERSION:
        raise ExplorerError("Unsupported session schema version", code="invalid_session")
    if data.get("task_protocol") != TASK_PROTOCOL:
        raise ExplorerError("Incompatible combat-task protocol", code="invalid_session")
    if data.get("native_sha256") != native_sha256():
        raise ExplorerError("Incompatible simulator/native fingerprint; session is not runnable", code="invalid_session")
    schemas = require_mapping(data.get("observation_schemas"), "observation_schemas")
    if schemas.get("run") != FAIR_RUN_OBSERVATION_SCHEMA_VERSION or schemas.get("combat") != FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION:
        raise ExplorerError("Incompatible observation schema versions", code="invalid_session")
    if not isinstance(data.get("spec_json"), str) or not data["spec_json"]:
        raise ExplorerError("Session is missing spec_json", code="invalid_session")
    if data.get("session_id") is not None:
        _require_id(data.get("session_id"), "session_id")
    provenance = data.get("provenance")
    if provenance is None:
        data["provenance"] = {}
    elif not isinstance(provenance, dict):
        raise ExplorerError("provenance must be an object", code="invalid_session")
    try:
        seed_to_str(data.get("seed"))
    except ValueError as error:
        raise ExplorerError("Invalid seed in session document", code="invalid_session") from error
    _require_id(data.get("root_id"), "root_id")
    nodes = data.get("nodes")
    if not isinstance(nodes, list) or not nodes:
        raise ExplorerError("Session has no nodes", code="invalid_session")
    if len(nodes) > MAX_SESSION_NODES:
        raise ExplorerError(f"Session exceeds {MAX_SESSION_NODES} nodes", code="invalid_session")
    by_id: dict[str, dict[str, Any]] = {}
    for index, raw in enumerate(nodes):
        node = _validate_node_record(raw, index)
        node_id = node["id"]
        if node_id in by_id:
            raise ExplorerError("Node ids must be unique strings", code="invalid_session")
        by_id[node_id] = node
    if data["root_id"] not in by_id:
        raise ExplorerError("root_id is not present in nodes", code="invalid_session")
    roots = [node for node in by_id.values() if node.get("parent_id") is None]
    if len(roots) != 1 or roots[0]["id"] != data["root_id"]:
        raise ExplorerError("Session must contain exactly one root node", code="invalid_session")
    for node in by_id.values():
        parent_id = node.get("parent_id")
        child_ids = node["child_ids"]
        if len(child_ids) != len(set(child_ids)):
            raise ExplorerError(f"Duplicate child ids on node {node['id']}", code="invalid_session")
        for child_id in child_ids:
            child = by_id.get(child_id)
            if child is None:
                raise ExplorerError(f"Unknown child id {child_id}", code="invalid_session")
            if child.get("parent_id") != node["id"]:
                raise ExplorerError("Child parent_id does not match parent", code="invalid_session")
        if parent_id is None:
            if node.get("incoming") is not None:
                raise ExplorerError("Root node must not have an incoming edge", code="invalid_session")
            if node["depth"] != 0:
                raise ExplorerError("Root node depth must be 0", code="invalid_session")
            continue
        parent_id = _require_id(parent_id, f"{node['id']}.parent_id")
        if parent_id not in by_id:
            raise ExplorerError(f"Missing parent {parent_id}", code="invalid_session")
        parent = by_id[parent_id]
        if node["id"] not in list(parent.get("child_ids") or []):
            raise ExplorerError("Parent/child references are inconsistent", code="invalid_session")
        if node["depth"] != parent["depth"] + 1:
            raise ExplorerError(f"Node {node['id']} depth does not match parent depth + 1", code="invalid_session")
        incoming = require_mapping(node.get("incoming"), f"{node['id']}.incoming")
        _validate_incoming(incoming, node["id"])
    if _has_cycle(by_id, data["root_id"]):
        raise ExplorerError("Session tree contains a cycle", code="invalid_session")
    reachable: set[str] = set()
    stack = [data["root_id"]]
    while stack:
        current = stack.pop()
        if current in reachable:
            continue
        reachable.add(current)
        stack.extend(str(child) for child in by_id[current].get("child_ids") or [])
    if reachable != set(by_id):
        raise ExplorerError("Session contains unreachable nodes", code="invalid_session")
    ui = data.get("ui")
    if ui is None:
        ui = {}
    if not isinstance(ui, dict):
        raise ExplorerError("ui must be an object", code="invalid_session")
    if "selected_node_id" in ui:
        selected = _require_id(ui.get("selected_node_id"), "ui.selected_node_id")
    else:
        selected = data["root_id"]
    if selected not in by_id:
        raise ExplorerError("selected_node_id is not in the session tree", code="invalid_session")
    preferred = ui.get("preferred_children")
    if preferred is None:
        preferred = {}
    if not isinstance(preferred, dict):
        raise ExplorerError("preferred_children must be an object", code="invalid_session")
    for parent, child in preferred.items():
        parent_id = _require_id(parent, "preferred_children.key")
        child_id = _require_id(child, "preferred_children.value")
        if parent_id not in by_id or child_id not in by_id:
            raise ExplorerError("preferred_children reference unknown nodes", code="invalid_session")
        if child_id not in (by_id[parent_id].get("child_ids") or []):
            raise ExplorerError("preferred child is not a child of its parent", code="invalid_session")
    jobs = data.get("jobs")
    if jobs is None:
        jobs = []
    if not isinstance(jobs, list):
        raise ExplorerError("jobs must be a list", code="invalid_session")
    seen_jobs: set[str] = set()
    for index, raw_job in enumerate(jobs):
        job = require_mapping(raw_job, f"jobs[{index}]")
        job_id = _require_id(job.get("id"), f"jobs[{index}].id")
        if job_id in seen_jobs:
            raise ExplorerError("Job ids must be unique", code="invalid_session")
        seen_jobs.add(job_id)
        source = _require_id(job.get("source_node_id"), f"jobs[{index}].source_node_id")
        if source not in by_id:
            raise ExplorerError("Job source_node_id is not in the tree", code="invalid_session")
        leaf = job.get("leaf_id")
        if leaf is not None:
            leaf_id = _require_id(leaf, f"jobs[{index}].leaf_id")
            if leaf_id not in by_id:
                raise ExplorerError("Job leaf_id is not in the tree", code="invalid_session")
        if job.get("session_id") is not None:
            _require_id(job.get("session_id"), f"jobs[{index}].session_id")
        status = job.get("status")
        if status not in ("running", "completed", "cancelled", "failed", "interrupted"):
            raise ExplorerError("Job status is invalid", code="invalid_session")
        if "generated" in job:
            _require_nonneg_int(job.get("generated"), f"jobs[{index}].generated")
        if "max_decisions" in job:
            _require_nonneg_int(job.get("max_decisions"), f"jobs[{index}].max_decisions")
        if job.get("attempted_action") is not None and not isinstance(job.get("attempted_action"), dict):
            raise ExplorerError("Job attempted_action must be an object", code="invalid_session")
    return data


def reconstruct(manager: SessionManager, document: dict[str, Any], *, session_id: str | None = None) -> Session:
    data = validate_document(document)
    spec_json = data["spec_json"]
    try:
        spec = json.loads(spec_json)
    except json.JSONDecodeError as error:
        raise ExplorerError("spec_json is not valid JSON", code="invalid_session") from error
    if not isinstance(spec, dict):
        raise ExplorerError("spec_json must encode an object", code="invalid_session")
    try:
        if seed_to_str(spec.get("seed")) != seed_to_str(data.get("seed", spec.get("seed"))):
            raise ExplorerError("Document seed does not match spec_json", code="invalid_session")
    except ValueError as error:
        raise ExplorerError("Invalid seed in session document", code="invalid_session") from error
    state = State.from_synthetic_spec(spec_json)
    decision = state.decision()
    by_id = {node["id"]: node for node in data["nodes"]}
    root_raw = by_id[data["root_id"]]
    _check_node_observation(root_raw, decision, state)
    session = Session(
        id=session_id or _new_id("session"),
        spec_json=spec_json,
        spec=spec,
        provenance=dict(data.get("provenance") or {}),
        root_id=data["root_id"],
        nodes={},
    )
    root = manager._node_from_state(
        state,
        decision,
        node_id=root_raw["id"],
        parent_id=None,
        incoming=None,
        depth=0,
        turn=1,
    )
    _assert_derived(root, root_raw)
    session.nodes[root.id] = root
    pending = list(root_raw.get("child_ids") or [])
    seen = {root.id}
    while pending:
        child_id = pending.pop(0)
        if child_id in seen:
            raise ExplorerError("Session tree contains a cycle", code="invalid_session")
        raw = by_id.get(child_id)
        if raw is None:
            raise ExplorerError(f"Unknown child id {child_id}", code="invalid_session")
        parent_id = raw.get("parent_id")
        parent = session.nodes.get(str(parent_id)) if parent_id is not None else None
        if parent is None:
            raise ExplorerError(f"Missing parent {parent_id}", code="invalid_session")
        saved_incoming = raw["incoming"]
        native_index = saved_incoming.get("native_index")
        descriptor = saved_incoming.get("descriptor")
        if not isinstance(native_index, int) or isinstance(native_index, bool) or not isinstance(descriptor, dict):
            raise ExplorerError("Incoming edge is missing native_index or descriptor", code="invalid_session")
        clone = parent.state.clone()
        decision = clone.decision()
        if native_index < 0 or native_index >= len(decision.actions):
            raise ExplorerError(f"Saved action index {native_index} is invalid at node {parent.id}", code="invalid_session")
        allowed = action_indices(decision)
        if native_index not in allowed:
            raise ExplorerError(
                f"Saved action is not allowed by the combat task filter at node {parent.id}",
                code="invalid_session",
            )
        action = decision.actions[native_index]
        if not descriptors_match(descriptor, action_descriptor(action)):
            raise ExplorerError(f"Saved action descriptor does not match native index at node {parent.id}", code="invalid_session")
        try:
            child_decision = clone.step(action)
        except ValueError as error:
            raise ExplorerError(f"Replay failed at node {parent.id}: {error}", code="invalid_session") from error
        expected_turn = parent.turn + 1 if action.kind == "end_turn" else parent.turn
        label = action_label(parent.observation, action)  # type: ignore[arg-type]
        try:
            summary = summarize_transition(parent.observation, child_decision.observation, action)  # type: ignore[arg-type]
            markers = transition_markers(parent.observation, child_decision.observation, action)  # type: ignore[arg-type]
        except Exception as error:
            summary = fallback_summary(action.kind, label, str(error))
            markers = fallback_markers()
        diagnostics = saved_incoming.get("diagnostics")
        incoming = {
            "parent_id": parent.id,
            "actor": saved_incoming.get("actor"),
            "native_index": native_index,
            "descriptor": action_descriptor(action),
            "label": label,
            "diagnostics": diagnostics if isinstance(diagnostics, dict) else None,
            "summary": summary,
            "markers": markers,
            "child_id": child_id,
        }
        child = manager._node_from_state(
            clone,
            child_decision,
            node_id=child_id,
            parent_id=parent.id,
            incoming=incoming,
            depth=parent.depth + 1,
            turn=expected_turn,
        )
        _check_node_observation(raw, child_decision, clone)
        _assert_derived(child, raw)
        if child.id not in parent.child_ids:
            parent.child_ids.append(child.id)
        session.nodes[child.id] = child
        seen.add(child.id)
        pending.extend(str(item) for item in raw.get("child_ids") or [])
    if set(session.nodes) != set(by_id):
        raise ExplorerError("Replay did not reconstruct every saved node", code="invalid_session")
    for raw in data["nodes"]:
        session.nodes[raw["id"]].child_ids = list(raw.get("child_ids") or [])
    for raw_job in data.get("jobs") or []:
        job = job_from_document(raw_job, session_id=session.id)
        session.job_ids.append(job.id)
        manager.attach_job(job)
    return session


def _validate_node_record(raw: object, index: int) -> dict[str, Any]:
    node = require_mapping(raw, f"nodes[{index}]")
    _require_id(node.get("id"), f"nodes[{index}].id")
    parent_id = node.get("parent_id")
    if parent_id is not None:
        _require_id(parent_id, f"nodes[{index}].parent_id")
    child_ids = node.get("child_ids")
    if not isinstance(child_ids, list):
        raise ExplorerError(f"nodes[{index}].child_ids must be a list", code="invalid_session")
    if len(child_ids) > MAX_NODE_CHILDREN:
        raise ExplorerError(f"nodes[{index}] has too many children", code="invalid_session")
    for child in child_ids:
        _require_id(child, f"nodes[{index}].child_ids")
    for name in ("depth", "turn", "revision", "schema_version"):
        value = node.get(name)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise ExplorerError(f"nodes[{index}].{name} must be a non-negative integer", code="invalid_session")
    if node.get("turn") < 1:
        raise ExplorerError(f"nodes[{index}].turn must be >= 1", code="invalid_session")
    digest = node.get("observation_sha256")
    if not isinstance(digest, str) or not SHA256_PATTERN.fullmatch(digest):
        raise ExplorerError(f"nodes[{index}].observation_sha256 is invalid", code="invalid_session")
    if not isinstance(node.get("action_manifest"), list):
        raise ExplorerError(f"nodes[{index}].action_manifest must be a list", code="invalid_session")
    allowed = node.get("allowed_native_indices")
    if not isinstance(allowed, list) or any(not isinstance(item, int) or isinstance(item, bool) for item in allowed):
        raise ExplorerError(f"nodes[{index}].allowed_native_indices must be a list of ints", code="invalid_session")
    outcome = node.get("outcome")
    if outcome not in (True, False, None):
        raise ExplorerError(f"nodes[{index}].outcome must be true, false, or null", code="invalid_session")
    return node


def _validate_incoming(incoming: dict[str, Any], node_id: str) -> None:
    actor = incoming.get("actor")
    if actor not in ("human", "model"):
        raise ExplorerError(f"Incoming actor on {node_id} must be human or model", code="invalid_session")
    native_index = incoming.get("native_index")
    if not isinstance(native_index, int) or isinstance(native_index, bool) or native_index < 0:
        raise ExplorerError(f"Incoming native_index on {node_id} is invalid", code="invalid_session")
    if not isinstance(incoming.get("descriptor"), dict):
        raise ExplorerError(f"Incoming descriptor on {node_id} must be an object", code="invalid_session")
    diagnostics = incoming.get("diagnostics")
    if diagnostics is not None:
        _validate_diagnostics(diagnostics, f"{node_id}.incoming.diagnostics")


def _validate_preferred(preferred: dict[str, str], nodes: dict[str, Node]) -> None:
    for parent, child in preferred.items():
        parent_id = _require_id(parent, "preferred_children.key")
        child_id = _require_id(child, "preferred_children.value")
        if parent_id not in nodes or child_id not in nodes:
            raise ExplorerError("preferred_children reference unknown nodes", code="invalid_session")
        if child_id not in nodes[parent_id].child_ids:
            raise ExplorerError("preferred child is not a child of its parent", code="invalid_session")


def _require_id(value: object, path: str) -> str:
    if not isinstance(value, str) or not value or len(value) > MAX_ID_LENGTH:
        raise ExplorerError(f"{path} must be a non-empty string", code="invalid_session")
    return value


def _require_nonneg_int(value: object, path: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ExplorerError(f"{path} must be a non-negative integer", code="invalid_session")
    return value


def _validate_diagnostics(diagnostics: object, path: str) -> None:
    mapping = require_mapping(diagnostics, path)
    natives = mapping.get("native_indices")
    if natives is not None:
        if not isinstance(natives, list) or any(not isinstance(item, int) or isinstance(item, bool) for item in natives):
            raise ExplorerError(f"{path}.native_indices must be a list of ints", code="invalid_session")
    for key in ("base_probabilities", "adjusted_probabilities"):
        values = mapping.get(key)
        if values is None:
            continue
        if not isinstance(values, list) or any(
            not isinstance(item, (int, float)) or isinstance(item, bool) for item in values
        ):
            raise ExplorerError(f"{path}.{key} must be a list of numbers", code="invalid_session")
    labels = mapping.get("labels")
    if labels is not None and not isinstance(labels, list):
        raise ExplorerError(f"{path}.labels must be a list", code="invalid_session")


def _check_json_depth(value: object, path: str, depth: int = 0) -> None:
    if depth > MAX_JSON_DEPTH:
        raise ExplorerError(f"{path} exceeds maximum JSON nesting depth {MAX_JSON_DEPTH}", code="invalid_session")
    if isinstance(value, dict):
        for key, item in value.items():
            _check_json_depth(item, f"{path}.{key}", depth + 1)
    elif isinstance(value, list):
        if value:
            _check_json_depth(value[0], f"{path}[]", depth + 1)
            for item in value[1:]:
                _check_json_depth(item, f"{path}[]", depth + 1)


def _assert_derived(node: Node, raw: dict[str, Any]) -> None:
    if int(raw["depth"]) != node.depth:
        raise ExplorerError(f"Saved depth does not match replay at node {node.id}", code="invalid_session")
    if int(raw["turn"]) != node.turn:
        raise ExplorerError(f"Saved turn does not match replay at node {node.id}", code="invalid_session")
    saved_outcome = raw.get("outcome")
    if saved_outcome != node.outcome:
        raise ExplorerError(f"Saved outcome does not match replay at node {node.id}", code="invalid_session")
    if list(raw.get("allowed_native_indices") or []) != list(node.allowed_native_indices):
        raise ExplorerError(f"Saved allowed actions do not match replay at node {node.id}", code="invalid_session")


def _check_node_observation(raw: dict[str, Any], decision: Any, state: State) -> None:
    del state
    digest = observation_sha256(decision.observation)
    expected = raw.get("observation_sha256")
    if expected != digest:
        raise ExplorerError(f"Observation hash mismatch at node {raw.get('id')}", code="invalid_session")
    saved_manifest = raw.get("action_manifest")
    actual_manifest = [action_descriptor(action) for action in decision.actions]
    if saved_manifest != actual_manifest:
        raise ExplorerError(f"Action manifest mismatch at node {raw.get('id')}", code="invalid_session")
    from combat_explorer.jsonutil import json_safe

    if canonical_dumps(raw.get("observation")) != canonical_dumps(json_safe(decision.observation)):
        raise ExplorerError(f"Recorded observation does not match replay at node {raw.get('id')}", code="invalid_session")


def _has_cycle(by_id: dict[str, dict[str, Any]], root_id: str) -> bool:
    visiting: set[str] = set()
    visited: set[str] = set()

    def walk(node_id: str) -> bool:
        if node_id in visiting:
            return True
        if node_id in visited:
            return False
        visiting.add(node_id)
        for child in by_id[node_id].get("child_ids") or []:
            if walk(str(child)):
                return True
        visiting.remove(node_id)
        visited.add(node_id)
        return False

    return walk(root_id)


def safe_filename(name: str) -> str:
    if not isinstance(name, str) or not FILENAME_PATTERN.fullmatch(name):
        raise ExplorerError("Filename must be a simple *.json name")
    return name


def resolve_under(directory: Path, name: str) -> Path:
    filename = safe_filename(name)
    directory = directory.resolve()
    path = (directory / filename).resolve()
    if path.parent != directory:
        raise ExplorerError("Path escapes the sessions directory")
    return path


def read_session_file(path: Path, *, max_bytes: int | None = None) -> dict[str, Any]:
    if not path.is_file():
        raise NotFoundError(f"Session file not found: {path.name}")
    limit = MAX_SESSION_FILE_BYTES if max_bytes is None else max_bytes
    size = path.stat().st_size
    if size > limit:
        raise ExplorerError(f"Session file exceeds {limit} bytes", code="invalid_session")
    try:
        document = json.loads(path.read_text())
    except json.JSONDecodeError as error:
        raise ExplorerError("Session file is not valid JSON", code="invalid_session") from error
    if not isinstance(document, dict):
        raise ExplorerError("Session file must contain a JSON object", code="invalid_session")
    return document


def atomic_write_json(path: Path, document: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = json.dumps(document, indent=2, allow_nan=False)
    fd, tmp_name = tempfile.mkstemp(prefix=path.name, suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(data)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp_name, path)
    except Exception:
        if os.path.exists(tmp_name):
            os.remove(tmp_name)
        raise


def allowed_file(path: Path, allowed: list[Path]) -> Path:
    resolved = path.expanduser().resolve()
    if not resolved.is_file():
        raise ExplorerError(f"File not found: {path}")
    for base in allowed:
        candidate = base.expanduser().resolve()
        if resolved == candidate:
            return resolved
        if candidate.is_dir() and candidate in resolved.parents:
            return resolved
    raise ExplorerError("Path is not an allowed local artifact")
