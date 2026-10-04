"""Small public-only macro baseline; no simulator objects or transport IDs in forward.

Feature hashing is deliberately a baseline, not the future lossless numeric run
transport. Stable hashes encode public semantic paths/values, never Python hash().
"""

import hashlib
import math
from dataclasses import dataclass, fields, is_dataclass

import torch
from sts_sim import FAIR_RUN_OBSERVATION_SCHEMA_VERSION, OBSERVATION_TYPES, Observation
from torch import Tensor, nn

from run_training.contracts import PolicyAction

FEATURE_VERSION = 3
BUCKETS = 8192
Features = tuple[tuple[int, float], ...]


def features(value: object) -> Features:
    result: dict[int, float] = {}

    def add(key: str, weight: float = 1.0) -> None:
        index = (
            int.from_bytes(
                hashlib.blake2b(key.encode(), digest_size=8).digest(), "little"
            )
            % BUCKETS
        )
        result[index] = result.get(index, 0.0) + weight

    def visit(item: object, path: str) -> None:
        if item is None or isinstance(item, (str, bool)):
            add(f"{path}={item!r}")
        elif isinstance(item, (int, float)):
            if not math.isfinite(item):
                raise ValueError("Nonfinite public feature")
            add(path + ":present")
            add(path + ":number", math.copysign(math.log1p(abs(item)), item))
        elif is_dataclass(item) and not isinstance(item, type):
            for field in fields(item):
                # Only typed observations and revision-free PolicyAction are accepted
                # by encode(). Observation schema is checked, never a model feature.
                if field.name != "schema_version":
                    visit(getattr(item, field.name), f"{path}.{field.name}")
        elif isinstance(item, tuple):
            add(path + ":length", math.log1p(len(item)))
            for index, child in enumerate(item):
                visit(child, f"{path}[{index}]")
        else:
            raise TypeError(f"Unsupported public feature type: {type(item).__name__}")

    visit(value, "public")
    return tuple(sorted(result.items()))


@dataclass(frozen=True)
class MacroInput:
    context: Features
    candidates: tuple[Features, ...]
    health: tuple[float, float, float] = (0.0, 0.0, 0.0)


def candidate_facts(
    observation: Observation, action: PolicyAction
) -> tuple[object, ...]:
    """Resolve decision-local references against public tables, never internal IDs."""
    facts: list[object] = [action]

    def at(items: tuple, slot: int | None) -> object:
        if slot is None or not 0 <= slot < len(items):
            raise ValueError(f"Unresolved public candidate reference: {action.kind}")
        return items[slot]

    if action.potion_slot is not None:
        facts.append(at(observation.context.potion_slots, action.potion_slot))
    if observation.kind == "event" and action.kind == "choose_event_option":
        facts.append(at(observation.screen.choices, action.option_slot))
    if action.card_slot is not None:
        cards = (
            observation.screen.cards
            if observation.kind == "grid"
            else observation.context.deck
        )
        facts.append(at(cards, action.card_slot))
    if action.node_slot is not None:
        if observation.kind != "map":
            raise ValueError("Map candidate outside public map")
        if not 0 <= action.node_slot < len(observation.screen.nodes):
            raise ValueError("Unresolved public map reference")
        node = observation.screen.nodes[action.node_slot]
        facts.extend((node, tuple(observation.screen.nodes[i] for i in node.children)))
    if action.shop_slot is not None:
        if observation.kind != "shop":
            raise ValueError("Shop candidate outside public merchant")
        offers = {
            "buy_shop_card": observation.screen.cards,
            "buy_shop_relic": observation.screen.relics,
            "buy_shop_potion": observation.screen.potions,
        }
        facts.append(at(offers[action.kind], action.shop_slot))
    if observation.kind == "reward":
        screen = observation.screen
        if action.reward_slot is not None:
            offers = {
                "take_card_reward": screen.cards,
                "take_relic_reward_at": screen.relic_offers,
                "choose_boss_relic_reward": screen.boss_relic_choices,
                "open_queued_card_reward": screen.queued_card_rewards,
                "take_potion_reward": screen.potion_offers or (screen.potion_offer,),
            }
            facts.append(at(offers[action.kind], action.reward_slot))
        if action.kind == "take_relic_reward":
            facts.append(screen.relic_offer)
        if action.kind == "take_sapphire_key":
            facts.append(at(screen.relic_offers, screen.sapphire_key_relic_slot))
        if action.kind in ("take_gold_reward", "take_stolen_gold_reward"):
            facts.append(
                screen.gold_offer
                if action.kind == "take_gold_reward"
                else screen.stolen_gold_offer
            )
    return tuple(facts)


