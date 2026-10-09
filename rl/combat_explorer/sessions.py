"""Immutable combat trees, transactional stepping, and continuation jobs."""

from __future__ import annotations

import concurrent.futures
import secrets
import threading
import traceback
import uuid
from dataclasses import dataclass, field
from typing import Any, Callable, Literal

from combat_task import action_indices, combat_outcome
from sts_sim import Action, Decision, State

from combat_explorer import DEFAULT_MAX_DECISIONS, MAX_DECISIONS_LIMIT, TASK_PROTOCOL
from combat_explorer.errors import ConflictError, ExplorerError, ModelError, NotFoundError, SimulatorError
from combat_explorer.jsonutil import canonical_dumps, observation_sha256, parse_seed, seed_to_str
from combat_explorer.policy import (
    LoadedCheckpoint,
    Mode,
    PolicyAdapter,
    analysis_from_scores,
    new_sampling_rng,
    sampling_metadata,
    validate_mode,
    validate_temperature,
)
from combat_explorer.presentation import (
    MARKER_LIMITATIONS,
    action_descriptor,
    action_label,
    action_manifest,
    descriptors_match,
    fallback_markers,
    fallback_summary,
    present_actions,
    present_board,
    public_hp,
    summarize_transition,
    transition_markers,
)
from combat_explorer.roots import PreparedRoot

JobStatus = Literal["running", "completed", "cancelled", "failed", "interrupted"]
JobReason = Literal["victory", "defeat", "limit", "cancelled", "model_error", "simulator_error", "interrupted"]


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex}"


def _outcome(observation: object) -> bool | None:
    try:
        return combat_outcome(observation)  # type: ignore[arg-type]
    except RuntimeError as error:
        raise SimulatorError(f"Unexpected screen after combat: {getattr(observation, 'kind', None)}") from error


@dataclass
class Node:
    id: str
    parent_id: str | None
    child_ids: list[str]
    incoming: dict[str, Any] | None
    observation: object
    observation_json: object
    observation_sha256: str
    action_manifest: list[dict[str, Any]]
    allowed_native_indices: list[int]
    schema_version: int
    revision: int
    depth: int
    turn: int
    outcome: bool | None
    state: State
    summary: dict[str, Any] | None
    markers: dict[str, Any]


@dataclass
class Job:
    id: str
    session_id: str
    source_node_id: str
    status: JobStatus = "running"
    reason: str | None = None
    generated: int = 0
    leaf_id: str | None = None
    error: str | None = None
    error_code: str | None = None
    traceback: str | None = None
    attempted_action: dict[str, Any] | None = None
    max_decisions: int = DEFAULT_MAX_DECISIONS
    mode: Mode = "sample"
    temperature: float | None = 1.0
    sampling_seed: str | None = None
    checkpoint_fingerprint: str | None = None
    cancel: threading.Event = field(default_factory=threading.Event)


@dataclass
class Session:
    id: str
    spec_json: str
    spec: dict[str, Any]
    provenance: dict[str, Any]
    root_id: str
    nodes: dict[str, Node]
    lock: threading.RLock = field(default_factory=threading.RLock)
    writer_job_id: str | None = None
    idempotency: dict[str, dict[str, Any]] = field(default_factory=dict)
    model: LoadedCheckpoint | None = None
    score_cache: dict[tuple[str, str, str], Any] = field(default_factory=dict)
    job_ids: list[str] = field(default_factory=list)


