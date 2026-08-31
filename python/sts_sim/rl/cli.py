"""Command-line entry points for the beam-cloning vertical slice."""

from __future__ import annotations

import argparse
import json
from collections.abc import Sequence
from pathlib import Path

from .data import generate_beam_dataset, generate_legal_roots
from .gameplay import evaluate_matched_gameplay, evaluate_matched_puct_gameplay
from .puct_data import generate_puct_dataset
from .tracking import OfflineWandbConfig, default_offline_wandb_directory
from .training import TrainingConfig, evaluate_beam_clone, train_beam_clone


def _data_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="sts-combat-data")
    subcommands = parser.add_subparsers(dest="command", required=True)
    roots = subcommands.add_parser("roots")
    roots.add_argument("--output", type=Path, required=True)
    roots.add_argument("--seed-prefix", default="BEAMCLONE")
    roots.add_argument("--start", type=int, default=0)
    roots.add_argument("--count", type=int, required=True)
    roots.add_argument("--ascension", type=int, default=0)
    roots.add_argument("--max-run-steps", type=int, default=256)
    roots.add_argument(
        "--combat-depth",
        type=int,
        default=1,
        help="1-based combat index to capture; 1 is the first combat",
    )
    roots.add_argument(
        "--materialize-audited-splits",
        action="store_true",
        help="explicitly write split-isolated sealed/audit root snapshots",
    )
    label = subcommands.add_parser("label")
    label.add_argument("--roots", type=Path, required=True)
    label.add_argument("--output", type=Path, required=True)
    label.add_argument("--split", default="train")
    label.add_argument("--depth", type=int, default=8)
    label.add_argument("--width", type=int, default=24)
    label.add_argument("--transition-budget", type=int, default=5_000)
    label.add_argument("--max-decisions", type=int, default=512)
    label.add_argument("--max-player-turns", type=int, default=100)
    label.add_argument(
        "--deduplicate-search-states", action=argparse.BooleanOptionalAction, default=True
    )
    label.add_argument("--allow-audited-split", action="store_true")
    puct_label = subcommands.add_parser("puct-label")
    puct_label.add_argument("--roots", type=Path, required=True)
    puct_label.add_argument("--output", type=Path, required=True)
    puct_label.add_argument("--checkpoint", type=Path, required=True)
    puct_label.add_argument("--split", default="train")
    puct_label.add_argument("--c-puct", type=float, default=1.5)
    puct_label.add_argument("--simulation-budget", type=int, default=64)
    puct_label.add_argument("--transition-budget", type=int, default=64)
    puct_label.add_argument("--max-decisions", type=int, default=512)
    puct_label.add_argument("--max-player-turns", type=int, default=100)
    puct_label.add_argument("--allow-audited-split", action="store_true")
    return parser


def data_main(argv: Sequence[str] | None = None) -> int:
    args = _data_parser().parse_args(argv)
    if args.command == "roots":
        if args.count <= 0:
            raise ValueError("count must be positive")
        seeds = [
            f"{args.seed_prefix}{index}" for index in range(args.start, args.start + args.count)
        ]
        manifest = generate_legal_roots(
            args.output,
            seeds,
            ascension=args.ascension,
            max_run_steps=args.max_run_steps,
            combat_depth=args.combat_depth,
            materialize_audited_splits=args.materialize_audited_splits,
        )
    elif args.command == "puct-label":
        manifest = generate_puct_dataset(
            args.roots,
            args.output,
            args.checkpoint,
            split=args.split,
            c_puct=args.c_puct,
            simulation_budget=args.simulation_budget,
            transition_budget=args.transition_budget,
            max_decisions=args.max_decisions,
            max_player_turns=args.max_player_turns,
            allow_audited_split=args.allow_audited_split,
        )
    else:
        manifest = generate_beam_dataset(
            args.roots,
            args.output,
            split=args.split,
            depth=args.depth,
            width=args.width,
            transition_budget=args.transition_budget,
            max_decisions=args.max_decisions,
            max_player_turns=args.max_player_turns,
            deduplicate_search_states=args.deduplicate_search_states,
            allow_audited_split=args.allow_audited_split,
        )
    print(json.dumps(manifest.to_dict(), sort_keys=True))
    return 0


def _training_config(args: argparse.Namespace) -> TrainingConfig:
    return TrainingConfig(
        seed=args.seed,
        batch_size=args.batch_size,
        total_steps=args.steps,
        learning_rate=args.learning_rate,
        weight_decay=args.weight_decay,
        torch_threads=args.torch_threads,
        minimum_roots=args.minimum_roots,
        minimum_lineages=args.minimum_lineages,
        model_width=args.model_width,
        model_heads=args.model_heads,
        model_layers=args.model_layers,
        feedforward_width=args.feedforward_width,
    )


