"""Deterministic non-ML combat-search benchmark helpers."""

from __future__ import annotations

from dataclasses import dataclass
import argparse
import hashlib
import json
import time
from typing import Any, Iterable

from sts import omni
from sts.search import CombatSearchConfig, SearchRecommendation, search_combat


@dataclass(frozen=True)
class BenchmarkRoot:
    name: str
    env_kind: str
    snapshot_json: str
    state_id: str
    source_depth: int
    split: str


@dataclass(frozen=True)
class SearchCandidate:
    name: str
    config: CombatSearchConfig


@dataclass(frozen=True)
class EpisodeResult:
    root_name: str
    candidate: str
    split: str
    won: bool
    lost: bool
    final_score: float
    initial_hp: float
    final_hp: float
    hp_loss: float
    monster_hp: float
    actions: int
    search_nodes: int
    potion_uses: int
    potion_use_names: tuple[str, ...]
    search_seconds: float
    mean_seconds_per_decision: float
    p50_seconds_per_decision: float
    p95_seconds_per_decision: float
    decision_seconds: tuple[float, ...]
    terminal_reason: str | None


def default_candidates() -> list[SearchCandidate]:
    return [
        SearchCandidate(
            "autopilot_hp_greedy_d40",
            CombatSearchConfig(max_depth=40, objective="hp_preserving_lethal", algorithm="greedy"),
        ),
        SearchCandidate(
            "autopilot_hp_portfolio_d40",
            CombatSearchConfig(max_depth=40, objective="hp_preserving_lethal", algorithm="portfolio", beam_width=8),
        ),
        SearchCandidate(
            "exhaustive_basic_d3",
            CombatSearchConfig(max_depth=3, objective="survive_then_damage", algorithm="exhaustive"),
        ),
        SearchCandidate(
            "exhaustive_tactical_d4",
            CombatSearchConfig(max_depth=4, objective="tactical_survival", algorithm="exhaustive"),
        ),
        SearchCandidate(
            "greedy_tactical_d20",
            CombatSearchConfig(max_depth=20, objective="tactical_survival", algorithm="greedy"),
        ),
        SearchCandidate(
            "beam_tactical_w4_d30",
            CombatSearchConfig(max_depth=30, objective="tactical_survival", algorithm="beam", beam_width=4),
        ),
        SearchCandidate(
            "beam_aggressive_w4_d30",
            CombatSearchConfig(max_depth=30, objective="aggressive_lethal", algorithm="beam", beam_width=4),
        ),
        SearchCandidate(
            "beam_tactical_w8_d40",
            CombatSearchConfig(max_depth=40, objective="tactical_survival", algorithm="beam", beam_width=8),
        ),
        SearchCandidate(
            "portfolio_rollout_d40",
            CombatSearchConfig(max_depth=40, objective="aggressive_lethal", algorithm="portfolio", beam_width=12),
        ),
    ]


def trace_autopilot_candidates() -> list[SearchCandidate]:
    return [
        SearchCandidate(
            "tactical_greedy_d40",
            CombatSearchConfig(max_depth=40, objective="tactical_survival", algorithm="greedy"),
        ),
        SearchCandidate(
            "hp_greedy_d40",
            CombatSearchConfig(max_depth=40, objective="hp_preserving_lethal", algorithm="greedy"),
        ),
        SearchCandidate(
            "trace_probe_d40",
            CombatSearchConfig(max_depth=40, objective="tactical_survival", algorithm="trace_probe"),
        ),
        SearchCandidate(
            "trace_probe_potion_rescue_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="tactical_survival",
                algorithm="potion_rescue_trace_probe",
            ),
        ),
        SearchCandidate(
            "trace_probe_aggressive_rescue_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="tactical_survival",
                algorithm="aggressive_rescue_trace_probe",
            ),
        ),
        SearchCandidate(
            "trace_probe_no_potions_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="tactical_survival",
                algorithm="trace_probe",
                allowed_potions=(),
            ),
        ),
        SearchCandidate(
            "rust_greedy_tactical_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="tactical_survival",
                algorithm="rust_greedy",
            ),
        ),
        SearchCandidate(
            "rust_beam_tactical_w16_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="tactical_survival",
                algorithm="rust_beam",
                beam_width=16,
            ),
        ),
        SearchCandidate(
            "rust_beam_terminal_w16_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_beam",
                beam_width=16,
            ),
        ),
        SearchCandidate(
            "rust_beam_terminal_w32_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_beam",
                beam_width=32,
            ),
        ),
        SearchCandidate(
            "rust_beam_terminal_w128_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_beam",
                beam_width=128,
            ),
        ),
        SearchCandidate(
            "rust_beam_terminal_w128_no_power_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_beam",
                beam_width=128,
                allowed_potions=(
                    "Weak Potion",
                    "Cultist Potion",
                    "Flex Potion",
                    "Elixir",
                    "Distilled Chaos",
                    "Explosive Potion",
                ),
            ),
        ),
        SearchCandidate(
            "rust_terminal_rescue_w32_w128_no_power_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_terminal_rescue",
            ),
        ),
        SearchCandidate(
            "rust_terminal_portfolio_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_terminal_portfolio",
            ),
        ),
    ]


