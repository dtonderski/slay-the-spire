"""Optional offline Weights & Biases tracking for beam-clone training."""

from __future__ import annotations

import os
import random
import warnings
from collections.abc import Iterator, Mapping
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol, cast

import numpy as np
import torch

_WANDB_ENV_KEYS = ("WANDB_MODE", "WANDB_SILENT")


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
    return Path(__file__).resolve().parents[3] / "target" / "wandb"


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


def _restore_env(previous: Mapping[str, str | None]) -> None:
    for key, value in previous.items():
        if value is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = value
