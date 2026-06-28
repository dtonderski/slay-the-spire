"""Deterministic non-ML combat-search benchmark helpers."""

from __future__ import annotations

from dataclasses import dataclass
import argparse
import heapq
import hashlib
import json
from pathlib import Path
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
    decision_trace: tuple[dict[str, Any], ...]
    terminal_reason: str | None


@dataclass(frozen=True)
class OracleProbeResult:
    fixture: str
    trace_step: int | None
    split: str | None
    initial_hp: float
    found_win: bool
    exhausted: bool
    nodes: int
    max_nodes: int
    max_actions: int
    best_terminal: str | None
    best_final_hp: float
    best_monster_hp: float
    best_score: float
    best_actions: tuple[str, ...]
    elapsed_seconds: float


SELECTED_COMBAT_AUTOPILOT_CANDIDATE = "rust_terminal_win_hp_selector_w32_w128_no_power_d40"


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
            "rust_terminal_rescue_keyed_w32_w128_no_power_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_terminal_rescue_keyed",
            ),
        ),
        SearchCandidate(
            "rust_terminal_win_hp_selector_w32_w128_no_power_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_terminal_win_hp_selector",
            ),
        ),
        SearchCandidate(
            "rust_terminal_hp_selector_w32_w64_w128_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_terminal_hp_selector",
            ),
        ),
        SearchCandidate(
            "rust_terminal_low_hp_rollout_selector_w32_w128_no_power_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_terminal_low_hp_rollout_selector",
            ),
        ),
        SearchCandidate(
            "rust_terminal_rollout_selector_w32_w128_no_power_d40",
            CombatSearchConfig(
                max_depth=40,
                objective="terminal_tactical",
                algorithm="rust_terminal_rollout_selector",
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


def trace_autopilot_candidate_by_name(name: str) -> SearchCandidate:
    available = {candidate.name: candidate for candidate in trace_autopilot_candidates()}
    try:
        return available[name]
    except KeyError as exc:
        choices = ", ".join(sorted(available))
        raise ValueError(f"unknown trace eval candidate: {name}; choices: {choices}") from exc


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
    decision_trace: list[dict[str, Any]] = []
    terminal = _terminal_reason(env)

    while terminal is None and actions_taken < max_actions:
        started_at = time.perf_counter()
        recommendation = search_combat(env, candidate.config)
        elapsed = time.perf_counter() - started_at
        decision_seconds.append(elapsed)
        search_nodes += recommendation.visits
        if recommendation.best_action is None:
            decision_trace.append(
                _decision_trace_row(
                    actions_taken,
                    recommendation,
                    elapsed,
                    potion_name=None,
                )
            )
            break
        actions_to_apply = (
            tuple(recommendation.principal_variation)
            if recommendation.diagnostics.get("follow_principal_variation")
            else (recommendation.best_action,)
        )
        remaining_actions = max_actions - actions_taken
        for planned_index, action in enumerate(actions_to_apply[:remaining_actions]):
            potion_name = None
            if getattr(action, "kind", lambda: "")() == "use_potion":
                potion_name = _potion_name_for_action(env, action)
                potion_use_names.append(potion_name)
            decision_trace.append(
                _decision_trace_row(
                    actions_taken,
                    recommendation,
                    elapsed if planned_index == 0 else 0.0,
                    potion_name=potion_name,
                    action=action,
                    planned_index=planned_index,
                )
            )
            env.step(action)
            actions_taken += 1
            terminal = _terminal_reason(env)
            if terminal is not None:
                break

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
        decision_trace=tuple(decision_trace),
        terminal_reason=terminal,
    )


def _decision_trace_row(
    index: int,
    recommendation: SearchRecommendation,
    seconds: float,
    *,
    potion_name: str | None,
    action: Any | None = None,
    planned_index: int = 0,
) -> dict[str, Any]:
    diagnostics = recommendation.diagnostics
    action = recommendation.best_action if action is None else action
    return {
        "index": index,
        "best_action": action.json() if action is not None else None,
        "planned_index": planned_index,
        "potion_name": potion_name,
        "terminal_reason": recommendation.terminal_reason,
        "value": recommendation.value,
        "nodes": recommendation.visits,
        "seconds": seconds,
        "algorithm": diagnostics.get("algorithm"),
        "objective": diagnostics.get("objective"),
        "beam_width": diagnostics.get("beam_width"),
        "rust_final_hp": diagnostics.get("rust_final_hp"),
        "rust_monster_hp": diagnostics.get("rust_monster_hp"),
        "rust_actions": diagnostics.get("rust_actions"),
        "selector_candidates": diagnostics.get("selector_candidates"),
        "portfolio_candidates": diagnostics.get("portfolio_candidates"),
        "fallback_algorithm": diagnostics.get("fallback_algorithm"),
        "rust_search_unavailable": diagnostics.get("rust_search_unavailable"),
    }


def probe_failure_fixture_oracles(
    failure_path: Path,
    *,
    max_actions: int = 16,
    max_nodes: int = 50_000,
    allowed_potions: tuple[str, ...] | None = None,
) -> dict[str, Any]:
    payload = json.loads(failure_path.read_text(encoding="utf-8"))
    fixtures = list(payload.get("fixtures") or [])
    results = [
        _probe_failure_fixture_oracle(
            fixture,
            max_actions=max_actions,
            max_nodes=max_nodes,
            allowed_potions=allowed_potions,
        )
        for fixture in fixtures
    ]
    rows = [result.__dict__ for result in results]
    return {
        "type": "combat_autopilot_failure_oracle_probe",
        "schema": 1,
        "source": "sts.search_lab",
        "failure_path": str(failure_path),
        "fixtures": rows,
        "fixture_count": len(rows),
        "wins_found": sum(1 for result in results if result.found_win),
        "exhausted": sum(1 for result in results if result.exhausted),
        "max_actions": max_actions,
        "max_nodes": max_nodes,
        "allowed_potions": allowed_potions,
    }


def _probe_failure_fixture_oracle(
    fixture: dict[str, Any],
    *,
    max_actions: int,
    max_nodes: int,
    allowed_potions: tuple[str, ...] | None,
) -> OracleProbeResult:
    started_at = time.perf_counter()
    env = omni.OmniRunEnv.from_snapshot_json(str(fixture["snapshot_json"]))
    initial_hp = float(_state(env).get("player", {}).get("hp", 0))
    nodes = 0
    best_env = env
    best_terminal = _terminal_reason(env)
    best_actions: tuple[str, ...] = ()
    best_score = _oracle_score(env, best_terminal)
    frontier: list[tuple[tuple[float, float, int, tuple[str, ...]], int, Any, tuple[str, ...]]] = []
    sequence = 0
    heapq.heappush(frontier, (_oracle_priority(env, best_terminal, ()), sequence, env, ()))
    seen_depth: dict[str, int] = {}
    found_win = False

    while frontier and nodes < max_nodes:
        _priority, _sequence, current, actions = heapq.heappop(frontier)
        state_id = _safe_snapshot_hash(current)
        previous_depth = seen_depth.get(state_id)
        if previous_depth is not None and previous_depth <= len(actions):
            continue
        seen_depth[state_id] = len(actions)
        nodes += 1

        terminal = _terminal_reason(current)
        score = _oracle_score(current, terminal)
        if score > best_score or (score == best_score and len(actions) < len(best_actions)):
            best_env = current
            best_terminal = terminal
            best_actions = actions
            best_score = score
        if terminal == "won":
            found_win = True
            best_env = current
            best_terminal = terminal
            best_actions = actions
            best_score = score
            break
        if terminal == "lost" or len(actions) >= max_actions:
            continue

        legal_actions = _filtered_oracle_actions(current, allowed_potions)
        for action in legal_actions:
            child = current.clone()
            try:
                result = child.step(action)
            except Exception:
                continue
            child_terminal = _terminal_reason(child)
            if getattr(result, "terminal", False):
                child_terminal = result.terminal_reason
            child_actions = (*actions, action.json())
            sequence += 1
            heapq.heappush(
                frontier,
                (
                    _oracle_priority(child, child_terminal, child_actions),
                    sequence,
                    child,
                    child_actions,
                ),
            )

    final_state = _state(best_env)
    return OracleProbeResult(
        fixture=str(fixture.get("name") or "fixture"),
        trace_step=(
            int(fixture["trace_step"])
            if isinstance(fixture.get("trace_step"), int)
            else None
        ),
        split=str(fixture.get("split")) if fixture.get("split") is not None else None,
        initial_hp=initial_hp,
        found_win=found_win,
        exhausted=not frontier,
        nodes=nodes,
        max_nodes=max_nodes,
        max_actions=max_actions,
        best_terminal=best_terminal,
        best_final_hp=float(final_state.get("player", {}).get("hp", 0)),
        best_monster_hp=_monster_hp(final_state),
        best_score=best_score,
        best_actions=best_actions,
        elapsed_seconds=time.perf_counter() - started_at,
    )


def _oracle_priority(
    env: Any,
    terminal: str | None,
    actions: tuple[str, ...],
) -> tuple[float, float, int, tuple[str, ...]]:
    state = _state(env)
    hp = float(state.get("player", {}).get("hp", 0))
    monster_hp = _monster_hp(state)
    terminal_rank = 0 if terminal == "won" else 2 if terminal == "lost" else 1
    return (float(terminal_rank), monster_hp - hp * 0.25, len(actions), actions)


def _oracle_score(env: Any, terminal: str | None) -> float:
    state = _state(env)
    hp = float(state.get("player", {}).get("hp", 0))
    monster_hp = _monster_hp(state)
    score = hp * 100.0 - monster_hp * 20.0
    if terminal == "won":
        score += 1_000_000.0
    elif terminal == "lost":
        score -= 1_000_000.0
    return score


def _filtered_oracle_actions(
    env: Any,
    allowed_potions: tuple[str, ...] | None,
) -> list[Any]:
    actions = _sorted_actions(env.exact_legal_actions())
    if allowed_potions is None:
        return actions
    allowed = {_normalize_potion_name(name) for name in allowed_potions}
    potions = _run_potion_names(env)
    filtered = []
    for action in actions:
        if getattr(action, "kind", lambda: "")() != "use_potion":
            filtered.append(action)
            continue
        slot = _potion_action_slot(action)
        potion_name = potions[slot] if slot is not None and slot < len(potions) else None
        if potion_name is not None and _normalize_potion_name(potion_name) in allowed:
            filtered.append(action)
    return filtered


def _run_potion_names(env: Any) -> list[str]:
    state = json.loads(env.state_json())
    run = state.get("state", state)
    return [str(potion) for potion in run.get("potions") or []]


def _potion_action_slot(action: Any) -> int | None:
    try:
        data = json.loads(action.json())
    except Exception:
        return None
    use = data.get("UsePotion") if isinstance(data, dict) else None
    slot = use.get("slot") if isinstance(use, dict) else None
    return int(slot) if isinstance(slot, int) else None


def _normalize_potion_name(name: str) -> str:
    normalized = "".join(char.lower() for char in name if char.isalnum())
    return normalized.removesuffix("potion")


def _safe_snapshot_hash(env: Any) -> str:
    try:
        return str(env.snapshot_hash())
    except Exception:
        return hashlib.sha256(env.snapshot_json().encode("utf-8")).hexdigest()


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
    subparsers = parser.add_subparsers(dest="command")

    benchmark_parser = subparsers.add_parser("benchmark")
    benchmark_parser.add_argument("--split", choices=["dev", "eval", "all"], default="eval")
    benchmark_parser.add_argument("--max-source-depth", type=int, default=5)
    benchmark_parser.add_argument("--max-roots", type=int, default=48)
    benchmark_parser.add_argument("--max-actions", type=int, default=40)
    benchmark_parser.add_argument("--json", action="store_true")

    oracle_parser = subparsers.add_parser("oracle-failures")
    oracle_parser.add_argument("failure_path", type=Path)
    oracle_parser.add_argument("--max-actions", type=int, default=16)
    oracle_parser.add_argument("--max-nodes", type=int, default=50_000)
    oracle_parser.add_argument("--allowed-potions")
    oracle_parser.add_argument("--output", type=Path)
    oracle_parser.add_argument("--json", action="store_true")

    parser.add_argument("--split", choices=["dev", "eval", "all"], default="eval")
    parser.add_argument("--max-source-depth", type=int, default=5)
    parser.add_argument("--max-roots", type=int, default=48)
    parser.add_argument("--max-actions", type=int, default=40)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    if args.command == "oracle-failures":
        report = probe_failure_fixture_oracles(
            args.failure_path,
            max_actions=args.max_actions,
            max_nodes=args.max_nodes,
            allowed_potions=_parse_allowed_potions(args.allowed_potions),
        )
        if args.output is not None:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
        if args.json or args.output is None:
            print(json.dumps(report, indent=2, sort_keys=True))
            return
        print(
            f"fixtures={report['fixture_count']} wins_found={report['wins_found']} "
            f"exhausted={report['exhausted']} output={args.output}"
        )
        return

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


def _parse_allowed_potions(value: str | None) -> tuple[str, ...] | None:
    if value is None:
        return None
    stripped = value.strip()
    if stripped.lower() in {"*", "all"}:
        return None
    if not stripped or stripped.lower() in {"none", "no", "false"}:
        return ()
    return tuple(part.strip() for part in stripped.split(",") if part.strip())


if __name__ == "__main__":
    main()
