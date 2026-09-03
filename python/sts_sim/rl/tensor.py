"""On-the-fly fair-combat tensor extraction.

This module accepts symbolic public values only.  Tensors are an experiment
representation and are deliberately not a serialized simulator contract.
"""

from __future__ import annotations

import math
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass, field, is_dataclass
from types import MappingProxyType
from typing import Final, Literal, Protocol, get_args, get_type_hints

import torch

from ..content import CARD_CATALOGUE, CARD_DEFINITIONS
from ..fair import (
    FairCard,
    FairCardDynamicValues,
    FairCombatContext,
    FairCombatObservation,
    FairCounter,
    FairHandCard,
    FairMonster,
    FairMonsterIntent,
    FairOrb,
    FairOrbSlot,
    FairPile,
    FairPlayer,
    FairPotionSlot,
    FairPower,
    FairRelic,
    FairSelection,
    FairSelectionOption,
)
from ..run import ActionDescriptor
from .provenance import canonical_bytes, sha256_bytes

PAD: Final = "<pad>"
UNK: Final = "<unk>"
NONE: Final = "<none>"
_RESERVED: Final = (PAD, UNK, NONE)
KNOWN_OBSERVATION_SCHEMA_VERSIONS: Final = frozenset({2})
REQUIRED_NAMESPACES: Final = frozenset(
    {
        "action_family",
        "action_kind",
        "card",
        "card_rarity",
        "card_target",
        "card_type",
        "counter",
        "entity_kind",
        "intent_category",
        "intent_visibility",
        "monster",
        "orb",
        "phase",
        "potion",
        "power",
        "relic",
        "selection",
        "slime_size",
        "zone",
    }
)

ENTITY_KINDS: Final = (
    "global",
    "player",
    "pile",
    "card",
    "monster",
    "relic",
    "potion",
    "orb",
    "selection",
)
ZONES: Final = ("none", "hand", "draw", "discard", "exhaust", "stasis", "selection")
CATEGORY_NAMESPACES: Final = (
    "card_type",
    "card_rarity",
    "card_target",
    "intent_visibility",
    "intent_category",
    "slime_size",
)
SCALAR_NAMES: Final = (
    "observation_schema_version",
    "ascension",
    "act",
    "floor",
    "gold",
    "hp",
    "max_hp",
    "block",
    "energy",
    "max_energy",
    "pile_count",
    "cost",
    "cost_is_modified",
    "cost_resets_next_turn",
    "upgrade_level",
    "bottled",
    "temporary",
    "rampage_damage_bonus",
    "ritual_dagger_damage_bonus",
    "windmill_retain_damage",
    "steam_barrier_block_reduction",
    "combat_cost_under_turn_override",
    "printed_cost",
    "printed_damage",
    "printed_block",
    "printed_vulnerable",
    "innate",
    "ethereal",
    "exhaust",
    "retain",
    "unplayable",
    "is_curse",
    "known_rank",
    "visible_rank",
    "stolen_gold",
    "alive",
    "escaped",
    "minion",
    "targetable",
    "in_defensive_mode",
    "intent_damage",
    "intent_hits",
    "orb_evoke",
    "selection_selected",
)
SCALAR_INDEX: Final = {name: index for index, name in enumerate(SCALAR_NAMES)}

