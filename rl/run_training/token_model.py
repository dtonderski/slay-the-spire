"""Token macro model: one position-free token per card, relic, potion, map node and candidate.

Public typed observations only. Deck order never enters the model: a card token
is its identity plus upgrade state, so two decks with the same cards in a
different order encode identically. Legal candidates are tokens in the same
sequence and attend to the deck, so an offer can be scored against what is held.

Not encoded: relic counters, card costs and dynamic card values, and map edges
beyond distance from the current position. See README "Token encoder".
"""

import hashlib
from collections import Counter
from dataclasses import dataclass
from functools import lru_cache

import numpy as np
import torch
from numpy.typing import NDArray
from sts_sim import (
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
    OBSERVATION_TYPES,
    CardKey,
    Observation,
    PotionKey,
    RelicKey,
)
from sts_sim.observations.common import Card, CardSlot, PotionSlot
from sts_sim.observations.screens import (
    EventChoice,
    EventScreen,
    GridScreen,
    MapNode,
    MapScreen,
    QueuedCardReward,
    RelicOffer,
    ShopCardOffer,
    ShopOffer,
    TreasureScreen,
)
from torch import Tensor, nn

from run_training.contracts import PolicyAction
from run_training.model import MacroPolicy, candidate_facts, policy_controls
from run_training.restrictions import ClosedCardRewards

# Identity rows are exact; free text (action kinds, event labels, screen tags)
# is hashed into a separate range so it can never collide with an identity.
_CARDS = sorted({key.value.removesuffix("+") for key in CardKey})
_RELICS = sorted(key.value for key in RelicKey)
_POTIONS = sorted(key.value for key in PotionKey)
_CARD0 = 1  # row 0 is padding
_RELIC0 = _CARD0 + len(_CARDS)
_POTION0 = _RELIC0 + len(_RELICS)
_TEXT0 = _POTION0 + len(_POTIONS)
TEXT_BUCKETS = 4096
VOCABULARY = _TEXT0 + TEXT_BUCKETS
_CARD = {name: _CARD0 + i for i, name in enumerate(_CARDS)}
_RELIC = {name: _RELIC0 + i for i, name in enumerate(_RELICS)}
_POTION = {name: _POTION0 + i for i, name in enumerate(_POTIONS)}
# Changes whenever the public content catalog or this layout changes, so a
# checkpoint is never loaded against different embedding rows.
TOKEN_FEATURE_VERSION = (
    "tokens-v1-"
    + hashlib.sha256(
        "\0".join(_CARDS + ["|"] + _RELICS + ["|"] + _POTIONS).encode()
    ).hexdigest()[:12]
)

IDS = 6  # embedding rows summed per token
ROOMS = ("combat", "elite", "event", "rest", "shop", "treasure", "boss")
# Numeric columns. Run scalars live on the summary token only.
HP_FRACTION, HP, MAX_HP, GOLD, FLOOR, DECK_SIZE, RUBY, EMERALD, SAPPHIRE, FINAL_ACT = (
    range(10)
)
PRICE, AFFORDABLE, AMOUNT, DISTANCE, BURNING, UPGRADES = range(10, 16)
AHEAD = 16  # then one column per ROOMS entry: share of rooms ahead of a map choice
NUMBERS = AHEAD + len(ROOMS)
# Attention over different padded shapes is not bit-identical between collection
# and learner batches; this bounds rounding, while a wrong input is far larger.
RECOMPUTE_ATOL = RECOMPUTE_RTOL = 1e-4

Token = tuple[list[int], dict[int, float]]


@lru_cache(maxsize=65536)
def _text(text: str) -> int:
    digest = hashlib.blake2b(text.encode(), digest_size=8).digest()
    return _TEXT0 + int.from_bytes(digest, "little") % TEXT_BUCKETS


def _card(card: Card) -> Token:
    key = card.content_key.value
    ids = [_CARD[key.removesuffix("+")]]
    if key.endswith("+") or card.upgrade_level:
        ids.append(_text("upgraded"))
    if card.bottled:
        ids.append(_text("bottled"))
    return ids, {UPGRADES: card.upgrade_level / 5}


def _ahead(nodes: tuple[MapNode, ...], start: tuple[int, ...]) -> dict[int, int]:
    """Distance in rooms from the given starting nodes to every node they lead to."""
    distance = {slot: 1 for slot in start}
    frontier = list(start)
    while frontier:
        following = []
        for slot in frontier:
            for child in nodes[slot].children:
                if child not in distance:
                    distance[child] = distance[slot] + 1
                    following.append(child)
        frontier = following
    return distance


@lru_cache(maxsize=256)
def _map_tokens(screen: MapScreen) -> tuple[Token, ...]:
    return tuple(
        (
            [_text("type=map"), _text(f"room={screen.nodes[slot].room_kind}")],
            {
                DISTANCE: distance / 15,
                BURNING: float(screen.nodes[slot].burning_elite),
            },
        )
        for slot, distance in _ahead(screen.nodes, screen.reachable_nodes).items()
    )


