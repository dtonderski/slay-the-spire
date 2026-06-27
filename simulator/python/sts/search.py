"""Small deterministic omniscient combat search helpers."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Iterable, Sequence

from sts.omni import ExactCombatAction, OmniCombatEnv


@dataclass(frozen=True)
class CombatSearchConfig:
    """Configuration for the first deterministic omniscient combat search."""

    max_depth: int = 1
    objective: str = "survive_then_damage"


@dataclass(frozen=True)
class SearchRecommendation:
    """A deterministic omniscient recommendation for the current combat state."""

    best_action: ExactCombatAction | None
    principal_variation: tuple[ExactCombatAction, ...]
    visits: int
    value: float
    win_probability: float | None
    expected_hp_delta: float | None
    terminal_rate: float
    diagnostics: dict[str, Any]
    terminal_reason: str | None = None

    @property
    def action(self) -> ExactCombatAction | None:
        return self.best_action

    @property
    def score(self) -> float:
        return self.value

    @property
    def nodes(self) -> int:
        return self.visits

    @property
    def depth(self) -> int:
        return int(self.diagnostics["max_depth"])


CombatSearchResult = SearchRecommendation


def recommend_action(env: OmniCombatEnv, depth: int = 1) -> CombatSearchResult:
    """Return the best exact action found by a tiny deterministic depth search.

    The search is intentionally omniscient: it uses exact simulator state, exact
    legal actions, and cloned environments. The supplied root environment is not
    mutated.
    """

    return search_combat(env, CombatSearchConfig(max_depth=depth))


def search_combat(env: Any, config: CombatSearchConfig | None = None) -> SearchRecommendation:
    """Search combat from a branch of the supplied omniscient environment."""

    config = config or CombatSearchConfig()
    depth = config.max_depth
    if depth < 1:
        raise ValueError("max_depth must be at least 1")
    if config.objective != "survive_then_damage":
        raise ValueError(f"unsupported objective: {config.objective}")

    _state(env)
    score, variation, nodes, terminal_reason = _search(env.clone(), depth)
    return SearchRecommendation(
        best_action=variation[0] if variation else None,
        principal_variation=tuple(variation),
        visits=nodes,
        value=score,
        win_probability=_terminal_probability(terminal_reason, won=1.0, lost=0.0),
        expected_hp_delta=None,
        terminal_rate=1.0 if terminal_reason else 0.0,
        diagnostics={
            "max_depth": depth,
            "objective": config.objective,
            "algorithm": "deterministic_depth_search",
            "unsupported_transitions": 0,
        },
        terminal_reason=terminal_reason,
    )


def _search(env: Any, depth: int) -> tuple[float, list[Any], int, str | None]:
    state = _state(env)
    terminal_reason = _terminal_reason(env, state)
    if terminal_reason == "won":
        return 1_000_000.0 + _evaluate_state(state), [], 1, "won"
    if terminal_reason == "lost":
        return -1_000_000.0 + _evaluate_state(state), [], 1, "lost"
    if depth <= 0:
        return _evaluate_state(state), [], 1, None

    actions = _sorted_actions(env.exact_legal_actions())
    if not actions:
        return _evaluate_state(state), [], 1, None

    best_score = float("-inf")
    best_variation: list[Any] = []
    best_terminal_reason: str | None = None
    nodes = 1

    for action in actions:
        child = env.clone()
        result = child.step(action)
        child_terminal = _result_terminal_reason(result, child)
        if child_terminal:
            child_score = _terminal_score(child_terminal, child)
            child_variation: list[Any] = []
            child_nodes = 1
            child_terminal_reason = child_terminal
        else:
            child_score, child_variation, child_nodes, child_terminal_reason = _search(
                child, depth - 1
            )

        nodes += child_nodes
        candidate_variation = [action, *child_variation]
        if _is_better(candidate_variation, child_score, best_variation, best_score):
            best_score = child_score
            best_variation = candidate_variation
            best_terminal_reason = child_terminal_reason

    return best_score, best_variation, nodes, best_terminal_reason


def _terminal_score(reason: str | None, env: Any) -> float:
    state_score = _evaluate_state(_state(env))
    if reason == "won":
        return 1_000_000.0 + state_score
    if reason == "lost":
        return -1_000_000.0 + state_score
    return state_score


def _evaluate_state(state: dict[str, Any]) -> float:
    player = state.get("player", {})
    monsters = state.get("monsters", [])
    alive_monsters = [monster for monster in monsters if monster.get("alive", False)]

    player_hp = float(player.get("hp", 0))
    player_block = float(player.get("block", 0))
    player_energy = float(player.get("energy", 0))
    monster_hp = sum(float(monster.get("hp", 0)) for monster in alive_monsters)
    monster_block = sum(float(monster.get("block", 0)) for monster in alive_monsters)

    return (
        player_hp * 10.0
        + player_block * 1.5
        + player_energy * 0.25
        - monster_hp * 3.0
        - monster_block * 0.5
        - len(alive_monsters) * 25.0
    )


def _state(env: Any) -> dict[str, Any]:
    state = json.loads(env.state_json())
    if isinstance(state.get("combat"), dict):
        if getattr(env, "phase", lambda: None)() != "combat":
            raise ValueError("combat search requires a run session currently in combat")
        return state["combat"]
    if "player" in state and "monsters" in state:
        return state
    raise ValueError("combat search requires a combat state")


def _sorted_actions(actions: Iterable[Any]) -> list[Any]:
    return sorted(actions, key=_action_key)


def _is_better(
    candidate_variation: Sequence[Any],
    candidate_score: float,
    best_variation: Sequence[Any],
    best_score: float,
) -> bool:
    if candidate_score != best_score:
        return candidate_score > best_score
    return _variation_key(candidate_variation) < _variation_key(best_variation)


def _variation_key(variation: Sequence[Any]) -> tuple[tuple[str, str], ...]:
    return tuple(_action_key(action) for action in variation)


def _action_key(action: Any) -> tuple[str, str]:
    family = getattr(action, "family", lambda: "combat")()
    return (f"{family}:{action.kind()}", action.json())


def _terminal_reason(env: Any, state: dict[str, Any]) -> str | None:
    phase = state.get("phase")
    if phase == "Won":
        return "won"
    if phase == "Lost":
        return "lost"
    run_phase = getattr(env, "phase", lambda: None)()
    if run_phase == "won":
        return "won"
    if run_phase == "lost":
        return "lost"
    return None


def _result_terminal_reason(result: Any, child: Any) -> str | None:
    if getattr(result, "terminal", False):
        return result.terminal_reason
    try:
        return _terminal_reason(child, _state(child))
    except ValueError:
        run_phase = getattr(child, "phase", lambda: None)()
        if run_phase == "lost":
            return "lost"
        if run_phase and run_phase != "combat":
            return "won"
        return None


def _terminal_probability(reason: str | None, won: float, lost: float) -> float | None:
    if reason == "won":
        return won
    if reason == "lost":
        return lost
    return None


__all__ = [
    "CombatSearchConfig",
    "CombatSearchResult",
    "SearchRecommendation",
    "recommend_action",
    "search_combat",
]