def generate_roots(max_source_depth: int = 5, max_roots: int = 48) -> list[BenchmarkRoot]:
    starts = _synthetic_start_envs()
    queue: list[tuple[Any, int]] = [(start, 0) for start in starts]
    seen: set[str] = set()
    roots: list[BenchmarkRoot] = []

    while queue and len(roots) < max_roots:
        env, depth = queue.pop(0)
        state_id = env.snapshot_hash()
        if state_id in seen:
            continue
        seen.add(state_id)

        if _is_searchable(env):
            roots.append(
                BenchmarkRoot(
                    name=f"combat-depth{depth}-{state_id}",
                    env_kind="combat",
                    snapshot_json=env.snapshot_json(),
                    state_id=state_id,
                    source_depth=depth,
                    split=_split_for_state(state_id),
                )
            )

        if depth >= max_source_depth or _terminal_reason(env):
            continue
        for action in _sorted_actions(env.exact_legal_actions()):
            child = env.clone()
            try:
                child.step(action)
            except Exception:
                continue
            if _is_searchable(child):
                queue.append((child, depth + 1))

    return roots


def _synthetic_start_envs() -> list[Any]:
    base = json.loads(omni.OmniCombatEnv.initial_fixture().state_json())
    cases = []
    enemy_groups = [
        [(18, 6)],
        [(32, 12)],
        [(44, 18)],
        [(10, 4), (14, 5)],
        [(12, 5), (16, 6)],
        [(8, 3), (10, 4), (12, 5)],
        [(10, 4), (12, 5), (14, 6)],
    ]
    for player_hp in [40, 60, 72]:
        for enemies in enemy_groups:
            state = json.loads(json.dumps(base))
            state["player"]["hp"] = player_hp
            state["player"]["max_hp"] = 80
            state["player"]["block"] = 0
            state["player"]["energy"] = 3
            state["monsters"] = []
            for index, (monster_hp, incoming) in enumerate(enemies, start=1):
                monster = json.loads(json.dumps(base["monsters"][0]))
                monster["id"] = index
                monster["hp"] = monster_hp
                monster["block"] = 0
                monster["alive"] = True
                monster["intent"] = {"Attack": {"damage": incoming}}
                state["monsters"].append(monster)
            state["piles"] = _starter_piles()
            cases.append(omni.OmniCombatEnv.from_state_json(json.dumps(state)))
    return cases


def _starter_piles() -> dict[str, list[dict[str, Any]]]:
    contents = [
        1,
        2,
        3,
        1,
        2,
        1,
        1,
        2,
        1,
        2,
        1,
        1,
        2,
        1,
        3,
    ]
    cards = [
        {"id": index + 1, "content_id": content_id, "temp_cost": None, "combat_only": False}
        for index, content_id in enumerate(contents)
    ]
    return {
        "hand": cards[:5],
        "draw_pile": cards[5:],
        "discard_pile": [],
        "exhaust_pile": [],
    }


def evaluate_candidate(
    root: BenchmarkRoot,
    candidate: SearchCandidate,
    *,
    max_actions: int = 40,
) -> EpisodeResult:
    env = _env_from_root(root)
    initial_hp = float(_state(env).get("player", {}).get("hp", 0))
    actions_taken = 0
    search_nodes = 0
    potion_use_names: list[str] = []
    decision_seconds: list[float] = []
    terminal = _terminal_reason(env)

    while terminal is None and actions_taken < max_actions:
        started_at = time.perf_counter()
        recommendation = search_combat(env, candidate.config)
        decision_seconds.append(time.perf_counter() - started_at)
        search_nodes += recommendation.visits
        if recommendation.best_action is None:
            break
        if getattr(recommendation.best_action, "kind", lambda: "")() == "use_potion":
            potion_use_names.append(_potion_name_for_action(env, recommendation.best_action))
        env.step(recommendation.best_action)
        actions_taken += 1
        terminal = _terminal_reason(env)

    final_state = _state(env)
    won = terminal == "won"
    lost = terminal == "lost"
    return EpisodeResult(
        root_name=root.name,
        candidate=candidate.name,
        split=root.split,
        won=won,
        lost=lost,
        final_score=_outcome_score(final_state, terminal),
        initial_hp=initial_hp,
        final_hp=float(final_state.get("player", {}).get("hp", 0)),
        hp_loss=initial_hp - float(final_state.get("player", {}).get("hp", 0)),
        monster_hp=_monster_hp(final_state),
        actions=actions_taken,
        search_nodes=search_nodes,
        potion_uses=len(potion_use_names),
        potion_use_names=tuple(potion_use_names),
        search_seconds=sum(decision_seconds),
        mean_seconds_per_decision=_mean(decision_seconds),
        p50_seconds_per_decision=_percentile(decision_seconds, 50),
        p95_seconds_per_decision=_percentile(decision_seconds, 95),
        decision_seconds=tuple(decision_seconds),
        terminal_reason=terminal,
    )


