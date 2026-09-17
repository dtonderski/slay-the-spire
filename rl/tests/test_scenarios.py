import random
import re
import unittest
from collections import Counter
from pathlib import Path

from scenarios import (
    BOSSES,
    COMBAT_FLOORS,
    ELITES,
    STRONG,
    WEAK,
    ScenarioConfig,
    sample_encounter,
    sample_scenario,
)


class ScenarioTests(unittest.TestCase):
    def test_tables_match_simulator(self) -> None:
        source = (
            Path(__file__).resolve().parents[2] / "simulator/crates/sts_core/src/content/encounters.rs"
        ).read_text()
        for index, act in enumerate(("EXORDIUM", "CITY", "BEYOND")):
            for kind, pools in (("WEAK", WEAK), ("STRONG", STRONG), ("ELITE", ELITES)):
                match = re.search(rf"pub const {act}_{kind}_ENCOUNTERS:.*?=\s*(.*?);", source, re.DOTALL)
                self.assertIsNotNone(match)
                assert match is not None
                entries = re.findall(r'\("([^"]+)",\s*([\d.]+)\)', match[1])
                self.assertEqual(tuple((name, float(weight)) for name, weight in entries), pools[index])

    def test_floor_boundaries_and_pools(self) -> None:
        rng = random.Random(0)
        config = ScenarioConfig(elite_probability=0)
        for index, offset in enumerate((0, 17, 34)):
            for local in range(1, 17):
                floor = offset + local
                if local in (9, 15):
                    with self.assertRaises(ValueError):
                        sample_encounter(rng, floor, config)
                    continue
                spec = sample_encounter(rng, floor, config)
                self.assertEqual(spec.act, index + 1)
                self.assertEqual(spec.ascension, 0)
                if local == 16:
                    self.assertEqual(spec.kind, "boss")
                    self.assertIn(spec.encounter, dict(BOSSES[index]))
                else:
                    expected = WEAK[index] if local <= config.weak_floor_windows[index] else STRONG[index]
                    self.assertEqual(spec.kind, "normal")
                    self.assertIn(spec.encounter, dict(expected))
        for floor in (0, 17, 34, 51, 52, 53, 56):
            with self.assertRaises(ValueError):
                sample_encounter(rng, floor)

    def test_act_four_is_fixed_and_noncombat_floors_excluded(self) -> None:
        excluded = {9, 15, 17, 26, 32, 34, 43, 49, 51, 52, 53}
        self.assertEqual(set(COMBAT_FLOORS), set(range(1, 56)) - excluded)
        for probability in (0, 1):
            config = ScenarioConfig(elite_probability=probability)
            elite = sample_encounter(random.Random(0), 54, config)
            boss = sample_encounter(random.Random(0), 55, config)
            self.assertEqual((elite.act, elite.kind, elite.encounter), (4, "elite", "Shield and Spear"))
            self.assertEqual((boss.act, boss.kind, boss.encounter), (4, "boss", "Corrupt Heart"))

    def test_elites_start_at_local_floor_six(self) -> None:
        config = ScenarioConfig(elite_probability=1)
        for offset in (0, 17, 34):
            for local in (1, 5, 6, 14, 16):
                expected = "normal" if local < 6 else "boss" if local == 16 else "elite"
                self.assertEqual(sample_encounter(random.Random(0), offset + local, config).kind, expected)

    def test_reproducible_uniform_floors_without_global_rng(self) -> None:
        global_state = random.getstate()
        a, b = random.Random(17), random.Random(17)
        samples = [sample_scenario(a) for _ in range(44000)]
        self.assertEqual(samples[:100], [sample_scenario(b) for _ in range(100)])
        self.assertEqual(random.getstate(), global_state)
        counts = Counter(spec.floor for spec in samples)
        self.assertEqual(set(counts), set(COMBAT_FLOORS))
        self.assertTrue(all(850 < n < 1150 for n in counts.values()))

    def test_weights_and_configuration(self) -> None:
        rng = random.Random(1)
        config = ScenarioConfig(elite_probability=0)
        counts = Counter(sample_encounter(rng, 24, config).encounter for _ in range(20000))
        self.assertGreater(counts["Snake Plant"], 2.5 * counts["Chosen and Byrds"])
        config = ScenarioConfig(min_floor=50, max_floor=50)
        self.assertEqual(sample_scenario(rng, config).floor, 50)
        invalid_configs: tuple[dict, ...] = (
            {"min_floor": 0},
            {"max_floor": 56},
            {"min_floor": 9, "max_floor": 9},
            {"elite_probability": float("nan")},
            {"elite_probability": -1},
            {"weak_floor_windows": (0, 2, 2)},
        )
        for kwargs in invalid_configs:
            with self.assertRaises(ValueError):
                ScenarioConfig(**kwargs)


if __name__ == "__main__":
    unittest.main()
