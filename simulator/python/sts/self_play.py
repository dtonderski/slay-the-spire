"""Deterministic simulator self-play trace generation and replay."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
import argparse
import hashlib
import json
import random
from pathlib import Path
from typing import Any, Iterable

from sts import omni
from sts.search import CombatSearchConfig, search_combat
from sts.search_lab import BenchmarkRoot, SearchCandidate, default_candidates, evaluate_candidate


DEFAULT_COMBAT_POLICY = CombatSearchConfig(
    max_depth=40,
    objective="aggressive_lethal",
    algorithm="portfolio",
    beam_width=12,
)


@dataclass(frozen=True)
class SelfPlayResult:
    trace_path: Path | None
    steps: int
    stop_reason: str
    final_phase: str | None
    verified: bool


@dataclass(frozen=True)
class CorpusBatchResult:
    output_dir: Path
    index_path: Path
    trace_count: int
    verified_count: int
    stop_reasons: dict[str, int]


@dataclass(frozen=True)
class TraceCombatRoot:
    trace_path: Path
    step: int
    state_id: str
    snapshot_json: str
    split: str
    potion_count: int
    legal_action_kinds: tuple[str, ...]


def run_self_play(
    *,
    output: Path,
    seed: str | None = None,
    ascension: int = 0,
    start: str = "seed",
    random_seed: int = 0,
    max_steps: int = 200,
    combat_policy: CombatSearchConfig = DEFAULT_COMBAT_POLICY,
) -> SelfPlayResult:
    """Run one simulator self-play episode and write a replayable JSONL trace."""

    rng = random.Random(random_seed)
    output.parent.mkdir(parents=True, exist_ok=True)
    started_at = _now()

    try:
        env = _start_env(start=start, seed=seed, ascension=ascension)
    except Exception as error:
        _write_jsonl(
            output,
            [
                _metadata(
                    started_at=started_at,
                    seed=seed,
                    ascension=ascension,
                    start=start,
                    random_seed=random_seed,
                    max_steps=max_steps,
                    combat_policy=combat_policy,
                    stop_reason=f"unsupported_start: {error}",
                )
            ],
        )
        return SelfPlayResult(output, 0, f"unsupported_start: {error}", None, False)

    records: list[dict[str, Any]] = [
        _metadata(
            started_at=started_at,
            seed=seed,
            ascension=ascension,
            start=start,
            random_seed=random_seed,
            max_steps=max_steps,
            combat_policy=combat_policy,
            initial_snapshot_json=env.snapshot_json(),
            initial_state_id=env.snapshot_hash(),
        )
    ]
    stop_reason = "max_steps"
    steps = 0

    for step in range(max_steps):
        before_hash = env.snapshot_hash()
        actions = env.exact_legal_actions()
        if not actions:
            unsupported = env.unsupported_reason()
            stop_reason = f"unsupported_transition: {unsupported}" if unsupported else "no_legal_actions"
            break

        try:
            action, policy_name, policy_diagnostics = _choose_action(
                env,
                actions,
                rng,
                combat_policy,
            )
        except Exception as error:
            stop_reason = f"policy_error: {error}"
            break

        before_snapshot = env.snapshot_json()
        before_summary = _summary(env)
        try:
            result = env.step(action)
        except Exception as error:
            records.append(
                _step_record(
                    step=step,
                    before_hash=before_hash,
                    before_snapshot_json=before_snapshot,
                    before_summary=before_summary,
                    legal_actions=actions,
                    action=action,
                    policy_name=policy_name,
                    policy_diagnostics=policy_diagnostics,
                    error=f"step_error: {error}",
                )
            )
            stop_reason = f"step_error: {error}"
            break

        records.append(
            _step_record(
                step=step,
                before_hash=before_hash,
                before_snapshot_json=before_snapshot,
                before_summary=before_summary,
                legal_actions=actions,
                action=action,
                policy_name=policy_name,
                policy_diagnostics=policy_diagnostics,
                after_hash=result.snapshot_hash,
                after_snapshot_json=result.snapshot_json,
                after_summary=_summary(env),
                transition=result.transition,
                unsupported_reason=result.unsupported_reason,
            )
        )
        steps += 1

        if result.unsupported_reason and not env.exact_legal_actions():
            stop_reason = f"unsupported_transition: {result.unsupported_reason}"
            break
        phase = env.phase()
        if phase in {"won", "lost"}:
            stop_reason = phase
            break
    else:
        stop_reason = "max_steps"

    records[0]["ended_at"] = _now()
    records[0]["stop_reason"] = stop_reason
    records[0]["steps"] = steps
    records[0]["final_phase"] = env.phase()
    records[0]["final_state_id"] = env.snapshot_hash()
    records[0]["final_summary"] = _summary(env)
    _write_jsonl(output, records)
    verification = verify_self_play_trace(output)
    return SelfPlayResult(output, steps, stop_reason, env.phase(), verification["ok"])


def verify_self_play_trace(path: Path) -> dict[str, Any]:
    """Replay a generated self-play trace from its initial snapshot."""

    records = _read_jsonl(path)
    if not records:
        return {"ok": False, "error": "empty trace", "steps": 0}
    metadata = records[0]
    if metadata.get("type") != "metadata":
        return {"ok": False, "error": "first record is not metadata", "steps": 0}
    initial_snapshot = metadata.get("initial_snapshot_json")
    if not isinstance(initial_snapshot, str):
        return {"ok": False, "error": "metadata has no initial_snapshot_json", "steps": 0}

    env = omni.OmniRunEnv.from_snapshot_json(initial_snapshot)
    steps = 0
    for record in records[1:]:
        if record.get("type") != "step":
            continue
        before_hash = record.get("before_hash")
        if env.snapshot_hash() != before_hash:
            return {
                "ok": False,
                "error": "before hash mismatch",
                "step": record.get("step"),
                "expected": before_hash,
                "actual": env.snapshot_hash(),
            }
        action_json = record.get("action_json")
        action = _find_action(env.exact_legal_actions(), action_json)
        if action is None:
            return {
                "ok": False,
                "error": "action not legal during replay",
                "step": record.get("step"),
                "action_json": action_json,
            }
        result = env.step(action)
        after_hash = record.get("after_hash")
        if after_hash and result.snapshot_hash != after_hash:
            return {
                "ok": False,
                "error": "after hash mismatch",
                "step": record.get("step"),
                "expected": after_hash,
                "actual": result.snapshot_hash,
            }
        steps += 1

    return {
        "ok": True,
        "steps": steps,
        "final_state_id": env.snapshot_hash(),
        "final_phase": env.phase(),
        "metadata_stop_reason": metadata.get("stop_reason"),
    }


def run_self_play_batch(
    *,
    output_dir: Path,
    seeds: Iterable[str],
    ascension: int = 0,
    start: str = "seed",
    random_seed: int = 0,
    max_steps: int = 200,
    combat_policy: CombatSearchConfig = DEFAULT_COMBAT_POLICY,
) -> CorpusBatchResult:
    """Generate a verified simulator-only self-play corpus and an index file."""

    output_dir.mkdir(parents=True, exist_ok=True)
    traces_dir = output_dir / "traces"
    traces_dir.mkdir(parents=True, exist_ok=True)

    trace_summaries = []
    stop_reasons: dict[str, int] = {}
    verified_count = 0
    for index, seed in enumerate(seeds):
        episode_random_seed = random_seed + index
        trace_path = traces_dir / f"seed-{_safe_file_stem(seed)}-rng-{episode_random_seed}.jsonl"
        result = run_self_play(
            output=trace_path,
            seed=seed,
            ascension=ascension,
            start=start,
            random_seed=episode_random_seed,
            max_steps=max_steps,
            combat_policy=combat_policy,
        )
        verification = verify_self_play_trace(trace_path)
        records = _read_jsonl(trace_path)
        summary = _trace_summary(
            trace_path=trace_path,
            records=records,
            result=result,
            verification=verification,
        )
        trace_summaries.append(summary)
        if result.verified and verification.get("ok"):
            verified_count += 1
        stop_reason = result.stop_reason
        stop_reasons[stop_reason] = stop_reasons.get(stop_reason, 0) + 1

    index_doc = {
        "type": "self_play_corpus_index",
        "schema": 1,
        "source": "sim_selfplay_corpus",
        "parity": "non_parity_simulator_only",
        "created_at": _now(),
        "start": start,
        "ascension": ascension,
        "random_seed": random_seed,
        "max_steps": max_steps,
        "combat_policy": combat_policy.__dict__,
        "trace_count": len(trace_summaries),
        "verified_count": verified_count,
        "stop_reasons": stop_reasons,
        "traces": trace_summaries,
    }
    index_path = output_dir / "index.json"
    index_path.write_text(json.dumps(index_doc, indent=2, sort_keys=True), encoding="utf-8")
    return CorpusBatchResult(output_dir, index_path, len(trace_summaries), verified_count, stop_reasons)


def evaluate_self_play_corpus(
    *,
    corpus_dir: Path | None = None,
    traces: Iterable[Path] | None = None,
    split: str = "all",
    max_roots: int = 64,
    max_actions: int = 40,
    candidates: Iterable[SearchCandidate] | None = None,
) -> dict[str, Any]:
    """Compare search candidates from exact combat states recorded in traces."""

    trace_paths = _trace_paths(corpus_dir=corpus_dir, traces=traces)
    roots = _trace_combat_roots(trace_paths)
    if split != "all":
        roots = [root for root in roots if root.split == split]
    roots = roots[:max_roots]
    candidates = list(candidates or default_candidates())

    episodes = []
    for candidate in candidates:
        for root in roots:
            benchmark_root = BenchmarkRoot(
                name=f"{root.trace_path.stem}:step{root.step}:{root.state_id}",
                env_kind="run_combat",
                snapshot_json=root.snapshot_json,
                state_id=root.state_id,
                source_depth=root.step,
                split=root.split,
            )
            result = evaluate_candidate(benchmark_root, candidate, max_actions=max_actions)
            row = result.__dict__ | {
                "trace_path": str(root.trace_path),
                "trace_step": root.step,
                "state_id": root.state_id,
                "potion_count": root.potion_count,
                "legal_action_kinds": list(root.legal_action_kinds),
                "has_potion_actions": any("potion" in kind for kind in root.legal_action_kinds),
            }
            episodes.append(row)

    return {
        "type": "self_play_trace_eval",
        "schema": 1,
        "source": "sim_selfplay_trace_eval",
        "parity": "non_parity_simulator_only",
        "split": split,
        "trace_count": len(trace_paths),
        "roots": len(roots),
        "max_roots": max_roots,
        "max_actions": max_actions,
        "root_family": "trace_combat_states",
        "potion_roots": sum(1 for root in roots if root.potion_count > 0),
        "potion_action_roots": sum(
            1 for root in roots if any("potion" in kind for kind in root.legal_action_kinds)
        ),
        "ranking": _rank_episode_dicts(episodes),
        "episodes": episodes,
    }


def _start_env(*, start: str, seed: str | None, ascension: int) -> Any:
    if start == "seed":
        return omni.OmniRunEnv.new_ironclad(seed=seed, ascension=ascension)
    if start == "map_fixture":
        return omni.OmniRunEnv.map_fixture()
    if start == "combat_fixture":
        return omni.OmniRunEnv.combat_fixture()
    raise ValueError(f"unsupported start mode: {start}")


def _trace_summary(
    *,
    trace_path: Path,
    records: list[dict[str, Any]],
    result: SelfPlayResult,
    verification: dict[str, Any],
) -> dict[str, Any]:
    metadata = records[0] if records else {}
    step_records = [record for record in records[1:] if record.get("type") == "step"]
    potion_action_steps = [
        record.get("step")
        for record in step_records
        if "potion" in str(record.get("action_kind", "")).lower()
    ]
    potion_state_steps = [
        record.get("step")
        for record in step_records
        if (record.get("before_summary") or {}).get("potions")
        or (record.get("after_summary") or {}).get("potions")
    ]
    combat_steps = [
        record.get("step")
        for record in step_records
        if (record.get("before_summary") or {}).get("phase") == "combat"
    ]
    return {
        "path": str(trace_path),
        "seed": metadata.get("seed"),
        "random_seed": metadata.get("random_seed"),
        "steps": result.steps,
        "verified": bool(result.verified and verification.get("ok")),
        "verification": verification,
        "stop_reason": result.stop_reason,
        "final_phase": result.final_phase,
        "final_summary": metadata.get("final_summary"),
        "combat_steps": len(combat_steps),
        "potion_state_steps": len(potion_state_steps),
        "potion_action_steps": potion_action_steps,
    }


def _trace_paths(*, corpus_dir: Path | None, traces: Iterable[Path] | None) -> list[Path]:
    if traces is not None:
        return sorted(Path(path) for path in traces)
    if corpus_dir is None:
        raise ValueError("corpus_dir or traces is required")
    traces_dir = corpus_dir / "traces"
    root = traces_dir if traces_dir.exists() else corpus_dir
    return sorted(path for path in root.glob("*.jsonl") if path.is_file())


def _trace_combat_roots(trace_paths: Iterable[Path]) -> list[TraceCombatRoot]:
    roots: list[TraceCombatRoot] = []
    seen: set[str] = set()
    for trace_path in trace_paths:
        verification = verify_self_play_trace(trace_path)
        if not verification.get("ok"):
            continue
        for record in _read_jsonl(trace_path)[1:]:
            if record.get("type") != "step":
                continue
            summary = record.get("before_summary") or {}
            if summary.get("phase") != "combat":
                continue
            snapshot_json = record.get("before_snapshot_json")
            state_id = record.get("before_hash")
            if not isinstance(snapshot_json, str) or not isinstance(state_id, str):
                continue
            key = f"{trace_path}:{record.get('step')}:{state_id}"
            if key in seen:
                continue
            seen.add(key)
            legal_action_kinds = tuple(
                str(action.get("kind", "")).lower()
                for action in record.get("legal_actions", [])
                if isinstance(action, dict)
            )
            roots.append(
                TraceCombatRoot(
                    trace_path=trace_path,
                    step=int(record.get("step", 0)),
                    state_id=state_id,
                    snapshot_json=snapshot_json,
                    split=_split_for_state(state_id),
                    potion_count=len(summary.get("potions") or []),
                    legal_action_kinds=legal_action_kinds,
                )
            )
    roots.sort(key=lambda root: (str(root.trace_path), root.step, root.state_id))
    return roots


def _choose_action(
    env: Any,
    actions: list[Any],
    rng: random.Random,
    combat_policy: CombatSearchConfig,
) -> tuple[Any, str, dict[str, Any]]:
    if env.phase() == "combat":
        recommendation = search_combat(env, combat_policy)
        if recommendation.best_action is not None:
            return (
                recommendation.best_action,
                "portfolio_combat",
                {
                    "score": recommendation.score,
                    "nodes": recommendation.nodes,
                    "terminal_reason": recommendation.terminal_reason,
                    **recommendation.diagnostics,
                },
            )

    action = _random_viable_noncombat_action(env, actions, rng)
    return action, "random_noncombat", {"candidate_count": len(actions)}


def _random_viable_noncombat_action(env: Any, actions: list[Any], rng: random.Random) -> Any:
    shuffled = list(actions)
    rng.shuffle(shuffled)
    fallback = shuffled[0]
    for action in shuffled:
        child = env.clone()
        try:
            result = child.step(action)
        except Exception:
            continue
        if result.unsupported_reason and not child.exact_legal_actions():
            continue
        return action
    return fallback


def _find_action(actions: Iterable[Any], action_json: Any) -> Any | None:
    for action in actions:
        if action.json() == action_json:
            return action
    return None


def _rank_episode_dicts(episodes: list[dict[str, Any]]) -> list[dict[str, Any]]:
    by_candidate: dict[str, list[dict[str, Any]]] = {}
    for episode in episodes:
        by_candidate.setdefault(str(episode["candidate"]), []).append(episode)

    ranking = []
    for name, candidate_episodes in by_candidate.items():
        count = len(candidate_episodes)
        wins = sum(1 for episode in candidate_episodes if episode["won"])
        losses = sum(1 for episode in candidate_episodes if episode["lost"])
        ranking.append(
            {
                "candidate": name,
                "episodes": count,
                "wins": wins,
                "losses": losses,
                "win_rate": wins / count if count else 0.0,
                "mean_score": _mean(float(episode["final_score"]) for episode in candidate_episodes),
                "mean_final_hp": _mean(float(episode["final_hp"]) for episode in candidate_episodes),
                "mean_monster_hp": _mean(float(episode["monster_hp"]) for episode in candidate_episodes),
                "mean_actions": _mean(float(episode["actions"]) for episode in candidate_episodes),
                "mean_search_nodes": _mean(
                    float(episode["search_nodes"]) for episode in candidate_episodes
                ),
                "potion_roots": sum(1 for episode in candidate_episodes if episode["potion_count"] > 0),
                "potion_action_roots": sum(
                    1 for episode in candidate_episodes if episode["has_potion_actions"]
                ),
            }
        )
    return sorted(
        ranking,
        key=lambda row: (
            -float(row["win_rate"]),
            -float(row["mean_score"]),
            float(row["mean_search_nodes"]),
            row["candidate"],
        ),
    )


def _metadata(
    *,
    started_at: str,
    seed: str | None,
    ascension: int,
    start: str,
    random_seed: int,
    max_steps: int,
    combat_policy: CombatSearchConfig,
    initial_snapshot_json: str | None = None,
    initial_state_id: str | None = None,
    stop_reason: str | None = None,
) -> dict[str, Any]:
    return {
        "type": "metadata",
        "schema": 1,
        "source": "sim_selfplay",
        "started_at": started_at,
        "seed": seed,
        "ascension": ascension,
        "start": start,
        "random_seed": random_seed,
        "max_steps": max_steps,
        "combat_policy": combat_policy.__dict__,
        "noncombat_policy": "random_viable_v1",
        "initial_state_id": initial_state_id,
        "initial_snapshot_json": initial_snapshot_json,
        "stop_reason": stop_reason,
    }


def _step_record(
    *,
    step: int,
    before_hash: str,
    before_snapshot_json: str,
    before_summary: dict[str, Any],
    legal_actions: list[Any],
    action: Any,
    policy_name: str,
    policy_diagnostics: dict[str, Any],
    after_hash: str | None = None,
    after_snapshot_json: str | None = None,
    after_summary: dict[str, Any] | None = None,
    transition: Any | None = None,
    unsupported_reason: str | None = None,
    error: str | None = None,
) -> dict[str, Any]:
    return {
        "type": "step",
        "step": step,
        "before_hash": before_hash,
        "before_snapshot_json": before_snapshot_json,
        "before_summary": before_summary,
        "legal_actions": [_action_record(action) for action in legal_actions],
        "action_family": action.family(),
        "action_kind": action.kind(),
        "action_json": action.json(),
        "policy": policy_name,
        "policy_diagnostics": policy_diagnostics,
        "after_hash": after_hash,
        "after_snapshot_json": after_snapshot_json,
        "after_summary": after_summary,
        "transition": _transition_record(transition),
        "unsupported_reason": unsupported_reason,
        "error": error,
    }


def _action_record(action: Any) -> dict[str, Any]:
    return {"family": action.family(), "kind": action.kind(), "json": action.json()}


def _transition_record(transition: Any | None) -> dict[str, Any] | None:
    if transition is None:
        return None
    return {
        "action_json": transition.action_json,
        "previous_hash": transition.previous_hash,
        "resulting_hash": transition.resulting_hash,
        "events_json": transition.events_json,
        "rng_draws_json": transition.rng_draws_json,
        "simulator_error": transition.simulator_error,
    }


def _summary(env: Any) -> dict[str, Any]:
    state = json.loads(env.state_json())
    run = state.get("state", state)
    combat = run.get("combat") or {}
    player = combat.get("player") or {}
    return {
        "phase": env.phase(),
        "decision": env.current_decision(),
        "unsupported_reason": env.unsupported_reason(),
        "player_hp": run.get("player_hp") if run.get("player_hp") is not None else player.get("hp"),
        "player_max_hp": run.get("player_max_hp")
        if run.get("player_max_hp") is not None
        else player.get("max_hp"),
        "gold": run.get("gold"),
        "floor": run.get("current_floor"),
        "act": run.get("current_act"),
        "potions": run.get("potions") or [],
        "relics": run.get("relics") or [],
        "combat": {
            "energy": player.get("energy"),
            "hand": [card.get("content_id") for card in ((combat.get("piles") or {}).get("hand") or [])],
            "monsters": [
                {
                    "id": monster.get("id"),
                    "hp": monster.get("hp"),
                    "block": monster.get("block"),
                    "alive": monster.get("alive"),
                    "intent": monster.get("intent"),
                }
                for monster in combat.get("monsters", [])
            ],
        }
        if combat
        else None,
    }


def _write_jsonl(path: Path, records: Iterable[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, sort_keys=True))
            handle.write("\n")


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    with path.open("r", encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _split_for_state(state_id: str) -> str:
    digest = hashlib.sha256(state_id.encode("utf-8")).digest()[0]
    return "eval" if digest % 5 == 0 else "dev"


def _mean(values: Iterable[float]) -> float:
    values = list(values)
    return sum(values) / len(values) if values else 0.0


def _safe_file_stem(value: str) -> str:
    safe = "".join(char if char.isalnum() or char in {"-", "_"} else "_" for char in value)
    return safe[:80] or "empty"


def _parse_seed_specs(specs: Iterable[str]) -> list[str]:
    seeds: list[str] = []
    for spec in specs:
        for part in spec.split(","):
            part = part.strip()
            if not part:
                continue
            if ".." in part:
                start_text, end_text = part.split("..", 1)
                start = int(start_text)
                end = int(end_text)
                step = 1 if end >= start else -1
                seeds.extend(str(value) for value in range(start, end + step, step))
            else:
                seeds.append(part)
    return seeds


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Generate or verify simulator self-play traces.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    run_parser = subparsers.add_parser("run")
    run_parser.add_argument("--output", type=Path, required=True)
    run_parser.add_argument("--seed")
    run_parser.add_argument("--ascension", type=int, default=0)
    run_parser.add_argument("--start", choices=["seed", "map_fixture", "combat_fixture"], default="seed")
    run_parser.add_argument("--random-seed", type=int, default=0)
    run_parser.add_argument("--max-steps", type=int, default=200)

    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("trace", type=Path)

    batch_parser = subparsers.add_parser("batch")
    batch_parser.add_argument("--output-dir", type=Path, required=True)
    batch_parser.add_argument("--seeds", nargs="+", required=True)
    batch_parser.add_argument("--ascension", type=int, default=0)
    batch_parser.add_argument("--start", choices=["seed", "map_fixture", "combat_fixture"], default="seed")
    batch_parser.add_argument("--random-seed", type=int, default=0)
    batch_parser.add_argument("--max-steps", type=int, default=200)

    eval_parser = subparsers.add_parser("eval")
    eval_parser.add_argument("--corpus-dir", type=Path)
    eval_parser.add_argument("--trace", dest="traces", type=Path, action="append")
    eval_parser.add_argument("--split", choices=["dev", "eval", "all"], default="all")
    eval_parser.add_argument("--max-roots", type=int, default=64)
    eval_parser.add_argument("--max-actions", type=int, default=40)
    eval_parser.add_argument("--output", type=Path)

    args = parser.parse_args(argv)
    if args.command == "run":
        result = run_self_play(
            output=args.output,
            seed=args.seed,
            ascension=args.ascension,
            start=args.start,
            random_seed=args.random_seed,
            max_steps=args.max_steps,
        )
        print(json.dumps(result.__dict__ | {"trace_path": str(result.trace_path)}, indent=2))
    elif args.command == "verify":
        print(json.dumps(verify_self_play_trace(args.trace), indent=2, sort_keys=True))
    elif args.command == "batch":
        result = run_self_play_batch(
            output_dir=args.output_dir,
            seeds=_parse_seed_specs(args.seeds),
            ascension=args.ascension,
            start=args.start,
            random_seed=args.random_seed,
            max_steps=args.max_steps,
        )
        print(json.dumps(result.__dict__ | {"output_dir": str(result.output_dir), "index_path": str(result.index_path)}, indent=2, sort_keys=True))
    elif args.command == "eval":
        report = evaluate_self_play_corpus(
            corpus_dir=args.corpus_dir,
            traces=args.traces,
            split=args.split,
            max_roots=args.max_roots,
            max_actions=args.max_actions,
        )
        text = json.dumps(report, indent=2, sort_keys=True)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(text, encoding="utf-8")
        print(text)


if __name__ == "__main__":
    main()
