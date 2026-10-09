import json
import random
import unittest
from dataclasses import replace
from pathlib import Path

from loadout_sampling import LoadoutSampler, LoadoutSpec, SampledCard
from scenarios import BOSSES, COMBAT_FLOORS, ELITES, STRONG, WEAK, EncounterSpec, sample_encounter
from sts_sim import State
from synthetic_roots import build_root, sample_root


def loadout(floor: int, relics: tuple[str, ...] = ()) -> LoadoutSpec:
    return LoadoutSpec(
        floor,
        (SampledCard("Strike_R", 0),) * 5 + (SampledCard("Defend_R", 0),) * 4 + (SampledCard("Bash", 0),),
        relics,
        (None,) * (5 if "Potion Belt" in relics else 3),
        80,
        40,
        "test",
    )


class SyntheticRootTests(unittest.TestCase):
    def test_all_encounters_enter_and_take_turns(self) -> None:
        specs = []
        for act in range(1, 4):
            for pool, kind, candidates in (
                ("weak", "normal", WEAK),
                ("strong", "normal", STRONG),
                ("elite", "elite", ELITES),
                ("boss", "boss", BOSSES),
            ):
                floor = (act - 1) * 17 + (16 if kind == "boss" else 6)
                specs.extend(EncounterSpec(floor, act, kind, pool, name) for name, _ in candidates[act - 1])
        specs += [sample_encounter(random.Random(0), floor) for floor in (54, 55)]
        for spec in specs:
            with self.subTest(encounter=spec.encounter):
                state = build_root(spec, loadout(spec.floor), 123).state
                for _ in range(3):
                    decision = state.decision()
                    if decision.observation.kind != "combat" or decision.observation.screen.player.hp <= 0:
                        break
                    self.assertTrue(decision.actions)
                    action = next(a for a in decision.actions if a.kind == "end_turn")
                    state.step(action)

    def test_every_floor_and_reconstruction(self) -> None:
        for floor in COMBAT_FLOORS:
            spec = sample_encounter(random.Random(floor), floor)
            root = build_root(spec, loadout(floor), floor)
            replay = State.from_synthetic_spec(root.spec_json)
            self.assertEqual(root.state.decision().observation, replay.decision().observation)
            obs = root.state.observation()
            self.assertEqual((obs.context.floor, obs.context.act), (floor, spec.act))
            self.assertEqual((obs.context.player_hp, obs.context.player_max_hp), (40, 80))

    def test_owned_items_do_not_reapply_pickup_but_apply_start_once(self) -> None:
        spec = sample_encounter(random.Random(0), 1)
        cards = loadout(1, ("Strawberry", "Pandora's Box", "Whetstone", "Vajra", "Anchor", "Coffee Dripper"))
        root = build_root(spec, cards, 123)
        obs = root.state.observation()
        assert obs.kind == "combat"
        self.assertEqual((obs.context.player_hp, obs.context.player_max_hp), (40, 80))
        self.assertEqual([str(c.content_key) for c in obs.context.deck], [c.content_key for c in cards.deck])
        self.assertEqual((obs.screen.player.block, obs.screen.player.max_energy), (10, 4))
        self.assertEqual([(str(p.key), p.amount) for p in obs.screen.player.powers], [("strength", 1)])

    def test_hp_is_installed_before_combat_healing(self) -> None:
        for floor, relic, expected in ((1, "Blood Vial", 42), (16, "Pantograph", 65)):
            root = build_root(sample_encounter(random.Random(0), floor), loadout(floor, (relic,)), 123)
            self.assertEqual(root.state.observation().context.player_hp, expected)
            self.assertEqual(root.loadout.hp, 40)

    def test_bottles_upgrades_potions_and_counters(self) -> None:
        spec = sample_encounter(random.Random(0), 1)
        cards = replace(
            loadout(1, ("Bottled Flame", "Potion Belt", "Sozu", "Incense Burner")),
            deck=(SampledCard("Searing Blow", 3),) + loadout(1).deck,
            potions=(None, "fire", None, "block", None),
        )
        root = build_root(spec, cards, 10)
        obs = root.state.observation()
        assert obs.kind == "combat"
        self.assertTrue(obs.context.deck[0].bottled)
        self.assertEqual(obs.context.deck[0].upgrade_level, 3)
        self.assertTrue(any(c.card.bottled for c in obs.screen.hand))
        self.assertEqual([p.content_key for p in obs.context.potion_slots], list(cards.potions))
        raw = json.loads(root.spec_json)
        raw["counters"] = {"incense_burner": 5}
        charged = State.from_synthetic_spec(json.dumps(raw)).observation()
        assert charged.kind == "combat"
        self.assertIn(("intangible", 1), [(str(p.key), p.amount) for p in charged.screen.player.powers])
        raw["counters"] = {"incense_burner": 6}
        with self.assertRaises(ValueError):
            State.from_synthetic_spec(json.dumps(raw))

    def test_invalid_inputs_fail_closed(self) -> None:
        spec = sample_encounter(random.Random(0), 1)
        for cards in (
            loadout(1, ("Bottled Tornado",)),
            loadout(1, ("Prismatic Shard",)),
            loadout(1, ("Burning Blood", "Black Blood")),
            replace(loadout(1), hp=81),
            replace(loadout(1), deck=(SampledCard("Searing Blow", 256),)),
        ):
            with self.assertRaises(ValueError):
                build_root(spec, cards, 123)
        for invalid in (
            replace(spec, floor=9),
            replace(spec, act=2),
            replace(spec, ascension=20),
            replace(spec, encounter="Lagavulin"),
        ):
            with self.assertRaises(ValueError):
                build_root(invalid, loadout(invalid.floor), 123)

    def test_strict_spec_and_cloned_transition_determinism(self) -> None:
        root = build_root(sample_encounter(random.Random(0), 1), loadout(1), 123)
        copy = root.state.clone()
        root.state.step(root.state.decision().actions[0])
        copy.step(copy.decision().actions[0])
        self.assertEqual(root.state.observation(), copy.observation())
        raw = json.loads(root.spec_json)
        raw["draw_order"] = []
        with self.assertRaises(ValueError):
            State.from_synthetic_spec(json.dumps(raw))
        del raw["draw_order"]
        raw["deck"][0]["key"] = "Strike_R+"
        with self.assertRaises(ValueError):
            State.from_synthetic_spec(json.dumps(raw))

    def test_empirical_sampler_smoke_when_available(self) -> None:
        path = Path(__file__).resolve().parents[2] / "data/slaythedata/loadout-a0-v3/fit.json"
        if not path.exists():
            self.skipTest("local fitted distribution unavailable")
        sampler = LoadoutSampler.load(path)
        for floor in COMBAT_FLOORS:
            a = sample_root(random.Random(floor), sampler, floor)
            b = sample_root(random.Random(floor), sampler, floor)
            self.assertEqual(a.spec_json, b.spec_json)
            self.assertEqual(a.state.observation(), b.state.observation())
            self.assertTrue(a.state.decision().actions)


if __name__ == "__main__":
    unittest.main()
