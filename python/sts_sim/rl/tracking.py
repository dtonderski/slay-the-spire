"""Optional offline Weights & Biases tracking for beam-clone training."""

from __future__ import annotations

import os
import random
import subprocess
import sys
import warnings
from collections.abc import Iterator, Mapping
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol, cast
from urllib.parse import urlparse

import numpy as np
import torch

_WANDB_ENV_KEYS = ("WANDB_MODE", "WANDB_SILENT")
_WANDB_SYNC_ENV_KEYS = ("WANDB_MODE", "WANDB_SILENT", "WANDB_BASE_URL")
MUTABLE_SYNCHRONIZATION_DIRECTORY_NAME = "wandb"
DEFAULT_LOCAL_WANDB_BASE_URL = "http://localhost:8080"
_LOCAL_WANDB_HOSTS = frozenset({"localhost", "127.0.0.1", "::1"})


class _WandbSettingsFactory(Protocol):
    def __call__(self, **kwargs: object) -> object: ...


class _WandbRun(Protocol):
    summary: dict[str, object]


class _WandbModule(Protocol):
    Settings: _WandbSettingsFactory

    def init(self, **kwargs: object) -> _WandbRun: ...

    def log(self, data: Mapping[str, object], step: int | None = None) -> None: ...

    def finish(self) -> None: ...


def default_offline_wandb_directory() -> Path:
    """Observational W&B directory. Never part of scientific artifact identity."""

    return Path(__file__).resolve().parents[3] / "target" / "wandb"


def is_wandb_synchronization_path(relative: Path) -> bool:
    """Return whether a path is mutable W&B metadata rather than a scientific artifact."""

    if relative.is_absolute():
        raise ValueError("W&B synchronization paths must be relative")
    return MUTABLE_SYNCHRONIZATION_DIRECTORY_NAME in relative.parts or relative.name.endswith(
        (".wandb", ".wandb.syncstate")
    )


def validate_local_wandb_base_url(base_url: str) -> str:
    if type(base_url) is not str or not base_url:
        raise ValueError("W&B base URL must be a nonempty string")
    parsed = urlparse(base_url)
    if parsed.scheme not in {"http", "https"}:
        raise ValueError("local W&B base URL must be http or https")
    host = (parsed.hostname or "").lower()
    if host not in _LOCAL_WANDB_HOSTS:
        raise ValueError("W&B sync is restricted to a localhost instance")
    if parsed.path not in {"", "/"}:
        raise ValueError("local W&B base URL must not include a path")
    return base_url


def discover_offline_wandb_runs(directory: Path) -> tuple[Path, ...]:
    """Find offline W&B run directories without walking scientific artifact roles."""

    if not directory.is_dir():
        raise FileNotFoundError(f"W&B directory does not exist: {directory}")
    runs: list[Path] = []
    for path in directory.rglob("offline-run-*"):
        if not path.is_dir():
            continue
        if MUTABLE_SYNCHRONIZATION_DIRECTORY_NAME not in path.parts:
            continue
        if not any(
            child.is_file() and (child.suffix == ".wandb" or child.name.endswith(".wandb"))
            for child in path.iterdir()
        ):
            continue
        runs.append(path)
    return tuple(sorted(set(runs), key=lambda path: path.as_posix()))


def sync_offline_wandb(
    directory: Path,
    *,
    base_url: str = DEFAULT_LOCAL_WANDB_BASE_URL,
) -> dict[str, object]:
    """Sync offline W&B runs to a local instance. Never writes scientific artifacts."""

    base_url = validate_local_wandb_base_url(base_url)
    runs = discover_offline_wandb_runs(directory)
    wandb = _import_wandb()
    previous_env = _apply_sync_env(base_url)
    try:
        with _isolated_rng():
            for run_dir in runs:
                _invoke_wandb_sync(wandb, run_dir, base_url)
    finally:
        _restore_env(previous_env)
    return {
        "kind": "offline_wandb_sync",
        "base_url": base_url,
        "run_count": len(runs),
        "runs": [str(path) for path in runs],
    }


@dataclass(frozen=True, slots=True)
class OfflineWandbConfig:
    project: str
    directory: Path
    run_name: str | None = None

    def __post_init__(self) -> None:
        if type(self.project) is not str or not self.project:
            raise ValueError("W&B project must be a nonempty string")
        if self.run_name is not None and (type(self.run_name) is not str or not self.run_name):
            raise ValueError("W&B run name must be a nonempty string when provided")
        if not isinstance(self.directory, Path):
            raise TypeError("W&B directory must be a path")


