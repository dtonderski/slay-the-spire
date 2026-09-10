from __future__ import annotations

import subprocess
import unittest
from dataclasses import fields, replace
from pathlib import Path
from typing import get_args, get_type_hints

import sts_sim
from sts_sim import (
    OBSERVATION_TYPES,
    Action,
    CardKey,
    CombatObservation,
    CompleteObservation,
    DarkOrb,
    Decision,
    EventObservation,
    GridObservation,
    IdleObservation,
    MapObservation,
    MonsterKey,
    Observation,
    RelicKey,
    RestObservation,
    RestSmith,
    RewardObservation,
    ShopObservation,
    State,
    TreasureObservation,
    VisibleIntent,
)
from sts_sim.observations import (
    FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
    INTENT_TYPES,
    ORB_TYPES,
    REST_OPTION_TYPES,
    ObservationKind,
    RestOption,
    decode_observation,
)

PYTHON_ROOT = Path(__file__).resolve().parents[1]
TYPING_DIR = Path(__file__).resolve().parent / "typing"


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
        "potion_slots": ({"slot": 0, "content_key": None},),
    }


def _observation(kind: str, screen: object, phase: str | None = None) -> dict[str, object]:
    return {
        "schema_version": FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
        "phase": phase if phase is not None else ("idle" if kind in {"map", "idle"} else kind),
        "kind": kind,
        "context": _context(),
        "screen": screen,
    }


def _combat_screen() -> dict[str, object]:
    return {
        "schema_version": 4,
        "phase": "waiting_for_player",
        "player": {
            "hp": 80,
            "max_hp": 80,
            "block": 0,
            "energy": 3,
            "max_energy": 3,
            "powers": ({"key": "strength", "amount": 2},),
        },
        "orb_slots": (
            {"slot": 0, "orb": {"type": "lightning"}},
            {"slot": 1, "orb": {"type": "dark", "evoke": 6}},
            {"slot": 2, "orb": None},
        ),
        "hand": ({"slot": 0, "card": _card()},),
        "draw_pile": {"cards": (_card(),), "known_positions": ()},
        "discard_pile": {"cards": (), "known_positions": ()},
        "exhaust_pile": {"cards": (), "known_positions": ()},
        "monsters": (
            {
                "slot": 0,
                "content_key": "Jaw Worm",
                "slime_size": None,
                "hp": 42,
                "max_hp": 42,
                "block": 0,
                "powers": (),
                "stolen_gold": 0,
                "stasis_card": None,
                "intent": {"visibility": "visible", "category": "attack", "damage": 11, "hits": 1},
                "alive": True,
                "escaped": False,
                "minion": False,
                "targetable": True,
                "in_defensive_mode": False,
            },
        ),
        "selection": {
            "kind": "armaments_upgrade",
            "options": ({"slot": 0, "card": _card()},),
            "selected_slots": (0,),
        },
        "public_counters": ({"key": "cards_played_this_turn", "value": 1},),
    }


