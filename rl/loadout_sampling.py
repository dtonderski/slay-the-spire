"""Independent synthetic loadout specs fitted to run-history marginals, not combat states.

Identities use the current simulator's PUBLIC catalog (base card keys plus
upgrade counts). Initialization still must handle relic state and validate
mechanical support, not just identity membership. No on-acquisition relic effects should be replayed blindly:
source final decks already reflect those effects. Ending-floor data is not an
exact precombat distribution. No simulator state is modified here.
"""

import json
import math
import random
from dataclasses import dataclass
from itertools import accumulate
from pathlib import Path

# Ending-floor cohorts. Floor 56 endings supply templates for the final fight.
FLOOR_BANDS = (
    (1, 5),
    (6, 10),
    (11, 15),
    (16, 17),
    (18, 22),
    (23, 27),
    (28, 32),
    (33, 34),
    (35, 39),
    (40, 44),
    (45, 49),
    (50, 53),
    (54, 56),
)
STARTERS = ("Burning Blood", "Black Blood")


def band_for(floor: int) -> str:
    if isinstance(floor, bool) or not isinstance(floor, int):
        raise TypeError("Floor must be an integer")
    for lo, hi in FLOOR_BANDS:
        if lo <= floor <= hi:
            return f"{lo}-{hi}"
    raise ValueError("Only standard A0 floors 1–56 are supported")


class _Weighted:
    """One validated table. Population order is the dict's insertion order.

    ``random.choices`` consumes one ``random()`` draw per selection. Caching the
    cumulative weights keeps that draw and the selected key identical while
    skipping the per-draw scan of the catalog.
    """

    __slots__ = ("population", "cum_weights")

    def __init__(self, counts: dict) -> None:
        if not counts or any(not math.isfinite(n) or n <= 0 for n in counts.values()):
            raise ValueError("Expected a nonempty positive finite frequency table")
        self.population = tuple(counts)
        self.cum_weights = tuple(accumulate(counts.values()))

    def choose(self, rng: random.Random):
        return rng.choices(self.population, cum_weights=self.cum_weights, k=1)[0]


def choose(rng: random.Random, counts: dict):
    """Draw one key. Equivalent to ``rng.choices(list(counts), weights=...)``."""
    return _Weighted(counts).choose(rng)


def _choose_without_replacement(rng: random.Random, counts: dict, count: int) -> list:
    """Match repeated ``choose`` plus deletion, including dict insertion order."""
    if count > len(counts):
        raise ValueError("Relic count exceeds available distinct identities")
    if count == 0:
        return []
    # Validate once. Every remaining subset of a valid table stays valid.
    population = list(_Weighted(counts).population)
    weights = [counts[key] for key in population]
    chosen = []
    for _ in range(count):
        pick = rng.choices(population, weights=weights, k=1)[0]
        chosen.append(pick)
        index = population.index(pick)
        del population[index]
        del weights[index]
    return chosen


@dataclass(frozen=True)
class SampledCard:
    content_key: str
    upgrades: int


@dataclass(frozen=True)
class LoadoutSpec:
    floor: int
    deck: tuple[SampledCard, ...]
    relics: tuple[str, ...]
    potions: tuple[str | None, ...]
    max_hp: int
    hp: int
    source_band: str
    identity_namespace: str = "sts_sim_public_base_keys"


class LoadoutSampler:
    def __init__(self, distributions: dict) -> None:
        if distributions.get("schema") != 1 or distributions.get("ascension") != 0:
            raise ValueError("Expected version-1 A0 distributions")
        if distributions.get("identity_namespace") != "sts_sim_public_base_keys":
            raise ValueError("Expected distributions mapped to the simulator public catalog")
        if distributions.get("split_role") != "fit":
            raise ValueError("Only fit-split distributions may drive training sampling")
        self.distributions = distributions
        # Lazily filled from the unmodified distribution dicts. Keys are band
        # names; values are static tables only. Shrinking relic sets are not cached.
        self._tables: dict[tuple[str, str], _Weighted] = {}

    @classmethod
    def load(cls, path: Path) -> "LoadoutSampler":
        return cls(json.loads(path.read_text()))

    def _table(self, band: str, name: str, counts: dict) -> _Weighted:
        key = (band, name)
        table = self._tables.get(key)
        if table is None:
            table = _Weighted(counts)
            self._tables[key] = table
        return table

    def sample(self, rng: random.Random, floor: int, *, minimum_hp_fraction: float = 0.1) -> LoadoutSpec:
        """Sample components independently; occupancy and HP fractions are explicit priors."""
        if not math.isfinite(minimum_hp_fraction) or not 0 < minimum_hp_fraction <= 1:
            raise ValueError("minimum_hp_fraction must be finite in (0, 1]")
        band = band_for(floor)
        data = self.distributions["bands"][band]
        if not data["runs"]:
            raise ValueError(f"No training data for ending-floor band {band}; no silent extrapolation")
        size = int(self._table(band, "deck_sizes", data["deck_sizes"]).choose(rng))
        card_weights = self._tables.get((band, "card_weights"))
        if card_weights is None:
            card_weights = self._table(
                band,
                "card_weights",
                {key: sum(upgrades.values()) for key, upgrades in data["cards"].items()},
            )
        deck = []
        for _ in range(size):
            key = card_weights.choose(rng)
            upgrades = self._table(band, f"card:{key}", data["cards"][key]).choose(rng)
            deck.append(SampledCard(key, int(upgrades)))
        starter = self._table(band, "starters", data["starters"]).choose(rng)
        relics = [] if starter == "none" else [starter]
        count = int(self._table(band, "other_relic_counts", data["other_relic_counts"]).choose(rng))
        relics.extend(_choose_without_replacement(rng, data["other_relics"], count))
        # A0 has 3 slots; Potion Belt adds 2 (sts_core relic::POTION_BELT_EXTRA_SLOTS).
        capacity = 3 + 2 * ("Potion Belt" in relics)
        # Logs omit discards and complete inventories. This is NOT a fitted occupancy distribution.
        occupied = rng.randrange(capacity + 1)
        potions: list[str | None] = [None] * capacity
        if occupied and not data["potions_obtained"]:
            raise ValueError(f"No observed potion identities for band {band}")
        for slot in rng.sample(range(capacity), occupied):
            potions[slot] = self._table(band, "potions_obtained", data["potions_obtained"]).choose(rng)
        # Sozu blocks future acquisition, not possession of previously acquired potions.
        maximum = int(self._table(band, "max_hp", data["max_hp"]).choose(rng))
        hp = rng.randint(max(1, math.ceil(maximum * minimum_hp_fraction)), maximum)
        return LoadoutSpec(floor, tuple(deck), tuple(relics), tuple(potions), maximum, hp, band)