# Every symbolic field must have an explicit disposition.  Tests compare this
# ledger recursively with dataclass definitions so additive public fields
# cannot disappear silently at the model boundary.
Disposition = Literal["encoded", "reference_only", "validated", "ignored"]
_FIELD_COVERAGE_DATA: dict[type[object], dict[str, tuple[Disposition, str]]] = {
    FairCombatObservation: {
        "schema_version": ("encoded", "validated symbolic schema version on global token"),
        "context": ("encoded", "global token"),
        "phase": ("encoded", "global primary category"),
        "player": ("encoded", "player token"),
        "orb_slots": ("encoded", "orb tokens in visible order"),
        "hand": ("encoded", "canonical card tokens and slot reference map"),
        "draw_pile": ("encoded", "pile summary and visible cards/order"),
        "discard_pile": ("encoded", "pile summary and card multiset"),
        "exhaust_pile": ("encoded", "pile summary and card multiset"),
        "monsters": ("encoded", "monster tokens"),
        "relics": ("encoded", "relic tokens"),
        "potion_slots": ("encoded", "potion and empty-slot tokens"),
        "selection": ("encoded", "selection and option tokens"),
        "public_counters": ("encoded", "global counter features"),
    },
    FairCombatContext: {
        "ascension": ("encoded", "global scalar"),
        "act": ("encoded", "global scalar"),
        "floor": ("encoded", "global scalar"),
        "gold": ("encoded", "global scalar"),
    },
    FairPlayer: {
        "hp": ("encoded", "scalar"),
        "max_hp": ("encoded", "scalar"),
        "block": ("encoded", "scalar"),
        "energy": ("encoded", "scalar"),
        "max_energy": ("encoded", "scalar"),
        "powers": ("encoded", "owner power vector"),
    },
    FairPower: {"key": ("encoded", "power vocabulary"), "amount": ("encoded", "power value")},
    FairCardDynamicValues: {
        name: ("encoded", "card scalar with presence mask")
        for name in (
            "rampage_damage_bonus",
            "ritual_dagger_damage_bonus",
            "windmill_retain_damage",
            "steam_barrier_block_reduction",
            "combat_cost_under_turn_override",
        )
    },
    FairCard: {
        "content_key": ("encoded", "card vocabulary and authoritative catalogue lookup"),
        "cost": ("encoded", "scalar"),
        "cost_is_modified": ("encoded", "scalar"),
        "cost_resets_next_turn": ("encoded", "scalar"),
        "upgrade_level": ("encoded", "scalar"),
        "bottled": ("encoded", "scalar"),
        "temporary": ("encoded", "scalar"),
        "dynamic": ("encoded", "dynamic scalar group"),
    },
    FairHandCard: {
        "slot": ("reference_only", "maps actions to card tokens"),
        "card": ("encoded", "card token"),
    },
    FairPile: {
        "count": ("encoded", "pile token"),
        "cards": ("encoded", "canonical public multiset"),
        "known_order": ("encoded", "validated against cards and used with known rank"),
    },
    FairMonsterIntent: {
        "visibility": ("encoded", "intent visibility category"),
        "category": ("encoded", "intent category"),
        "damage": ("encoded", "masked scalar"),
        "hits": ("encoded", "masked scalar"),
    },
    FairMonster: {
        "slot": ("reference_only", "maps action targets"),
        "content_key": ("encoded", "monster vocabulary"),
        "slime_size": ("encoded", "slime category"),
        "hp": ("encoded", "scalar"),
        "max_hp": ("encoded", "scalar"),
        "block": ("encoded", "scalar"),
        "powers": ("encoded", "owner power vector"),
        "stolen_gold": ("encoded", "scalar"),
        "stasis_card": ("encoded", "owned public card token"),
        "intent": ("encoded", "intent categories/scalars"),
        "alive": ("encoded", "scalar"),
        "escaped": ("encoded", "scalar"),
        "minion": ("encoded", "scalar"),
        "targetable": ("encoded", "scalar"),
        "in_defensive_mode": ("encoded", "scalar"),
    },
    FairCounter: {"key": ("encoded", "counter vocabulary"), "value": ("encoded", "counter value")},
    FairRelic: {
        "slot": ("encoded", "validated public relic order encoded as visible rank"),
        "content_key": ("encoded", "relic vocabulary"),
        "state": ("encoded", "owner counter vector"),
    },
    FairPotionSlot: {
        "slot": ("reference_only", "maps potion actions"),
        "content_key": ("encoded", "potion vocabulary/NONE"),
    },
    FairOrb: {"type": ("encoded", "orb vocabulary"), "evoke": ("encoded", "masked scalar")},
    FairOrbSlot: {
        "slot": ("encoded", "validated public orb order encoded as visible rank"),
        "orb": ("encoded", "orb or empty token"),
    },
    FairSelectionOption: {
        "slot": ("reference_only", "maps selection actions"),
        "card": ("encoded", "option card token"),
    },
    FairSelection: {
        "kind": ("encoded", "selection vocabulary"),
        "options": ("encoded", "option tokens"),
        "selected_slots": ("encoded", "selected flag on option tokens"),
    },
    ActionDescriptor: {
        "family": ("encoded", "action-family vocabulary"),
        "kind": ("encoded", "action-kind vocabulary"),
        "hand_slot": ("reference_only", "source entity"),
        "potion_slot": ("reference_only", "source entity"),
        "option_slot": ("reference_only", "source entity"),
        "target_slot": ("reference_only", "target entity"),
        "card_slot": ("ignored", "non-combat descriptor rejected"),
        "node_slot": ("ignored", "non-combat descriptor rejected"),
        "reward_slot": ("ignored", "non-combat descriptor rejected"),
        "shop_slot": ("ignored", "non-combat descriptor rejected"),
        "slot": ("ignored", "non-combat descriptor rejected"),
    },
}
FIELD_COVERAGE: Final[Mapping[type[object], Mapping[str, tuple[Disposition, str]]]] = (
    MappingProxyType(
        {
            data_type: MappingProxyType(dispositions)
            for data_type, dispositions in _FIELD_COVERAGE_DATA.items()
        }
    )
)


