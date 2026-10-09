"""Small public-only macro baseline; no simulator objects or transport IDs in forward.

Feature hashing is deliberately a baseline, not the future lossless numeric run
transport. Stable hashes encode public semantic paths/values, never Python hash().
"""

import hashlib
import math
from collections.abc import Callable
from dataclasses import dataclass, fields, is_dataclass
from functools import lru_cache

import torch
from sts_sim import FAIR_RUN_OBSERVATION_SCHEMA_VERSION, OBSERVATION_TYPES, Observation
from sts_sim.observations.screens import MapScreen
from torch import Tensor, nn

from run_training.contracts import PolicyAction

# The hashed representation is unchanged from #93. Only the opt-in health
# encoder consumes the additional normalized-HP input introduced in version 3.
FEATURE_VERSION = 2
HEALTH_FEATURE_VERSION = 3
BUCKETS = 8192
Features = tuple[tuple[int, float], ...]


@lru_cache(maxsize=65536)
def _bucket(key: str) -> int:
    """Bounded, RNG-free cache of public semantic paths; not learned state."""
    return (
        int.from_bytes(hashlib.blake2b(key.encode(), digest_size=8).digest(), "little")
        % BUCKETS
    )


@lru_cache(maxsize=128)
def _fields(kind: type):
    if not is_dataclass(kind):
        raise TypeError("Expected public dataclass fields")
    return tuple(field.name for field in fields(kind) if field.name != "schema_version")


def _visit(
    item: object,
    path: str,
    add: Callable[[int, float], None],
    *,
    cache_maps: bool = True,
) -> None:
    if item is None or isinstance(item, (str, bool)):
        add(_bucket(f"{path}={item!r}"), 1.0)
    elif isinstance(item, (int, float)):
        if not math.isfinite(item):
            raise ValueError("Nonfinite public feature")
        add(_bucket(path + ":present"), 1.0)
        add(_bucket(path + ":number"), math.copysign(math.log1p(abs(item)), item))
    elif cache_maps and isinstance(item, MapScreen):
        for index, weight in _map_terms(item, path):
            add(index, weight)
    elif is_dataclass(item) and not isinstance(item, type):
        for name in _fields(type(item)):
            _visit(getattr(item, name), f"{path}.{name}", add)
    elif isinstance(item, tuple):
        add(_bucket(path + ":length"), math.log1p(len(item)))
        for index, child in enumerate(item):
            _visit(child, f"{path}[{index}]", add)
    else:
        raise TypeError(f"Unsupported public feature type: {type(item).__name__}")


# Cache-key contract (_map_terms and _candidate_features): keys come from
# strictly decoded public schemas with a fixed primitive type per field. Python
# structural equality conflates True/1 and 0.0/-0.0; these caches are not for
# arbitrary hand-built objects. If a schema admits mixed types or signed-zero
# floats, use type/bit-sensitive nested keys before caching that schema.
@lru_cache(maxsize=256)
def _map_terms(screen: MapScreen, path: str) -> Features:
    # Cache immutable *public* maps only, including their semantic path. Store
    # contributions in traversal order, not pre-summed buckets: collision addition
    # must remain bit-for-bit identical when merged into a different context.
    terms: list[tuple[int, float]] = []
    _visit(screen, path, lambda i, w: terms.append((i, w)), cache_maps=False)
    return tuple(terms)


def features(value: object) -> Features:
    result: dict[int, float] = {}

    def add(index: int, weight: float) -> None:
        result[index] = result.get(index, 0.0) + weight

    _visit(value, "public", add)
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


@lru_cache(maxsize=8192)
def _candidate_features(facts: tuple[object, ...]) -> Features:
    # Same strict-schema cache-key contract as _map_terms above.
    return features(facts)


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
        tuple(
            _candidate_features(candidate_facts(observation, action))
            for action in actions
        ),
        (
            observation.context.player_hp / max(1, observation.context.player_max_hp),
            observation.context.player_hp / 100.0,
            observation.context.player_max_hp / 100.0,
        ),
    )


