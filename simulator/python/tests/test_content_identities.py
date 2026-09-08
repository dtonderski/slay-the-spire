from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path
from typing import get_type_hints

from sts_sim import (
    CardKey,
    CounterKey,
    EventKey,
    MonsterKey,
    PotionKey,
    PowerKey,
    RelicKey,
    ShopOffer,
    ShopScreen,
)
from sts_sim.observations import (
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
    Card,
    Counter,
    EventScreen,
    Monster,
    PotionSlot,
    Power,
    Relic,
    RewardScreen,
    decode_observation,
)

PYTHON_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = PYTHON_ROOT.parent.parent
GENERATOR = PYTHON_ROOT / "tools" / "generate_content_ids.py"


def _card() -> dict[str, object]:
    return {
        "content_key": "Strike_R",
        "cost": 1,
        "cost_is_modified": False,
        "cost_resets_next_turn": False,
        "upgrade_level": 0,
        "bottled": False,
        "temporary": False,
        "dynamic": {},
    }


def _context() -> dict[str, object]:
    return {
        "ascension": 0,
        "act": 1,
        "floor": 1,
        "gold": 99,
        "player_hp": 80,
        "player_max_hp": 80,
        "deck": (_card(),),
        "relics": ({"slot": 0, "content_key": "Burning Blood", "state": ()},),
        "potion_slots": (
            {"slot": 0, "content_key": None},
            {"slot": 1, "content_key": "fire"},
        ),
    }


class ContentIdentityRuntimeTest(unittest.TestCase):
    def test_accepted_enum_values_and_empty_potion(self) -> None:
        observation = decode_observation(
            {
                "schema_version": FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
                "phase": "reward",
                "kind": "reward",
                "context": _context(),
                "screen": {
                    "cards": ({"slot": 0, "card": _card()},),
                    "queued_card_rewards": (),
                    "gold_offer": 10,
                    "stolen_gold_offer": 0,
                    "potion_offer": "fire",
                    "potion_offers": ("block",),
                    "relic_offer": "Anchor",
                    "boss_relic_choices": ("Astrolabe",),
                    "card_reward_flow": "none",
                },
            }
        )
        self.assertIs(observation.context.deck[0].content_key, CardKey.STRIKE_R)
        self.assertIs(observation.context.relics[0].content_key, RelicKey.BURNING_BLOOD)
        self.assertIsNone(observation.context.potion_slots[0].content_key)
        self.assertIs(observation.context.potion_slots[1].content_key, PotionKey.FIRE)
        if observation.kind == "reward":
            self.assertIs(observation.screen.potion_offer, PotionKey.FIRE)
            self.assertIs(observation.screen.potion_offers[0], PotionKey.BLOCK)
            self.assertIs(observation.screen.relic_offer, RelicKey.ANCHOR)
            self.assertIs(observation.screen.boss_relic_choices[0], RelicKey.ASTROLABE)

    def test_rejects_unknown_and_arbitrary_strings(self) -> None:
        base = {
            "schema_version": FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
            "phase": "event",
            "kind": "event",
            "context": _context(),
            "screen": {
                "event": "Neow",
                "choices": ({"slot": 0, "label": "Gain a blessing"},),
                "match_and_keep": None,
            },
        }
        unknown_event = dict(base)
        unknown_event["screen"] = {
            "event": "not-an-event",
            "choices": ({"slot": 0, "label": "Gain a blessing"},),
            "match_and_keep": None,
        }
        with self.assertRaisesRegex(ValueError, "unknown EventKey"):
            decode_observation(unknown_event)

        unknown_potion = dict(base)
        unknown_potion["kind"] = "reward"
        unknown_potion["phase"] = "reward"
        unknown_potion["screen"] = {
            "cards": (),
            "queued_card_rewards": (),
            "gold_offer": 0,
            "stolen_gold_offer": 0,
            "potion_offer": "Fire Potion",
            "potion_offers": (),
            "relic_offer": None,
            "boss_relic_choices": (),
            "card_reward_flow": "none",
        }
        with self.assertRaisesRegex(ValueError, "unknown PotionKey"):
            decode_observation(unknown_potion)

        empty_potion_string = dict(unknown_potion)
        empty_potion_string["screen"] = dict(unknown_potion["screen"])  # type: ignore[arg-type]
        screen = empty_potion_string["screen"]
        assert isinstance(screen, dict)
        screen["potion_offer"] = ""
        with self.assertRaisesRegex(ValueError, "unknown PotionKey"):
            decode_observation(empty_potion_string)

    def test_nested_annotations_use_generated_enums(self) -> None:
        self.assertIs(get_type_hints(Card)["content_key"], CardKey)
        self.assertIs(get_type_hints(Relic)["content_key"], RelicKey)
        self.assertEqual(get_type_hints(Relic)["state"], tuple[Counter, ...])
        self.assertEqual(get_type_hints(PotionSlot)["content_key"], PotionKey | None)
        self.assertIs(get_type_hints(Monster)["content_key"], MonsterKey)
        self.assertIs(get_type_hints(Power)["key"], PowerKey)
        self.assertIs(get_type_hints(EventScreen)["event"], EventKey)
        reward_hints = get_type_hints(RewardScreen)
        self.assertEqual(reward_hints["potion_offer"], PotionKey | None)
        self.assertEqual(reward_hints["relic_offer"], RelicKey | None)
        shop_hints = get_type_hints(ShopScreen)
        self.assertEqual(shop_hints["cards"], tuple[ShopOffer[CardKey], ...])
        self.assertEqual(shop_hints["relics"], tuple[ShopOffer[RelicKey], ...])
        self.assertEqual(shop_hints["potions"], tuple[ShopOffer[PotionKey], ...])

    def test_public_exports_agree_with_nested_types(self) -> None:
        import sts_sim

        for name in (
            "CardKey",
            "RelicKey",
            "PotionKey",
            "MonsterKey",
            "PowerKey",
            "EventKey",
            "CounterKey",
        ):
            self.assertIn(name, sts_sim.__all__)
            self.assertIs(getattr(sts_sim, name), getattr(sts_sim.observations, name))


class ContentIdentityCatalogTest(unittest.TestCase):
    def test_generated_enums_match_authoritative_catalog(self) -> None:
        exported = subprocess.run(
            ["cargo", "run", "-q", "-p", "sts_env", "--bin", "export_fair_catalog"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(exported.returncode, 0, exported.stderr)
        catalog = json.loads(exported.stdout)
        mapping = {
            "relics": RelicKey,
            "potions": PotionKey,
            "cards": CardKey,
            "monsters": MonsterKey,
            "events": EventKey,
            "powers": PowerKey,
            "counters": CounterKey,
        }
        for key, enum_cls in mapping.items():
            self.assertEqual(list(enum_cls), catalog[key], key)
            self.assertTrue(all(isinstance(member, str) for member in enum_cls))
            self.assertNotEqual(list(enum_cls), list(range(len(enum_cls))))

    def test_checked_in_generated_file_is_fresh(self) -> None:
        result = subprocess.run(
            [sys.executable, str(GENERATOR), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