@dataclass(frozen=True, slots=True)
class FrozenVocabulary:
    tokens: tuple[str, ...]
    _token_to_index: Mapping[str, int] = field(init=False, repr=False, compare=False)

    def __post_init__(self) -> None:
        if not all(isinstance(token, str) for token in self.tokens):
            raise ValueError("vocabulary tokens must all be strings")
        if (
            self.tokens[:3] != _RESERVED
            or len(self.tokens) != len(set(self.tokens))
            or self.tokens[3:] != tuple(sorted(self.tokens[3:]))
        ):
            raise ValueError("vocabulary must have reserved prefix and sorted unique remainder")
        object.__setattr__(
            self,
            "_token_to_index",
            MappingProxyType({token: index for index, token in enumerate(self.tokens)}),
        )

    @classmethod
    def build(cls, values: Iterable[str]) -> FrozenVocabulary:
        return cls(_RESERVED + tuple(sorted(set(values).difference(_RESERVED))))

    def encode(self, value: str | None) -> tuple[int, bool]:
        token = NONE if value is None else value
        index = self._token_to_index.get(token)
        if index is not None:
            return index, False
        return self._token_to_index[UNK], True

    def to_list(self) -> list[str]:
        return list(self.tokens)


@dataclass(frozen=True, slots=True)
class Vocabularies:
    namespaces: Mapping[str, FrozenVocabulary]
    _fingerprint: str = field(init=False, repr=False, compare=False)

    def __post_init__(self) -> None:
        if set(self.namespaces) != REQUIRED_NAMESPACES:
            missing = sorted(REQUIRED_NAMESPACES.difference(self.namespaces))
            extra = sorted(set(self.namespaces).difference(REQUIRED_NAMESPACES))
            raise ValueError(f"invalid vocabulary namespaces: missing={missing}, extra={extra}")
        frozen = MappingProxyType(dict(self.namespaces))
        object.__setattr__(self, "namespaces", frozen)
        payload = canonical_bytes({key: vocab.to_list() for key, vocab in sorted(frozen.items())})
        object.__setattr__(self, "_fingerprint", sha256_bytes(payload))

    def encode(self, namespace: str, value: str | None) -> tuple[int, bool]:
        return self.namespaces[namespace].encode(value)

    def to_dict(self) -> dict[str, list[str]]:
        return {key: vocab.to_list() for key, vocab in sorted(self.namespaces.items())}

    @property
    def fingerprint(self) -> str:
        return self._fingerprint

    @classmethod
    def from_dict(cls, payload: Mapping[str, Sequence[str]]) -> Vocabularies:
        if set(payload) != REQUIRED_NAMESPACES:
            raise ValueError("serialized vocabularies do not contain the exact required namespaces")
        return cls(
            {key: FrozenVocabulary(tuple(values)) for key, values in sorted(payload.items())}
        )


class VocabularyBuilder:
    def __init__(self) -> None:
        self._values: defaultdict[str, set[str]] = defaultdict(set)
        self._values["card"].update(card.content_key for card in CARD_CATALOGUE)
        for namespace, values in {
            "entity_kind": ENTITY_KINDS,
            "zone": ZONES,
            "card_type": ("attack", "skill", "power", "status"),
            "card_rarity": ("common", "uncommon", "rare"),
            "card_target": ("enemy", "all_enemies", "none"),
            "phase": ("waiting_for_player", "monster_turn", "won", "lost"),
            "intent_visibility": ("hidden", "none", "visible"),
            "intent_category": (
                "unknown",
                "attack",
                "attack_buff",
                "attack_debuff",
                "attack_defend",
                "buff",
                "debuff",
                "strong_debuff",
                "defend",
                "defend_buff",
                "escape",
                "sleep",
                "stun",
            ),
            "orb": ("lightning", "frost", "dark", "empty"),
        }.items():
            self._values[namespace].update(values)

    def add(self, observation: FairCombatObservation, actions: Sequence[ActionDescriptor]) -> None:
        self._values["phase"].add(observation.phase)
        self._values["power"].update(power.key for power in observation.player.powers)
        for monster in observation.monsters:
            self._values["monster"].add(monster.content_key)
            self._values["power"].update(power.key for power in monster.powers)
            if monster.slime_size is not None:
                self._values["slime_size"].add(monster.slime_size)
        for relic in observation.relics:
            self._values["relic"].add(relic.content_key)
            self._values["counter"].update(counter.key for counter in relic.state)
        self._values["potion"].update(
            slot.content_key for slot in observation.potion_slots if slot.content_key is not None
        )
        self._values["counter"].update(counter.key for counter in observation.public_counters)
        if observation.selection is not None:
            self._values["selection"].add(observation.selection.kind)
        for action in actions:
            self._values["action_family"].add(action.family)
            self._values["action_kind"].add(action.kind)

    def freeze(self) -> Vocabularies:
        required = (
            "monster",
            "relic",
            "potion",
            "power",
            "counter",
            "selection",
            "slime_size",
            "action_family",
            "action_kind",
        )
        for namespace in required:
            self._values[namespace]
        return Vocabularies(
            {key: FrozenVocabulary.build(values) for key, values in sorted(self._values.items())}
        )


