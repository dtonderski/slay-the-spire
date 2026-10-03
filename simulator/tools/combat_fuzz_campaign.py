"""Run bounded, isolated simulator probes; never runs training.

uv run --no-project python simulator/tools/combat_fuzz_campaign.py \
    --binary target/release/examples/combat_fuzz --start 0 --count 1000 \
    --out tmp/combat-fuzz/campaign-0

Timeouts are candidates, not proven bugs. Keep their live JSONL journals for
reproduction. Successful cases are summarized and their temporary files removed.
"""

import argparse
import hashlib
import json
import os
import shutil
import signal
import subprocess
import time
from pathlib import Path


def run_case(binary: Path, worktree: Path, seed: int, output: Path, timeout: float,
             profile: str = "cards-relics") -> dict:
    started = time.monotonic()
    log = output.parent / f"seed-{seed}.log"
    timed_out = False
    environment = os.environ.copy()
    manifest_path = output.parent / "manifest.json"
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text())
        environment["COMBAT_FUZZ_REVISION"] = manifest["revision"]
        environment["COMBAT_FUZZ_PATCH"] = str(output.parent / "implementation.patch")
    command = [str(binary), str(seed), "1", str(output)]
    if profile == "cards-relics-potions":
        command.append("--potions")
    elif profile == "cards-relics-potions-endurance":
        command.append("--endurance")
    with log.open("xb") as stream:
        process = subprocess.Popen(
            command,
            cwd=worktree,
            env=environment,
            stdout=stream,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass  # The child can finish between timeout and kill.
            process.wait()
    text = log.read_text(errors="replace")
    expected = f"done start={seed} count=1 terminal=1 failures=0 capped=0"
    if timed_out:
        status = "wall_timeout_candidate"
    elif process.returncode:
        status = "process_failure"
    elif expected in text:
        status = "terminal"
    elif "capped=1" in text:
        status = "action_limit_candidate"
    else:
        status = "failure_candidate"
    coverage = None
    coverage_error = None
    for line in text.splitlines():
        if line.startswith("coverage="):
            try:
                coverage = json.loads(line.removeprefix("coverage="))
                if coverage is not None and (
                    not isinstance(coverage, dict)
                    or not isinstance(coverage.get("encounter"), str)
                    or not isinstance(coverage.get("ascension"), int)
                ):
                    raise ValueError("invalid coverage fields")
            except ValueError as error:
                coverage = None
                coverage_error = str(error)
                if status == "terminal":
                    status = "probe_output_failure"
            break
    if status == "terminal" and profile != "cards-relics" and (
        coverage is None or coverage.get("generation_profile") != profile
    ):
        status = "probe_profile_mismatch"
        coverage_error = "binary did not confirm requested generation profile"
    if status == "terminal" and profile == "cards-relics-potions-endurance":
        health = coverage.get("initial_max_hp") if coverage else None
        if not isinstance(health, int) or not 400 <= health <= 800 or (
            coverage is None or coverage.get("initial_hp") != health
        ):
            status = "probe_profile_mismatch"
            coverage_error = "binary did not confirm full 400-800 HP endurance setup"
    result = {
        "seed": seed,
        "coverage": coverage,
        "coverage_error": coverage_error,
        "status": status,
        "exit_code": process.returncode,
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "artifacts": str(output) if status != "terminal" else None,
        "log": str(log) if status != "terminal" else None,
    }
    if status == "terminal":
        shutil.rmtree(output)
        log.unlink()
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--worktree", type=Path, default=Path.cwd())
    parser.add_argument("--start", type=int, required=True)
    parser.add_argument("--count", type=int, default=1000)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=10.0)
    parser.add_argument("--profile", choices=["cards-relics", "cards-relics-potions",
                                             "cards-relics-potions-endurance"],
                        default="cards-relics")
    args = parser.parse_args()
    if args.start < 0 or args.count <= 0 or args.start + args.count > 2**64 or args.timeout <= 0:
        parser.error("invalid seed range, count, or timeout")
    binary = args.binary.resolve(strict=True)
    worktree = args.worktree.resolve(strict=True)
    root = args.out.resolve()
    root.mkdir(parents=True, exist_ok=False)
    # Preserve the executable once per campaign, including uncommitted builds.
    frozen_binary = root / "combat_fuzz"
    shutil.copy2(binary, frozen_binary)
    shutil.copy2(Path(__file__), root / "runner.py")
    manifest = {
        "schema": 1,
        "start": args.start,
        "count": args.count,
        "timeout_seconds": args.timeout,
        "generation_profile": args.profile,
        "binary_sha256": hashlib.sha256(frozen_binary.read_bytes()).hexdigest(),
        "worktree": str(worktree),
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=worktree, text=True).strip(),
        "timeout_classification": "candidate, not proof of a hang",
    }
    (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (root / "implementation.patch").write_bytes(subprocess.check_output(["git", "diff", "HEAD"], cwd=worktree))
    counts: dict[str, int] = {}
    encounters: dict[str, int] = {}
    ascensions: dict[str, int] = {}
    potions: dict[str, int] = {}
    accepted_actions = 0
    potion_actions = 0
    with (root / "results.jsonl").open("x") as results:
        for seed in range(args.start, args.start + args.count):
            result = run_case(frozen_binary, worktree, seed, root / f"seed-{seed}", args.timeout, args.profile)
            results.write(json.dumps(result) + "\n")
            results.flush()
            status = result["status"]
            counts[status] = counts.get(status, 0) + 1
            if result["coverage"]:
                encounter = result["coverage"]["encounter"]
                ascension = str(result["coverage"]["ascension"])
                encounters[encounter] = encounters.get(encounter, 0) + 1
                ascensions[ascension] = ascensions.get(ascension, 0) + 1
                for potion in result["coverage"].get("potions", []):
                    potions[potion] = potions.get(potion, 0) + 1
                accepted_actions += result["coverage"].get("accepted_actions", 0)
                potion_actions += result["coverage"].get("potion_actions", 0)
            if status != "terminal" or (seed - args.start + 1) % 100 == 0:
                print(json.dumps({"latest": result, "counts": counts}), flush=True)
    (root / "summary.json").write_text(json.dumps({"outcomes": counts,
        "encounters": encounters, "ascensions": ascensions, "potions": potions,
        "accepted_actions": accepted_actions, "potion_actions": potion_actions}, indent=2) + "\n")
    print(json.dumps({"complete": True, "counts": counts}), flush=True)


if __name__ == "__main__":
    main()
