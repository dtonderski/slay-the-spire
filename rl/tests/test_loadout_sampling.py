import json
import random
import unittest
from typing import cast

from loadout_sampling import LoadoutSampler, band_for
from tools.fit_loadout_distributions import (
    accumulate,
    empty_band,
    parse_card,
    split_for,
    validated_event,
)


def event():
    return {
        "is_daily": False,
        "is_trial": False,
        "is_endless": False,
        "is_beta": False,
        "chose_seed": False,
        "is_prod": True,
        "victory": False,
        "master_deck": ["Strike_R", "Bash+1", "Searing Blow+3"],
        "relics": ["Burning Blood", "Potion Belt", "Sozu"],
        "max_hp_per_floor": [80, 85],
        "potions_obtained": [{"floor": 1, "key": "Fire Potion"}],
    }


class LoadoutSamplingTests(unittest.TestCase):
    def sampler(self):
        data = empty_band()
        value = event()
        accumulate(data, value, validated_event(value))
        # Round-trip the actual JSON representation (histogram integer keys become strings).
        return LoadoutSampler(
            json.loads(
                json.dumps(
                    {
                        "schema": 1,
                        "ascension": 0,
                        "identity_namespace": "sts_sim_public_base_keys",
                        "split_role": "fit",
                        "bands": {"1-5": data},
                    }
                )
            )
        )

    def test_reproducible_bounds_slots_and_no_global_randomness(self):
        sampler = self.sampler()
        before = random.getstate()
        a, b = random.Random(44), random.Random(44)
        samples = [sampler.sample(a, 3) for _ in range(100)]
        self.assertEqual(samples, [sampler.sample(b, 3) for _ in range(100)])
        self.assertEqual(before, random.getstate())
        for sample in samples:
            self.assertEqual(len(sample.deck), 3)
            self.assertEqual(len(set(sample.relics)), len(sample.relics))
            self.assertEqual(len(sample.potions), 5)
            self.assertTrue(1 <= sample.hp <= sample.max_hp == 85)
            self.assertEqual(sample.identity_namespace, "sts_sim_public_base_keys")
        # Sozu may coexist with potions acquired before it; do not force empty slots.
        self.assertTrue(any(any(p is not None for p in sample.potions) for sample in samples))

    def test_card_parsing_and_fail_closed_configuration(self):
        self.assertEqual(parse_card("Bash+1"), ("Bash", 1))
        self.assertEqual(parse_card("Searing Blow+5"), ("Searing Blow", 5))
        with self.assertRaises(ValueError):
            parse_card("Bash+3")
        for floor in (0, 57, True, 2.5):
            with self.assertRaises((TypeError, ValueError)):
                band_for(cast(int, floor))
        for fraction in (0, 2, float("nan")):
            with self.assertRaises(ValueError):
                self.sampler().sample(random.Random(1), 1, minimum_hp_fraction=fraction)
        with self.assertRaises(ValueError):
            LoadoutSampler(
                {
                    "schema": 1,
                    "ascension": 0,
                    "identity_namespace": "sts_sim_public_base_keys",
                    "split_role": "held_out",
                }
            )

    def test_filters_split_and_aggregation(self):
        value = event()
        self.assertEqual(split_for("test-run"), split_for("test-run"))
        self.assertEqual({split_for(str(i)) for i in range(100)}, {"fit", "held_out"})
        data = empty_band()
        accumulate(data, value, validated_event(value))
        self.assertEqual(data["runs"], 1)
        self.assertEqual(data["wins"], 0)  # losses are retained
        self.assertEqual(data["cards"]["Searing Blow"][3], 1)
        value["is_daily"] = True
        with self.assertRaises(ValueError):
            validated_event(value)
        value = event()
        value["master_deck"].append("ExampleMod:FakeCard")
        with self.assertRaisesRegex(ValueError, "unmapped_card"):
            validated_event(value)
        value = event()
        value["relics"].append("Black Blood")
        with self.assertRaises(ValueError):
            validated_event(value)


if __name__ == "__main__":
    unittest.main()