@dataclass(frozen=True, slots=True)
class TensorizedCombatDecision:
    vocabulary_fingerprint: str
    entity_kind: torch.Tensor
    entity_content: torch.Tensor
    entity_zone: torch.Tensor
    entity_categories: torch.Tensor
    entity_scalars: torch.Tensor
    entity_scalar_mask: torch.Tensor
    entity_powers: torch.Tensor
    entity_power_mask: torch.Tensor
    entity_power_counts: torch.Tensor
    entity_counters: torch.Tensor
    entity_counter_mask: torch.Tensor
    entity_counter_counts: torch.Tensor
    entity_parent: torch.Tensor
    action_family: torch.Tensor
    action_kind: torch.Tensor
    action_source: torch.Tensor
    action_source_mask: torch.Tensor
    action_target: torch.Tensor
    action_target_mask: torch.Tensor
    oov_counts: Mapping[str, int]

    @property
    def action_count(self) -> int:
        return int(self.action_kind.shape[0])


@dataclass(frozen=True, slots=True)
class BatchedCombatDecision:
    vocabulary_fingerprint: str
    entity_kind: torch.Tensor
    entity_content: torch.Tensor
    entity_zone: torch.Tensor
    entity_categories: torch.Tensor
    entity_scalars: torch.Tensor
    entity_scalar_mask: torch.Tensor
    entity_powers: torch.Tensor
    entity_power_mask: torch.Tensor
    entity_power_counts: torch.Tensor
    entity_counters: torch.Tensor
    entity_counter_mask: torch.Tensor
    entity_counter_counts: torch.Tensor
    entity_parent: torch.Tensor
    entity_mask: torch.Tensor
    action_family: torch.Tensor
    action_kind: torch.Tensor
    action_source: torch.Tensor
    action_source_mask: torch.Tensor
    action_target: torch.Tensor
    action_target_mask: torch.Tensor
    action_mask: torch.Tensor


def _float32_value(value: float | bool, label: str) -> float:
    try:
        converted = float(value)
    except OverflowError as error:
        raise ValueError(f"{label} is not representable as float32") from error
    if not math.isfinite(converted) or abs(converted) > torch.finfo(torch.float32).max:
        raise ValueError(f"{label} must be finite and representable as float32")
    return converted


