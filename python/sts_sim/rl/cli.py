"""Command-line entry points for the beam-cloning vertical slice."""

from __future__ import annotations

import argparse
import json
from collections.abc import Sequence
from pathlib import Path

from .data import generate_beam_dataset, generate_legal_roots
from .diagnostics import calibrate_combat_proxy_win_loss
from .experiment import (
    ArtifactIntegrityError,
    ExperimentReproductionError,
    _read_regular_file_bytes,
    reproduce_experiment,
    verify_artifact_integrity,
    write_scientific_artifact,
)
from .gameplay import (
    DEFAULT_MATCHED_PUCT_MAX_DECISIONS,
    DEFAULT_MATCHED_PUCT_MAX_PLAYER_TURNS,
    evaluate_matched_puct_gameplay,
)
from .label_ab import (
    _bind_live_source,
    identity_verify_experiment,
    load_label_ab_plan,
    rerun_label_ab_experiment,
    run_label_ab_experiment,
    verify_label_ab_rerun,
    write_label_ab_plan,
)
from .provenance import canonical_bytes
from .puct_data import generate_puct_dataset
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
    args = parser.parse_args(argv)
    result = train_beam_clone(
        args.dataset,
        args.checkpoint,
        _training_config(args),
        resume=args.resume,
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
    parser.add_argument("--evaluation-seed", type=int, default=0)
    parser.add_argument("--authorization", type=Path, default=None)
    parser.add_argument("--training-roots", type=Path, default=None)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    report = evaluate_beam_clone(
        args.dataset,
        args.checkpoint,
        split=args.split,
        evaluation_seed=args.evaluation_seed,
        authorization_path=args.authorization,
        training_root_manifest_path=args.training_roots,
    )
    if args.output is not None:
        write_scientific_artifact(args.output, canonical_bytes(report))
    print(json.dumps(report, sort_keys=True, separators=(",", ":"), allow_nan=False))
    return 0


def puct_rollout_main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="sts-combat-puct-rollout")
    parser.add_argument("--roots", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path, required=True)
    parser.add_argument("--split", default="development")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--authorization", type=Path, default=None)
    parser.add_argument("--training-roots", type=Path, default=None)
    parser.add_argument("--c-puct", type=float, default=1.5)
    parser.add_argument("--simulation-budget", type=int, default=64)
    parser.add_argument("--transition-budget", type=int, default=64)
    parser.add_argument("--beam-depth", type=int, default=8)
    parser.add_argument("--beam-width", type=int, default=24)
    parser.add_argument("--max-decisions", type=int, default=DEFAULT_MATCHED_PUCT_MAX_DECISIONS)
    parser.add_argument(
        "--max-player-turns", type=int, default=DEFAULT_MATCHED_PUCT_MAX_PLAYER_TURNS
    )
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
        authorization_path=args.authorization,
        training_root_manifest_path=args.training_roots,
        c_puct=args.c_puct,
        simulation_budget=args.simulation_budget,
        transition_budget=args.transition_budget,
        beam_depth=args.beam_depth,
        beam_width=args.beam_width,
        max_decisions=args.max_decisions,
        max_player_turns=args.max_player_turns,
        deduplicate_search_states=args.deduplicate_search_states,
    )
    if args.output is not None:
        write_scientific_artifact(args.output, canonical_bytes(report))
    print(json.dumps(report, sort_keys=True, separators=(",", ":"), allow_nan=False))
    return 0


def experiment_main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="sts-combat-experiment")
    subcommands = parser.add_subparsers(dest="command", required=True)
    verify = subcommands.add_parser("verify")
    verify.add_argument("--experiment-dir", type=Path, required=True)
    reproduce = subcommands.add_parser("reproduce")
    reproduce.add_argument("--experiment-dir", type=Path, required=True)
    reproduce.add_argument("--repository", type=Path, required=True)
    calibrate = subcommands.add_parser("calibrate")
    calibrate.add_argument("--static", type=Path, required=True)
    calibrate.add_argument("--gameplay", type=Path, default=None)
    calibrate.add_argument("--policy", default="network")
    calibrate.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)
    try:
        if args.command == "verify":
            report = verify_artifact_integrity(args.experiment_dir)
            payload: dict[str, object] = report.to_dict()
        elif args.command == "reproduce":
            payload = reproduce_experiment(
                args.experiment_dir,
                repository=args.repository,
            )
        else:
            static_bytes = _read_regular_file_bytes(args.static)
            static_report = json.loads(static_bytes)
            gameplay_report = None
            gameplay_bytes = None
            if args.gameplay is not None:
                gameplay_bytes = _read_regular_file_bytes(args.gameplay)
                gameplay_report = json.loads(gameplay_bytes)
            payload = calibrate_combat_proxy_win_loss(
                static_report=static_report,
                gameplay_report=gameplay_report,
                policy=args.policy,
                static_path=args.static,
                gameplay_path=args.gameplay,
                static_bytes=static_bytes,
                gameplay_bytes=gameplay_bytes,
            )
            if args.output is not None:
                write_scientific_artifact(args.output, canonical_bytes(payload))
    except ArtifactIntegrityError as error:
        print(json.dumps(error.report.to_dict(), sort_keys=True, allow_nan=False))
        return 1
    except ExperimentReproductionError as error:
        print(json.dumps({"ok": False, "error": str(error)}, sort_keys=True, allow_nan=False))
        return 1
    except (ValueError, TypeError) as error:
        print(json.dumps({"ok": False, "error": str(error)}, sort_keys=True, allow_nan=False))
        return 1
    print(json.dumps(payload, sort_keys=True, allow_nan=False))
    return 0


