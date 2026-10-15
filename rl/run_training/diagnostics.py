"""Descriptive policy diagnostics, not causal explanations or policy features."""

import math
from collections import Counter
from collections.abc import Iterable
from dataclasses import asdict, is_dataclass
from enum import Enum

from sts_sim import Observation

from run_training.contracts import PolicyAction
from run_training.model import candidate_facts

DIAGNOSTIC_PROTOCOL = "public_macro_candidates_probabilities_context_v1"


def probabilities(logits: tuple[float, ...]) -> tuple[float, ...]:
    if not logits or any(not math.isfinite(x) and x != -math.inf for x in logits):
        raise ValueError("Invalid effective diagnostic logits")
    largest = max(logits)
    if not math.isfinite(largest):
        raise ValueError("No eligible diagnostic candidate")
    weights = [math.exp(x - largest) for x in logits]
    total = sum(weights)
    return tuple(weight / total for weight in weights)


def _public_json(value):
    if is_dataclass(value) and not isinstance(value, type):
        return _public_json(asdict(value))
    if isinstance(value, Enum):
        return value.value
    if isinstance(value, dict):
        return {key: _public_json(item) for key, item in value.items()}
    if isinstance(value, (tuple, list)):
        return [_public_json(item) for item in value]
    if value is None or isinstance(value, (str, int, bool)):
        return value
    if isinstance(value, float) and math.isfinite(value):
        return value
    raise TypeError("Unexpected public diagnostic value")


def decision_diagnostic(
    observation: Observation,
    actions: tuple[PolicyAction, ...],
    logits: tuple[float, ...],
    choice: int,
    value: float,
) -> dict:
    if len(actions) != len(logits) or not math.isfinite(value):
        raise ValueError("Invalid macro diagnostic record")
    p = probabilities(logits)
    if not 0 <= choice < len(actions) or p[choice] <= 0:
        raise ValueError("Invalid diagnostic choice")
    context = observation.context
    deck = Counter((c.content_key.value, c.upgrade_level, c.cost) for c in context.deck)
    return {
        "protocol": DIAGNOSTIC_PROTOCOL,
        "interpretation": "proposed choice; acceptance is recorded separately; probabilities are not causal explanations",
        "act": context.act,
        "floor": context.floor,
        "hp": context.player_hp,
        "max_hp": context.player_max_hp,
        "gold": context.gold,
        "keys": asdict(context.keys),
        "deck": [
            {"key": key, "upgrade": upgrade, "cost": cost, "count": count}
            for (key, upgrade, cost), count in sorted(deck.items())
        ],
        "relics": _public_json(context.relics),
        "choice": choice,
        "chosen_probability": p[choice],
        "value": value,
        "entropy": -sum(x * math.log(x) for x in p if x),
        "candidates": [
            {
                "index": index,
                "facts": _public_json(candidate_facts(observation, action)),
                "eligible": math.isfinite(logit),
                "effective_logit": logit if math.isfinite(logit) else None,
                "probability": probability,
            }
            for index, (action, logit, probability) in enumerate(
                zip(actions, logits, p, strict=True)
            )
        ],
    }


def policy_scores(rows: Iterable[tuple[tuple[float, ...], int]]) -> dict[str, float]:
    count = 0
    entropy = chosen = maximum = near_deterministic = 0.0
    for logits, choice in rows:
        p = probabilities(logits)
        if not 0 <= choice < len(p) or p[choice] <= 0:
            raise ValueError("Invalid recorded policy choice")
        count += 1
        entropy -= sum(x * math.log(x) for x in p if x)
        chosen += p[choice]
        maximum += max(p)
        near_deterministic += max(p) >= 0.99
    result = {"policy/accepted_macro_decisions": float(count)}
    if count:
        result.update(
            {
                "policy/entropy_mean": entropy / count,
                "policy/chosen_probability_mean": chosen / count,
                "policy/max_probability_mean": maximum / count,
                "policy/near_deterministic_fraction": near_deterministic / count,
            }
        )
    return result
