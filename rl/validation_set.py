"""Immutable synthetic validation inputs, initialized before combat-start effects."""

import argparse
import hashlib
import json
import math
import random
from dataclasses import asdict, replace
from pathlib import Path

import sts_sim._native as native
from loadout_sampling import LoadoutSampler
from scenarios import COMBAT_FLOORS, ScenarioConfig
from sts_sim import State
from synthetic_roots import build_root, sample_root
from train_roots import Root


def native_sha256() -> str:
    return hashlib.sha256(Path(native.__file__).read_bytes()).hexdigest()


def observation_sha256(state: State) -> str:
    data = json.dumps(asdict(state.observation()), sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(data.encode()).hexdigest()


def build_validation(
    sampler: LoadoutSampler,
    seed: int,
    main_count: int = 1024,
    stress_per_stratum: int = 32,
    repeats: int = 3,
    max_decisions: int = 512,
) -> dict:
    """Freeze draws, never filter by policy performance or combat outcomes."""
    if min(main_count, stress_per_stratum, repeats, max_decisions) < 1:
        raise ValueError("Dataset sizes and evaluation repeats must be positive")
    rng = random.Random(seed)
    requests = [("main", None, ScenarioConfig()) for _ in range(main_count)]
    for act in range(1, 5):
        for kind in ("normal", "elite", "boss") if act < 4 else ("elite", "boss"):
            if act == 4:
                floors = [54 if kind == "elite" else 55]
            elif kind == "boss":
                floors = [17 * (act - 1) + 16]
            else:
                floors = [
                    floor
                    for floor in COMBAT_FLOORS
                    if (floor - 1) // 17 + 1 == act
                    and (floor - 1) % 17 + 1 != 16
                    and (kind != "elite" or (floor - 1) % 17 + 1 >= 6)
                ]
            config = ScenarioConfig(elite_probability=1.0 if kind == "elite" else 0.0)
            requests.extend(("stress", rng.choice(floors), config) for _ in range(stress_per_stratum))
    cases: list[dict] = []
    for index, (label, floor, config) in enumerate(requests):
        sampled = sample_root(rng, sampler, floor, config=config)
        spec = json.loads(sampled.spec_json)
        if label == "stress":
            low, high = math.ceil(0.1 * sampled.loadout.max_hp), math.floor(0.3 * sampled.loadout.max_hp)
            if low > high:
                raise ValueError("Max HP cannot support the requested positive stress HP interval")
            stressed = replace(sampled.loadout, hp=rng.randint(low, high))
            # A NEW initial specification; never patch an initialized combat's HP.
            root = build_root(sampled.encounter, stressed, spec["seed"])
            spec = json.loads(root.spec_json)
        else:
            root = sampled
        spec["counters"] = {
            "incense_burner": 0,
            "pen_nib": 0,
            "ink_bottle": 0,
            "happy_flower": 0,
            "sundial": 0,
            "nunchaku": 0,
            "lizard_tail_used": False,
        }
        for card in spec["deck"]:
            card["ritual_dagger_damage_bonus"] = 0
        cases.append(
            {
                "id": f"{label}-{index:06d}",
                "label": label,
                "act": sampled.encounter.act,
                "kind": sampled.encounter.kind,
                "spec": spec,
                "initial_observation_sha256": observation_sha256(root.state),
                "source_band": sampled.loadout.source_band,
                "rejected_loadouts": list(sampled.rejected_loadouts),
            }
        )
    if len({case["spec"]["seed"] for case in cases}) != len(cases):
        raise ValueError("Duplicate combat seeds; choose a different dataset generation seed")
    return {
        "schema": 1,
        "protocol": "synthetic_pre_entry_hp_A0",
        "generation_seed": seed,
        "native_sha256": native_sha256(),
        "evaluation": {"seed_base": 90000, "repeats": repeats, "max_decisions": max_decisions},
        "provenance": {
            "loadouts": "independent draws from the historical fit; not rollout-derived validation",
            "main_hp": "empirical max HP; uniform integer ceil(0.1*max_hp)..max_hp",
            "stress_hp": "empirical max HP; uniform integer ceil(0.1*max_hp)..floor(0.3*max_hp)",
            "stress_coverage": "equal counts per act/encounter-kind stratum (11 strata)",
            "counters": "constructor defaults; zero combat counters, unused Lizard Tail, expired Neow's Lament",
        },
        "cases": cases,
    }


def load_validation(path: Path) -> tuple[dict, dict[str, list[Root]]]:
    """Load fixed inputs, checking version and public projections without repairing state."""
    document = json.loads(path.read_text())
    if document.get("schema") != 1 or document.get("protocol") != "synthetic_pre_entry_hp_A0":
        raise ValueError("Expected a frozen pre-entry-HP validation dataset, not legacy rollout roots")
    if document["native_sha256"] != native_sha256():
        raise ValueError("Validation simulator version changed; explicitly create a newly versioned dataset")
    evaluation = document["evaluation"]
    if evaluation["seed_base"] != 90000 or type(evaluation["repeats"]) is not int or evaluation["repeats"] < 1:
        raise ValueError("Invalid fixed evaluation protocol")
    if type(evaluation["max_decisions"]) is not int or evaluation["max_decisions"] < 1:
        raise ValueError("Invalid fixed validation decision limit")
    groups: dict[str, list[Root]] = {"main": [], "stress": []}
    identities, seeds = set(), set()
    for case in document["cases"]:
        label, spec = case["label"], case["spec"]
        if label not in groups or case["id"] in identities or spec["seed"] in seeds:
            raise ValueError("Invalid label, duplicate case ID, or duplicate combat seed")
        identities.add(case["id"])
        seeds.add(spec["seed"])
        low = math.ceil(0.1 * spec["max_hp"])
        high = spec["max_hp"] if label == "main" else math.floor(0.3 * spec["max_hp"])
        if not low <= spec["hp"] <= high:
            raise ValueError("Starting HP does not match the labeled validation prior")
        state = State.from_synthetic_spec(json.dumps(spec))
        obs = state.observation()
        if observation_sha256(state) != case["initial_observation_sha256"]:
            raise ValueError(f"Validation initial observation mismatch: {case['id']}")
        if obs.context.act != case["act"] or spec["kind"] != case["kind"]:
            raise ValueError("Validation stratum metadata mismatch")
        groups[label].append(Root(state, str(spec["seed"]), spec["floor"], obs.context.player_hp, case["act"]))
    if not all(groups.values()):
        raise ValueError("Both main and stress validation sets must be nonempty")
    return document, groups


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--distributions", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=20260918)
    parser.add_argument("--main-count", type=int, default=1024)
    parser.add_argument("--stress-per-stratum", type=int, default=32)
    parser.add_argument("--eval-repeats", type=int, default=3)
    parser.add_argument("--eval-max-decisions", type=int, default=512)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("Output already exists; validation datasets are never overwritten")
    document = build_validation(
        LoadoutSampler.load(args.distributions),
        args.seed,
        args.main_count,
        args.stress_per_stratum,
        args.eval_repeats,
        args.eval_max_decisions,
    )
    document["provenance"]["distributions_sha256"] = hashlib.sha256(args.distributions.read_bytes()).hexdigest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as handle:
        json.dump(document, handle, indent=2)
        handle.write("\n")
    _, groups = load_validation(args.output)
    print(f"Saved {args.output}: " + ", ".join(f"{label}={len(roots)}" for label, roots in groups.items()))


if __name__ == "__main__":
    main()
