"""Test route compatibility, NOT full-map equality or observed routing labels.

Generate maps from seed alone through the native research exporter. Match the
original path_taken symbols (never realized path_per_floor) without treating '?'
as a wildcard. Wrong-seed controls quantify how permissive this test is.
"""

import argparse
import json
import math
import random
import re
import subprocess
from collections import Counter
from pathlib import Path

from .source import digest, ordinary, shuffled_records, write_json

SYMBOLS = {"M", "E", "?", "R", "$", "T"}


def numeric_seed(value) -> int:
    # Run logs store signed decimal Java longs, NOT the UI's base-35 seed text.
    if isinstance(value, str) and re.fullmatch(r"-?\d+", value, flags=re.ASCII):
        value = int(value)
    if type(value) is not int or not -(2**63) <= value < 2**63 - 1:
        raise ValueError("invalid_signed_decimal_seed_or_offset_overflow")
    return value


def act_route(event: dict, act: int = 1) -> list[str]:
    if act not in (1, 2, 3):
        raise ValueError("expected_act_1_2_or_3")
    ordinary(event)
    if event["ascension_level"] != 0:
        raise ValueError("native_map_generator_only_a0")
    path = event.get("path_taken")
    if not isinstance(path, list):
        raise ValueError("missing_path_taken")
    boundaries = [i for i, symbol in enumerate(path) if symbol == "BOSS"]
    if len(boundaries) < act:
        raise ValueError("requested_act_not_complete")
    start = 0 if act == 1 else boundaries[act - 2] + 1
    route = path[start : boundaries[act - 1]]
    if len(route) != 15:
        raise ValueError("no_unambiguous_complete_15_row_act")
    if any(not isinstance(symbol, str) or symbol not in SYMBOLS for symbol in route):
        raise ValueError("unsupported_map_symbol")
    # Flight permits leaving graph edges. Excluding whole runs (even if acquired
    # later) is a declared conservative cohort restriction, not a parity repair.
    if "WingedGreaves" in event.get("relics", []) or "Wing Boots" in event.get("relics", []):
        raise ValueError("wing_boots_final_inventory")
    if any(entry.get("key") in {"WingedGreaves", "Wing Boots"} for entry in event.get("relics_obtained", [])):
        raise ValueError("wing_boots_acquisition")
    return route


def match_route(map_data: dict, symbols: list[str]) -> dict:
    """Count ALL matching coordinate paths. No candidate selected as truth.

    Forward counts give multiplicity; backwards viability reports coordinate
    ambiguity conditional on the whole recorded route. Those retrospective
    coordinates are not observed policy inputs. The separate hypothesis export
    consumes them only under an explicit, unproven map-equality assumption.
    """
    if not symbols or len(symbols) > 15 or any(s not in SYMBOLS for s in symbols):
        raise ValueError("expected 1..15 exact map symbols")
    nodes = {(n["row"], n["x"]): n for n in map_data["nodes"]}
    if len(nodes) != len(map_data["nodes"]):
        raise ValueError("duplicate_map_node")
    counts = {(0, x): 1 for x in map_data["starts"]}
    levels = []
    for row, symbol in enumerate(symbols):
        counts = {
            pos: count
            for pos, count in counts.items()
            if pos in nodes and pos[0] == row and nodes[pos]["symbol"] == symbol
        }
        levels.append(counts)
        if row + 1 == len(symbols):
            break
        next_counts = Counter()
        for pos, count in counts.items():
            for child in set(tuple(c) for c in nodes[pos]["children"]):
                if child[0] == row + 1:
                    next_counts[child] += count
        counts = dict(next_counts)
    total = sum(levels[-1].values())
    viable = set(levels[-1])
    positions = [sorted(x for _, x in viable)]
    for level in reversed(levels[:-1]):
        viable = {pos for pos in level if any(tuple(c) in viable for c in nodes[pos]["children"])}
        positions.append(sorted(x for _, x in viable))
    return {
        "compatible_paths": total,
        "possible_xs_by_row": list(reversed(positions)),
        "first_incompatible_row": next((i for i, level in enumerate(levels) if not level), None),
    }