class OfflineWandbSession:
    def __init__(
        self,
        wandb_module: _WandbModule,
        run: _WandbRun,
        previous_env: Mapping[str, str | None],
    ) -> None:
        self._wandb = wandb_module
        self._run = run
        self._previous_env = dict(previous_env)
        self._finished = False
        self._logging_enabled = True

    @property
    def active(self) -> bool:
        return self._logging_enabled and not self._finished

    def disable_logging(self) -> None:
        self._logging_enabled = False

    def log_scalars(self, metrics: Mapping[str, object], *, step: int) -> None:
        _reject_tensors(metrics)
        payload: dict[str, float | int] = {}
        for key, value in metrics.items():
            if type(value) is not float and type(value) is not int:
                raise TypeError("offline W&B scalar logs must be int or float")
            payload[key] = value
        with _isolated_rng():
            self._wandb.log(payload, step=step)

    def log_summary(self, values: Mapping[str, object]) -> None:
        _reject_tensors(values)
        with _isolated_rng():
            for key, value in values.items():
                self._run.summary[key] = value

    def finish(self) -> None:
        if self._finished:
            return
        self._finished = True
        try:
            with _isolated_rng():
                self._wandb.finish()
        finally:
            _restore_env(self._previous_env)


def start_offline_wandb(
    config: OfflineWandbConfig,
    run_config: Mapping[str, object],
) -> OfflineWandbSession:
    """Start an offline-only W&B run. Never uploads, syncs, or logs tensors."""

    _reject_tensors(run_config)
    wandb = _import_wandb()
    config.directory.mkdir(parents=True, exist_ok=True)
    previous_env = _apply_offline_env()
    run: _WandbRun | None = None
    try:
        with _isolated_rng():
            settings = wandb.Settings(mode="offline")
            run = wandb.init(
                project=config.project,
                name=config.run_name,
                dir=str(config.directory),
                mode="offline",
                config=dict(run_config),
                settings=settings,
                reinit=True,
                save_code=False,
            )
        if run is None:
            raise RuntimeError("wandb.init returned no run")
        return OfflineWandbSession(wandb, run, previous_env)
    except Exception:
        try:
            if run is not None:
                with _isolated_rng():
                    wandb.finish()
        except Exception as cleanup_error:  # noqa: BLE001
            warnings.warn(
                f"offline W&B init cleanup finish failed: {cleanup_error}",
                RuntimeWarning,
                stacklevel=2,
            )
        finally:
            _restore_env(previous_env)
        raise


def _import_wandb() -> _WandbModule:
    try:
        import wandb
    except ImportError as error:
        raise RuntimeError(
            "offline W&B tracking requires the optional extra: uv sync --extra tracking"
        ) from error
    return cast(_WandbModule, wandb)


def _reject_tensors(payload: object) -> None:
    if isinstance(payload, torch.Tensor):
        raise TypeError("offline W&B tracking refuses tensors")
    if isinstance(payload, Mapping):
        for value in payload.values():
            _reject_tensors(value)
        return
    if isinstance(payload, (list, tuple)):
        for value in payload:
            _reject_tensors(value)


class _RngSnapshot:
    __slots__ = ("numpy", "python", "torch")

    def __init__(self) -> None:
        self.python = random.getstate()
        self.numpy = np.random.get_state()
        self.torch = torch.get_rng_state()

    def restore(self) -> None:
        random.setstate(self.python)
        np.random.set_state(self.numpy)
        torch.set_rng_state(self.torch)


@contextmanager
def _isolated_rng() -> Iterator[None]:
    snapshot = _RngSnapshot()
    try:
        yield
    finally:
        snapshot.restore()


def _apply_offline_env() -> dict[str, str | None]:
    previous = {key: os.environ.get(key) for key in _WANDB_ENV_KEYS}
    os.environ["WANDB_MODE"] = "offline"
    os.environ["WANDB_SILENT"] = "true"
    return previous


def _apply_sync_env(base_url: str) -> dict[str, str | None]:
    previous = {key: os.environ.get(key) for key in _WANDB_SYNC_ENV_KEYS}
    os.environ["WANDB_BASE_URL"] = base_url
    os.environ["WANDB_SILENT"] = "true"
    os.environ.pop("WANDB_MODE", None)
    return previous


def _invoke_wandb_sync(wandb_module: _WandbModule, run_dir: Path, base_url: str) -> None:
    del base_url
    sync = getattr(wandb_module, "sync", None)
    if callable(sync):
        sync(str(run_dir))
        return
    completed = subprocess.run(
        [sys.executable, "-m", "wandb", "sync", str(run_dir)],
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        stderr = completed.stderr.decode("utf-8", errors="replace")
        raise RuntimeError(f"offline W&B sync failed for {run_dir}: {stderr}")


def _restore_env(previous: Mapping[str, str | None]) -> None:
    for key, value in previous.items():
        if value is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = value
