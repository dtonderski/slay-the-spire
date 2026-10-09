"""Bounded, simulator-only run-interface probe; no learning or parity claims.

uv run --project simulator/python python rl/run_training/probe.py --help
Every case has an append-only accepted/attempted-action journal, including success.
Synthetic high-HP cases exercise interfaces, not a natural training distribution.
"""

import argparse
import hashlib
import json
import random
import subprocess
import sys
import time
from collections import Counter
from dataclasses import asdict
from pathlib import Path

# Also support execution as a script from the repository root.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from sts_sim import State

from run_training.contracts import PolicyAction, controller, outcome


def choose(
    actions: tuple[PolicyAction, ...],
    rng: random.Random,
    style: str,
    is_combat: bool = False,
) -> int:
    """Public-only diagnostic driver, not a strength baseline or trained policy.

    The native State/revision/environment seed never enters this function.
    Random is uniform over complete legal candidates. Exercise favors card plays,
    collecting rewards, recalls, and advancing interfaces to improve coverage.
    """
    if style == "random" or (style == "mixed" and not is_combat):
        return rng.randrange(len(actions))
    priorities = (
        "take_emerald_key",
        "take_sapphire_key",
        "rest_recall",
        "take_gold_reward",
        "take_stolen_gold_reward",
        "take_relic_reward",
        "take_relic_reward_at",
        "take_card_reward",
        "choose_boss_relic_reward",
        "open_chest",
        "open_card_reward",
        "open_queued_card_reward",
        "confirm_grid",
        "confirm_selection",
        "confirm_selection_without_retrieval",
        "choose_visible_option",
        "play_hand_slot",
        "use_potion_slot",
        "end_turn",
        "rest_heal",
        "rest_proceed",
        "proceed",
        "skip_reward",
        "leave_shop",
        "enter_shop",
    )
    for kind in priorities:
        indices = [i for i, action in enumerate(actions) if action.kind == kind]
        if indices:
            return rng.choice(indices)
    return rng.randrange(len(actions))


def emit(stream, value: dict) -> None:
    stream.write(json.dumps(value, sort_keys=True) + "\n")
    stream.flush()


