"""Prepare ignored local artifacts for the simulator combat explorer.

Creates a current-native synthetic explorer-roots document from the existing
validation generator, plus byte-for-byte copies of the caller-supplied
distributions and checkpoint. This is local explorer data, not the frozen
validation-a0-v1 benchmark, and it makes no same-native training or real-game
parity claims.

From `rl/`:

    uv run python -m combat_explorer.prepare_local \\
      --distributions /path/to/loadout-a0-v3/fit.json \\
      --checkpoint /path/to/checkpoint.pt \\
      --output-dir combat_explorer_local
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from loadout_sampling import LoadoutSampler
from validation_set import build_validation, load_validation, native_sha256

ROOTS_NAME = "explorer-roots-a0-v1.json"
DISTRIBUTIONS_NAME = "fit.json"
CHECKPOINT_NAME = "checkpoint.pt"
ARTIFACTS_NAME = "ARTIFACTS.json"
LAUNCH_NAME = "LAUNCH.txt"

# Distinct from frozen validation-a0-v1 (generation_seed 20260918, 1024+32/stratum).
DEFAULT_SEED = 202609181
DEFAULT_MAIN_COUNT = 64
DEFAULT_STRESS_PER_STRATUM = 2


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def copy_bytes(source: Path, destination: Path) -> str:
    data = source.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    destination.write_bytes(data)
    if sha256_file(destination) != digest:
        raise RuntimeError(f"Copy of {source} failed SHA-256 verification")
    return digest


def _refuse_existing(path: Path) -> None:
    if path.exists():
        raise FileExistsError(f"Refusing to overwrite existing {path}")


def prepare_local(
    *,
    distributions: Path,
    output_dir: Path,
    checkpoint: Path | None = None,
    seed: int = DEFAULT_SEED,
    main_count: int = DEFAULT_MAIN_COUNT,
    stress_per_stratum: int = DEFAULT_STRESS_PER_STRATUM,
    eval_repeats: int = 1,
    eval_max_decisions: int = 512,
    verify_checkpoint_load: bool = True,
) -> dict[str, Any]:
    if not distributions.is_file():
        raise FileNotFoundError(f"Distributions file not found: {distributions}")
    if checkpoint is not None and not checkpoint.is_file():
        raise FileNotFoundError(f"Checkpoint file not found: {checkpoint}")
    if min(main_count, stress_per_stratum, eval_repeats, eval_max_decisions) < 1:
        raise ValueError("Dataset sizes and evaluation repeats must be positive")

    output_dir = output_dir.resolve()
    roots_path = output_dir / ROOTS_NAME
    dist_dest = output_dir / DISTRIBUTIONS_NAME
    checkpoint_dest = output_dir / CHECKPOINT_NAME if checkpoint is not None else None
    artifacts_path = output_dir / ARTIFACTS_NAME
    launch_path = output_dir / LAUNCH_NAME
    screenshots_dir = output_dir / "screenshots"
    for path in (roots_path, dist_dest, artifacts_path, launch_path):
        _refuse_existing(path)
    if checkpoint_dest is not None:
        _refuse_existing(checkpoint_dest)

    output_dir.mkdir(parents=True, exist_ok=True)
    screenshots_dir.mkdir(exist_ok=True)

    source_dist_sha256 = sha256_file(distributions)
    copied_dist_sha256 = copy_bytes(distributions, dist_dest)
    if copied_dist_sha256 != source_dist_sha256:
        raise RuntimeError("Distributions copy hash mismatch")

    checkpoint_info: dict[str, Any] | None = None
    if checkpoint is not None and checkpoint_dest is not None:
        source_ckpt_sha256 = sha256_file(checkpoint)
        copied_ckpt_sha256 = copy_bytes(checkpoint, checkpoint_dest)
        if copied_ckpt_sha256 != source_ckpt_sha256:
            raise RuntimeError("Checkpoint copy hash mismatch")
        checkpoint_info = {
            "source_path": str(checkpoint.resolve()),
            "local_path": str(checkpoint_dest),
            "sha256": copied_ckpt_sha256,
            "bytes": checkpoint_dest.stat().st_size,
            "claim": "usable inference on the current CombatValueModel architecture; not same-native training evidence",
        }
        if verify_checkpoint_load:
            from combat_explorer.policy import PolicyAdapter

            loaded = PolicyAdapter().load(checkpoint_dest, device="cpu")
            checkpoint_info["architecture"] = loaded.architecture
            checkpoint_info["fingerprint"] = loaded.fingerprint
            if loaded.fingerprint != copied_ckpt_sha256:
                raise RuntimeError("Loaded checkpoint fingerprint does not match copied bytes")

    sampler = LoadoutSampler.load(dist_dest)
    document = build_validation(
        sampler,
        seed,
        main_count,
        stress_per_stratum,
        eval_repeats,
        eval_max_decisions,
    )
    provenance = dict(document.get("provenance") or {})
    provenance.update(
        {
            "distributions_sha256": copied_dist_sha256,
            "role": "combat_explorer_saved_roots",
            "lineage": (
                "Current-native synthetic explorer roots generated with "
                "validation_set.build_validation and the supplied loadout fit. "
                "Not the frozen validation-a0-v1 benchmark; no evaluation-parity "
                "or same-native training claim."
            ),
            "generator": "rl/validation_set.py:build_validation",
            "explorer_dataset_name": ROOTS_NAME,
            "main_count": main_count,
            "stress_per_stratum": stress_per_stratum,
        }
    )
    document["provenance"] = provenance
    with roots_path.open("x", encoding="utf-8") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    loaded_document, groups = load_validation(roots_path)
    case_counts = {label: len(roots) for label, roots in groups.items()}
    if loaded_document["native_sha256"] != native_sha256():
        raise RuntimeError("Generated explorer roots native fingerprint does not match this worktree")
    if sum(case_counts.values()) != len(document["cases"]):
        raise RuntimeError("Loader did not accept every generated explorer case")

    artifacts = {
        "schema": 1,
        "role": "combat_explorer_local_artifacts",
        "not_frozen_benchmark": True,
        "frozen_benchmark_note": (
            "Distinct from validation-a0-v1.json. That frozen file is left "
            "byte-for-byte unchanged and is not loaded by this explorer dataset."
        ),
        "native_sha256": native_sha256(),
        "output_dir": str(output_dir),
        "roots": {
            "path": str(roots_path),
            "sha256": sha256_file(roots_path),
            "protocol": document["protocol"],
            "schema": document["schema"],
            "generation_seed": seed,
            "case_count": len(document["cases"]),
            "case_counts": case_counts,
            "native_sha256": document["native_sha256"],
        },
        "distributions": {
            "source_path": str(distributions.resolve()),
            "local_path": str(dist_dest),
            "sha256": copied_dist_sha256,
        },
        "checkpoint": checkpoint_info,
        "screenshots_dir": str(screenshots_dir),
    }
    with artifacts_path.open("x", encoding="utf-8") as handle:
        json.dump(artifacts, handle, indent=2)
        handle.write("\n")

    checkpoint_line = (
        f"  --checkpoint {output_dir.name}/{CHECKPOINT_NAME} \\\n"
        if checkpoint_dest is not None
        else ""
    )
    launch = (
        "From rl/:\n"
        "uv run python -m combat_explorer \\\n"
        f"  --validation-manifest {output_dir.name}/{ROOTS_NAME} \\\n"
        f"  --distributions {output_dir.name}/{DISTRIBUTIONS_NAME} \\\n"
        f"{checkpoint_line}"
        "  --sessions-dir combat_explorer_sessions\n"
        "\nThen open http://127.0.0.1:8765\n"
    )
    launch_path.write_text(launch)
    artifacts["launch"] = launch
    return artifacts


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--distributions", type=Path, required=True)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--main-count", type=int, default=DEFAULT_MAIN_COUNT)
    parser.add_argument("--stress-per-stratum", type=int, default=DEFAULT_STRESS_PER_STRATUM)
    parser.add_argument("--eval-repeats", type=int, default=1)
    parser.add_argument("--eval-max-decisions", type=int, default=512)
    parser.add_argument(
        "--skip-checkpoint-load",
        action="store_true",
        help="Copy the checkpoint without loading CombatValueModel (not recommended)",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    artifacts = prepare_local(
        distributions=args.distributions,
        output_dir=args.output_dir,
        checkpoint=args.checkpoint,
        seed=args.seed,
        main_count=args.main_count,
        stress_per_stratum=args.stress_per_stratum,
        eval_repeats=args.eval_repeats,
        eval_max_decisions=args.eval_max_decisions,
        verify_checkpoint_load=not args.skip_checkpoint_load,
    )
    print(json.dumps({k: v for k, v in artifacts.items() if k != "launch"}, indent=2))
    print()
    print(artifacts["launch"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