class _Encoder:
    def __init__(self, vocabularies: Vocabularies) -> None:
        self.vocabularies = vocabularies
        self.oov: defaultdict[str, int] = defaultdict(int)
        self.kind: list[int] = []
        self.content: list[int] = []
        self.zone: list[int] = []
        self.categories: list[list[int]] = []
        self.scalars: list[list[float]] = []
        self.scalar_mask: list[list[bool]] = []
        self.powers: list[list[float]] = []
        self.power_mask: list[list[bool]] = []
        self.power_counts: list[list[int]] = []
        self.counters: list[list[float]] = []
        self.counter_mask: list[list[bool]] = []
        self.counter_counts: list[list[int]] = []
        self.parent: list[int] = []

    def code(self, namespace: str, value: str | None) -> int:
        code, unknown = self.vocabularies.encode(namespace, value)
        if unknown:
            self.oov[namespace] += 1
        return code

    def add_entity(
        self,
        kind: str,
        content_namespace: str,
        content: str | None,
        *,
        zone: str = "none",
        category_values: Sequence[tuple[str, str | None]] = (),
        scalar_values: Mapping[str, int | float | bool | None] | None = None,
        powers: Sequence[FairPower] = (),
        counters: Sequence[FairCounter] = (),
        parent: int = -1,
    ) -> int:
        index = len(self.kind)
        self.kind.append(self.code("entity_kind", kind))
        self.content.append(self.code(content_namespace, content))
        self.zone.append(self.code("zone", zone))
        categories = [self.code(namespace, None) for namespace in CATEGORY_NAMESPACES]
        for namespace, value in category_values:
            categories[CATEGORY_NAMESPACES.index(namespace)] = self.code(namespace, value)
        self.categories.append(categories)
        values = [0.0] * len(SCALAR_NAMES)
        masks = [False] * len(SCALAR_NAMES)
        for name, value in (scalar_values or {}).items():
            if value is not None:
                values[SCALAR_INDEX[name]] = _float32_value(value, name)
                masks[SCALAR_INDEX[name]] = True
        self.scalars.append(values)
        self.scalar_mask.append(masks)
        power_values = [0.0] * len(self.vocabularies.namespaces["power"].tokens)
        power_masks = [False] * len(power_values)
        power_counts = [0] * len(power_values)
        for power in powers:
            power_index = self.code("power", power.key)
            power_values[power_index] = _float32_value(
                power_values[power_index] + _float32_value(power.amount, power.key),
                power.key,
            )
            power_counts[power_index] += 1
            power_masks[power_index] = True
        self.powers.append(power_values)
        self.power_mask.append(power_masks)
        self.power_counts.append(power_counts)
        counter_values = [0.0] * len(self.vocabularies.namespaces["counter"].tokens)
        counter_masks = [False] * len(counter_values)
        counter_counts = [0] * len(counter_values)
        for counter in counters:
            counter_index = self.code("counter", counter.key)
            counter_values[counter_index] = _float32_value(
                counter_values[counter_index] + _float32_value(counter.value, counter.key),
                counter.key,
            )
            counter_counts[counter_index] += 1
            counter_masks[counter_index] = True
        self.counters.append(counter_values)
        self.counter_mask.append(counter_masks)
        self.counter_counts.append(counter_counts)
        self.parent.append(parent)
        return index

    def add_card(
        self,
        card: FairCard,
        zone: str,
        *,
        known_rank: int | None = None,
        selected: bool | None = None,
        parent: int = -1,
    ) -> int:
        definition = CARD_DEFINITIONS.get(card.content_key)
        if definition is None:
            raise ValueError(f"card is absent from authoritative catalogue: {card.content_key}")
        dynamic = card.dynamic
        scalars: dict[str, int | bool | None] = {
            "cost": card.cost,
            "cost_is_modified": card.cost_is_modified,
            "cost_resets_next_turn": card.cost_resets_next_turn,
            "upgrade_level": card.upgrade_level,
            "bottled": card.bottled,
            "temporary": card.temporary,
            "rampage_damage_bonus": dynamic.rampage_damage_bonus,
            "ritual_dagger_damage_bonus": dynamic.ritual_dagger_damage_bonus,
            "windmill_retain_damage": dynamic.windmill_retain_damage,
            "steam_barrier_block_reduction": dynamic.steam_barrier_block_reduction,
            "combat_cost_under_turn_override": dynamic.combat_cost_under_turn_override,
            "printed_cost": definition.printed_cost,
            "printed_damage": definition.values.damage,
            "printed_block": definition.values.block,
            "printed_vulnerable": definition.values.vulnerable,
            "innate": definition.keywords.innate,
            "ethereal": definition.keywords.ethereal,
            "exhaust": definition.keywords.exhaust,
            "retain": definition.keywords.retain,
            "unplayable": definition.keywords.unplayable,
            "is_curse": definition.is_curse,
            "known_rank": known_rank,
            "selection_selected": selected,
        }
        return self.add_entity(
            "card",
            "card",
            card.content_key,
            zone=zone,
            category_values=(
                ("card_type", definition.card_type),
                ("card_rarity", definition.rarity),
                ("card_target", definition.target),
            ),
            scalar_values=scalars,
            parent=parent,
        )


def _optional_int_key(value: int | None) -> tuple[bool, int]:
    return value is not None, 0 if value is None else value


def _card_key(card: FairCard) -> tuple[object, ...]:
    return (
        card.content_key,
        card.cost,
        card.cost_is_modified,
        card.cost_resets_next_turn,
        card.upgrade_level,
        card.bottled,
        card.temporary,
        _optional_int_key(card.dynamic.rampage_damage_bonus),
        _optional_int_key(card.dynamic.ritual_dagger_damage_bonus),
        _optional_int_key(card.dynamic.windmill_retain_damage),
        _optional_int_key(card.dynamic.steam_barrier_block_reduction),
        _optional_int_key(card.dynamic.combat_cost_under_turn_override),
    )


ENCODER_CONTRACT_VERSION: Final = 1
SCALAR_PREPROCESSING: Final = "identity_float32_v1"

_ACTION_SPECS: Final[Mapping[str, tuple[str | None, bool]]] = MappingProxyType(
    {
        "play_hand_slot": ("hand", True),
        "end_turn": (None, False),
        "use_potion_slot": ("potion", True),
        "discard_potion_slot": ("potion", False),
        "toggle_visible_card": ("option", False),
        "choose_visible_option": ("option", False),
        "confirm_selection": (None, False),
        "skip_selection": (None, False),
        "proceed": (None, False),
    }
)


