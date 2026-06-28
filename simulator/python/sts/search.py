"""Small deterministic omniscient combat search helpers."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Callable, Iterable, Sequence

from sts.omni import ExactCombatAction, OmniCombatEnv


@dataclass(frozen=True)
class CombatSearchConfig:
    """Configuration for the first deterministic omniscient combat search."""

    max_depth: int = 1
    objective: str = "survive_then_damage"
    algorithm: str = "exhaustive"
    beam_width: int = 8
    allowed_potions: tuple[str, ...] | None = None


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
    evaluator = _evaluator(config.objective)
    if config.algorithm not in {"exhaustive", "beam", "greedy", "portfolio"}:
        raise ValueError(f"unsupported algorithm: {config.algorithm}")
    if config.beam_width < 1:
        raise ValueError("beam_width must be at least 1")
    if config.algorithm == "exhaustive" and depth > 8:
        raise ValueError("exhaustive search max_depth is capped at 8")

    _state(env)
    if config.algorithm == "exhaustive":
        score, variation, nodes, terminal_reason = _search(env.clone(), depth, evaluator, config)
    elif config.algorithm == "portfolio":
        score, variation, nodes, terminal_reason = _portfolio_search(env.clone(), depth, config)
    else:
        width = 1 if config.algorithm == "greedy" else config.beam_width
        score, variation, nodes, terminal_reason = _beam_search(
            env.clone(), depth, width, evaluator, config
        )
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
            "algorithm": config.algorithm,
            "beam_width": width if config.algorithm in {"beam", "greedy"} else None,
            "allowed_potions": config.allowed_potions,
            "unsupported_transitions": 0,
        },
        terminal_reason=terminal_reason,
    )


def _evaluator(objective: str) -> Callable[[dict[str, Any]], float]:
    if objective == "survive_then_damage":
        return _evaluate_basic
    if objective == "tactical_survival":
        return _evaluate_tactical_survival
    if objective == "hp_preserving_lethal":
        return _evaluate_hp_preserving_lethal
    if objective == "aggressive_lethal":
        return _evaluate_aggressive
    raise ValueError(f"unsupported objective: {objective}")


def _search(
    env: Any,
    depth: int,
    evaluator: Callable[[dict[str, Any]], float],
    config: CombatSearchConfig,
) -> tuple[float, list[Any], int, str | None]:
    state = _state(env)
    terminal_reason = _terminal_reason(env, state)
    if terminal_reason == "won":
        return 1_000_000.0 + evaluator(state), [], 1, "won"
    if terminal_reason == "lost":
        return -1_000_000.0 + evaluator(state), [], 1, "lost"
    if depth <= 0:
        return evaluator(state), [], 1, None

    actions = _legal_search_actions(env, config)
    if not actions:
        return evaluator(state), [], 1, None

    best_score = float("-inf")
    best_variation: list[Any] = []
    best_terminal_reason: str | None = None
    nodes = 1

    for action in actions:
        child = env.clone()
        result = child.step(action)
        child_terminal = _result_terminal_reason(result, child)
        if child_terminal:
            child_score = _terminal_score(child_terminal, child, evaluator)
            child_variation: list[Any] = []
            child_nodes = 1
            child_terminal_reason = child_terminal
        else:
            child_score, child_variation, child_nodes, child_terminal_reason = _search(
                child, depth - 1, evaluator, config
            )

        nodes += child_nodes
        candidate_variation = [action, *child_variation]
        if _is_better(candidate_variation, child_score, best_variation, best_score):
            best_score = child_score
            best_variation = candidate_variation
            best_terminal_reason = child_terminal_reason

    return best_score, best_variation, nodes, best_terminal_reason


def _beam_search(
    env: Any,
    depth: int,
    beam_width: int,
    evaluator: Callable[[dict[str, Any]], float],
    config: CombatSearchConfig,
) -> tuple[float, list[Any], int, str | None]:
    state = _state(env)
    terminal_reason = _terminal_reason(env, state)
    if terminal_reason:
        return _terminal_score(terminal_reason, env, evaluator), [], 1, terminal_reason

    frontier: list[tuple[Any, list[Any], float, str | None]] = [(env, [], evaluator(state), None)]
    best_score = float("-inf")
    best_variation: list[Any] = []
    best_terminal_reason: str | None = None
    nodes = 1

    for _level in range(depth):
        candidates: list[tuple[Any, list[Any], float, str | None]] = []
        for current, variation, _score, _reason in frontier:
            actions = _legal_search_actions(current, config)
            if not actions:
                score = evaluator(_state(current))
                candidates.append((current, variation, score, None))
                continue
            for action in actions:
                child = current.clone()
                result = child.step(action)
                nodes += 1
                child_terminal = _result_terminal_reason(result, child)
                score = (
                    _terminal_score(child_terminal, child, evaluator)
                    if child_terminal
                    else evaluator(_state(child))
                )
                candidates.append((child, [*variation, action], score, child_terminal))

        if not candidates:
            break
        candidates.sort(key=lambda item: (-item[2], _variation_key(item[1])))
        current_best = candidates[0]
        if _is_better(current_best[1], current_best[2], best_variation, best_score):
            best_score = current_best[2]
            best_variation = current_best[1]
            best_terminal_reason = current_best[3]
        frontier = [candidate for candidate in candidates if candidate[3] is None][:beam_width]
        if not frontier:
            break

    return best_score, best_variation, nodes, best_terminal_reason


def _portfolio_search(
    env: Any,
    depth: int,
    config: CombatSearchConfig,
) -> tuple[float, list[Any], int, str | None]:
    if config.objective == "hp_preserving_lethal":
        policies = [
            CombatSearchConfig(
                max_depth=40,
                objective="hp_preserving_lethal",
                algorithm="greedy",
                allowed_potions=config.allowed_potions,
            ),
            CombatSearchConfig(
                max_depth=20,
                objective="hp_preserving_lethal",
                algorithm="beam",
                beam_width=8,
                allowed_potions=config.allowed_potions,
            ),
            CombatSearchConfig(
                max_depth=4,
                objective="tactical_survival",
                algorithm="exhaustive",
                allowed_potions=config.allowed_potions,
            ),
            CombatSearchConfig(
                max_depth=3,
                objective="survive_then_damage",
                algorithm="exhaustive",
                allowed_potions=config.allowed_potions,
            ),
            CombatSearchConfig(
                max_depth=30,
                objective="aggressive_lethal",
                algorithm="beam",
                beam_width=4,
                allowed_potions=config.allowed_potions,
            ),
        ]
        rollout_configs = [policies[0], policies[1], policies[2], policies[3]]
        outcome_score = _hp_preserving_outcome_score
    else:
        policies = [
            CombatSearchConfig(
                max_depth=40,
                objective="aggressive_lethal",
                algorithm="beam",
                beam_width=12,
                allowed_potions=config.allowed_potions,
            ),
            CombatSearchConfig(
                max_depth=3,
                objective="survive_then_damage",
                algorithm="exhaustive",
                allowed_potions=config.allowed_potions,
            ),
            CombatSearchConfig(
                max_depth=4,
                objective="tactical_survival",
                algorithm="exhaustive",
                allowed_potions=config.allowed_potions,
            ),
            CombatSearchConfig(
                max_depth=40,
                objective="tactical_survival",
                algorithm="beam",
                beam_width=8,
                allowed_potions=config.allowed_potions,
            ),
            CombatSearchConfig(
                max_depth=40,
                objective="survive_then_damage",
                algorithm="beam",
                beam_width=12,
                allowed_potions=config.allowed_potions,
            ),
        ]
        rollout_configs = [policies[0], policies[1], policies[2], policies[4]]
        outcome_score = _portfolio_outcome_score

    best_score = float("-inf")
    best_variation: list[Any] = []
    best_terminal_reason: str | None = None
    nodes = 1
    seen_actions: set[str] = set()

    for policy in policies:
        recommendation = search_combat(env, policy)
        nodes += recommendation.visits
        action = recommendation.best_action
        if action is None or action.json() in seen_actions:
            continue
        seen_actions.add(action.json())
        child = env.clone()
        step_result = child.step(action)
        terminal_reason = _result_terminal_reason(step_result, child)
        if terminal_reason:
            score = outcome_score(child, terminal_reason)
            variation = [action]
            rollout_nodes = 1
        else:
            score, rollout, rollout_nodes, terminal_reason = _best_rollout(
                child, rollout_configs, max_actions=depth, outcome_score=outcome_score
            )
            variation = [action, *rollout]
        nodes += rollout_nodes
        if _is_better(variation, score, best_variation, best_score):
            best_score = score
            best_variation = variation
            best_terminal_reason = terminal_reason

    if not best_variation:
        return _beam_search(env, depth, 12, _evaluate_aggressive, config)
    return best_score, best_variation, nodes, best_terminal_reason


def _best_rollout(
    env: Any,
    configs: Sequence[CombatSearchConfig],
    *,
    max_actions: int,
    outcome_score: Callable[[Any, str | None], float] | None = None,
) -> tuple[float, list[Any], int, str | None]:
    outcome_score = outcome_score or _portfolio_outcome_score
    best_score = float("-inf")
    best_variation: list[Any] = []
    best_terminal_reason: str | None = None
    nodes = 1
    for config in configs:
        score, variation, rollout_nodes, terminal_reason = _rollout(
            env, config, max_actions=max_actions, outcome_score=outcome_score
        )
        nodes += rollout_nodes
        if _is_better(variation, score, best_variation, best_score):
            best_score = score
            best_variation = variation
            best_terminal_reason = terminal_reason
    return best_score, best_variation, nodes, best_terminal_reason


def _rollout(
    env: Any,
    config: CombatSearchConfig,
    *,
    max_actions: int,
    outcome_score: Callable[[Any, str | None], float] | None = None,
) -> tuple[float, list[Any], int, str | None]:
    outcome_score = outcome_score or _portfolio_outcome_score
    current = env.clone()
    variation = []
    nodes = 1
    terminal_reason = _terminal_reason(current, _state(current))
    while terminal_reason is None and len(variation) < max_actions:
        recommendation = search_combat(current, config)
        nodes += recommendation.visits
        action = recommendation.best_action
        if action is None:
            break
        result = current.step(action)
        variation.append(action)
        terminal_reason = _result_terminal_reason(result, current)
    return outcome_score(current, terminal_reason), variation, nodes, terminal_reason


def _portfolio_outcome_score(env: Any, terminal_reason: str | None) -> float:
    try:
        state = _state(env)
    except ValueError:
        if terminal_reason == "won":
            return 100_000.0
        if terminal_reason == "lost":
            return -100_000.0
        raise
    player_hp = float(state.get("player", {}).get("hp", 0))
    monster_hp = sum(
        float(monster.get("hp", 0))
        for monster in state.get("monsters", [])
        if monster.get("alive", False)
    )
    score = player_hp * 100.0 - monster_hp * 20.0
    if terminal_reason == "won":
        return 100_000.0 + score
    if terminal_reason == "lost":
        return -100_000.0 + score
    return score


def _hp_preserving_outcome_score(env: Any, terminal_reason: str | None) -> float:
    try:
        state = _state(env)
    except ValueError:
        if terminal_reason == "won":
            return 1_000_000.0
        if terminal_reason == "lost":
            return -1_000_000.0
        raise
    player_hp = float(state.get("player", {}).get("hp", 0))
    player_block = float(state.get("player", {}).get("block", 0))
    alive_monsters = [
        monster for monster in state.get("monsters", []) if monster.get("alive", False)
    ]
    incoming = sum(_intent_damage(monster.get("intent")) for monster in alive_monsters)
    monster_hp = sum(float(monster.get("hp", 0)) for monster in alive_monsters)
    score = (
        player_hp * 500.0
        + min(player_block, incoming) * 25.0
        - max(0.0, incoming - player_block) * 150.0
        - monster_hp * 8.0
        - len(alive_monsters) * 1_000.0
    )
    if terminal_reason == "won":
        return 1_000_000.0 + score
    if terminal_reason == "lost":
        return -1_000_000.0 + score
    return score


def _terminal_score(
    reason: str | None,
    env: Any,
    evaluator: Callable[[dict[str, Any]], float],
) -> float:
    try:
        state_score = evaluator(_state(env))
    except ValueError:
        if reason == "won":
            return 1_000_000.0
        if reason == "lost":
            return -1_000_000.0
        raise
    if reason == "won":
        return 1_000_000.0 + state_score
    if reason == "lost":
        return -1_000_000.0 + state_score
    return state_score


def _evaluate_basic(state: dict[str, Any]) -> float:
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


def _evaluate_tactical_survival(state: dict[str, Any]) -> float:
    player = state.get("player", {})
    monsters = state.get("monsters", [])
    alive_monsters = [monster for monster in monsters if monster.get("alive", False)]

    player_hp = float(player.get("hp", 0))
    player_block = float(player.get("block", 0))
    player_energy = float(player.get("energy", 0))
    incoming = sum(_intent_damage(monster.get("intent")) for monster in alive_monsters)
    unblocked = max(0.0, incoming - player_block)
    useful_block = min(player_block, incoming)
    monster_hp = sum(float(monster.get("hp", 0)) for monster in alive_monsters)
    monster_block = sum(float(monster.get("block", 0)) for monster in alive_monsters)
    hand_count = len(((state.get("piles") or {}).get("hand")) or [])

    return (
        player_hp * 25.0
        - unblocked * 45.0
        + useful_block * 7.5
        + player_energy * 0.5
        + hand_count * 0.25
        - monster_hp * 4.0
        - monster_block * 0.75
        - len(alive_monsters) * 60.0
    )


def _evaluate_aggressive(state: dict[str, Any]) -> float:
    player = state.get("player", {})
    monsters = state.get("monsters", [])
    alive_monsters = [monster for monster in monsters if monster.get("alive", False)]
    player_hp = float(player.get("hp", 0))
    player_block = float(player.get("block", 0))
    incoming = sum(_intent_damage(monster.get("intent")) for monster in alive_monsters)
    monster_hp = sum(float(monster.get("hp", 0)) for monster in alive_monsters)
    return (
        player_hp * 8.0
        + min(player_block, incoming) * 2.0
        - max(0.0, incoming - player_block) * 10.0
        - monster_hp * 9.0
        - len(alive_monsters) * 100.0
    )


def _evaluate_hp_preserving_lethal(state: dict[str, Any]) -> float:
    player = state.get("player", {})
    monsters = state.get("monsters", [])
    alive_monsters = [monster for monster in monsters if monster.get("alive", False)]

    player_hp = float(player.get("hp", 0))
    player_block = float(player.get("block", 0))
    player_energy = float(player.get("energy", 0))
    incoming = sum(_intent_damage(monster.get("intent")) for monster in alive_monsters)
    unblocked = max(0.0, incoming - player_block)
    useful_block = min(player_block, incoming)
    monster_hp = sum(float(monster.get("hp", 0)) for monster in alive_monsters)
    monster_block = sum(float(monster.get("block", 0)) for monster in alive_monsters)
    hand_count = len(((state.get("piles") or {}).get("hand")) or [])

    return (
        player_hp * 120.0
        + useful_block * 20.0
        - unblocked * 160.0
        + player_energy * 1.0
        + hand_count * 0.5
        - monster_hp * 6.0
        - monster_block * 0.5
        - len(alive_monsters) * 300.0
    )


def _intent_damage(intent: Any) -> float:
    if not isinstance(intent, dict):
        return 0.0
    if "Attack" in intent and isinstance(intent["Attack"], dict):
        return float(intent["Attack"].get("damage", 0))
    if "AttackBuff" in intent and isinstance(intent["AttackBuff"], dict):
        return float(intent["AttackBuff"].get("damage", 0))
    if "AttackDebuff" in intent and isinstance(intent["AttackDebuff"], dict):
        return float(intent["AttackDebuff"].get("damage", 0))
    if "AttackDefend" in intent and isinstance(intent["AttackDefend"], dict):
        return float(intent["AttackDefend"].get("damage", 0))
    return 0.0


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


def _legal_search_actions(env: Any, config: CombatSearchConfig) -> list[Any]:
    return _sorted_actions(_filter_allowed_potion_actions(env, env.exact_legal_actions(), config))


def _filter_allowed_potion_actions(
    env: Any,
    actions: Iterable[Any],
    config: CombatSearchConfig,
) -> list[Any]:
    allowed = config.allowed_potions
    if allowed is None:
        return list(actions)
    allowed_names = {_normalize_potion_name(name) for name in allowed}
    potions = _run_potion_names(env)
    filtered = []
    for action in actions:
        if getattr(action, "kind", lambda: "")() != "use_potion":
            filtered.append(action)
            continue
        slot = _potion_action_slot(action)
        potion_name = potions[slot] if slot is not None and slot < len(potions) else None
        if potion_name is not None and _normalize_potion_name(potion_name) in allowed_names:
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
    return "".join(char.lower() for char in name if char.isalnum())


def _is_better(
    candidate_variation: Sequence[Any],
    candidate_score: float,
    best_variation: Sequence[Any],
    best_score: float,
) -> bool:
    if candidate_variation and not best_variation:
        return True
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