class TypedObservationRuntimeTest(unittest.TestCase):
    def test_public_package_does_not_export_record_escape_hatch(self) -> None:
        self.assertFalse(hasattr(sts_sim, "Record"))
        self.assertFalse(hasattr(sts_sim, "CombatRelic"))
        self.assertFalse(hasattr(sts_sim, "CombatContext"))
        self.assertNotIn("Record", sts_sim.__all__)
        self.assertNotIn("CombatRelic", sts_sim.__all__)
        self.assertNotIn("CombatContext", sts_sim.__all__)
        state = State.new("HUMAN1")
        observation = state.observation()
        self.assertIsInstance(observation, OBSERVATION_TYPES)
        self.assertFalse(hasattr(observation, "__getattr__"))
        with self.assertRaises(AttributeError):
            _ = observation.seed  # type: ignore[attr-defined]
        with self.assertRaises(AttributeError):
            _ = observation.context.seed  # type: ignore[attr-defined]

    def test_native_environment_preserves_clone_and_stale_actions(self) -> None:
        state = State.new("1")
        clone = state.clone()
        decision = state.decision()
        self.assertIsInstance(decision, Decision)
        self.assertIsInstance(decision.actions, tuple)
        self.assertIsInstance(decision.actions[0], Action)
        with self.assertRaises(AttributeError):
            decision.actions.append(decision.actions[0])  # type: ignore[attr-defined]
        self.assertEqual(clone.revision, decision.revision)
        result = state.step(decision.actions[0])
        self.assertEqual(result.revision, decision.revision + 1)
        self.assertEqual(clone.revision, decision.revision)
        with self.assertRaisesRegex(ValueError, "stale"):
            state.step(decision.actions[0])

    def test_live_observation_has_nested_types(self) -> None:
        observation = State.new("HUMAN1").observation()
        self.assertIsInstance(observation, Observation)
        self.assertEqual(observation.schema_version, FAIR_RUN_OBSERVATION_SCHEMA_VERSION)
        self.assertGreater(len(observation.context.deck), 0)
        self.assertIs(observation.context.deck[0].content_key, CardKey.STRIKE_R)
        self.assertEqual(observation.context.deck[0].content_key, "Strike_R")
        self.assertIs(observation.context.relics[0].content_key, RelicKey.BURNING_BLOOD)
        self.assertEqual(observation.context.relics[0].state, ())
        self.assertIn(observation.kind, {"event", "map"})
        if observation.kind == "event":
            self.assertTrue(observation.screen.choices)
            self.assertIsInstance(observation.screen.choices[0].label, str)
        elif observation.kind == "map":
            self.assertTrue(observation.screen.nodes)
            self.assertGreaterEqual(observation.screen.current_node, 0)

    def test_live_combat_nested_types(self) -> None:
        state = State.new("HUMAN1")
        observation = state.observation()
        for _ in range(20):
            if observation.kind == "combat":
                break
            actions = state.legal_actions()
            self.assertTrue(actions)
            observation = state.step(actions[0]).observation
        self.assertIsInstance(observation, CombatObservation)
        if observation.kind != "combat":
            self.fail("expected a combat observation")
        self.assertEqual(observation.screen.schema_version, 4)
        self.assertGreaterEqual(observation.screen.player.energy, 0)
        self.assertTrue(observation.screen.hand)
        self.assertTrue(observation.screen.monsters)
        self.assertIsInstance(observation.screen.draw_pile.known_positions, tuple)
        self.assertFalse(hasattr(observation.screen.draw_pile, "count"))
        self.assertFalse(hasattr(observation.screen.draw_pile, "known_order"))
        self.assertFalse(hasattr(observation.screen, "relics"))
        self.assertFalse(hasattr(observation.screen, "context"))
        self.assertFalse(hasattr(observation.screen, "potion_slots"))
        self.assertTrue(observation.context.relics)
        self.assertIsInstance(observation.context.relics[0].state, tuple)
        self.assertTrue(observation.context.potion_slots)
        monster = observation.screen.monsters[0]
        self.assertIsInstance(monster.content_key, MonsterKey)
        if monster.intent.visibility == "visible":
            self.assertIsInstance(monster.intent.category, str)

    def test_decode_covers_every_screen_kind_and_nested_unions(self) -> None:
        combat = decode_observation(_observation("combat", _combat_screen(), phase="combat"))
        self.assertIsInstance(combat, CombatObservation)
        self.assertEqual(combat.screen.player.energy, 3)
        self.assertIsInstance(combat.screen.orb_slots[1].orb, DarkOrb)
        if combat.kind == "combat":
            monster = combat.screen.monsters[0]
            self.assertIs(monster.content_key, MonsterKey.JAW_WORM)
            self.assertIsInstance(monster.intent, VisibleIntent)
            if monster.intent.visibility == "visible":
                self.assertEqual(monster.intent.damage, 11)
            self.assertEqual(combat.screen.hand[0].card.dynamic.rampage_damage_bonus, None)
            self.assertIs(combat.context.relics[0].content_key, RelicKey.BURNING_BLOOD)
            self.assertEqual(combat.context.relics[0].state, ())
            self.assertIsNone(combat.context.potion_slots[0].content_key)
            self.assertFalse(hasattr(combat.screen, "relics"))
            self.assertFalse(hasattr(combat.screen, "context"))
            self.assertFalse(hasattr(combat.screen, "potion_slots"))

        mapping = decode_observation(
            _observation(
                "map",
                {
                    "act": 1,
                    "floor": 1,
                    "current_node": 0,
                    "reachable_nodes": (1,),
                    "nodes": ({"slot": 0, "act": 1, "room_kind": "combat", "children": (1,)},),
                },
            )
        )
        self.assertIsInstance(mapping, MapObservation)

        event = decode_observation(
            _observation(
                "event",
                {
                    "event": "Neow",
                    "choices": ({"slot": 0, "label": "Gain a blessing"},),
                    "match_and_keep": ({"content_key": None, "revealed": False, "matched": False},),
                },
                phase="event",
            )
        )
        self.assertIsInstance(event, EventObservation)
        self.assertIsNotNone(event.screen.match_and_keep)

        reward = decode_observation(
            _observation(
                "reward",
                {
                    "cards": ({"slot": 0, "card": _card()},),
                    "queued_card_rewards": ({"slot": 0, "choice_count": 3},),
                    "gold_offer": 10,
                    "stolen_gold_offer": 0,
                    "potion_offer": None,
                    "potion_offers": (),
                    "relic_offer": None,
                    "boss_relic_choices": (),
                    "card_reward_flow": "active",
                },
                phase="reward",
            )
        )
        self.assertIsInstance(reward, RewardObservation)

        treasure = decode_observation(
            _observation(
                "treasure",
                {"chest_size": "medium", "opened": False},
                phase="treasure",
            )
        )
        self.assertIsInstance(treasure, TreasureObservation)

        rest = decode_observation(
            _observation(
                "rest",
                {
                    "complete": False,
                    "options": (
                        {"kind": "heal"},
                        {"kind": "smith", "card_slot": 2},
                    ),
                },
                phase="rest",
            )
        )
        self.assertIsInstance(rest, RestObservation)
        self.assertIsInstance(rest.screen.options[1], RestSmith)
        if rest.screen.options[1].kind == "smith":
            self.assertEqual(rest.screen.options[1].card_slot, 2)

        shop = decode_observation(
            _observation(
                "shop",
                {
                    "merchant_open": True,
                    "remove_cost": 75,
                    "cards": ({"slot": 0, "content_key": "Bash", "price": 50, "sold": False},),
                    "relics": (),
                    "potions": (),
                },
                phase="shop",
            )
        )
        self.assertIsInstance(shop, ShopObservation)

        grid = decode_observation(
            _observation(
                "grid",
                {
                    "purpose": "rest_smith",
                    "cards": ({"slot": 0, "card": _card()},),
                    "selected": 0,
                    "selected_indices": (0,),
                },
                phase="rest",
            )
        )
        self.assertIsInstance(grid, GridObservation)

        idle = decode_observation(_observation("idle", None, phase="idle"))
        self.assertIsInstance(idle, IdleObservation)
        self.assertIsNone(idle.screen)

        complete = decode_observation(_observation("complete", None, phase="complete"))
        self.assertIsInstance(complete, CompleteObservation)

    def test_decoder_rejects_unknown_and_missing_fields(self) -> None:
        extra = _observation(
            "map",
            {
                "act": 1,
                "floor": 1,
                "current_node": 0,
                "reachable_nodes": (),
                "nodes": (),
                "seed": 123,
            },
        )
        with self.assertRaisesRegex(ValueError, "extra"):
            decode_observation(extra)

        missing = _observation(
            "map",
            {
                "act": 1,
                "floor": 1,
                "current_node": 0,
                "reachable_nodes": (),
            },
        )
        with self.assertRaisesRegex(ValueError, "missing"):
            decode_observation(missing)

        unknown_kind = _observation("map", None)
        unknown_kind["kind"] = "secret"
        with self.assertRaisesRegex(ValueError, "expected one of"):
            decode_observation(unknown_kind)

        combat = _observation("combat", _combat_screen(), phase="combat")
        screen = combat["screen"]
        assert isinstance(screen, dict)
        screen["hidden_draw_order"] = (1, 2, 3)
        with self.assertRaisesRegex(ValueError, "extra"):
            decode_observation(combat)

        unknown_relic = _observation("combat", _combat_screen(), phase="combat")
        unknown_relic_context = unknown_relic["context"]
        assert isinstance(unknown_relic_context, dict)
        unknown_relic_context["relics"] = ({"slot": 0, "content_key": "not-a-relic", "state": ()},)
        with self.assertRaisesRegex(ValueError, "unknown RelicKey"):
            decode_observation(unknown_relic)

        missing_relic_state = _observation("combat", _combat_screen(), phase="combat")
        missing_relic_context = missing_relic_state["context"]
        assert isinstance(missing_relic_context, dict)
        missing_relic_context["relics"] = ({"slot": 0, "content_key": "Burning Blood"},)
        with self.assertRaisesRegex(ValueError, "missing"):
            decode_observation(missing_relic_state)

        duplicate_screen_relics = _observation("combat", _combat_screen(), phase="combat")
        duplicate_screen = duplicate_screen_relics["screen"]
        assert isinstance(duplicate_screen, dict)
        duplicate_screen["relics"] = ()
        with self.assertRaisesRegex(ValueError, "extra"):
            decode_observation(duplicate_screen_relics)

    def test_decoder_rejects_old_pile_fields_and_invalid_known_positions(self) -> None:
        old_count = _observation("combat", _combat_screen(), phase="combat")
        screen = old_count["screen"]
        assert isinstance(screen, dict)
        screen["draw_pile"] = {"count": 1, "cards": (_card(),), "known_positions": ()}
        with self.assertRaisesRegex(ValueError, "extra"):
            decode_observation(old_count)

        old_order = _observation("combat", _combat_screen(), phase="combat")
        old_order_screen = old_order["screen"]
        assert isinstance(old_order_screen, dict)
        old_order_screen["draw_pile"] = {
            "cards": (_card(),),
            "known_order": (_card(),),
        }
        with self.assertRaisesRegex(ValueError, "extra|missing"):
            decode_observation(old_order)

        duplicate = _observation("combat", _combat_screen(), phase="combat")
        duplicate_screen = duplicate["screen"]
        assert isinstance(duplicate_screen, dict)
        duplicate_screen["draw_pile"] = {
            "cards": (_card(), _card()),
            "known_positions": (
                {"position": 0, "card": _card()},
                {"position": 0, "card": _card()},
            ),
        }
        with self.assertRaisesRegex(ValueError, "unique"):
            decode_observation(duplicate)

        unsorted = _observation("combat", _combat_screen(), phase="combat")
        unsorted_screen = unsorted["screen"]
        assert isinstance(unsorted_screen, dict)
        unsorted_screen["draw_pile"] = {
            "cards": (_card(), _card()),
            "known_positions": (
                {"position": 1, "card": _card()},
                {"position": 0, "card": _card()},
            ),
        }
        with self.assertRaisesRegex(ValueError, "sorted"):
            decode_observation(unsorted)

        oob = _observation("combat", _combat_screen(), phase="combat")
        oob_screen = oob["screen"]
        assert isinstance(oob_screen, dict)
        oob_screen["draw_pile"] = {
            "cards": (_card(),),
            "known_positions": ({"position": 1, "card": _card()},),
        }
        with self.assertRaisesRegex(ValueError, "out of bounds"):
            decode_observation(oob)

        missing_member = _observation("combat", _combat_screen(), phase="combat")
        missing_screen = missing_member["screen"]
        assert isinstance(missing_screen, dict)
        bash = _card()
        bash["content_key"] = "Bash"
        missing_screen["draw_pile"] = {
            "cards": (_card(),),
            "known_positions": ({"position": 0, "card": bash},),
        }
        with self.assertRaisesRegex(ValueError, "subset"):
            decode_observation(missing_member)

    def test_observations_are_immutable(self) -> None:
        observation = decode_observation(_observation("combat", _combat_screen(), phase="combat"))
        with self.assertRaises(AttributeError):
            observation.kind = "map"  # type: ignore[misc]
        with self.assertRaises(AttributeError):
            observation.context.gold = 0  # type: ignore[misc]
        mutated = replace(observation.context, gold=1)
        self.assertEqual(observation.context.gold, 99)
        self.assertEqual(mutated.gold, 1)

    def test_union_variants_cover_kind_literals(self) -> None:
        observed_kinds = []
        for cls in OBSERVATION_TYPES:
            kind_type = get_type_hints(cls)["kind"]
            observed_kinds.extend(get_args(kind_type))
        self.assertEqual(frozenset(observed_kinds), frozenset(get_args(ObservationKind)))
        self.assertEqual(len(OBSERVATION_TYPES), len(get_args(ObservationKind)))

        rest_kinds = []
        for cls in REST_OPTION_TYPES:
            rest_kinds.extend(get_args(get_type_hints(cls)["kind"]))
        self.assertEqual(
            frozenset(rest_kinds),
            {
                "heal",
                "open_smith",
                "open_remove",
                "smith",
                "remove_card",
                "lift",
                "dig",
                "recall",
                "proceed",
            },
        )
        self.assertEqual(len(REST_OPTION_TYPES), len(get_args(RestOption)))
        self.assertEqual(len(ORB_TYPES), 3)
        self.assertEqual(len(INTENT_TYPES), 3)
        self.assertEqual(
            {field.name for field in fields(CombatObservation)},
            {
                "schema_version",
                "phase",
                "kind",
                "context",
                "screen",
            },
        )