def encode(
    observation: Observation,
    actions: tuple[PolicyAction, ...],
    *,
    visible_map: Observation | None = None,
    previous: PolicyAction | None = None,
) -> MacroInput:
    if not isinstance(observation, OBSERVATION_TYPES) or any(
        not isinstance(a, PolicyAction) for a in actions
    ):
        raise TypeError(
            "Only public typed observations and revision-free actions are model inputs"
        )
    if observation.schema_version != FAIR_RUN_OBSERVATION_SCHEMA_VERSION:
        raise ValueError("Unsupported run observation schema")
    if observation.kind == "combat" or not actions:
        raise ValueError("Macro encoding requires noncombat legal candidates")
    # Retain only an actually observed current-act map, not a simulator forecast.
    map_screen = None
    if visible_map is not None and visible_map.context.act == observation.context.act:
        if visible_map.kind != "map":
            raise ValueError("Map history must be a public map observation")
        map_screen = visible_map.screen
    return MacroInput(
        features((observation, map_screen, previous)),
        tuple(features(candidate_facts(observation, action)) for action in actions),
        (
            observation.context.player_hp / max(1, observation.context.player_max_hp),
            observation.context.player_hp / 100.0,
            observation.context.player_max_hp / 100.0,
        ),
    )


class MacroModel(nn.Module):
    """One score per complete legal action and a fresh run-value head."""

    def __init__(self, width: int = 64) -> None:
        super().__init__()
        self.embedding = nn.Embedding(BUCKETS, width)
        self.context = nn.Sequential(nn.Linear(width, width), nn.Tanh())
        self.policy = nn.Sequential(
            nn.Linear(width * 2, width), nn.Tanh(), nn.Linear(width, 1)
        )
        self.value = nn.Linear(width, 1)
        # Start uniform and with zero return baseline, not arbitrary preferences
        # or a large invented success value before any positive-return evidence.
        policy_output = self.policy[-1]
        assert isinstance(policy_output, nn.Linear)
        nn.init.zeros_(policy_output.weight)
        nn.init.zeros_(policy_output.bias)
        nn.init.zeros_(self.value.weight)
        nn.init.zeros_(self.value.bias)

    def pool(self, rows: Features) -> Tensor:
        device = self.embedding.weight.device
        ids = torch.tensor([i for i, _ in rows], device=device, dtype=torch.long)
        weights = torch.tensor([w for _, w in rows], device=device, dtype=torch.float32)
        return (self.embedding(ids) * weights[:, None]).sum(0) / max(
            1, len(rows)
        ) ** 0.5

    def forward(self, inputs: MacroInput) -> tuple[Tensor, Tensor]:
        context = self.context(self.pool(inputs.context))
        candidates = torch.stack([self.pool(row) for row in inputs.candidates])
        logits = self.policy(
            torch.cat((context.expand_as(candidates), candidates), dim=1)
        ).squeeze(-1)
        return logits, self.value(context).squeeze(-1)


class HealthMacroModel(MacroModel):
    """Ablation: normalized public HP bypasses the pooled/saturated context.

    No heuristic action preference or known healing effect is installed. Candidate
    semantics remain the same hashed features; all candidates are still scored.
    Zero output heads preserve uniform initial policy and zero initial value.
    """

    def __init__(self, width: int = 64) -> None:
        super().__init__(width)
        self.health_value = nn.Sequential(
            nn.Linear(3, width), nn.ReLU(), nn.Linear(width, 1)
        )
        self.health_policy = nn.Sequential(
            nn.Linear(width + 3, width), nn.ReLU(), nn.Linear(width, 1)
        )
        for head in (self.health_value, self.health_policy):
            output = head[-1]
            assert isinstance(output, nn.Linear)
            nn.init.zeros_(output.weight)
            nn.init.zeros_(output.bias)

    def forward(self, inputs: MacroInput) -> tuple[Tensor, Tensor]:
        logits, value = super().forward(inputs)
        hp = self.embedding.weight.new_tensor(inputs.health)
        candidates = torch.stack([self.pool(row) for row in inputs.candidates])
        residual = self.health_policy(
            torch.cat((candidates, hp.expand(len(candidates), -1)), dim=1)
        ).squeeze(-1)
        return logits + residual, value + self.health_value(hp).squeeze(-1)
