"""On-demand synthetic A0 roots; never replay or observation-based state repair."""

import json
import random
from dataclasses import dataclass

from loadout_sampling import LoadoutSampler, LoadoutSpec
from scenarios import DEFAULT_CONFIG, EncounterSpec, ScenarioConfig, sample_encounter, sample_scenario
from sts_sim import State


@dataclass(frozen=True)
class SyntheticRoot:
    state: State
    encounter: EncounterSpec
    loadout: LoadoutSpec
    spec_json: str
    rejected_loadouts: tuple[str, ...]


def build_root(encounter: EncounterSpec, loadout: LoadoutSpec, seed: int, *, gold: int = 99) -> SyntheticRoot:
    """Install owned items directly, then run ordinary combat initialization once."""
    if encounter.ascension != 0 or encounter.floor != loadout.floor:
        raise ValueError("Expected matching A0 encounter and loadout floors")
    expected_act = 4 if encounter.floor >= 54 else (encounter.floor - 1) // 17 + 1
    if encounter.act != expected_act or loadout.identity_namespace != "sts_sim_public_base_keys":
        raise ValueError("Invalid act or identity namespace")
    spec = {
        "seed": seed,
        "floor": encounter.floor,
        "kind": encounter.kind,
        "encounter": encounter.encounter,
        "deck": [{"key": card.content_key, "upgrades": card.upgrades} for card in loadout.deck],
        "relics": loadout.relics,
        "potions": loadout.potions,
        "hp": loadout.hp,
        "max_hp": loadout.max_hp,
        "gold": gold,
        "counters": {},
    }
    spec_json = json.dumps(spec, sort_keys=True, separators=(",", ":"))
    return SyntheticRoot(State.from_synthetic_spec(spec_json), encounter, loadout, spec_json, ())


def sample_root(
    rng: random.Random,
    sampler: LoadoutSampler,
    floor: int | None = None,
    *,
    config: ScenarioConfig = DEFAULT_CONFIG,
    minimum_hp_fraction: float = 0.1,
    max_attempts: int = 64,
) -> SyntheticRoot:
    """Sample a playable root, recording only explicitly incompatible loadout rejections."""
    if max_attempts < 1:
        raise ValueError("max_attempts must be positive")
    encounter = sample_scenario(rng, config) if floor is None else sample_encounter(rng, floor, config)
    seed = rng.getrandbits(64)
    rejected = []
    for _ in range(max_attempts):
        loadout = sampler.sample(rng, encounter.floor, minimum_hp_fraction=minimum_hp_fraction)
        try:
            root = build_root(encounter, loadout, seed)
        except ValueError as error:
            if not str(error).startswith("incompatible loadout:"):
                raise
            rejected.append(str(error))
            continue
        return SyntheticRoot(root.state, encounter, loadout, root.spec_json, tuple(rejected))
    raise ValueError(f"No compatible loadout after {max_attempts} attempts: {rejected}")