class TypedObservationStaticTest(unittest.TestCase):
    def test_ty_accepts_discriminant_narrowing_and_nested_fields(self) -> None:
        result = subprocess.run(
            ["uv", "run", "ty", "check", "tests/typing/narrow_observations.py"],
            cwd=PYTHON_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_ty_accepts_fair_simulator_demo(self) -> None:
        demo = PYTHON_ROOT.parent.parent / "rl" / "examples" / "fair_simulator.py"
        result = subprocess.run(
            ["uv", "run", "ty", "check", str(demo)],
            cwd=PYTHON_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_ty_rejects_invalid_fields_and_unnarrowed_access(self) -> None:
        result = subprocess.run(
            [
                "uv",
                "run",
                "ty",
                "check",
                "--output-format",
                "concise",
                "tests/typing/invalid_observation_access.py",
            ],
            cwd=PYTHON_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        output = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, output)
        diagnostics = [line for line in output.splitlines() if "unresolved-attribute" in line]
        self.assertEqual(len(diagnostics), 7, output)

        def has_diagnostic(*needles: str) -> bool:
            return any(all(needle in line for needle in needles) for line in diagnostics)

        self.assertTrue(
            has_diagnostic(":7:", "has no attribute `seed`"),
            output,
        )
        self.assertTrue(
            has_diagnostic(":11:", "RunContext", "has no attribute `hidden_draw_order`"),
            output,
        )
        self.assertTrue(
            has_diagnostic(":15:", "Attribute `player` is not defined on", "union"),
            output,
        )
        self.assertTrue(
            has_diagnostic(":20:", "Object of type `MapScreen` has no attribute `player`"),
            output,
        )
        self.assertTrue(
            has_diagnostic(":26:", "CombatScreen", "has no attribute `relics`"),
            output,
        )
        self.assertTrue(
            has_diagnostic(":32:", "CombatScreen", "has no attribute `context`"),
            output,
        )
        self.assertTrue(
            has_diagnostic(":38:", "CombatScreen", "has no attribute `potion_slots`"),
            output,
        )


if __name__ == "__main__":
    unittest.main()
