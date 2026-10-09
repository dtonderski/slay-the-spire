"""Local loopback HTTP API and static UI for the combat explorer."""

from __future__ import annotations

import argparse
import ipaddress
import re
from contextlib import asynccontextmanager
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

from fastapi import FastAPI, Request
from fastapi.exception_handlers import request_validation_exception_handler
from fastapi.exceptions import RequestValidationError
from fastapi.responses import FileResponse, JSONResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, Field

from combat_explorer import (
    DEFAULT_MAX_DECISIONS,
    MAX_DECISIONS_LIMIT,
    MAX_TEMPERATURE,
    MIN_TEMPERATURE,
    POLICY_ADAPTER_VERSION,
    TASK_PROTOCOL,
)
from combat_explorer.errors import ConflictError, ExplorerError
from combat_explorer.persistence import (
    allowed_file,
    atomic_write_json,
    jobs_for_save,
    read_session_file,
    reconstruct,
    resolve_under,
    session_document,
    validate_document,
)
from combat_explorer.policy import PolicyAdapter
from combat_explorer.roots import RootService
from combat_explorer.sessions import SessionManager, job_payload
from validation_set import native_sha256

STATIC_DIR = Path(__file__).resolve().parent / "static"


@dataclass
class AppConfig:
    validation_manifest: Path | None = None
    distributions: Path | None = None
    sessions_dir: Path = Path("combat_explorer_sessions")
    checkpoint: Path | None = None
    host: str = "127.0.0.1"
    port: int = 8765
    device: str = "cpu"
    allowed_hosts: tuple[str, ...] = ("127.0.0.1", "localhost")
    extra_allowed_files: list[Path] = field(default_factory=list)


class FromRootBody(BaseModel):
    case_id: str
    request_id: str | None = None


class GenerateBody(BaseModel):
    generation_seed: str
    floor: int | None = None
    min_floor: int = 1
    max_floor: int = 55
    request_id: str | None = None


class ActBody(BaseModel):
    native_index: int
    revision: int
    descriptor: dict[str, Any] | None = None
    request_id: str | None = None


class AnalyzeBody(BaseModel):
    mode: str = "sample"
    temperature: float = 1.0


class ModelStepBody(BaseModel):
    mode: str = "sample"
    temperature: float = 1.0
    sampling_seed: str | None = None
    request_id: str | None = None


class ContinueBody(BaseModel):
    mode: str = "sample"
    temperature: float = 1.0
    sampling_seed: str | None = None
    max_decisions: int = DEFAULT_MAX_DECISIONS
    request_id: str | None = None


class SaveBody(BaseModel):
    filename: str
    selected_node_id: str | None = None
    preferred_children: dict[str, str] = Field(default_factory=dict)


class LoadBody(BaseModel):
    filename: str
    request_id: str | None = None


class LoadModelBody(BaseModel):
    path: str
    device: str | None = None


class ExplorerState:
    def __init__(self, config: AppConfig) -> None:
        self.config = config
        self.roots = RootService(validation_manifest=config.validation_manifest, distributions=config.distributions)
        self.policy = PolicyAdapter()
        self.sessions = SessionManager(self.policy)
        if config.checkpoint is not None:
            self.policy.load(config.checkpoint, device=config.device)

    def allowed_paths(self) -> list[Path]:
        paths: list[Path] = []
        for item in (
            self.config.validation_manifest,
            self.config.distributions,
            self.config.checkpoint,
            *self.config.extra_allowed_files,
        ):
            if item is not None:
                paths.append(item)
        return paths

    def close(self) -> None:
        self.sessions.close()