def encoder_contract_digest(vocabularies: Vocabularies) -> str:
    payload = {
        "version": ENCODER_CONTRACT_VERSION,
        "observation_schemas": sorted(KNOWN_OBSERVATION_SCHEMA_VERSIONS),
        "entity_kinds": list(ENTITY_KINDS),
        "zones": list(ZONES),
        "categories": list(CATEGORY_NAMESPACES),
        "scalars": list(SCALAR_NAMES),
        "scalar_preprocessing": SCALAR_PREPROCESSING,
        "action_specs": {key: list(value) for key, value in sorted(_ACTION_SPECS.items())},
        "vocabulary_fingerprint": vocabularies.fingerprint,
    }
    return sha256_bytes(canonical_bytes(payload))


def _validate_actions(actions: Sequence[ActionDescriptor]) -> None:
    for action in actions:
        if action.family != "combat":
            raise ValueError(f"non-combat action family: {action.family}")
        spec = _ACTION_SPECS.get(action.kind)
        if spec is None:
            raise ValueError(f"unknown combat action kind: {action.kind}")
        if any(
            value is not None
            for value in (
                action.card_slot,
                action.node_slot,
                action.reward_slot,
                action.shop_slot,
                action.slot,
            )
        ):
            raise ValueError("non-combat slot on combat action descriptor")
        source_name, target_allowed = spec
        sources = {
            "hand": action.hand_slot,
            "potion": action.potion_slot,
            "option": action.option_slot,
        }
        populated_sources = [name for name, value in sources.items() if value is not None]
        if populated_sources != ([] if source_name is None else [source_name]):
            raise ValueError(f"malformed sources for combat action kind: {action.kind}")
        if action.target_slot is not None and not target_allowed:
            raise ValueError(f"target is not allowed for combat action kind: {action.kind}")


class _Slotted(Protocol):
    @property
    def slot(self) -> int: ...


def _canonical_by_slot[T: _Slotted](items: Sequence[T], label: str) -> tuple[T, ...]:
    ordered = tuple(sorted(items, key=lambda item: item.slot))
    if [item.slot for item in ordered] != list(range(len(ordered))):
        raise ValueError(f"{label} slots must be unique and contiguous from zero")
    return ordered


