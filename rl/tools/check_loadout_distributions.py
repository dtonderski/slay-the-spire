"""Compare fitted/held-out marginals and smoke-test generated specifications."""

import argparse
import json
import random
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from loadout_sampling import LoadoutSampler


def mean(counts: dict) -> float:
    return sum(float(key) * count for key, count in counts.items()) / sum(counts.values())


def total_variation(left: dict, right: dict) -> float:
    nl, nr = sum(left.values()), sum(right.values())
    return 0.5 * sum(abs(left.get(k, 0) / nl - right.get(k, 0) / nr) for k in left.keys() | right.keys())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--samples-per-band", type=int, default=1000)
    args = parser.parse_args()
    if args.samples_per_band < 1:
        parser.error("samples-per-band must be positive")
    fit = json.loads((args.directory / "fit.json").read_text())
    held = json.loads((args.directory / "held_out.json").read_text())
    sampler = LoadoutSampler(fit)
    rng = random.Random(12345)
    report = {"samples_per_band": args.samples_per_band, "seed": 12345, "bands": {}}
    for band, data in fit["bands"].items():
        reference = held["bands"][band]
        sizes, maxima, occupied, cards = Counter(), Counter(), Counter(), Counter()
        relic_counts = Counter()
        for _ in range(args.samples_per_band):
            spec = sampler.sample(rng, int(band.split("-")[0]))
            assert 1 <= spec.hp <= spec.max_hp
            assert len(spec.relics) == len(set(spec.relics))
            assert not {"Burning Blood", "Black Blood"}.issubset(spec.relics)
            assert len(spec.potions) == (5 if "Potion Belt" in spec.relics else 3)
            sizes[str(len(spec.deck))] += 1
            maxima[str(spec.max_hp)] += 1
            occupied[str(sum(p is not None for p in spec.potions))] += 1
            cards.update(card.content_key for card in spec.deck)
            relic_counts[str(len(spec.relics))] += 1
        expected_cards = {key: sum(value.values()) for key, value in data["cards"].items()}
        held_cards = {key: sum(value.values()) for key, value in reference["cards"].items()}
        row = {
            "fit_runs": data["runs"],
            "held_out_runs": reference["runs"],
            "fit_win_fraction": data["wins"] / data["runs"],
            "fit_mean_deck_size": mean(data["deck_sizes"]),
            "held_out_mean_deck_size": mean(reference["deck_sizes"]),
            "generated_mean_deck_size": mean(sizes),
            "generated_mean_relic_count": mean(relic_counts),
            "fit_mean_max_hp": mean(data["max_hp"]),
            "generated_mean_max_hp": mean(maxima),
            "fit_vs_heldout_deck_size_tv": total_variation(data["deck_sizes"], reference["deck_sizes"]),
            "fit_vs_heldout_card_frequency_tv": total_variation(expected_cards, held_cards),
            "fit_vs_generated_card_frequency_tv": total_variation(expected_cards, cards),
            "generated_occupied_potion_slots": occupied,
        }
        report["bands"][band] = row
        print(band, json.dumps(row), flush=True)
    (args.directory / "diagnostics.json").write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
