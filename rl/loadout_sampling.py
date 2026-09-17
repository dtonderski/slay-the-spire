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


def choose(rng: random.Random, counts: dict):
    if not counts or any(not math.isfinite(n) or n <= 0 for n in counts.values()):
        raise ValueError("Expected a nonempty positive finite frequency table")
    return rng.choices(list(counts), weights=list(counts.values()), k=1)[0]


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

    @classmethod
    def load(cls, path: Path) -> "LoadoutSampler":
        return cls(json.loads(path.read_text()))

    def sample(self, rng: random.Random, floor: int, *, minimum_hp_fraction: float = 0.1) -> LoadoutSpec:
        """Sample components independently; occupancy and HP fractions are explicit priors."""
        if not math.isfinite(minimum_hp_fraction) or not 0 < minimum_hp_fraction <= 1:
            raise ValueError("minimum_hp_fraction must be finite in (0, 1]")
        band = band_for(floor)
        data = self.distributions["bands"][band]
        if not data["runs"]:
            raise ValueError(f"No training data for ending-floor band {band}; no silent extrapolation")
        size = int(choose(rng, data["deck_sizes"]))
        card_weights = {key: sum(upgrades.values()) for key, upgrades in data["cards"].items()}
        deck = []
        for _ in range(size):
            key = choose(rng, card_weights)
            deck.append(SampledCard(key, int(choose(rng, data["cards"][key]))))
        starter = choose(rng, data["starters"])
        relics = [] if starter == "none" else [starter]
        count = int(choose(rng, data["other_relic_counts"]))
        candidates = dict(data["other_relics"])
        if count > len(candidates):
            raise ValueError("Relic count exceeds available distinct identities")
        for _ in range(count):
            relic = choose(rng, candidates)
            relics.append(relic)
            del candidates[relic]
        # A0 has 3 slots; Potion Belt adds 2 (sts_core relic::POTION_BELT_EXTRA_SLOTS).
        capacity = 3 + 2 * ("Potion Belt" in relics)
        # Logs omit discards and complete inventories. This is NOT a fitted occupancy distribution.
        occupied = rng.randrange(capacity + 1)
        potions: list[str | None] = [None] * capacity
        if occupied and not data["potions_obtained"]:
            raise ValueError(f"No observed potion identities for band {band}")
        for slot in rng.sample(range(capacity), occupied):
            potions[slot] = choose(rng, data["potions_obtained"])
        # Sozu blocks future acquisition, not possession of previously acquired potions.
        maximum = int(choose(rng, data["max_hp"]))
        hp = rng.randint(max(1, math.ceil(maximum * minimum_hp_fraction)), maximum)
        return LoadoutSpec(floor, tuple(deck), tuple(relics), tuple(potions), maximum, hp, band)
