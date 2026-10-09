"""Synthetic Ironclad A0 floor/encounter specifications, not replay or initialized states.

Floors are uniform over combat-capable floors in acts 1–4 on the standard A0
no-shortcut schedule. Fixed treasure/rest/shop and transition floors are excluded.
Actual runs can have shifted floor numbers (e.g. Secret Portal). Event encounters
are not included. Act 4 is fixed elite then boss, after its rest and shop.
Encounter weights mirror sts_core/src/content/encounters.rs; Act 4 setup is in
sts_core/src/run/{event,map}.rs. Act 4 labels here identify its fixed pair/Heart.

Normal runs switch pools after 3 normal fights in act 1, 2 in later acts.
Without path history we instead use those many local floors as a configurable
weak-pool window. Elite probability is a training mixture, not a map frequency.
No encounter-list exclusions or no-repeat history are simulated.
"""

import math
import random
from dataclasses import dataclass
from typing import Literal

Pool = tuple[tuple[str, float], ...]

# Keep these aligned with the simulator; test_scenarios checks the weighted tables.
WEAK: tuple[Pool, ...] = (
    (("Cultist", 2), ("Jaw Worm", 2), ("2 Louse", 2), ("Small Slimes", 2)),
    (("Spheric Guardian", 2), ("Chosen", 2), ("Shell Parasite", 2), ("3 Byrds", 2), ("2 Thieves", 2)),
    (("3 Darklings", 2), ("Orb Walker", 2), ("3 Shapes", 2)),
)
STRONG: tuple[Pool, ...] = (
    (
        ("Blue Slaver", 2),
        ("Gremlin Gang", 1),
        ("Looter", 2),
        ("Large Slime", 2),
        ("Lots of Slimes", 1),
        ("Exordium Thugs", 1.5),
        ("Exordium Wildlife", 1.5),
        ("Red Slaver", 1),
        ("3 Louse", 2),
        ("2 Fungi Beasts", 2),
    ),
    (
        ("Chosen and Byrds", 2),
        ("Sentry and Sphere", 2),
        ("Snake Plant", 6),
        ("Snecko", 4),
        ("Centurion and Healer", 6),
        ("Cultist and Chosen", 3),
        ("3 Cultists", 3),
        ("Shelled Parasite and Fungi", 3),
    ),
    (
        ("Spire Growth", 1),
        ("Transient", 1),
        ("4 Shapes", 1),
        ("Maw", 1),
        ("Sphere and 2 Shapes", 1),
        ("Jaw Worm Horde", 1),
        ("3 Darklings", 1),
        ("Writhing Mass", 1),
    ),
)
ELITES: tuple[Pool, ...] = (
    (("GremlinNob", 1), ("Lagavulin", 1), ("3 Sentries", 1)),
    (("Gremlin Leader", 1), ("Slavers", 1), ("Book of Stabbing", 1)),
    (("Giant Head", 2), ("Nemesis", 2), ("Reptomancer", 2)),
)
BOSSES: tuple[Pool, ...] = (
    (("Hexaghost", 1), ("Slime Boss", 1), ("The Guardian", 1)),
    (("Automaton", 1), ("Collector", 1), ("Champ", 1)),
    (("Awakened One", 1), ("Time Eater", 1), ("Donu and Deca", 1)),
)
COMBAT_FLOORS = tuple(offset + local for offset in (0, 17, 34) for local in range(1, 17) if local not in (9, 15)) + (
    54,
    55,
)


@dataclass(frozen=True)
class ScenarioConfig:
    min_floor: int = 1
    max_floor: int = 55
    elite_probability: float = 0.2
    weak_floor_windows: tuple[int, int, int] = (3, 2, 2)

    def __post_init__(self) -> None:
        if not 1 <= self.min_floor <= self.max_floor <= 55:
            raise ValueError("Expected 1 <= min_floor <= max_floor <= 55")
        if not any(self.min_floor <= floor <= self.max_floor for floor in COMBAT_FLOORS):
            raise ValueError("Range contains no combat-capable floors")
        if not math.isfinite(self.elite_probability) or not 0 <= self.elite_probability <= 1:
            raise ValueError("elite_probability must be finite and between 0 and 1")
        if len(self.weak_floor_windows) != 3 or any(not 1 <= n <= 14 for n in self.weak_floor_windows):
            raise ValueError("Expected three weak-pool floor windows between 1 and 14")


@dataclass(frozen=True)
class EncounterSpec:
    floor: int
    act: int
    kind: Literal["normal", "elite", "boss"]
    pool: Literal["weak", "strong", "elite", "boss"]
    encounter: str
    ascension: int = 0


DEFAULT_CONFIG = ScenarioConfig()


def sample_encounter(rng: random.Random, floor: int, config: ScenarioConfig = DEFAULT_CONFIG) -> EncounterSpec:
    """Choose an encounter for a specified combat-capable floor using caller-owned RNG."""
    if floor not in COMBAT_FLOORS or not config.min_floor <= floor <= config.max_floor:
        raise ValueError(f"Not a configured combat floor: {floor}")
    if floor == 54:
        return EncounterSpec(floor, 4, "elite", "elite", "Shield and Spear")
    if floor == 55:
        return EncounterSpec(floor, 4, "boss", "boss", "Corrupt Heart")
    index, local = divmod(floor - 1, 17)
    local += 1
    kind: Literal["normal", "elite", "boss"]
    pool: Literal["weak", "strong", "elite", "boss"]
    if local == 16:
        kind, pool, candidates = "boss", "boss", BOSSES[index]
    elif local >= 6 and rng.random() < config.elite_probability:
        kind, pool, candidates = "elite", "elite", ELITES[index]
    elif local <= config.weak_floor_windows[index]:
        kind, pool, candidates = "normal", "weak", WEAK[index]
    else:
        kind, pool, candidates = "normal", "strong", STRONG[index]
    names, weights = zip(*candidates)
    return EncounterSpec(floor, index + 1, kind, pool, rng.choices(names, weights=weights, k=1)[0])


def sample_scenario(rng: random.Random, config: ScenarioConfig = DEFAULT_CONFIG) -> EncounterSpec:
    """Uniformly sample an eligible floor, then sample its encounter."""
    floors = [floor for floor in COMBAT_FLOORS if config.min_floor <= floor <= config.max_floor]
    return sample_encounter(rng, rng.choice(floors), config)