def tensorize_combat(
    observation: FairCombatObservation,
    actions: Sequence[ActionDescriptor],
    vocabularies: Vocabularies,
) -> TensorizedCombatDecision:
    """Encode one symbolic decision while retaining the input action-row order."""

    _validate_actions(actions)
    if observation.schema_version not in KNOWN_OBSERVATION_SCHEMA_VERSIONS:
        raise ValueError(f"unsupported fair observation schema: {observation.schema_version}")
    encoder = _Encoder(vocabularies)
    global_index = encoder.add_entity(
        "global",
        "phase",
        observation.phase,
        scalar_values={
            "observation_schema_version": observation.schema_version,
            "ascension": observation.context.ascension,
            "act": observation.context.act,
            "floor": observation.context.floor,
            "gold": observation.context.gold,
        },
        counters=observation.public_counters,
    )
    encoder.add_entity(
        "player",
        "entity_kind",
        "player",
        scalar_values={
            "hp": observation.player.hp,
            "max_hp": observation.player.max_hp,
            "block": observation.player.block,
            "energy": observation.player.energy,
            "max_energy": observation.player.max_energy,
        },
        powers=observation.player.powers,
        parent=global_index,
    )

    hand_map: dict[int, int] = {}
    canonical_hand = _canonical_by_slot(observation.hand, "hand")
    for hand in sorted(canonical_hand, key=lambda item: _card_key(item.card)):
        hand_map[hand.slot] = encoder.add_card(hand.card, "hand")

    for zone, pile in (
        ("draw", observation.draw_pile),
        ("discard", observation.discard_pile),
        ("exhaust", observation.exhaust_pile),
    ):
        if pile.count != len(pile.cards):
            raise ValueError(f"{zone} pile count does not match public cards")
        pile_index = encoder.add_entity(
            "pile", "zone", zone, zone=zone, scalar_values={"pile_count": pile.count}
        )
        if pile.known_order:
            # Observation schemas 1 and 2 expose either no order or the complete
            # Frozen Eye permutation. A future ranked-prefix representation needs
            # an explicit observation-contract extension before it can be encoded.
            if Counter(map(_card_key, pile.cards)) != Counter(map(_card_key, pile.known_order)):
                raise ValueError(
                    "known_order must be empty or a complete pile permutation in schema 1/2"
                )
            visible_cards = tuple(enumerate(pile.known_order))
        else:
            visible_cards = tuple((None, card) for card in sorted(pile.cards, key=_card_key))
        for rank, card in visible_cards:
            encoder.add_card(card, zone, known_rank=rank, parent=pile_index)

    monster_map: dict[int, int] = {}
    for monster in _canonical_by_slot(observation.monsters, "monster"):
        intent = monster.intent
        monster_index = encoder.add_entity(
            "monster",
            "monster",
            monster.content_key,
            category_values=(
                ("intent_visibility", intent.visibility),
                ("intent_category", intent.category),
                ("slime_size", monster.slime_size),
            ),
            scalar_values={
                "hp": monster.hp,
                "max_hp": monster.max_hp,
                "block": monster.block,
                "stolen_gold": monster.stolen_gold,
                "alive": monster.alive,
                "escaped": monster.escaped,
                "minion": monster.minion,
                "targetable": monster.targetable,
                "in_defensive_mode": monster.in_defensive_mode,
                "intent_damage": intent.damage,
                "intent_hits": intent.hits,
            },
            powers=monster.powers,
        )
        monster_map[monster.slot] = monster_index
        if monster.stasis_card is not None:
            encoder.add_card(monster.stasis_card, "stasis", parent=monster_index)

    for relic in _canonical_by_slot(observation.relics, "relic"):
        encoder.add_entity(
            "relic",
            "relic",
            relic.content_key,
            scalar_values={"visible_rank": relic.slot},
            counters=relic.state,
        )

    potion_map: dict[int, int] = {}
    for potion in _canonical_by_slot(observation.potion_slots, "potion"):
        potion_map[potion.slot] = encoder.add_entity("potion", "potion", potion.content_key)

    for orb_slot in _canonical_by_slot(observation.orb_slots, "orb"):
        orb = orb_slot.orb
        encoder.add_entity(
            "orb",
            "orb",
            "empty" if orb is None else orb.type,
            scalar_values={
                "orb_evoke": None if orb is None else orb.evoke,
                "visible_rank": orb_slot.slot,
            },
        )

    option_map: dict[int, int] = {}
    if observation.selection is not None:
        selection_index = encoder.add_entity("selection", "selection", observation.selection.kind)
        selected = set(observation.selection.selected_slots)
        canonical_options = _canonical_by_slot(observation.selection.options, "selection option")
        option_slots = {option.slot for option in canonical_options}
        if not selected <= option_slots:
            raise ValueError("invalid selected option slots")
        for option in sorted(canonical_options, key=lambda item: _card_key(item.card)):
            option_map[option.slot] = encoder.add_card(
                option.card, "selection", selected=option.slot in selected, parent=selection_index
            )

    action_family: list[int] = []
    action_kind: list[int] = []
    action_source: list[int] = []
    action_source_mask: list[bool] = []
    action_target: list[int] = []
    action_target_mask: list[bool] = []
    for action in actions:
        action_family.append(encoder.code("action_family", action.family))
        action_kind.append(encoder.code("action_kind", action.kind))
        source: int | None = None
        if action.hand_slot is not None:
            source = hand_map.get(action.hand_slot)
        elif action.potion_slot is not None:
            source = potion_map.get(action.potion_slot)
        elif action.option_slot is not None:
            source = option_map.get(action.option_slot)
        if source is None and any(
            value is not None
            for value in (action.hand_slot, action.potion_slot, action.option_slot)
        ):
            raise ValueError("action source slot is absent from observation")
        target = None if action.target_slot is None else monster_map.get(action.target_slot)
        if target is None and action.target_slot is not None:
            raise ValueError("action target slot is absent from observation")
        action_source.append(-1 if source is None else source)
        action_source_mask.append(source is not None)
        action_target.append(-1 if target is None else target)
        action_target_mask.append(target is not None)

    return TensorizedCombatDecision(
        vocabulary_fingerprint=vocabularies.fingerprint,
        entity_kind=torch.tensor(encoder.kind, dtype=torch.long),
        entity_content=torch.tensor(encoder.content, dtype=torch.long),
        entity_zone=torch.tensor(encoder.zone, dtype=torch.long),
        entity_categories=torch.tensor(encoder.categories, dtype=torch.long),
        entity_scalars=torch.tensor(encoder.scalars, dtype=torch.float32),
        entity_scalar_mask=torch.tensor(encoder.scalar_mask, dtype=torch.bool),
        entity_powers=torch.tensor(encoder.powers, dtype=torch.float32),
        entity_power_mask=torch.tensor(encoder.power_mask, dtype=torch.bool),
        entity_power_counts=torch.tensor(encoder.power_counts, dtype=torch.long),
        entity_counters=torch.tensor(encoder.counters, dtype=torch.float32),
        entity_counter_mask=torch.tensor(encoder.counter_mask, dtype=torch.bool),
        entity_counter_counts=torch.tensor(encoder.counter_counts, dtype=torch.long),
        entity_parent=torch.tensor(encoder.parent, dtype=torch.long),
        action_family=torch.tensor(action_family, dtype=torch.long),
        action_kind=torch.tensor(action_kind, dtype=torch.long),
        action_source=torch.tensor(action_source, dtype=torch.long),
        action_source_mask=torch.tensor(action_source_mask, dtype=torch.bool),
        action_target=torch.tensor(action_target, dtype=torch.long),
        action_target_mask=torch.tensor(action_target_mask, dtype=torch.bool),
        oov_counts=dict(encoder.oov),
    )