class SessionManager:
    def __init__(self, policy: PolicyAdapter) -> None:
        self.policy = policy
        self.sessions: dict[str, Session] = {}
        self.jobs: dict[str, Job] = {}
        self._lock = threading.Lock()
        self._executor = concurrent.futures.ThreadPoolExecutor(max_workers=4, thread_name_prefix="explorer")

    def close(self) -> None:
        self._executor.shutdown(wait=False, cancel_futures=True)

    def create(self, prepared: PreparedRoot, *, model: LoadedCheckpoint | None) -> Session:
        state = prepared.state.clone()
        decision = state.decision()
        node = self._node_from_state(
            state,
            decision,
            node_id=_new_id("node"),
            parent_id=None,
            incoming=None,
            depth=0,
            turn=1,
        )
        session = Session(
            id=_new_id("session"),
            spec_json=prepared.spec_json,
            spec=prepared.spec,
            provenance=dict(prepared.provenance),
            root_id=node.id,
            nodes={node.id: node},
            model=model,
        )
        with self._lock:
            self.sessions[session.id] = session
        return session

    def get(self, session_id: str) -> Session:
        session = self.sessions.get(session_id)
        if session is None:
            raise NotFoundError(f"Unknown session {session_id}")
        return session

    def _node_from_state(
        self,
        state: State,
        decision: Decision,
        *,
        node_id: str,
        parent_id: str | None,
        incoming: dict[str, Any] | None,
        depth: int,
        turn: int,
    ) -> Node:
        observation = decision.observation
        outcome = _outcome(observation)
        actions = tuple(decision.actions)
        allowed = [] if outcome is not None else action_indices(decision)
        markers = incoming["markers"] if incoming and "markers" in incoming else {
            "turn_end": False,
            "kills": [],
            "uncertain_removals": [],
            "escaped": [],
            "combat_win": outcome is True,
            "combat_loss": outcome is False,
            "limitations": MARKER_LIMITATIONS,
        }
        return Node(
            id=node_id,
            parent_id=parent_id,
            child_ids=[],
            incoming=incoming,
            observation=observation,
            observation_json=_safe_observation(observation),
            observation_sha256=observation_sha256(observation),
            action_manifest=action_manifest(actions),
            allowed_native_indices=list(allowed),
            schema_version=decision.schema_version,
            revision=decision.revision,
            depth=depth,
            turn=turn,
            outcome=outcome,
            state=state,
            summary=None if incoming is None else incoming.get("summary"),
            markers=markers,
        )

    def tree_payload(self, session: Session) -> dict[str, Any]:
        with session.lock:
            job = self.jobs.get(session.writer_job_id) if session.writer_job_id else None
            if job is not None and job.status != "running":
                job = None
            jobs = [job_payload(self.jobs[job_id]) for job_id in session.job_ids if job_id in self.jobs]
            nodes = [
                {
                    "id": node.id,
                    "parent_id": node.parent_id,
                    "child_ids": list(node.child_ids),
                    "depth": node.depth,
                    "turn": node.turn,
                    "revision": node.revision,
                    "outcome": _outcome_label(node.outcome),
                    "terminal": node.outcome is not None,
                    "incoming_label": None if node.incoming is None else node.incoming.get("label"),
                    "actor": None if node.incoming is None else node.incoming.get("actor"),
                    **public_hp(node.observation),
                    "markers": {
                        "turn_end": bool(node.markers.get("turn_end")),
                        "kills": list(node.markers.get("kills") or []),
                        "uncertain_removals": list(node.markers.get("uncertain_removals") or []),
                        "combat_win": bool(node.markers.get("combat_win")),
                        "combat_loss": bool(node.markers.get("combat_loss")),
                    },
                }
                for node in session.nodes.values()
            ]
            return {
                "session_id": session.id,
                "root_id": session.root_id,
                "provenance": session.provenance,
                "task_protocol": TASK_PROTOCOL,
                "model": None if session.model is None else _model_public(session.model),
                "busy": None if job is None else job_payload(job),
                "jobs": jobs,
                "marker_limitations": MARKER_LIMITATIONS,
                "nodes": nodes,
            }

    def node_payload(self, session: Session, node_id: str) -> dict[str, Any]:
        with session.lock:
            node = self._node(session, node_id)
            actions = present_actions(node.observation, node.state.decision().actions)  # type: ignore[arg-type]
            outgoing = [_outgoing(session.nodes[child_id]) for child_id in node.child_ids]
            return {
                "id": node.id,
                "parent_id": node.parent_id,
                "child_ids": list(node.child_ids),
                "depth": node.depth,
                "turn": node.turn,
                "revision": node.revision,
                "schema_version": node.schema_version,
                "outcome": _outcome_label(node.outcome),
                "terminal": node.outcome is not None,
                "board": present_board(node.observation),  # type: ignore[arg-type]
                "observation": node.observation_json,
                "observation_sha256": node.observation_sha256,
                "actions": actions,
                "allowed_native_indices": list(node.allowed_native_indices),
                "incoming": node.incoming,
                "outgoing": outgoing,
                "summary": node.summary,
                "markers": node.markers,
                "marker_limitations": MARKER_LIMITATIONS,
                "current_analysis": None,
            }

    def analyze(self, session: Session, node_id: str, *, mode: str, temperature: float) -> dict[str, Any]:
        mode_name = validate_mode(mode)
        temp = 1.0 if mode_name == "greedy" else validate_temperature(temperature)
        with session.lock:
            node = self._node(session, node_id)
            if node.outcome is not None:
                raise ExplorerError("Terminal nodes have no policy actions")
        return self._analyze_node(session, node, mode=mode_name, temperature=temp)

    def act(
        self,
        session: Session,
        node_id: str,
        *,
        native_index: int,
        revision: int,
        descriptor: dict[str, Any] | None,
        request_id: str | None,
    ) -> dict[str, Any]:
        fingerprint = canonical_dumps(
            {
                "op": "act",
                "session_id": session.id,
                "node_id": node_id,
                "native_index": native_index,
                "revision": revision,
                "descriptor": descriptor,
            }
        )
        return self._mutating(
            session,
            request_id,
            fingerprint,
            lambda: self._act_locked(session, node_id, native_index, revision, descriptor, actor="human", diagnostics=None),
        )

    def delete_node(self, session: Session, node_id: str) -> dict[str, Any]:
        with session.lock:
            self._ensure_writable(session)
            node = self._node(session, node_id)
            if node.parent_id is None:
                raise ExplorerError("The root node cannot be deleted")
            parent = self._node(session, node.parent_id)
            removed: list[str] = []
            stack = [node.id]
            seen: set[str] = set()
            while stack:
                current = stack.pop()
                if current in seen:
                    continue
                seen.add(current)
                removed.append(current)
                child = session.nodes.get(current)
                if child is not None:
                    stack.extend(child.child_ids)
            removed_set = set(removed)
            parent.child_ids = [child_id for child_id in parent.child_ids if child_id not in removed_set]
            for nid in removed:
                session.nodes.pop(nid, None)
            session.score_cache = {key: value for key, value in session.score_cache.items() if key[0] not in removed_set}
            # Completed continuation history belongs to its source/leaf path.
            # Remove jobs referencing deleted nodes so exported sessions remain
            # loadable and the UI cannot jump to a non-existent leaf.
            stale_jobs = [
                job_id for job_id in session.job_ids
                if (job := self.jobs.get(job_id)) is not None
                and (job.source_node_id in removed_set or job.leaf_id in removed_set)
            ]
            stale_job_ids = set(stale_jobs)
            stale_keys = [
                key
                for key, cached in session.idempotency.items()
                if isinstance(cached, dict)
                and (
                    (cached.get("result") or {}).get("child_id") in removed_set
                    or (cached.get("result") or {}).get("id") in stale_job_ids
                )
            ]
            for key in stale_keys:
                session.idempotency.pop(key, None)
            for job_id in stale_jobs:
                self.jobs.pop(job_id, None)
            session.job_ids = [job_id for job_id in session.job_ids if job_id not in stale_job_ids]
            return {"deleted": removed, "parent_id": parent.id, "count": len(removed)}

    def model_step(
        self,
        session: Session,
        node_id: str,
        *,
        mode: str,
        temperature: float,
        sampling_seed: str | None,
        request_id: str | None,
    ) -> dict[str, Any]:
        mode_name = validate_mode(mode)
        temp = 1.0 if mode_name == "greedy" else validate_temperature(temperature)
        if sampling_seed is not None:
            parse_seed(sampling_seed, field="sampling_seed")
        seed = sampling_seed if sampling_seed is not None else seed_to_str(int(secrets.randbits(64)))
        fingerprint = canonical_dumps(
            {
                "op": "model_step",
                "session_id": session.id,
                "node_id": node_id,
                "mode": mode_name,
                "temperature": None if mode_name == "greedy" else temp,
                "sampling_seed": sampling_seed,
            }
        )

        def run() -> dict[str, Any]:
            child = self._model_step_apply(
                session,
                node_id,
                mode=mode_name,
                temperature=temp,
                rng_seed=seed,
                rng=new_sampling_rng(seed) if mode_name == "sample" else None,
            )
            return {"parent_id": child.parent_id, "child_id": child.id, "node": self._public_node(child)}

        return self._mutating(session, request_id, fingerprint, run)

    def start_continue(
        self,
        session: Session,
        node_id: str,
        *,
        mode: str,
        temperature: float,
        sampling_seed: str | None,
        max_decisions: int,
        request_id: str | None,
    ) -> dict[str, Any]:
        mode_name = validate_mode(mode)
        temp = 1.0 if mode_name == "greedy" else validate_temperature(temperature)
        if not isinstance(max_decisions, int) or isinstance(max_decisions, bool) or not 1 <= max_decisions <= MAX_DECISIONS_LIMIT:
            raise ExplorerError(f"max_decisions must be between 1 and {MAX_DECISIONS_LIMIT}")
        if sampling_seed is not None:
            parse_seed(sampling_seed, field="sampling_seed")
        seed = sampling_seed if sampling_seed is not None else seed_to_str(int(secrets.randbits(64)))
        fingerprint = canonical_dumps(
            {
                "op": "continue",
                "session_id": session.id,
                "node_id": node_id,
                "mode": mode_name,
                "temperature": None if mode_name == "greedy" else temp,
                "sampling_seed": sampling_seed,
                "max_decisions": max_decisions,
            }
        )
        with session.lock:
            cached = self._cached_result(session, request_id, fingerprint)
            if cached is not None:
                return cached
            self._ensure_writable(session)
            node = self._node(session, node_id)
            if session.model is None:
                raise ModelError("No model checkpoint is loaded")
            if node.outcome is not None:
                raise ExplorerError("Terminal nodes cannot be continued")
            job = Job(
                id=_new_id("job"),
                session_id=session.id,
                source_node_id=node.id,
                max_decisions=max_decisions,
                mode=mode_name,
                temperature=None if mode_name == "greedy" else temp,
                sampling_seed=seed if mode_name == "sample" else None,
                checkpoint_fingerprint=session.model.fingerprint,
                leaf_id=node.id,
            )
            session.writer_job_id = job.id
            session.job_ids.append(job.id)
            self.jobs[job.id] = job
            payload = job_payload(job)
            if request_id:
                session.idempotency[request_id] = {"fingerprint": fingerprint, "result": payload}
        self._executor.submit(self._run_continue, session.id, job.id, node.id, mode_name, temp, seed)
        return payload

    def job(self, job_id: str) -> Job:
        job = self.jobs.get(job_id)
        if job is None:
            raise NotFoundError(f"Unknown job {job_id}")
        return job

    def cancel_job(self, job_id: str) -> Job:
        job = self.job(job_id)
        job.cancel.set()
        return job

    def attach_job(self, job: Job) -> None:
        with self._lock:
            if job.id in self.jobs:
                raise ExplorerError("Job id already exists in the manager", code="invalid_session")
            self.jobs[job.id] = job

    def _run_continue(self, session_id: str, job_id: str, source_id: str, mode: Mode, temperature: float, seed: str) -> None:
        session = self.sessions[session_id]
        job = self.jobs[job_id]
        current_id = source_id
        try:
            rng = new_sampling_rng(seed) if mode == "sample" else None
            while True:
                if job.cancel.is_set():
                    _finish_job(session, job, "cancelled", "cancelled", current_id)
                    return
                with session.lock:
                    node = self._node(session, current_id)
                    if node.outcome is True:
                        _finish_job(session, job, "completed", "victory", current_id)
                        return
                    if node.outcome is False:
                        _finish_job(session, job, "completed", "defeat", current_id)
                        return
                    if job.generated >= job.max_decisions:
                        _finish_job(session, job, "completed", "limit", current_id)
                        return
                child = self._model_step_apply(
                    session,
                    current_id,
                    mode=mode,
                    temperature=temperature,
                    rng_seed=seed,
                    rng=rng,
                    job=job,
                )
                current_id = child.id
                job.generated += 1
                job.leaf_id = current_id
                job.attempted_action = None
        except SimulatorError as error:
            _fail_job(session, job, "simulator_error", error, current_id)
        except ModelError as error:
            _fail_job(session, job, "model_error", error, current_id)
        except ExplorerError as error:
            reason = "simulator_error" if error.code == "simulator_error" else "model_error"
            _fail_job(session, job, reason, error, current_id)
        except Exception as error:
            _fail_job(session, job, "model_error", error, current_id, tb=traceback.format_exc())
        finally:
            with session.lock:
                _release_writer(session, job)
                if job.status == "running":
                    job.status = "failed"
                    job.reason = "model_error"
                    job.error_code = "model_error"
                    job.error = job.error or "Continuation ended without a terminal status"

    def _mutating(self, session: Session, request_id: str | None, fingerprint: str, fn: Callable[[], dict[str, Any]]) -> dict[str, Any]:
        with session.lock:
            cached = self._cached_result(session, request_id, fingerprint)
            if cached is not None:
                return cached
            self._ensure_writable(session)
            result = fn()
            if request_id:
                session.idempotency[request_id] = {"fingerprint": fingerprint, "result": result}
            return result

    def _cached_result(self, session: Session, request_id: str | None, fingerprint: str) -> dict[str, Any] | None:
        if not request_id:
            return None
        cached = session.idempotency.get(request_id)
        if cached is None:
            return None
        if cached.get("fingerprint") != fingerprint:
            raise ConflictError("Idempotency key reused with a different payload", code="idempotency_conflict")
        result = cached.get("result")
        if not isinstance(result, dict):
            raise ConflictError("Corrupt idempotency cache")
        return result

    def _ensure_writable(self, session: Session) -> None:
        if session.writer_job_id:
            raise ConflictError("Session is busy with a continuation job")

    def _node(self, session: Session, node_id: str) -> Node:
        node = session.nodes.get(node_id)
        if node is None:
            raise NotFoundError(f"Unknown node {node_id}")
        return node

    def _act_locked(
        self,
        session: Session,
        node_id: str,
        native_index: int,
        revision: int,
        descriptor: dict[str, Any] | None,
        *,
        actor: str,
        diagnostics: dict[str, Any] | None,
    ) -> dict[str, Any]:
        node = self._node(session, node_id)
        child = self._apply(session, node, native_index, revision, descriptor, actor=actor, diagnostics=diagnostics)
        return {"parent_id": node.id, "child_id": child.id, "node": self._public_node(child)}

    def _model_step_apply(
        self,
        session: Session,
        node_id: str,
        *,
        mode: Mode,
        temperature: float,
        rng_seed: str,
        rng: Any,
        job: Job | None = None,
    ) -> Node:
        with session.lock:
            node = self._node(session, node_id)
            if session.model is None:
                raise ModelError("No model checkpoint is loaded")
            if node.outcome is not None:
                raise ExplorerError("Terminal nodes cannot be continued")
            decision = node.state.decision()
            revision = decision.revision
            fingerprint = session.model.fingerprint
        scored = self._score_cached(session, node, decision, fingerprint)
        native_index, analysis = self.policy.choose(scored, mode=mode, temperature=temperature, rng=rng)
        analysis.update(sampling_metadata(rng_seed) if mode == "sample" else {})
        analysis["historical"] = True
        descriptor = scored.descriptors[scored.native_indices.index(native_index)]
        attempted = {
            "native_index": native_index,
            "descriptor": descriptor,
            "label": scored.labels[scored.native_indices.index(native_index)],
            "mode": mode,
            "temperature": None if mode == "greedy" else temperature,
            "checkpoint_fingerprint": scored.checkpoint_fingerprint,
        }
        if job is not None:
            job.attempted_action = attempted
        with session.lock:
            node = self._node(session, node_id)
            return self._apply(
                session,
                node,
                native_index,
                revision,
                descriptor,
                actor="model",
                diagnostics=analysis,
            )

    def _score_cached(self, session: Session, node: Node, decision: Decision, fingerprint: str) -> Any:
        if session.model is None:
            raise ModelError("No model checkpoint is loaded")
        adapter_version = session.model.adapter_version
        key = (node.id, fingerprint, adapter_version)
        with session.lock:
            cached = session.score_cache.get(key)
            if cached is not None:
                return cached
        scored = self.policy.score(node.state, session.model)
        with session.lock:
            existing = session.score_cache.get(key)
            if existing is not None:
                return existing
            session.score_cache[key] = scored
            return scored

    def _analyze_node(self, session: Session, node: Node, *, mode: Mode, temperature: float) -> dict[str, Any]:
        if session.model is None:
            raise ModelError("No model checkpoint is loaded")
        decision = node.state.decision()
        scored = self._score_cached(session, node, decision, session.model.fingerprint)
        analysis = analysis_from_scores(scored, mode=mode, temperature=temperature)
        analysis["historical"] = False
        analysis["kind"] = "current_reanalysis"
        return analysis


    def _equivalent_child(
        self,
        session: Session,
        node: Node,
        native_index: int,
        descriptor: dict[str, Any],
    ) -> Node | None:
        for child_id in node.child_ids:
            child = session.nodes.get(child_id)
            incoming = None if child is None else child.incoming
            if not incoming:
                continue
            if incoming.get("native_index") != native_index:
                continue
            saved = incoming.get("descriptor")
            if isinstance(saved, dict) and descriptors_match(saved, descriptor):
                return child
        return None

    def _apply(
        self,
        session: Session,
        node: Node,
        native_index: int,
        revision: int,
        descriptor: dict[str, Any] | None,
        *,
        actor: str,
        diagnostics: dict[str, Any] | None,
    ) -> Node:
        if node.outcome is not None:
            raise ExplorerError("Terminal nodes cannot be advanced")
        if revision != node.revision:
            raise ExplorerError("Stale decision revision", code="stale_revision")
        clone = node.state.clone()
        decision = clone.decision()
        if decision.revision != node.revision:
            raise SimulatorError("Cloned decision revision does not match the historical node")
        if native_index < 0 or native_index >= len(decision.actions):
            raise ExplorerError("Action index is out of range")
        allowed = action_indices(decision)
        if native_index not in allowed:
            raise ExplorerError("Action is not allowed by the combat task filter (Smoke Bomb use is excluded)")
        action: Action = decision.actions[native_index]
        actual = action_descriptor(action)
        if descriptor is not None and not descriptors_match(descriptor, actual):
            raise ExplorerError("Action descriptor does not match the native index")
        existing = self._equivalent_child(session, node, native_index, actual)
        if existing is not None:
            return existing
        try:
            child_decision = clone.step(action)
        except ValueError as error:
            raise SimulatorError(f"Simulator rejected the action: {error}") from error
        turn = node.turn + 1 if action.kind == "end_turn" else node.turn
        label = action_label(node.observation, action)  # type: ignore[arg-type]
        try:
            summary = summarize_transition(node.observation, child_decision.observation, action)  # type: ignore[arg-type]
            markers = transition_markers(node.observation, child_decision.observation, action)  # type: ignore[arg-type]
        except Exception as error:
            summary = fallback_summary(action.kind, label, str(error))
            markers = fallback_markers()
        incoming = {
            "parent_id": node.id,
            "actor": actor,
            "native_index": native_index,
            "descriptor": actual,
            "label": label,
            "diagnostics": diagnostics,
            "summary": summary,
            "markers": markers,
        }
        child = self._node_from_state(
            clone,
            child_decision,
            node_id=_new_id("node"),
            parent_id=node.id,
            incoming=incoming,
            depth=node.depth + 1,
            turn=turn,
        )
        incoming["child_id"] = child.id
        node.child_ids.append(child.id)
        session.nodes[child.id] = child
        return child

    def _public_node(self, node: Node) -> dict[str, Any]:
        return {
            "id": node.id,
            "parent_id": node.parent_id,
            "child_ids": list(node.child_ids),
            "depth": node.depth,
            "turn": node.turn,
            "revision": node.revision,
            "outcome": _outcome_label(node.outcome),
            "terminal": node.outcome is not None,
            "summary": node.summary,
            "markers": node.markers,
            "incoming": node.incoming,
        }

    def attach_reconstructed(self, session: Session) -> None:
        with self._lock:
            self.sessions[session.id] = session


