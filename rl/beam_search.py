"""Small privileged beam-search reference, not a fair policy or an optimality guarantee."""

from dataclasses import dataclass

from combat_task import action_indices, combat_outcome, terminal_reward
from sts_sim import Action, Decision, State


@dataclass
class SearchResult:
    actions: tuple[Action, ...] = ()
    reward: float | None = None  # None means no completed fight found, not a defeat.
    won: bool | None = None
    hp: int | None = None
    transitions: int = 0
    limit_reached: bool = False


@dataclass
class Node:
    state: State
    decision: Decision
    actions: tuple[Action, ...]


def pruning_score(node: Node) -> float:
    """Approximate unfinished combat value; deliberately separate from terminal reward."""
    obs = node.decision.observation
    assert obs.kind == "combat"
    player = obs.screen.player
    enemies = [enemy for enemy in obs.screen.monsters if enemy.alive]
    incoming = sum(
        (enemy.intent.damage or 0) * (enemy.intent.hits if enemy.intent.hits is not None else 1)
        for enemy in enemies
        if enemy.intent.visibility == "visible"
    )
    # Hidden intents contribute no estimate; search still simulates their actual outcomes.
    return (
        obs.context.player_hp
        - 5.0 * max(0, incoming - player.block)
        + 0.75 * min(player.block, incoming)
        + 0.25 * player.energy
        - 1.75 * sum(enemy.hp for enemy in enemies)
        - 0.25 * sum(enemy.block for enemy in enemies)
        - 12.0 * len(enemies)
    )


def prune(nodes: list[Node], width: int) -> list[Node]:
    """Reserve a slot per distinct opening action, then fill by score. Ties are stable."""
    ranked = sorted(nodes, key=pruning_score, reverse=True)
    selected = set()
    openings = set()
    for index, node in enumerate(ranked):
        opening = repr(node.actions[0])
        if opening not in openings and len(selected) < width:
            openings.add(opening)
            selected.add(index)
    for index in range(len(ranked)):
        if len(selected) == width:
            break
        selected.add(index)
    # Preserve the original score/tie order when expanding the selected paths.
    return [ranked[index] for index in sorted(selected)]


def beam_search(
    root: State, *, width: int = 64, max_decisions: int = 128, max_transitions: int = 10000
) -> SearchResult:
    """Search cloned actual states. Return the highest-return completed path found.

    Terminal ranking uses only terminal HP / starting max HP (defeat = 0).
    Unfinished paths use a weighted combat heuristic with opening-action diversity.
    This is not a reward or an upper bound. Ties retain generation order.
    No deduplication, caching, warm starts, or potion bonuses.
    """
    if min(width, max_decisions, max_transitions) < 1:
        raise ValueError("Search limits must be positive")
    state = root.clone()
    decision = state.decision()
    starting_max_hp = decision.observation.context.player_max_hp
    result = SearchResult()

    def completed(node: Node) -> bool:
        reward = terminal_reward(node.decision.observation, starting_max_hp)
        if reward is None:
            return False
        if result.reward is None or reward > result.reward:
            result.actions = node.actions
            result.reward = reward
            result.won = combat_outcome(node.decision.observation)
            result.hp = node.decision.observation.context.player_hp if result.won else 0
        return True

    initial = Node(state, decision, ())
    if completed(initial):
        return result
    frontier = [initial]
    for _ in range(max_decisions):
        children = []
        for node in frontier:
            allowed = action_indices(node.decision)
            if not allowed:
                raise RuntimeError("No allowed actions in unfinished combat")
            for index in allowed:
                if result.transitions >= max_transitions:
                    result.limit_reached = True
                    return result
                action = node.decision.actions[index]
                child_state = node.state.clone()
                child = Node(child_state, child_state.step(action), (*node.actions, action))
                result.transitions += 1
                if not completed(child):
                    children.append(child)
        if not children:
            return result
        frontier = prune(children, width)
    result.limit_reached = True
    return result