def create_app(config: AppConfig) -> FastAPI:
    config.sessions_dir.mkdir(parents=True, exist_ok=True)
    state = ExplorerState(config)

    @asynccontextmanager
    async def lifespan(_app: FastAPI):
        yield
        state.close()

    app = FastAPI(title="Simulator Combat Explorer", docs_url=None, redoc_url=None, lifespan=lifespan)
    app.state.explorer = state

    @app.middleware("http")
    async def local_only(request: Request, call_next):  # type: ignore[no-untyped-def]
        host = request.headers.get("host", "")
        hostname = host.split(":")[0].lower()
        if hostname not in config.allowed_hosts:
            return JSONResponse({"error": "Host not allowed", "code": "forbidden"}, status_code=403)
        origin = request.headers.get("origin")
        if origin is not None:
            if _origin_forbidden(origin, host, config.allowed_hosts):
                return JSONResponse({"error": "Origin not allowed", "code": "forbidden"}, status_code=403)
        return await call_next(request)

    @app.exception_handler(ExplorerError)
    async def explorer_error(_request: Request, exc: ExplorerError) -> JSONResponse:
        return JSONResponse({"error": str(exc), "code": exc.code}, status_code=exc.status_code)

    @app.exception_handler(RequestValidationError)
    async def validation_error(request: Request, exc: RequestValidationError) -> JSONResponse:
        if request.url.path.startswith("/api/"):
            return JSONResponse({"error": "Invalid request", "code": "invalid_request", "detail": exc.errors()}, status_code=422)
        return await request_validation_exception_handler(request, exc)

    @app.get("/api/capabilities")
    def capabilities() -> dict[str, Any]:
        loaded = state.policy.loaded
        return {
            "task_protocol": TASK_PROTOCOL,
            "policy_adapter": POLICY_ADAPTER_VERSION,
            "native_sha256": native_sha256(),
            "temperature_range": [MIN_TEMPERATURE, MAX_TEMPERATURE],
            "default_mode": "sample",
            "default_temperature": 1.0,
            "default_max_decisions": DEFAULT_MAX_DECISIONS,
            "max_decisions_limit": MAX_DECISIONS_LIMIT,
            "evaluation_sampling": "Categorical(logits) at temperature 1, matching train.play_combat",
            "checkpoint_loaded": None if loaded is None else {
                "path": loaded.path,
                "fingerprint": loaded.fingerprint,
                "architecture": loaded.architecture,
            },
            "sessions_dir": str(config.sessions_dir),
            "bind": f"{config.host}:{config.port}",
            "roots": state.roots.capabilities(),
            "smoke_bomb": "Use is excluded by the combat task filter for both human and model actions.",
            "summary_coverage": "Net public-state deltas only; individual enemy actions are unavailable.",
        }

    @app.get("/api/roots")
    def list_roots(q: str = "", act: int | None = None, kind: str | None = None) -> dict[str, Any]:
        return {"cases": state.roots.list_cases(query=q, act=act, kind=kind)}

    @app.post("/api/sessions/from-root")
    def from_root(body: FromRootBody) -> dict[str, Any]:
        prepared = state.roots.from_case(body.case_id)
        session = state.sessions.create(prepared, model=state.policy.loaded)
        return state.sessions.tree_payload(session)

    @app.post("/api/sessions/generated")
    def generated(body: GenerateBody) -> dict[str, Any]:
        prepared = state.roots.generate(
            generation_seed=body.generation_seed,
            floor=body.floor,
            min_floor=body.min_floor,
            max_floor=body.max_floor,
        )
        session = state.sessions.create(prepared, model=state.policy.loaded)
        return state.sessions.tree_payload(session)

    @app.get("/api/sessions/{session_id}/tree")
    def tree(session_id: str) -> dict[str, Any]:
        return state.sessions.tree_payload(state.sessions.get(session_id))

    @app.get("/api/sessions/{session_id}/nodes/{node_id}")
    def node(session_id: str, node_id: str) -> dict[str, Any]:
        session = state.sessions.get(session_id)
        return state.sessions.node_payload(session, node_id)

    @app.post("/api/sessions/{session_id}/nodes/{node_id}/analyze")
    def analyze(session_id: str, node_id: str, body: AnalyzeBody) -> dict[str, Any]:
        session = state.sessions.get(session_id)
        return state.sessions.analyze(session, node_id, mode=body.mode, temperature=body.temperature)

    @app.post("/api/sessions/{session_id}/nodes/{node_id}/act")
    def act(session_id: str, node_id: str, body: ActBody) -> dict[str, Any]:
        session = state.sessions.get(session_id)
        return state.sessions.act(
            session,
            node_id,
            native_index=body.native_index,
            revision=body.revision,
            descriptor=body.descriptor,
            request_id=body.request_id,
        )

    @app.delete("/api/sessions/{session_id}/nodes/{node_id}")
    def delete_node(session_id: str, node_id: str) -> dict[str, Any]:
        session = state.sessions.get(session_id)
        return state.sessions.delete_node(session, node_id)

    @app.post("/api/sessions/{session_id}/nodes/{node_id}/model-step")
    def model_step(session_id: str, node_id: str, body: ModelStepBody) -> dict[str, Any]:
        session = state.sessions.get(session_id)
        return state.sessions.model_step(
            session,
            node_id,
            mode=body.mode,
            temperature=body.temperature,
            sampling_seed=body.sampling_seed,
            request_id=body.request_id,
        )

    @app.post("/api/sessions/{session_id}/nodes/{node_id}/continue")
    def continue_from(session_id: str, node_id: str, body: ContinueBody) -> dict[str, Any]:
        session = state.sessions.get(session_id)
        return state.sessions.start_continue(
            session,
            node_id,
            mode=body.mode,
            temperature=body.temperature,
            sampling_seed=body.sampling_seed,
            max_decisions=body.max_decisions,
            request_id=body.request_id,
        )

    @app.get("/api/jobs/{job_id}")
    def get_job(job_id: str) -> dict[str, Any]:
        return job_payload(state.sessions.job(job_id))

    @app.post("/api/jobs/{job_id}/cancel")
    def cancel_job(job_id: str) -> dict[str, Any]:
        return job_payload(state.sessions.cancel_job(job_id))

    @app.post("/api/sessions/{session_id}/save")
    def save(session_id: str, body: SaveBody) -> dict[str, Any]:
        session = state.sessions.get(session_id)
        path = resolve_under(config.sessions_dir, body.filename)
        document = session_document(
            session,
            selected_node_id=body.selected_node_id,
            preferred_children=body.preferred_children,
            jobs=jobs_for_save(session, state.sessions),
        )
        atomic_write_json(path, document)
        return {"path": str(path), "filename": path.name, "session_id": session.id}

    @app.post("/api/sessions/load")
    def load(body: LoadBody) -> dict[str, Any]:
        path = resolve_under(config.sessions_dir, body.filename)
        document = read_session_file(path)
        validate_document(document)
        session = reconstruct(state.sessions, document)
        session.model = state.policy.loaded
        state.sessions.attach_reconstructed(session)
        payload = state.sessions.tree_payload(session)
        payload["ui"] = document.get("ui") or {}
        return payload

    @app.post("/api/sessions/{session_id}/model")
    def load_model(session_id: str, body: LoadModelBody) -> dict[str, Any]:
        session = state.sessions.get(session_id)
        with session.lock:
            if session.writer_job_id:
                raise ConflictError("Session is busy with a continuation job")
        path = allowed_file(Path(body.path), state.allowed_paths())
        loaded = state.policy.load(path, device=body.device or config.device)
        with session.lock:
            if session.writer_job_id:
                raise ConflictError("Session is busy with a continuation job")
            session.model = loaded
            session.score_cache.clear()
        return {"model": {"path": loaded.path, "fingerprint": loaded.fingerprint, "architecture": loaded.architecture}}

    @app.get("/")
    def index() -> FileResponse:
        return FileResponse(STATIC_DIR / "index.html")

    app.mount("/static", StaticFiles(directory=STATIC_DIR), name="static")

    return app