def _candidate(observation: Observation, action: PolicyAction) -> Token:
    ids = [_text("type=candidate"), _text(f"kind={action.kind}")]
    numbers: dict[int, float] = {}
    gold = observation.context.gold
    screen = observation.screen

    def price(value: int) -> None:
        numbers[PRICE] = value / 100
        numbers[AFFORDABLE] = float(gold >= value)

    for fact in candidate_facts(observation, action)[1:]:
        if isinstance(fact, CardSlot):
            fact = fact.card
        if isinstance(fact, Card):
            card_ids, card_numbers = _card(fact)
            ids += card_ids
            numbers |= card_numbers
        elif isinstance(fact, ShopCardOffer):
            card_ids, card_numbers = _card(fact.card)
            ids += card_ids
            numbers |= card_numbers
            price(fact.price)
        elif isinstance(fact, ShopOffer):
            key = fact.content_key
            ids.append(
                _RELIC[key.value] if isinstance(key, RelicKey) else _POTION[key.value]
            )
            price(fact.price)
        elif isinstance(fact, RelicOffer):
            ids.append(_RELIC[fact.content_key.value])
        elif isinstance(fact, RelicKey):
            ids.append(_RELIC[fact.value])
        elif isinstance(fact, PotionKey):
            ids.append(_POTION[fact.value])
        elif isinstance(fact, PotionSlot):
            if fact.content_key is not None:
                ids.append(_POTION[fact.content_key.value])
        elif isinstance(fact, EventChoice):
            assert isinstance(screen, EventScreen)
            ids.append(_text(f"choice={screen.event.value}:{fact.label}"))
        elif isinstance(fact, MapNode):
            assert isinstance(screen, MapScreen)
            ids.append(_text(f"room={fact.room_kind}"))
            numbers[BURNING] = float(fact.burning_elite)
            rooms: Counter[str] = Counter(
                screen.nodes[slot].room_kind
                for slot in _ahead(screen.nodes, fact.children)
            )
            for column, room in enumerate(ROOMS):
                numbers[AHEAD + column] = rooms[room] / max(1, sum(rooms.values()))
        elif isinstance(fact, QueuedCardReward):
            numbers[AMOUNT] = fact.choice_count / 100
        elif isinstance(fact, int):  # a gold offer
            numbers[AMOUNT] = fact / 100
        elif isinstance(fact, tuple):  # a map node's children: covered by AHEAD
            continue
        elif fact is not None:
            raise TypeError(f"Unsupported candidate fact: {type(fact).__name__}")
    return ids, numbers


@dataclass(frozen=True, eq=False)
class TokenInput:
    """One macro decision: context tokens followed by one token per legal candidate."""

    ids: NDArray[np.int64]  # [tokens, IDS], zero padded
    numbers: NDArray[np.float32]  # [tokens, NUMBERS]
    count: int  # candidates, the last `count` tokens
    eligibility: tuple[bool, ...] = ()
    temperature: float = 1.0

    @property
    def candidates(self) -> range:
        return range(self.count)


def encode_tokens(
    observation: Observation,
    actions: tuple[PolicyAction, ...],
    *,
    previous: PolicyAction | None = None,
    avoid_prismatic: bool = False,
    commit_navigation: bool = False,
    shop_finished: bool = False,
    exploration_profile: str = "none",
    direct_smith: bool = False,
    closed_card_rewards: ClosedCardRewards | None = None,
) -> TokenInput:
    if not isinstance(observation, OBSERVATION_TYPES) or any(
        not isinstance(a, PolicyAction) for a in actions
    ):
        raise TypeError(
            "Only public typed observations and revision-free actions are model inputs"
        )
    if observation.schema_version != FAIR_RUN_OBSERVATION_SCHEMA_VERSION:
        raise ValueError("Unsupported run observation schema")
    eligibility, temperature = policy_controls(
        observation,
        actions,
        avoid_prismatic=avoid_prismatic,
        commit_navigation=commit_navigation,
        shop_finished=shop_finished,
        exploration_profile=exploration_profile,
        direct_smith=direct_smith,
        closed_card_rewards=closed_card_rewards,
    )
    context = observation.context
    screen = observation.screen
    detail = (
        screen.event.value
        if isinstance(screen, EventScreen)
        else screen.purpose
        if isinstance(screen, GridScreen)
        else screen.chest_size
        if isinstance(screen, TreasureScreen)
        else ""
    )
    tokens: list[Token] = [
        (
            [
                _text("type=summary"),
                _text(f"screen={observation.kind}"),
                _text(f"act={context.act}"),
                _text(f"boss={context.act_boss}"),
                _text(f"detail={detail}"),
                _text(f"previous={previous.kind if previous else None}"),
            ],
            {
                HP_FRACTION: context.player_hp / max(1, context.player_max_hp),
                HP: context.player_hp / 100,
                MAX_HP: context.player_max_hp / 100,
                GOLD: float(np.log1p(context.gold)) / 7,
                FLOOR: context.floor / 55,
                DECK_SIZE: len(context.deck) / 40,
                RUBY: float(context.keys.ruby),
                EMERALD: float(context.keys.emerald),
                SAPPHIRE: float(context.keys.sapphire),
                FINAL_ACT: float(context.final_act_available),
            },
        )
    ]
    for card in context.deck:
        ids, numbers = _card(card)
        tokens.append(([_text("type=deck"), *ids], numbers))
    tokens += [
        ([_text("type=relic"), _RELIC[relic.content_key.value]], {})
        for relic in context.relics
    ]
    tokens += [
        (
            [
                _text("type=potion"),
                _POTION[slot.content_key.value]
                if slot.content_key is not None
                else _text("empty"),
            ],
            {},
        )
        for slot in context.potion_slots
    ]
    if context.map is not None:
        tokens += _map_tokens(context.map)
    tokens += [_candidate(observation, action) for action in actions]
    ids = np.zeros((len(tokens), IDS), dtype=np.int64)
    numbers = np.zeros((len(tokens), NUMBERS), dtype=np.float32)
    for row, (token_ids, token_numbers) in enumerate(tokens):
        ids[row, : len(token_ids)] = token_ids
        for column, value in token_numbers.items():
            numbers[row, column] = value
    return TokenInput(ids, numbers, len(actions), eligibility, temperature)