def _pad(items: Sequence[torch.Tensor], length: int, value: float | bool = 0) -> torch.Tensor:
    shape = (len(items), length, *items[0].shape[1:])
    output = torch.full(shape, value, dtype=items[0].dtype)
    for index, item in enumerate(items):
        output[index, : item.shape[0]] = item
    return output


def collate_combat_tensors(items: Sequence[TensorizedCombatDecision]) -> BatchedCombatDecision:
    if not items:
        raise ValueError("cannot collate an empty batch")
    fingerprints = {item.vocabulary_fingerprint for item in items}
    if len(fingerprints) != 1:
        raise ValueError("cannot collate decisions encoded with different vocabularies")
    vocabulary_fingerprint = next(iter(fingerprints))
    entity_count = max(item.entity_kind.shape[0] for item in items)
    action_count = max(item.action_kind.shape[0] for item in items)
    entity_mask = torch.zeros((len(items), entity_count), dtype=torch.bool)
    action_mask = torch.zeros((len(items), action_count), dtype=torch.bool)
    for index, item in enumerate(items):
        entity_mask[index, : item.entity_kind.shape[0]] = True
        action_mask[index, : item.action_kind.shape[0]] = True
    return BatchedCombatDecision(
        vocabulary_fingerprint=vocabulary_fingerprint,
        entity_kind=_pad([item.entity_kind for item in items], entity_count),
        entity_content=_pad([item.entity_content for item in items], entity_count),
        entity_zone=_pad([item.entity_zone for item in items], entity_count),
        entity_categories=_pad([item.entity_categories for item in items], entity_count),
        entity_scalars=_pad([item.entity_scalars for item in items], entity_count),
        entity_scalar_mask=_pad([item.entity_scalar_mask for item in items], entity_count),
        entity_powers=_pad([item.entity_powers for item in items], entity_count),
        entity_power_mask=_pad([item.entity_power_mask for item in items], entity_count),
        entity_power_counts=_pad([item.entity_power_counts for item in items], entity_count),
        entity_counters=_pad([item.entity_counters for item in items], entity_count),
        entity_counter_mask=_pad([item.entity_counter_mask for item in items], entity_count),
        entity_counter_counts=_pad([item.entity_counter_counts for item in items], entity_count),
        entity_parent=_pad([item.entity_parent for item in items], entity_count, -1),
        entity_mask=entity_mask,
        action_family=_pad([item.action_family for item in items], action_count),
        action_kind=_pad([item.action_kind for item in items], action_count),
        action_source=_pad([item.action_source for item in items], action_count, -1),
        action_source_mask=_pad([item.action_source_mask for item in items], action_count),
        action_target=_pad([item.action_target for item in items], action_count, -1),
        action_target_mask=_pad([item.action_target_mask for item in items], action_count),
        action_mask=action_mask,
    )


def _nested_dataclasses(annotation: object) -> set[type[object]]:
    nested: set[type[object]] = set()
    if isinstance(annotation, type) and is_dataclass(annotation):
        nested.add(annotation)
    for argument in get_args(annotation):
        nested.update(_nested_dataclasses(argument))
    return nested


def field_coverage_mismatches(
    roots: Sequence[type[object]] = (FairCombatObservation, ActionDescriptor),
    ledger: Mapping[type[object], Mapping[str, tuple[str, str]]] = FIELD_COVERAGE,
) -> dict[str, tuple[set[str], set[str]]]:
    """Walk resolved dataclass types and return missing/extra ledger fields."""

    mismatches: dict[str, tuple[set[str], set[str]]] = {}
    pending = list(roots)
    visited: set[type[object]] = set()
    while pending:
        data_type = pending.pop()
        if data_type in visited:
            continue
        visited.add(data_type)
        hints = get_type_hints(data_type)
        declared = ledger.get(data_type)
        if declared is None:
            mismatches[data_type.__name__] = (set(hints), set())
        elif set(hints) != set(declared):
            mismatches[data_type.__name__] = (
                set(hints).difference(declared),
                set(declared).difference(hints),
            )
        for annotation in hints.values():
            pending.extend(_nested_dataclasses(annotation))
    return mismatches