def _outcome_label(outcome: bool | None) -> str | None:
    if outcome is True:
        return "win"
    if outcome is False:
        return "loss"
    return None


def _model_public(model: LoadedCheckpoint) -> dict[str, Any]:
    return {
        "path": model.path,
        "fingerprint": model.fingerprint,
        "architecture": model.architecture,
        "adapter_version": model.adapter_version,
        "device": model.device,
    }


def _outgoing(child: Node) -> dict[str, Any]:
    incoming = child.incoming or {}
    diagnostics = incoming.get("diagnostics")
    historical = None
    if isinstance(diagnostics, dict):
        historical = {
            "checkpoint_fingerprint": diagnostics.get("checkpoint_fingerprint"),
            "adapter_version": diagnostics.get("adapter_version"),
            "mode": diagnostics.get("mode"),
            "temperature": diagnostics.get("temperature"),
            "sampling_seed": diagnostics.get("sampling_seed"),
            "generator": diagnostics.get("generator"),
            "generator_version": diagnostics.get("generator_version"),
            "chosen_native_index": diagnostics.get("chosen_native_index"),
            "base_probabilities": diagnostics.get("base_probabilities"),
            "adjusted_probabilities": diagnostics.get("adjusted_probabilities"),
            "native_indices": diagnostics.get("native_indices"),
            "labels": diagnostics.get("labels"),
            "descriptors": diagnostics.get("descriptors"),
            "note": diagnostics.get("note"),
            "historical": True,
            "kind": "historical_choice",
        }
    return {
        "child_id": child.id,
        "native_index": incoming.get("native_index"),
        "descriptor": incoming.get("descriptor"),
        "label": incoming.get("label"),
        "actor": incoming.get("actor"),
        "diagnostics": historical,
    }


