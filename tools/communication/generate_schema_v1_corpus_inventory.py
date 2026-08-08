#!/usr/bin/env python3
"""Inventory and verify every active strict schema-v1 CommunicationMod trace."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess
from typing import Any

ACTIVE_ROOTS = (
    "simulator/verification/corpus/permanent_traces",
    "simulator/verification/corpus/fidelity_regressions",
    "simulator/verification/corpus/open_failures",
    "simulator/verification/corpus/quarantined_traces",
    "random_traces_loop/traces",
    "random_traces_loop/minimized",
    "random_traces_loop/schema_v1_smoke/traces",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def integer(value: Any) -> bool:
    return type(value) is int


def inspect(path: Path) -> dict[str, Any]:
    counts = {"records": 0, "actions": 0, "states": 0, "errors": 0, "external_rng": 0}
    boundary_kinds: dict[str, int] = {}
    metadata = None
    pending: tuple[int, str] | None = None
    last_step = 0
    terminal = False
    for line_number, line in enumerate(path.open(encoding="utf-8"), 1):
        if not line.strip():
            continue
        counts["records"] += 1
        record = json.loads(line)
        kind = record.get("type")
        if kind == "metadata":
            if counts["records"] != 1 or metadata is not None:
                raise RuntimeError(f"{path}:{line_number}: metadata must be the unique first record")
            metadata = record
            if type(record.get("schema")) is not int or record["schema"] != 1:
                raise RuntimeError(f"{path}:{line_number}: metadata schema must be integer 1")
            if type(record.get("boundary_schema")) is not int or record["boundary_schema"] != 1:
                raise RuntimeError(f"{path}:{line_number}: metadata boundary_schema must be integer 1")
            if record.get("source") != "communication_mod":
                raise RuntimeError(f"{path}:{line_number}: metadata source must be communication_mod")
            profile = record.get("run_config", {}).get("profile")
            if not isinstance(profile, dict) or not isinstance(profile.get("note_card"), str):
                raise RuntimeError(f"{path}:{line_number}: typed profile is required")
            upgrades = profile.get("note_upgrades")
            if not integer(upgrades) or upgrades < 0:
                raise RuntimeError(f"{path}:{line_number}: note_upgrades must be a non-negative integer")
        elif kind == "action":
            counts["actions"] += 1
            step = record.get("step")
            command = record.get("command")
            if not integer(step) or step != last_step + 1:
                raise RuntimeError(f"{path}:{line_number}: action step is not exact and contiguous")
            if pending is not None:
                raise RuntimeError(f"{path}:{line_number}: prior action has no immediate completion")
            if not isinstance(command, str) or not command.strip():
                raise RuntimeError(f"{path}:{line_number}: action command must be a non-empty string")
            pending = (step, command.split()[0].upper())
            last_step = step
        elif kind == "state":
            counts["states"] += 1
            step = record.get("step")
            message = record.get("message")
            if pending is None or not integer(step) or step != pending[0]:
                raise RuntimeError(f"{path}:{line_number}: state does not match the pending action")
            if not isinstance(message, dict):
                raise RuntimeError(f"{path}:{line_number}: state message must be an object")
            if type(message.get("boundary_schema")) is not int or message["boundary_schema"] != 1:
                raise RuntimeError(f"{path}:{line_number}: state boundary_schema must be integer 1")
            boundary_kind = message.get("boundary_kind")
            if boundary_kind not in {"poll", "interaction_ready", "quiescent", "terminal"}:
                raise RuntimeError(f"{path}:{line_number}: invalid authoritative boundary kind")
            expected = {"poll"} if pending[1] == "STATE" else {"interaction_ready", "quiescent", "terminal"}
            if boundary_kind not in expected:
                raise RuntimeError(f"{path}:{line_number}: boundary kind does not match command")
            for field in (
                "game_update_seq",
                "dungeon_update_seq",
                "actions_queued",
                "card_queue_size",
                "pre_turn_actions_size",
            ):
                if not integer(message.get(field)) or message[field] < 0:
                    raise RuntimeError(f"{path}:{line_number}: {field} must be a non-negative integer")
            current = message.get("current_action")
            if current is not None:
                if not isinstance(current, str):
                    raise RuntimeError(f"{path}:{line_number}: current_action must be string or null")
                for field in ("current_action_instance", "current_action_update_count"):
                    if not integer(message.get(field)) or message[field] < 0:
                        raise RuntimeError(f"{path}:{line_number}: {field} must be a non-negative integer")
            boundary_kinds[boundary_kind] = boundary_kinds.get(boundary_kind, 0) + 1
            terminal = terminal or boundary_kind == "terminal"
            pending = None
        elif kind == "error":
            counts["errors"] += 1
            step = record.get("step")
            if pending is None or not integer(step) or step != pending[0]:
                raise RuntimeError(f"{path}:{line_number}: error does not match pending action")
            pending = None
        elif kind == "external_rng":
            counts["external_rng"] += 1
            step = record.get("step")
            if pending is None or not integer(step) or step != pending[0]:
                raise RuntimeError(f"{path}:{line_number}: external RNG is orphaned")
            draws = record.get("draws")
            if not isinstance(draws, list):
                raise RuntimeError(f"{path}:{line_number}: external RNG draws must be an array")
            for draw in draws:
                if not isinstance(draw, dict) or not isinstance(draw.get("kind"), str):
                    raise RuntimeError(f"{path}:{line_number}: external RNG draw is not typed")
                if not integer(draw.get("range_inclusive")) or draw["range_inclusive"] < 0:
                    raise RuntimeError(f"{path}:{line_number}: external RNG range must be an integer")
                state = draw.get("state")
                if not isinstance(state, dict) or not all(
                    isinstance(state.get(field), str) for field in ("state0", "state1")
                ):
                    raise RuntimeError(f"{path}:{line_number}: external RNG state words must be strings")
        else:
            raise RuntimeError(f"{path}:{line_number}: unsupported active record type {kind!r}")
    if metadata is None or pending is not None or counts["actions"] == 0:
        raise RuntimeError(f"{path}: incomplete strict trace")
    return {
        **counts,
        "last_step": last_step,
        "terminal": terminal,
        "boundary_kinds": boundary_kinds,
        "profile": metadata["run_config"]["profile"],
    }


def parse_report(text: str) -> dict[str, Any]:
    values: dict[str, Any] = {}
    for line in text.splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        if key in {
            "total_actions",
            "verified",
            "unsupported",
            "unexpected_diffs",
            "applicable_actions",
            "disposed_actions",
            "target_rejected_actions",
            "duplicate_dispositions",
        }:
            values[key] = int(value)
        elif key in {"outcome", "seed_start.first_boundary.category", "seed_start.first_boundary.path"}:
            values[key] = value
    return values


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--verifier", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1))
    args = parser.parse_args()
    if not 1 <= args.jobs <= 8:
        parser.error("--jobs must be between 1 and 8")
    repo = args.repo.resolve()
    verifier = (args.verifier or repo / "simulator/target/release/sts_verify").resolve()
    output = args.output.resolve()
    reports = output / "reports"
    reports.mkdir(parents=True, exist_ok=True)
    paths: list[Path] = []
    for relative in ACTIVE_ROOTS:
        root = repo / relative
        if root.exists():
            paths.extend(root.glob("*.jsonl"))
    paths = sorted(set(paths))
    def verify(path: Path) -> tuple[dict[str, Any], subprocess.CompletedProcess[str]]:
        structural = inspect(path)
        result = subprocess.run(
            [str(verifier), "parity", str(path), "--require-terminal"],
            cwd=repo,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        return structural, result

    with ThreadPoolExecutor(max_workers=min(args.jobs, len(paths) or 1)) as executor:
        verified = list(executor.map(verify, paths))

    entries = []
    outcomes: dict[str, int] = {}
    for index, (path, (structural, result)) in enumerate(zip(paths, verified, strict=True)):
        report_path = reports / f"{index:03d}-{path.stem}.txt"
        report_path.write_text(result.stdout, encoding="utf-8")
        parsed = parse_report(result.stdout)
        outcome = parsed.get("outcome", "invalid_input")
        outcomes[outcome] = outcomes.get(outcome, 0) + 1
        if result.returncode == 1 or "total_actions" not in parsed:
            raise RuntimeError(f"{path}: verifier rejected strict corpus input\n{result.stdout}")
        if parsed["total_actions"] != structural["actions"]:
            raise RuntimeError(f"{path}: verifier/structural action count mismatch")
        if parsed["applicable_actions"] + parsed["target_rejected_actions"] != structural["actions"]:
            raise RuntimeError(f"{path}: applicability accounting gap")
        if parsed["disposed_actions"] != parsed["applicable_actions"]:
            raise RuntimeError(f"{path}: disposition accounting gap")
        if parsed["duplicate_dispositions"] != 0:
            raise RuntimeError(f"{path}: duplicate dispositions")
        entries.append(
            {
                "path": path.relative_to(repo).as_posix(),
                "size": path.stat().st_size,
                "sha256": sha256(path),
                "verifier_exit": result.returncode,
                "report": report_path.relative_to(output).as_posix(),
                "report_sha256": sha256(report_path),
                **structural,
                **parsed,
            }
        )
    summary = {
        "inventory_schema": 1,
        "policy": "explicit metadata/state boundary schema 1; direct simulator replay only",
        "active_roots": list(ACTIVE_ROOTS),
        "verifier": str(verifier),
        "verifier_sha256": sha256(verifier),
        "trace_count": len(entries),
        "content_unique_traces": len({entry["sha256"] for entry in entries}),
        "permanent_passes": sum(
            entry["outcome"] == "complete_pass" and "/permanent_traces/" in entry["path"]
            for entry in entries
        ),
        "honest_failures": sum(entry["outcome"] == "failed" for entry in entries),
        "outcomes": outcomes,
        "first_boundary_categories": {
            category: sum(
                entry.get("seed_start.first_boundary.category") == category for entry in entries
            )
            for category in sorted(
                {
                    entry.get("seed_start.first_boundary.category", "missing")
                    for entry in entries
                }
            )
        },
        "actions": sum(entry["actions"] for entry in entries),
        "verified_actions_before_first_boundary": sum(entry["verified"] for entry in entries),
        "applicable_actions": sum(entry["applicable_actions"] for entry in entries),
        "disposed_actions": sum(entry["disposed_actions"] for entry in entries),
        "rejected_actions": sum(entry["target_rejected_actions"] for entry in entries),
        "duplicate_dispositions": sum(entry["duplicate_dispositions"] for entry in entries),
        "boundary_kinds": {
            kind: sum(entry["boundary_kinds"].get(kind, 0) for entry in entries)
            for kind in ("poll", "interaction_ready", "quiescent", "terminal")
        },
        "external_rng_records": sum(entry["external_rng"] for entry in entries),
        "error_records": sum(entry["errors"] for entry in entries),
        "entries": entries,
    }
    manifest = output / "inventory.json"
    manifest.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    digest = sha256(manifest)
    (output / "inventory.json.sha256").write_text(f"{digest}  inventory.json\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in summary.items() if key != "entries"}, indent=2))
    print(f"inventory_sha256={digest}")


if __name__ == "__main__":
    main()
