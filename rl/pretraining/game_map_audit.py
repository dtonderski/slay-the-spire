"""Differential A0 map test against original game bytecode, not a copied oracle.

Requires a legally supplied PC game JAR and a Java runtime with jdk.compiler.
No recorded room symbols, paths, or expected maps enter either generator.
"""

import argparse
import json
import random
import subprocess
from pathlib import Path

from .map_audit import numeric_seed, zero_failure_upper
from .source import digest, write_json


def freeze_requests(audits: list[Path], runs: int, seed: int) -> tuple[list[dict], dict]:
    if runs < 1:
        raise ValueError("positive fresh-seed count required")
    requests, provenance = [], []
    for directory in audits:
        raw = (directory / "selection.json").read_bytes()
        selection = json.loads(raw)
        act = selection["act"]
        if type(act) is not int or act not in (1, 2, 3):
            raise ValueError("unsupported historical act")
        requests.extend({"seed": numeric_seed(c["seed"]), "act": act, "ascension": 0} for c in selection["cases"])
        provenance.append({"act": act, "build": selection["build"], "selection_sha256": digest(raw)})
    historical_count = len(requests)
    seen = {r["seed"] for r in requests}
    rng = random.Random(seed)
    seeds = []
    while len(seeds) < runs:
        value = rng.randrange(-(2**63), 2**63 - 600)
        if value in seen:
            continue
        seen.add(value)
        seeds.append(value)
    requests.extend({"seed": s, "act": a, "ascension": 0} for s in seeds for a in (1, 2, 3))
    return requests, {
        "status": "frozen_before_generation",
        "historical_requests": historical_count,
        "historical_selections": provenance,
        "fresh_distinct_seeds": runs,
        "fresh_maps": runs * 3,
        "sampling_seed": seed,
        "scope": "normal A0, acts 1..3, no burning-elite marker or boss identity",
    }


def compare_maps(game, rust, requests: list[dict], historical: int) -> dict:
    counts = dict.fromkeys(
        ("historical_maps", "historical_matches", "fresh_maps", "fresh_matches", "nodes_compared", "edges_compared"), 0
    )
    mismatches, failed_seeds = [], set()
    for index, (expected, actual, request) in enumerate(zip(game, rust, requests, strict=True)):
        if any(expected[k] != actual[k] or actual[k] != request[k] for k in ("seed", "act", "ascension")):
            raise ValueError("oracle/exporter request mismatch")
        group = "historical" if index < historical else "fresh"
        counts[group + "_maps"] += 1
        counts["nodes_compared"] += len(expected["nodes"])
        counts["edges_compared"] += sum(len(n["children"]) for n in expected["nodes"])
        # All connected nodes, room symbols, starts and edges, not a route subset.
        if expected == actual:
            counts[group + "_matches"] += 1
        else:
            mismatches.append({"index": index, **request})
            if group == "fresh":
                failed_seeds.add(request["seed"])
    return {"counts": counts, "fresh_seeds_with_any_act_mismatch": len(failed_seeds), "mismatches": mismatches}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--map-binary", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--route-audit-dir", type=Path, nargs="*", default=[])
    parser.add_argument("--runs", type=int, default=7000, help="distinct fresh seeds; each tested in all three acts")
    parser.add_argument("--seed", type=int, default=20261017)
    parser.add_argument("--cosmetic-seed", type=int, default=1776)
    args = parser.parse_args()
    requests, manifest = freeze_requests(args.route_audit_dir, args.runs, args.seed)
    out, jar, binary = args.output_dir.resolve(), args.jar.resolve(strict=True), args.map_binary.resolve(strict=True)
    out.mkdir(parents=True, exist_ok=False)
    raw = "".join(json.dumps(r) + "\n" for r in requests).encode()
    (out / "requests.jsonl").write_bytes(raw)
    manifest["requests_sha256"] = digest(raw)
    manifest["cosmetic_seed"] = args.cosmetic_seed
    write_json(out / "selection.json", manifest)
    source = Path(__file__).with_name("GameMapOracle.java")
    (out / source.name).write_bytes(source.read_bytes())
    classes = out / "classes"
    classes.mkdir()
    with (out / "compiler.log").open("w") as log:
        subprocess.run(
            ["java", "-m", "jdk.compiler/com.sun.tools.javac.Main", "-cp", str(jar), "-d", str(classes), str(source)],
            stdout=log,
            stderr=log,
            check=True,
            timeout=120,
        )
    # Game UI initialization can write display preferences; sandbox cwd in out.
    with (
        (out / "requests.jsonl").open("rb") as inp,
        (out / "game-maps.jsonl").open("wb") as output,
        (out / "game.stderr").open("wb") as error,
    ):
        subprocess.run(
            [
                "java",
                "-Xmx512m",
                "--enable-native-access=ALL-UNNAMED",
                "-cp",
                f"{classes}:{jar}",
                "GameMapOracle",
                str(args.cosmetic_seed),
            ],
            stdin=inp,
            stdout=output,
            stderr=error,
            cwd=out,
            check=True,
            timeout=600,
        )
    with (out / "requests.jsonl").open("rb") as inp, (out / "rust-maps.jsonl").open("wb") as output:
        subprocess.run([str(binary)], stdin=inp, stdout=output, check=True, timeout=600)
    with (out / "game-maps.jsonl").open() as game, (out / "rust-maps.jsonl").open() as rust:
        report = compare_maps(map(json.loads, game), map(json.loads, rust), requests, manifest["historical_requests"])
    with (out / "game.stderr").open() as log:
        authorities = [line.strip() for line in log if line.startswith("Authority: ")]
    if len(authorities) != 1:
        raise ValueError("missing or ambiguous game authority identity")
    summary = {
        **manifest,
        **report,
        "status": "complete",
        "authority": authorities[0],
        "jar_sha256": digest(jar.read_bytes()),
        "rust_binary_sha256": digest(binary.read_bytes()),
        "oracle_sha256": digest(source.read_bytes()),
        "audit_sha256": digest(Path(__file__).read_bytes()),
        "if_zero_failures_99_9_upper_seed_failure_rate": zero_failure_upper(args.runs)
        if not report["fresh_seeds_with_any_act_mismatch"]
        else None,
        "limit": "Bytecode differential evidence under a headless A0 setup, not a captured real-game trace, "
        "all-seed proof, historical-version equivalence, A1+ validation or full-run parity.",
    }
    for name in ("game-maps.jsonl", "rust-maps.jsonl"):
        summary[name + "_sha256"] = digest((out / name).read_bytes())
    write_json(out / "summary.json", summary)
    print(json.dumps(summary, indent=2))
    if report["mismatches"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