@dataclass(frozen=True)
class MacroBatch:
    """Parameter-independent ragged public features, packed once per device batch."""

    ids: Tensor
    weights: Tensor
    offsets: Tensor
    scales: Tensor
    owners: Tensor
    positions: Tensor
    health: Tensor
    lengths: tuple[int, ...]


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

    def pack(self, inputs: list[MacroInput]) -> MacroBatch:
        if not inputs or any(not row.candidates for row in inputs):
            raise ValueError("A macro batch requires nonempty complete candidate sets")
        lengths = tuple(len(row.candidates) for row in inputs)
        rows = [row.context for row in inputs] + [
            c for row in inputs for c in row.candidates
        ]
        ids, weights, offsets = [], [], [0]
        for row in rows:
            ids.extend(i for i, _ in row)
            weights.extend(w for _, w in row)
            offsets.append(len(ids))
        device = self.embedding.weight.device
        owners = [i for i, n in enumerate(lengths) for _ in range(n)]
        width = max(lengths)
        positions = [i * width + j for i, n in enumerate(lengths) for j in range(n)]
        scales = [max(1, len(row)) ** 0.5 for row in rows]
        health = [value for row in inputs for value in row.health]
        # Two allocator-managed pinned uploads, rather than a synchronizing copy
        # per table. No learned activations are cached; views retain packed storage.
        integer = torch.tensor(
            ids + offsets + owners + positions,
            dtype=torch.long,
            device="cpu",
            pin_memory=device.type == "cuda",
        ).to(device, non_blocking=True)
        floating = torch.tensor(
            weights + scales + health,
            dtype=self.embedding.weight.dtype,
            device="cpu",
            pin_memory=device.type == "cuda",
        ).to(device, non_blocking=True)
        index_rows, offset_rows, owner_rows, position_rows = integer.split(
            [len(ids), len(offsets), len(owners), len(positions)]
        )
        weight_rows, scale_rows, health_rows = floating.split(
            [len(weights), len(scales), len(health)]
        )
        return MacroBatch(
            index_rows,
            weight_rows,
            offset_rows,
            scale_rows,
            owner_rows,
            position_rows,
            health_rows.view(len(inputs), 3),
            lengths,
        )

    def _batch_heads(
        self, context: Tensor, candidates: Tensor, batch: MacroBatch
    ) -> tuple[Tensor, Tensor]:
        logits = self.policy(
            torch.cat((context[batch.owners], candidates), dim=1)
        ).squeeze(-1)
        return logits, self.value(context).squeeze(-1)

    def forward_batch(self, batch: MacroBatch) -> tuple[Tensor, Tensor]:
        # Fused weighted segment reduction avoids one embedding/gather per candidate
        # and the enormous [all_feature_occurrences, width] intermediate.
        pooled = (
            torch.nn.functional.embedding_bag(
                batch.ids,
                self.embedding.weight,
                batch.offsets,
                mode="sum",
                per_sample_weights=batch.weights,
                include_last_offset=True,
            )
            / batch.scales[:, None]
        )
        count = len(batch.lengths)
        context = self.context(pooled[:count])
        logits, values = self._batch_heads(context, pooled[count:], batch)
        padded = logits.new_full((count * max(batch.lengths),), -torch.inf)
        padded = padded.scatter(0, batch.positions, logits)
        return padded.view(count, max(batch.lengths)), values

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

    def _batch_heads(
        self, context: Tensor, candidates: Tensor, batch: MacroBatch
    ) -> tuple[Tensor, Tensor]:
        logits, values = super()._batch_heads(context, candidates, batch)
        residual = self.health_policy(
            torch.cat((candidates, batch.health[batch.owners]), dim=1)
        ).squeeze(-1)
        return logits + residual, values + self.health_value(batch.health).squeeze(-1)

    def forward(self, inputs: MacroInput) -> tuple[Tensor, Tensor]:
        logits, value = super().forward(inputs)
        hp = self.embedding.weight.new_tensor(inputs.health)
        candidates = torch.stack([self.pool(row) for row in inputs.candidates])
        residual = self.health_policy(
            torch.cat((candidates, hp.expand(len(candidates), -1)), dim=1)
        ).squeeze(-1)
        return logits + residual, value + self.health_value(hp).squeeze(-1)
