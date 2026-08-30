from __future__ import annotations

import builtins
import random
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import cast

import numpy as np
import pytest
import torch

from sts_sim.rl.cli import train_main
from sts_sim.rl.data import generate_beam_dataset, generate_legal_roots
from sts_sim.rl.tracking import OfflineWandbConfig, start_offline_wandb
from sts_sim.rl.training import TrainingConfig, TrainingResult, train_beam_clone


class _FakeSettings:
    def __init__(self, **kwargs: object) -> None:
        self.kwargs = kwargs


class _FakeRun:
    def __init__(self, config: Mapping[str, object]) -> None:
        self.config = dict(config)
        self.summary: dict[str, object] = {}


class _FakeWandb:
    def __init__(self) -> None:
        self.Settings = _FakeSettings
        self.init_calls: list[dict[str, object]] = []
        self.logs: list[tuple[dict[str, object], int | None]] = []
        self.finish_calls = 0
        self.run: _FakeRun | None = None
        self.fail_log = False
        self.network_calls = 0

    def init(self, **kwargs: object) -> _FakeRun:
        if kwargs.get("mode") != "offline":
            raise AssertionError("W&B tests require mode=offline")
        self.init_calls.append(dict(kwargs))
        self.run = _FakeRun(cast(Mapping[str, object], kwargs.get("config") or {}))
        return self.run

    def log(self, data: Mapping[str, object], step: int | None = None) -> None:
        if self.fail_log:
            raise RuntimeError("injected wandb.log failure")
        self.logs.append((dict(data), step))

    def finish(self) -> None:
        self.finish_calls += 1

    def sync(self, *_args: object, **_kwargs: object) -> None:
        self.network_calls += 1
        raise AssertionError("offline tracking must not sync")


def _install_fake_wandb(monkeypatch: pytest.MonkeyPatch) -> _FakeWandb:
    fake = _FakeWandb()
    monkeypatch.setattr("sts_sim.rl.tracking._import_wandb", lambda: fake)
    return fake


def _hide_wandb_module(monkeypatch: pytest.MonkeyPatch) -> None:
    real_import = builtins.__import__

    def fake_import(
        name: str,
        globals: Mapping[str, object] | None = None,
        locals: Mapping[str, object] | None = None,
        fromlist: tuple[str, ...] = (),
        level: int = 0,
    ) -> object:
        if name == "wandb" or name.startswith("wandb."):
            raise ImportError("No module named 'wandb'")
        return real_import(name, globals, locals, fromlist, level)

    monkeypatch.setattr(builtins, "__import__", fake_import)
    monkeypatch.delitem(sys.modules, "wandb", raising=False)


def _smoke_training_config() -> TrainingConfig:
    return TrainingConfig(
        batch_size=2,
        total_steps=1,
        model_width=16,
        model_heads=4,
        model_layers=1,
        feedforward_width=32,
        minimum_roots=1,
        minimum_lineages=1,
    )


def _tiny_train_manifest(tmp_path: Path) -> Path:
    generate_legal_roots(tmp_path / "roots", ["BEAMCLONE0"], max_run_steps=128)
    generate_beam_dataset(
        tmp_path / "roots/root-manifest.json",
        tmp_path / "train",
        split="train",
        depth=2,
        width=4,
        transition_budget=100,
        max_decisions=8,
        max_player_turns=3,
    )
    return tmp_path / "train/dataset-manifest.json"