def hypothetical_choices(map_data: dict, match: dict) -> list[dict]:
    """Research-only labels assuming this generator is the historical map.

    Require ONE complete coordinate path; never arbitrarily resolve ambiguity.
    Context is the generated public map and current position only, not the
    remaining recorded route. This schema is refused by the choice BC trainer.
    """
    if match["compatible_paths"] != 1:
        return []
    path = [positions[0] for positions in match["possible_xs_by_row"]]
    nodes = {(n["row"], n["x"]): n for n in map_data["nodes"]}
    result = []
    for row, chosen in enumerate(path):
        current = None if row == 0 else [row - 1, path[row - 1]]
        available = (
            map_data["starts"]
            if current is None
            else [x for next_row, x in nodes[(current[0], current[1])]["children"] if next_row == row]
        )
        available = sorted(set(available))
        if chosen not in available:
            raise ValueError("inferred route is not connected")
        if len(available) > 1:
            result.append(
                {
                    "schema": "hypothetical_route_v1",
                    "evidence": "conditional_on_unproven_map_equality",
                    "context": {
                        "act": map_data["act"],
                        "current": current,
                        "map_nodes": map_data["nodes"],
                        "starts": map_data["starts"],
                    },
                    "candidates": [[row, x] for x in available],
                    "label": [row, chosen],
                }
            )
    return result


def zero_failure_upper(n: int, confidence: float = 0.999) -> float:
    """One-sided exact binomial upper bound, only if zero failures in n trials."""
    if n <= 0 or not 0 < confidence < 1:
        raise ValueError("positive n and confidence in (0,1) required")
    return -math.expm1(math.log1p(-confidence) / n)