def label_ab_main(argv: Sequence[str] | None = None) -> int:
    """Production CLI for the current beam-vs-PUCT paired-state label A/B.

    Plans loaded from disk must encode the frozen production constants and bind
    ``source_commit`` to the current clean HEAD. Tiny knobs are not reachable
    from this CLI. ``run`` executes the full protocol. ``rerun`` re-executes
    into fresh unpublished directories and then compares designated bytes.
    ``identity-verify`` is identity verification: it does not retrain.
    """

    parser = argparse.ArgumentParser(prog="sts-combat-label-ab")
    subcommands = parser.add_subparsers(dest="command", required=True)
    write_plan = subcommands.add_parser("write-plan")
    write_plan.add_argument("--output", type=Path, required=True)
    write_plan.add_argument("--repository", type=Path, default=None)
    validate_plan = subcommands.add_parser("validate-plan")
    validate_plan.add_argument("--plan", type=Path, required=True)
    validate_plan.add_argument("--repository", type=Path, default=None)
    run = subcommands.add_parser("run")
    run.add_argument("--plan", type=Path, required=True)
    run.add_argument("--work-dir", type=Path, required=True)
    run.add_argument("--experiment-dir", type=Path, required=True)
    run.add_argument("--repository", type=Path, default=None)
    rerun = subcommands.add_parser("rerun")
    rerun.add_argument("--plan", type=Path, required=True)
    rerun.add_argument("--work-dir", type=Path, required=True)
    rerun.add_argument("--experiment-dir", type=Path, required=True)
    rerun.add_argument("--reference", type=Path, required=True)
    rerun.add_argument("--repository", type=Path, default=None)
    verify_rerun = subcommands.add_parser("verify-rerun")
    verify_rerun.add_argument("--reference", type=Path, required=True)
    verify_rerun.add_argument("--candidate", type=Path, required=True)
    identity = subcommands.add_parser("identity-verify")
    identity.add_argument("--experiment-dir", type=Path, required=True)
    identity.add_argument("--repository", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "write-plan":
            plan = write_label_ab_plan(args.output, repository=args.repository)
            payload: dict[str, object] = plan.to_dict()
        elif args.command == "validate-plan":
            plan = load_label_ab_plan(args.plan)
            _bind_live_source(plan, repository=args.repository)
            payload = plan.to_dict()
        elif args.command == "run":
            payload = run_label_ab_experiment(
                args.plan,
                args.work_dir,
                args.experiment_dir,
                repository=args.repository,
            )
        elif args.command == "rerun":
            payload = rerun_label_ab_experiment(
                args.plan,
                args.work_dir,
                args.experiment_dir,
                reference=args.reference,
                repository=args.repository,
            )
        elif args.command == "verify-rerun":
            payload = verify_label_ab_rerun(reference=args.reference, candidate=args.candidate)
        else:
            payload = identity_verify_experiment(args.experiment_dir, repository=args.repository)
    except ArtifactIntegrityError as error:
        print(json.dumps(error.report.to_dict(), sort_keys=True, allow_nan=False))
        return 1
    except ExperimentReproductionError as error:
        print(json.dumps({"ok": False, "error": str(error)}, sort_keys=True, allow_nan=False))
        return 1
    except (ValueError, TypeError) as error:
        print(json.dumps({"ok": False, "error": str(error)}, sort_keys=True, allow_nan=False))
        return 1
    print(json.dumps(payload, sort_keys=True, allow_nan=False))
    return 0