def job_payload(job: Job) -> dict[str, Any]:
    return {
        "id": job.id,
        "session_id": job.session_id,
        "source_node_id": job.source_node_id,
        "status": job.status,
        "reason": job.reason,
        "generated": job.generated,
        "leaf_id": job.leaf_id,
        "error": job.error,
        "error_code": job.error_code,
        "attempted_action": job.attempted_action,
        "max_decisions": job.max_decisions,
        "mode": job.mode,
        "temperature": job.temperature,
        "sampling_seed": job.sampling_seed,
        "checkpoint_fingerprint": job.checkpoint_fingerprint,
    }


def job_from_document(raw: dict[str, Any], *, session_id: str, job_id: str | None = None) -> Job:
    status = raw.get("status")
    if status == "running":
        status = "interrupted"
        raw = {**raw, "reason": raw.get("reason") or "interrupted"}
    if status not in ("completed", "cancelled", "failed", "interrupted"):
        raise ExplorerError("Saved job has an invalid status", code="invalid_session")
    return Job(
        id=job_id or _new_id("job"),
        session_id=session_id,
        source_node_id=str(raw["source_node_id"]),
        status=status,
        reason=None if raw.get("reason") is None else str(raw["reason"]),
        generated=int(raw.get("generated") or 0),
        leaf_id=None if raw.get("leaf_id") is None else str(raw["leaf_id"]),
        error=None if raw.get("error") is None else str(raw["error"]),
        error_code=None if raw.get("error_code") is None else str(raw["error_code"]),
        attempted_action=raw.get("attempted_action") if isinstance(raw.get("attempted_action"), dict) else None,
        max_decisions=int(raw.get("max_decisions") or DEFAULT_MAX_DECISIONS),
        mode=raw.get("mode") if raw.get("mode") in ("greedy", "sample") else "sample",
        temperature=raw.get("temperature"),
        sampling_seed=None if raw.get("sampling_seed") is None else str(raw["sampling_seed"]),
        checkpoint_fingerprint=None if raw.get("checkpoint_fingerprint") is None else str(raw["checkpoint_fingerprint"]),
    )


def _release_writer(session: Session, job: Job) -> None:
    if session.writer_job_id == job.id:
        session.writer_job_id = None


def _finish_job(session: Session, job: Job, status: str, reason: str, current_id: str) -> None:
    with session.lock:
        _release_writer(session, job)
        job.status = status  # type: ignore[assignment]
        job.reason = reason
        job.leaf_id = current_id


def _fail_job(session: Session, job: Job, reason: str, error: Exception, current_id: str, *, tb: str | None = None) -> None:
    with session.lock:
        _release_writer(session, job)
        job.status = "failed"
        job.reason = reason
        job.error = str(error)
        job.error_code = getattr(error, "code", reason)
        job.traceback = tb
        job.leaf_id = current_id


def _safe_observation(observation: object) -> object:
    from combat_explorer.jsonutil import json_safe

    return json_safe(observation)