def _origin_forbidden(origin: str, host: str, allowed_hosts: tuple[str, ...]) -> bool:
    parsed = urlparse(origin)
    if parsed.scheme not in ("http", "https") or parsed.hostname is None:
        return True
    if parsed.username or parsed.password:
        return True
    if parsed.path not in ("", "/") or parsed.params or parsed.query or parsed.fragment:
        return True
    if parsed.hostname.lower() not in allowed_hosts:
        return True
    return parsed.netloc.lower() != host.lower()


def safe_bind_host(host: str) -> bool:
    """Keep wildcard/LAN/public binds disabled; tailnet access is opt-in."""
    if host in ("127.0.0.1", "localhost"):
        return True
    try:
        return ipaddress.ip_address(host) in ipaddress.ip_network("100.64.0.0/10")
    except ValueError:
        return False


def valid_allowed_host(host: str) -> bool:
    return bool(len(host) <= 253 and all(
        re.fullmatch(r"[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?", label)
        for label in host.split(".")
    ))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Simulator-only combat explorer")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--allowed-host", action="append", default=[],
                        help="Additional exact hostname (no scheme/port), e.g. sorry.tail76d105.ts.net")
    parser.add_argument("--validation-manifest", type=Path)
    parser.add_argument("--distributions", type=Path)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--sessions-dir", type=Path, default=Path("combat_explorer_sessions"))
    parser.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if not safe_bind_host(args.host):
        parser.error("Bind to localhost, 127.0.0.1, or an explicit Tailscale IPv4 address (100.64.0.0/10)")
    for hostname in args.allowed_host:
        if not valid_allowed_host(hostname):
            parser.error("--allowed-host must be an exact hostname without scheme, port, path, or wildcard")
    config = AppConfig(
        validation_manifest=args.validation_manifest,
        distributions=args.distributions,
        sessions_dir=args.sessions_dir,
        checkpoint=args.checkpoint,
        host=args.host,
        port=args.port,
        device=args.device,
        allowed_hosts=tuple(dict.fromkeys(("127.0.0.1", "localhost", args.host, *(h.lower() for h in args.allowed_host)))),
    )
    import uvicorn

    app = create_app(config)
    uvicorn.run(app, host=config.host, port=config.port, log_level="info")
    return 0