def train_main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="sts-combat-train")
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--batch-size", type=int, default=32)
    parser.add_argument("--steps", type=int, default=100)
    parser.add_argument("--learning-rate", type=float, default=1e-3)
    parser.add_argument("--weight-decay", type=float, default=1e-4)
    parser.add_argument("--torch-threads", type=int, default=1)
    parser.add_argument(
        "--minimum-roots",
        type=int,
        default=225,
        help="versioned training corpus floor (default: 225; lower only for explicit tests/smoke)",
    )
    parser.add_argument(
        "--minimum-lineages",
        type=int,
        default=100,
        help="versioned lineage floor (default: 100; lower only for explicit tests/smoke)",
    )
    parser.add_argument("--model-width", type=int, default=96)
    parser.add_argument("--model-heads", type=int, default=4)
    parser.add_argument("--model-layers", type=int, default=2)
    parser.add_argument("--feedforward-width", type=int, default=192)
    parser.add_argument(
        "--wandb-offline",
        action="store_true",
        help="opt-in offline W&B tracking; requires the tracking extra and never uploads",
    )
    parser.add_argument("--wandb-project", default="sts-combat")
    parser.add_argument("--wandb-run-name", default=None)
    parser.add_argument(
        "--wandb-dir",
        type=Path,
        default=None,
        help="offline W&B directory (default: <repo>/target/wandb)",
    )
    args = parser.parse_args(argv)
    wandb_offline = None
    if args.wandb_offline:
        wandb_offline = OfflineWandbConfig(
            project=args.wandb_project,
            directory=args.wandb_dir or default_offline_wandb_directory(),
            run_name=args.wandb_run_name,
        )
    result = train_beam_clone(
        args.dataset,
        args.checkpoint,
        _training_config(args),
        resume=args.resume,
        wandb_offline=wandb_offline,
    )
    print(
        json.dumps(
            {
                "checkpoint": str(result.checkpoint_path),
                "global_step": result.global_step,
                "runtime_identity_digest": result.runtime_identity_digest,
                "vocabulary_fingerprint": result.vocabulary_fingerprint,
                "encoder_contract_digest": result.encoder_contract_digest,
                "metrics": list(result.metrics),
            },
            sort_keys=True,
        )
    )
    return 0


def evaluate_main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="sts-combat-evaluate")
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--split", default="development")
    parser.add_argument("--allow-audited-split", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    report = evaluate_beam_clone(
        args.dataset,
        args.checkpoint,
        split=args.split,
        allow_audited_split=args.allow_audited_split,
    )
    content = json.dumps(report, sort_keys=True, separators=(",", ":"), allow_nan=False)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(content, encoding="utf-8")
    print(content)
    return 0


def puct_rollout_main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="sts-combat-puct-rollout")
    parser.add_argument("--roots", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--split", default="development")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--allow-audited-split", action="store_true")
    parser.add_argument("--c-puct", type=float, default=1.5)
    parser.add_argument("--simulation-budget", type=int, default=64)
    parser.add_argument("--transition-budget", type=int, default=64)
    parser.add_argument("--beam-depth", type=int, default=8)
    parser.add_argument("--beam-width", type=int, default=24)
    parser.add_argument("--max-decisions", type=int, default=512)
    parser.add_argument("--max-player-turns", type=int, default=100)
    parser.add_argument(
        "--deduplicate-search-states", action=argparse.BooleanOptionalAction, default=True
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    report = evaluate_matched_puct_gameplay(
        args.roots,
        args.checkpoint,
        split=args.split,
        evaluation_seed=args.seed,
        allow_audited_split=args.allow_audited_split,
        c_puct=args.c_puct,
        simulation_budget=args.simulation_budget,
        transition_budget=args.transition_budget,
        beam_depth=args.beam_depth,
        beam_width=args.beam_width,
        max_decisions=args.max_decisions,
        max_player_turns=args.max_player_turns,
        deduplicate_search_states=args.deduplicate_search_states,
    )
    content = json.dumps(report, sort_keys=True, separators=(",", ":"), allow_nan=False)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(content, encoding="utf-8")
    print(content)
    return 0


def rollout_main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="sts-combat-rollout")
    parser.add_argument("--roots", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--split", default="development")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--allow-audited-split", action="store_true")
    parser.add_argument("--depth", type=int, default=8)
    parser.add_argument("--width", type=int, default=24)
    parser.add_argument("--transition-budget", type=int, default=5_000)
    parser.add_argument("--max-decisions", type=int, default=512)
    parser.add_argument("--max-player-turns", type=int, default=100)
    parser.add_argument(
        "--deduplicate-search-states", action=argparse.BooleanOptionalAction, default=True
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    report = evaluate_matched_gameplay(
        args.roots,
        args.checkpoint,
        split=args.split,
        evaluation_seed=args.seed,
        allow_audited_split=args.allow_audited_split,
        depth=args.depth,
        width=args.width,
        transition_budget=args.transition_budget,
        max_decisions=args.max_decisions,
        max_player_turns=args.max_player_turns,
        deduplicate_search_states=args.deduplicate_search_states,
    )
    content = json.dumps(report, sort_keys=True, separators=(",", ":"), allow_nan=False)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(content, encoding="utf-8")
    print(content)
    return 0