def run_benchmark(
    *,
    split: str = "eval",
    max_source_depth: int = 5,
    max_roots: int = 48,
    max_actions: int = 40,
    candidates: Iterable[SearchCandidate] | None = None,
) -> dict[str, Any]:
    roots = [root for root in generate_roots(max_source_depth, max_roots) if split == "all" or root.split == split]
    candidates = list(candidates or default_candidates())
    results = [
        evaluate_candidate(root, candidate, max_actions=max_actions)
        for candidate in candidates
        for root in roots
    ]
    return {
        "benchmark": {
            "split": split,
            "roots": len(roots),
            "max_source_depth": max_source_depth,
            "max_actions": max_actions,
            "mean_start_hp": _mean_start_hp(roots),
            "root_names": [root.name for root in roots],
        },
        "ranking": _rank(results),
        "episodes": [result.__dict__ for result in results],
    }


def _rank(results: list[EpisodeResult]) -> list[dict[str, Any]]:
    by_candidate: dict[str, list[EpisodeResult]] = {}
    for result in results:
        by_candidate.setdefault(result.candidate, []).append(result)

    ranking = []
    for name, candidate_results in by_candidate.items():
        count = len(candidate_results)
        wins = sum(1 for result in candidate_results if result.won)
        losses = sum(1 for result in candidate_results if result.lost)
        nonterminal = count - wins - losses
        ranking.append(
            {
                "candidate": name,
                "episodes": count,
                "wins": wins,
                "losses": losses,
                "nonterminal": nonterminal,
                "win_rate": wins / count if count else 0.0,
                "mean_score": _mean(result.final_score for result in candidate_results),
                "mean_hp_loss": _mean(result.hp_loss for result in candidate_results),
                "median_hp_loss": _percentile(
                    (result.hp_loss for result in candidate_results), 50
                ),
                "p95_hp_loss": _percentile(
                    (result.hp_loss for result in candidate_results), 95
                ),
                "mean_final_hp": _mean(result.final_hp for result in candidate_results),
                "mean_monster_hp": _mean(result.monster_hp for result in candidate_results),
                "mean_actions": _mean(result.actions for result in candidate_results),
                "mean_potion_uses": _mean(result.potion_uses for result in candidate_results),
                "total_potion_uses": sum(result.potion_uses for result in candidate_results),
                "potion_use_counts": _potion_use_counts(candidate_results),
                "mean_seconds_per_combat": _mean(
                    result.search_seconds for result in candidate_results
                ),
                "mean_seconds_per_decision": _mean(
                    result.mean_seconds_per_decision for result in candidate_results
                ),
                "p50_seconds_per_decision": _percentile(
                    (
                        second
                        for result in candidate_results
                        for second in result.decision_seconds
                    ),
                    50,
                ),
                "p95_seconds_per_decision": _percentile(
                    (
                        second
                        for result in candidate_results
                        for second in result.decision_seconds
                    ),
                    95,
                ),
                "mean_search_nodes": _mean(result.search_nodes for result in candidate_results),
                "p95_search_nodes": _percentile(
                    (result.search_nodes for result in candidate_results), 95
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


def _outcome_score(state: dict[str, Any], terminal: str | None) -> float:
    score = float(state.get("player", {}).get("hp", 0)) * 100.0 - _monster_hp(state) * 20.0
    if terminal == "won":
        score += 100_000.0
    elif terminal == "lost":
        score -= 100_000.0
    return score


def _state(env: Any) -> dict[str, Any]:
    state = json.loads(env.state_json())
    if isinstance(state.get("combat"), dict):
        return state["combat"]
    if "player" in state and "monsters" in state:
        return state
    return {
        "player": {"hp": state.get("player_hp", 0), "block": 0, "energy": 0},
        "monsters": [],
        "phase": state.get("phase"),
    }


def _is_searchable(env: Any) -> bool:
    try:
        state = _state(env)
        return "player" in state and "monsters" in state and bool(env.exact_legal_actions())
    except Exception:
        return False


def _terminal_reason(env: Any) -> str | None:
    phase = getattr(env, "phase", lambda: None)()
    if phase == "won":
        return "won"
    if phase == "lost":
        return "lost"
    if phase and phase not in {"combat", "waiting_for_player"}:
        return "won"
    state = _state(env)
    if state.get("phase") == "Won":
        return "won"
    if state.get("phase") == "Lost":
        return "lost"
    if float(state.get("player", {}).get("hp", 1)) <= 0:
        return "lost"
    monsters = state.get("monsters", [])
    if monsters and not any(monster.get("alive", False) for monster in monsters):
        return "won"
    return None


def _env_from_root(root: BenchmarkRoot) -> Any:
    if root.env_kind == "run_combat":
        return omni.OmniRunEnv.from_snapshot_json(root.snapshot_json)
    return omni.OmniCombatEnv.from_snapshot_json(root.snapshot_json)


def _monster_hp(state: dict[str, Any]) -> float:
    return sum(float(monster.get("hp", 0)) for monster in state.get("monsters", []) if monster.get("alive", False))


def _mean_start_hp(roots: Iterable[BenchmarkRoot]) -> float:
    roots = list(roots)
    return _mean(float(_state(_env_from_root(root)).get("player", {}).get("hp", 0)) for root in roots)


def _sorted_actions(actions: Iterable[Any]) -> list[Any]:
    return sorted(actions, key=lambda action: (action.kind(), action.json()))


def _potion_use_counts(results: Iterable[EpisodeResult]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for result in results:
        for name in result.potion_use_names:
            counts[name] = counts.get(name, 0) + 1
    return dict(sorted(counts.items()))


def _potion_name_for_action(env: Any, action: Any) -> str:
    data = _action_json_data(action)
    use = data.get("UsePotion") if isinstance(data, dict) else None
    slot = use.get("slot") if isinstance(use, dict) else None
    if not isinstance(slot, int):
        return "unknown"
    state = _raw_state(env)
    potions = state.get("potions") or []
    if 0 <= slot < len(potions):
        return str(potions[slot])
    return "unknown"


def _raw_state(env: Any) -> dict[str, Any]:
    state = json.loads(env.state_json())
    run = state.get("state")
    return run if isinstance(run, dict) else state


def _action_json_data(action: Any) -> dict[str, Any]:
    try:
        data = json.loads(action.json())
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def _split_for_state(state_id: str) -> str:
    digest = hashlib.sha256(state_id.encode("utf-8")).digest()[0]
    return "eval" if digest % 5 == 0 else "dev"


def _mean(values: Iterable[float]) -> float:
    values = list(values)
    return sum(values) / len(values) if values else 0.0


def _percentile(values: Iterable[float], percentile: float) -> float:
    values = sorted(float(value) for value in values)
    if not values:
        return 0.0
    if len(values) == 1:
        return values[0]
    rank = (len(values) - 1) * percentile / 100.0
    lower = int(rank)
    upper = min(lower + 1, len(values) - 1)
    weight = rank - lower
    return values[lower] * (1.0 - weight) + values[upper] * weight


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="Run deterministic combat search candidate benchmarks.")
    parser.add_argument("--split", choices=["dev", "eval", "all"], default="eval")
    parser.add_argument("--max-source-depth", type=int, default=5)
    parser.add_argument("--max-roots", type=int, default=48)
    parser.add_argument("--max-actions", type=int, default=40)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    report = run_benchmark(
        split=args.split,
        max_source_depth=args.max_source_depth,
        max_roots=args.max_roots,
        max_actions=args.max_actions,
    )
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
        return
    print(
        f"split={report['benchmark']['split']} roots={report['benchmark']['roots']} "
        f"mean_start_hp={report['benchmark']['mean_start_hp']:.1f}"
    )
    for index, row in enumerate(report["ranking"], start=1):
        print(
            f"{index}. {row['candidate']}: win_rate={row['win_rate']:.2f} "
            f"score={row['mean_score']:.1f} hp={row['mean_final_hp']:.1f} "
            f"monster_hp={row['mean_monster_hp']:.1f} nodes={row['mean_search_nodes']:.1f}"
        )


if __name__ == "__main__":
    main()