def test_missing_tracking_extra_fails_closed(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _hide_wandb_module(monkeypatch)
    with pytest.raises(RuntimeError, match="uv sync --extra tracking"):
        start_offline_wandb(
            OfflineWandbConfig(project="sts-combat", directory=tmp_path / "wandb"),
            {"trainer": "beam_clone"},
        )


def test_offline_session_init_config_log_finish(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    fake = _install_fake_wandb(monkeypatch)
    session = start_offline_wandb(
        OfflineWandbConfig(project="demo", directory=tmp_path / "wandb", run_name="run-1"),
        {"trainer": "beam_clone", "resume_starting_step": 3},
    )
    session.log_scalars({"loss": 0.5}, step=4)
    session.log_summary({"checkpoint_path": "ckpt.pt", "global_step": 4})
    session.finish()
    assert len(fake.init_calls) == 1
    init = fake.init_calls[0]
    assert init["project"] == "demo"
    assert init["name"] == "run-1"
    assert init["mode"] == "offline"
    assert init["dir"] == str(tmp_path / "wandb")
    assert cast(_FakeSettings, init["settings"]).kwargs["mode"] == "offline"
    assert fake.logs == [({"loss": 0.5}, 4)]
    assert fake.run is not None
    assert fake.run.config["resume_starting_step"] == 3
    assert fake.run.summary["checkpoint_path"] == "ckpt.pt"
    assert fake.finish_calls == 1
    assert fake.network_calls == 0


def test_offline_session_finishes_on_log_failure(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    fake = _install_fake_wandb(monkeypatch)
    fake.fail_log = True
    session = start_offline_wandb(
        OfflineWandbConfig(project="demo", directory=tmp_path / "wandb"),
        {"trainer": "beam_clone"},
    )
    with pytest.raises(RuntimeError, match="injected wandb.log failure"):
        session.log_scalars({"loss": 1.0}, step=1)
    session.finish()
    assert fake.finish_calls == 1


def test_offline_session_refuses_tensors(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _install_fake_wandb(monkeypatch)
    session = start_offline_wandb(
        OfflineWandbConfig(project="demo", directory=tmp_path / "wandb"),
        {"trainer": "beam_clone"},
    )
    with pytest.raises(TypeError, match="refuses tensors"):
        session.log_scalars({"loss": torch.tensor(1.0)}, step=1)


def test_wandb_calls_restore_python_numpy_torch_rng(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    class _NoisyWandb(_FakeWandb):
        def init(self, **kwargs: object) -> _FakeRun:
            random.random()
            np.random.random()
            torch.rand(1)
            return super().init(**kwargs)

    fake = _NoisyWandb()
    monkeypatch.setattr("sts_sim.rl.tracking._import_wandb", lambda: fake)
    random.seed(11)
    np.random.seed(11)
    torch.manual_seed(11)
    before_python = random.getstate()
    before_numpy = np.random.get_state()
    before_torch = torch.get_rng_state().clone()
    start_offline_wandb(
        OfflineWandbConfig(project="demo", directory=tmp_path / "wandb"),
        {"trainer": "beam_clone"},
    )
    assert random.getstate() == before_python
    after_numpy = np.random.get_state()
    assert after_numpy[0] == before_numpy[0]
    np.testing.assert_array_equal(after_numpy[1], before_numpy[1])
    assert torch.equal(torch.get_rng_state(), before_torch)


def test_cli_wandb_offline_flags(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    captured: dict[str, object] = {}

    def fake_train(
        dataset: Path,
        checkpoint: Path,
        config: TrainingConfig,
        *,
        resume: bool = False,
        stop_after_steps: int | None = None,
        wandb_offline: OfflineWandbConfig | None = None,
    ) -> TrainingResult:
        del dataset, config, stop_after_steps
        captured["resume"] = resume
        captured["wandb_offline"] = wandb_offline
        return TrainingResult(checkpoint, 1, ({"step": 1, "loss": 0.0},), "a" * 64, "b" * 64, "c" * 64)

    monkeypatch.setattr("sts_sim.rl.cli.train_beam_clone", fake_train)
    dataset = tmp_path / "dataset-manifest.json"
    checkpoint = tmp_path / "checkpoint.pt"
    common = [
        "--dataset",
        str(dataset),
        "--checkpoint",
        str(checkpoint),
        "--minimum-roots",
        "1",
        "--minimum-lineages",
        "1",
    ]
    train_main(common)
    assert captured["wandb_offline"] is None
    wandb_dir = tmp_path / "offline-wandb"
    train_main(
        [
            *common,
            "--wandb-offline",
            "--wandb-project",
            "demo",
            "--wandb-run-name",
            "run-1",
            "--wandb-dir",
            str(wandb_dir),
        ]
    )
    config = captured["wandb_offline"]
    assert isinstance(config, OfflineWandbConfig)
    assert config.project == "demo"
    assert config.run_name == "run-1"
    assert config.directory == wandb_dir


def test_cli_missing_tracking_extra_fails_closed(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    _hide_wandb_module(monkeypatch)
    manifest = _tiny_train_manifest(tmp_path)
    with pytest.raises(RuntimeError, match="uv sync --extra tracking"):
        train_main(
            [
                "--dataset",
                str(manifest),
                "--checkpoint",
                str(tmp_path / "checkpoint.pt"),
                "--steps",
                "1",
                "--batch-size",
                "2",
                "--model-width",
                "16",
                "--model-heads",
                "4",
                "--model-layers",
                "1",
                "--feedforward-width",
                "32",
                "--minimum-roots",
                "1",
                "--minimum-lineages",
                "1",
                "--wandb-offline",
                "--wandb-dir",
                str(tmp_path / "wandb"),
            ]
        )


def test_training_failure_still_finishes_wandb(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    fake = _install_fake_wandb(monkeypatch)
    manifest = _tiny_train_manifest(tmp_path)

    def boom(*_args: object, **_kwargs: object) -> torch.Tensor:
        raise RuntimeError("injected training failure")

    monkeypatch.setattr("sts_sim.rl.training.policy_value_loss", boom)
    with pytest.raises(RuntimeError, match="injected training failure"):
        train_beam_clone(
            manifest,
            tmp_path / "checkpoint.pt",
            _smoke_training_config(),
            wandb_offline=OfflineWandbConfig(project="demo", directory=tmp_path / "wandb"),
        )
    assert fake.finish_calls == 1
    assert fake.init_calls


def test_tracked_and_untracked_training_are_byte_identical(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    fake = _install_fake_wandb(monkeypatch)
    manifest = _tiny_train_manifest(tmp_path)
    config = _smoke_training_config()
    untracked = tmp_path / "untracked.pt"
    tracked = tmp_path / "tracked.pt"
    train_beam_clone(manifest, untracked, config)
    train_beam_clone(
        manifest,
        tracked,
        config,
        wandb_offline=OfflineWandbConfig(project="demo", directory=tmp_path / "wandb"),
    )
    left = torch.load(untracked, map_location="cpu", weights_only=False)
    right = torch.load(tracked, map_location="cpu", weights_only=False)
    for key in ("model_state", "optimizer_state", "scheduler_state", "global_step", "cursor"):
        _assert_nested_equal(left[key], right[key])
    assert torch.equal(left["torch_rng_state"], right["torch_rng_state"])
    assert fake.finish_calls == 1
    assert fake.logs
    assert fake.logs[0][0] == {"loss": fake.logs[0][0]["loss"]}
    assert fake.run is not None
    assert fake.run.config["trainer"] == "beam_clone"
    assert fake.run.config["puct_targets_in_training"] is False
    assert fake.run.config["resume_starting_step"] == 0
    assert fake.run.summary["checkpoint_path"] == str(tracked)
    assert type(fake.run.summary["checkpoint_digest"]) is str
    assert type(fake.run.summary["model_state_digest"]) is str
    assert fake.network_calls == 0


def _assert_nested_equal(left: object, right: object) -> None:
    if isinstance(left, torch.Tensor):
        assert isinstance(right, torch.Tensor) and torch.equal(left, right)
    elif isinstance(left, dict):
        assert isinstance(right, dict)
        left_map = cast(dict[object, object], left)
        right_map = cast(dict[object, object], right)
        assert left_map.keys() == right_map.keys()
        for key in left_map:
            _assert_nested_equal(left_map[key], right_map[key])
    elif isinstance(left, (list, tuple)):
        assert isinstance(right, type(left))
        left_items = cast(list[object] | tuple[object, ...], left)
        right_items = cast(list[object] | tuple[object, ...], right)
        assert len(left_items) == len(right_items)
        for a, b in zip(left_items, right_items, strict=True):
            _assert_nested_equal(a, b)
    else:
        assert left == right