def permutation_test(matrix: list[list[bool]], repeats: int, seed: int) -> dict:
    n = len(matrix)
    if n < 2 or any(len(row) != n for row in matrix) or repeats < 1:
        raise ValueError("square matrix of at least 2 cases and positive repeats required")
    observed = sum(matrix[i][i] for i in range(n))
    rng = random.Random(seed)
    exceed = 0
    indices = list(range(n))
    for _ in range(repeats):
        rng.shuffle(indices)
        score = sum(matrix[i][indices[i]] for i in range(n))
        exceed += score >= observed
    return {
        "observed_matching_seeds_compatible": observed,
        "permutations": repeats,
        "null_scores_ge_observed": exceed,
        "monte_carlo_p": (exceed + 1) / (repeats + 1),
        "null": "exchangeable seed-route pairing within selected cohort, NOT incorrect-map probability",
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--map-binary", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=200)
    parser.add_argument("--act", type=int, choices=(1, 2, 3), default=1)
    parser.add_argument("--build", default="2020-07-30")
    parser.add_argument("--seed", type=int, default=20261016)
    parser.add_argument("--permutations", type=int, default=9999)
    args = parser.parse_args()
    if args.runs < 2 or args.permutations < 1:
        parser.error("at least two runs and positive permutations required")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    cases, rejected, seen_ids, seen_seeds = [], Counter(), set(), set()
    for event, provenance in shuffled_records(
        args.database, ascension=0, build=args.build, seed=args.seed, min_floor=16 + 17 * (args.act - 1)
    ):
        try:
            if event.get("build_version") != args.build:
                raise ValueError("build_mismatch")
            route = act_route(event, args.act)
            seed = numeric_seed(event.get("seed_played"))
            if event["play_id"] in seen_ids or seed in seen_seeds:
                raise ValueError("duplicate_play_id_or_seed")
        except (ValueError, KeyError, TypeError, AttributeError) as error:
            rejected[str(error)] += 1
            continue
        seen_ids.add(event["play_id"])
        seen_seeds.add(seed)
        cases.append(
            {"source": provenance, "run_group": digest(event["play_id"].encode()), "seed": seed, "route": route}
        )
        if len(cases) == args.runs:
            break
    # Freeze source selection BEFORE generating a map; failures stay in results.
    write_json(
        args.output_dir / "selection.json",
        {
            "cases": cases,
            "rejected_before_generation": dict(rejected),
            "sampling_seed": args.seed,
            "build": args.build,
            "act": args.act,
        },
    )
    if len(cases) != args.runs:
        raise RuntimeError("Not enough eligible runs; incomplete output retained")
    requests = "".join(json.dumps({"seed": case["seed"], "act": args.act, "ascension": 0}) + "\n" for case in cases)
    binary = args.map_binary.resolve()
    result = subprocess.run([str(binary)], input=requests, text=True, capture_output=True, timeout=120, check=True)
    maps = [json.loads(line) for line in result.stdout.splitlines()]
    if len(maps) != len(cases) or any(
        m["seed"] != c["seed"] or m["act"] != args.act or m["ascension"] != 0 for m, c in zip(maps, cases, strict=True)
    ):
        raise ValueError("exported map/request mismatch")
    (args.output_dir / "maps.jsonl").write_text(result.stdout)
    matches = [match_route(m, c["route"]) for m, c in zip(maps, cases, strict=True)]
    with (args.output_dir / "matches.jsonl").open("w") as out:
        for case, match in zip(cases, matches, strict=True):
            out.write(json.dumps({**case, **match}, sort_keys=True) + "\n")
    hypotheses = 0
    with (args.output_dir / "hypothetical_routes.jsonl").open("w") as out:
        for case, map_data, match in zip(cases, maps, matches, strict=True):
            for example in hypothetical_choices(map_data, match):
                out.write(json.dumps({"source": case["source"], "run_group": case["run_group"], **example}) + "\n")
                hypotheses += 1
    matrix = [[bool(match_route(m, c["route"])["compatible_paths"]) for m in maps] for c in cases]
    compatible = sum(matrix[i][i] for i in range(len(cases)))
    wrong_hits = sum(value for i, row in enumerate(matrix) for j, value in enumerate(row) if i != j)
    summary = {
        "status": "complete",
        "claim": "A0 route compatibility only; not map equality or trainable route labels",
        "act": args.act,
        "runs": len(cases),
        "compatible": compatible,
        "incompatible": len(cases) - compatible,
        "unique_coordinate_path": sum(m["compatible_paths"] == 1 for m in matches),
        "hypothetical_branch_decisions_not_training_labels": hypotheses,
        "ambiguous_coordinate_paths": sum(m["compatible_paths"] > 1 for m in matches),
        "wrong_seed_compatible_pairs": wrong_hits,
        "wrong_seed_pairs": len(cases) * (len(cases) - 1),
        "wrong_seed_rate": wrong_hits / (len(cases) * (len(cases) - 1)),
        "association_test": permutation_test(matrix, args.permutations, args.seed + 1),
        "if_zero_failures_99_9_percent_upper_failure_rate": zero_failure_upper(len(cases))
        if compatible == len(cases)
        else None,
        "zero_failure_trials_for_upper_0_001_at_99_9_percent": math.ceil(math.log(0.001) / math.log(0.999)),
        "statistics_limit": "Binomial bound assumes independent representative trials of this observable test; "
        "off-diagonal pairs are dependent. Neither bound nor association p proves hidden map equality.",
        "rejected_before_generation": dict(rejected),
        "sampling_seed": args.seed,
        "build": args.build,
        "map_binary_sha256": digest(binary.read_bytes()),
        "selection_sha256": digest((args.output_dir / "selection.json").read_bytes()),
        "maps_sha256": digest((args.output_dir / "maps.jsonl").read_bytes()),
    }
    write_json(args.output_dir / "summary.json", summary)
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