@dataclass(frozen=True)
class TokenBatch:
    ids: Tensor  # [rows, tokens, IDS]
    numbers: Tensor  # [rows, tokens, NUMBERS]
    padding: Tensor  # [rows, tokens], True where padded
    sources: Tensor  # candidate tokens as indices into flattened [rows * tokens]
    owners: Tensor  # the row of each candidate
    positions: Tensor  # each candidate's index into flattened padded logits
    lengths: tuple[int, ...]
    temperatures: Tensor


class TokenMacroModel(MacroPolicy):
    """A small transformer over all tokens; value from the summary, one logit per candidate."""

    recompute_atol = RECOMPUTE_ATOL
    recompute_rtol = RECOMPUTE_RTOL

    def __init__(self, width: int = 64, layers: int = 2, heads: int = 4) -> None:
        super().__init__()
        self.embedding = nn.Embedding(VOCABULARY, width, padding_idx=0)
        self.numbers = nn.Linear(NUMBERS, width)
        self.encoder = nn.TransformerEncoder(
            nn.TransformerEncoderLayer(
                width, heads, 4 * width, dropout=0.0, batch_first=True, norm_first=True
            ),
            layers,
            enable_nested_tensor=False,
        )
        self.norm = nn.LayerNorm(width)
        self.policy = nn.Linear(width, 1)
        self.value = nn.Linear(width, 1)
        # Start uniform over candidates and with a zero return baseline.
        for head in (self.policy, self.value):
            nn.init.zeros_(head.weight)
            nn.init.zeros_(head.bias)

    def pack(self, inputs: list[TokenInput]) -> TokenBatch:
        if not inputs or any(row.count < 1 for row in inputs):
            raise ValueError("A macro batch requires nonempty complete candidate sets")
        if any(
            not np.isfinite(row.temperature) or row.temperature < 1 for row in inputs
        ):
            raise ValueError("Invalid macro policy temperature")
        lengths = tuple(row.count for row in inputs)
        tokens = max(len(row.ids) for row in inputs)
        ids = np.zeros((len(inputs), tokens, IDS), dtype=np.int64)
        numbers = np.zeros((len(inputs), tokens, NUMBERS), dtype=np.float32)
        padding = np.ones((len(inputs), tokens), dtype=np.bool_)
        sources, owners, positions = [], [], []
        for i, row in enumerate(inputs):
            size = len(row.ids)
            ids[i, :size] = row.ids
            numbers[i, :size] = row.numbers
            padding[i, :size] = False
            for j in range(row.count):
                sources.append(i * tokens + size - row.count + j)
                owners.append(i)
                positions.append(i * max(lengths) + j)
        device = self.embedding.weight.device
        return TokenBatch(
            torch.from_numpy(ids).to(device),
            torch.from_numpy(numbers).to(device),
            torch.from_numpy(padding).to(device),
            torch.tensor(sources, device=device),
            torch.tensor(owners, device=device),
            torch.tensor(positions, device=device),
            lengths,
            torch.tensor(
                [row.temperature for row in inputs], dtype=torch.float32, device=device
            ),
        )

    def forward_batch(self, batch: TokenBatch) -> tuple[Tensor, Tensor]:
        tokens = self.embedding(batch.ids).sum(2) + self.numbers(batch.numbers)
        hidden = self.norm(self.encoder(tokens, src_key_padding_mask=batch.padding))
        values = self.value(hidden[:, 0]).squeeze(-1)
        logits = self.policy(hidden.flatten(0, 1)[batch.sources]).squeeze(-1)
        # In forward, not sampling only: collection, diagnostics and gradients
        # all see the same tempered distribution.
        logits = logits / batch.temperatures[batch.owners]
        count = len(batch.lengths)
        padded = logits.new_full((count * max(batch.lengths),), -torch.inf)
        padded = padded.scatter(0, batch.positions, logits)
        return padded.view(count, max(batch.lengths)), values

    def forward(self, inputs: TokenInput) -> tuple[Tensor, Tensor]:
        logits, values = self.forward_batch(self.pack([inputs]))
        return logits[0, : inputs.count], values[0]