def run_case(args, seed: str, policy_seed: int, path: Path) -> dict:
    start = time.perf_counter()
    screens, owners, kinds = Counter(), Counter(), Counter()
    maximum_candidates = 0
    step_seconds = 0.0
    export_seconds = 0.0
    attempted = accepted = 0
    decision = None
    status = "error"
    error = None
    rng = random.Random(policy_seed)
    with path.open("x") as journal:
        emit(
            journal,
            {
                "type": "setup",
                "seed": seed,
                "policy_seed": policy_seed,
                "mode": args.mode,
                "hp": args.hp if args.mode != "natural" else None,
                "ascension": args.ascension,
                "style": args.style,
                "max_decisions": args.max_decisions,
                "case_seconds": args.case_seconds,
            },
        )
        try:
            state = (
                State.new(seed, args.ascension)
                if args.mode == "natural"
                else State.new_synthetic(seed, args.ascension, args.hp, final_act=True)
            )
            begin = time.perf_counter()
            decision = state.decision()
            export_seconds += time.perf_counter() - begin
            while True:
                observation = decision.observation
                screens[observation.kind] += 1
                maximum_candidates = max(maximum_candidates, len(decision.actions))
                status = outcome(decision)
                if status != "ongoing":
                    break
                if (
                    accepted >= args.max_decisions
                    or time.perf_counter() - start >= args.case_seconds
                ):
                    status = "truncated"
                    break
                owner = controller(decision)
                owners[owner] += 1
                descriptors = tuple(
                    PolicyAction.from_action(action) for action in decision.actions
                )
                index = choose(
                    descriptors, rng, args.style, observation.kind == "combat"
                )
                action = decision.actions[index]
                descriptor = asdict(PolicyAction.from_action(action))
                emit(
                    journal,
                    {
                        "type": "attempt",
                        "step": accepted,
                        "index": index,
                        "revision": decision.revision,
                        "action": descriptor,
                        "screen": observation.kind,
                        "act": observation.context.act,
                        "floor": observation.context.floor,
                        "hp": observation.context.player_hp,
                        "candidates": len(decision.actions),
                    },
                )
                attempted += 1
                begin = time.perf_counter()
                successor = state.step(action)
                step_seconds += time.perf_counter() - begin
                accepted += 1
                kinds[action.kind] += 1
                emit(
                    journal,
                    {
                        "type": "accepted",
                        "step": accepted - 1,
                        "revision": successor.revision,
                    },
                )
                decision = successor
        except (ValueError, RuntimeError) as exception:
            status, error = "error", f"{type(exception).__name__}: {exception}"
        context = decision.observation.context if decision is not None else None
        result = {
            "seed": seed,
            "policy_seed": policy_seed,
            "outcome": status,
            "error": error,
            "accepted": accepted,
            "attempted": attempted,
            "act": context.act if context else None,
            "floor": context.floor if context else None,
            "hp": context.player_hp if context else None,
            "seconds": time.perf_counter() - start,
            "step_seconds": step_seconds,
            "initial_export_seconds": export_seconds,
            "screens": dict(screens),
            "controllers": dict(owners),
            "action_kinds": dict(kinds),
            "max_candidates": maximum_candidates,
            "journal": str(path),
        }
        emit(journal, {"type": "result", **result})
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode", choices=("natural", "synthetic-act4"), default="natural"
    )
    parser.add_argument(
        "--style", choices=("random", "exercise", "mixed"), default="exercise"
    )
    parser.add_argument("--ascension", type=int, choices=range(21), default=0)
    parser.add_argument("--hp", type=int, default=10000)
    parser.add_argument("--count", type=int, default=32)
    parser.add_argument("--start", type=int, default=0)
    parser.add_argument("--driver-seed", type=int, default=981723)
    parser.add_argument("--max-decisions", type=int, default=20000)
    parser.add_argument("--case-seconds", type=float, default=60)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if (
        min(args.hp, args.count, args.max_decisions) <= 0
        or args.case_seconds <= 0
        or args.start < 0
    ):
        parser.error("Budgets/count/HP must be positive and start nonnegative")
    args.out.mkdir(parents=True, exist_ok=False)
    native = sys.modules["sts_sim._native"].__file__
    if native is None:
        raise RuntimeError("Native module has no file to pin for reproducible research")
    manifest = {
        "args": {
            k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()
        },
        "git_head": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True
        ).strip(),
        "native_sha256": hashlib.sha256(Path(native).read_bytes()).hexdigest(),
        "python": sys.version,
        "parity_evidence": False,
    }
    (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (args.out / "implementation.diff").write_bytes(
        subprocess.check_output(["git", "diff"])
    )
    # New/untracked prototype sources are pinned separately from the tracked diff.
    source_dir = args.out / "source"
    source_dir.mkdir()
    for source in Path(__file__).parent.glob("*.py"):
        (source_dir / source.name).write_bytes(source.read_bytes())
    results = []
    with (args.out / "results.jsonl").open("x") as stream:
        for number in range(args.start, args.start + args.count):
            # Valid base-35 seed text; no corpus-specific implementation behavior.
            seed = f"RUN{number:X}"
            result = run_case(
                args, seed, args.driver_seed + number, args.out / f"case-{number}.jsonl"
            )
            emit(stream, result)
            results.append(result)
            print(
                f"{seed}: {result['outcome']} floor={result['floor']} actions={result['accepted']}",
                flush=True,
            )
    summary = {
        "cases": len(results),
        "outcomes": dict(Counter(row["outcome"] for row in results)),
        "accepted": sum(row["accepted"] for row in results),
        "seconds": sum(row["seconds"] for row in results),
        "screen_counts": dict(
            sum((Counter(row["screens"]) for row in results), Counter())
        ),
        "controller_counts": dict(
            sum((Counter(row["controllers"]) for row in results), Counter())
        ),
        "max_candidates": max(row["max_candidates"] for row in results),
    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, sort_keys=True))


if __name__ == "__main__":
    main()
