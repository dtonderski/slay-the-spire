"""Schema metadata caching must not weaken payload validation."""

import copy
import unittest
from collections import UserDict
from dataclasses import FrozenInstanceError, asdict, dataclass

from sts_sim import State
from sts_sim.observations import decode_observation
from sts_sim.observations._decode import _exact, _field_names, _mapping
from sts_sim.observations.common import (
    RunKeys,
    _cached_card,
    _decode_card,
    decode_card,
    decode_run_keys,
)


class DecodeCacheTests(unittest.TestCase):
    def test_cached_schema_still_rejects_bad_fields_and_types(self) -> None:
        _field_names.cache_clear()
        valid = {"ruby": False, "emerald": True, "sapphire": False}
        self.assertEqual(decode_run_keys(valid, "keys"), RunKeys(**valid))
        self.assertEqual(decode_run_keys(dict(valid), "keys"), RunKeys(**valid))
        self.assertGreater(_field_names.cache_info().hits, 0)
        for invalid in ({**valid, "hidden": 1}, {"ruby": False}):
            with self.assertRaises(ValueError):
                decode_run_keys(invalid, "keys")
        with self.assertRaises(TypeError):
            decode_run_keys({**valid, "ruby": 1}, "keys")
        with self.assertRaises(TypeError):
            _exact({**valid, 7: False}, "keys", RunKeys)
        # No observation values are retained in the metadata cache.
        valid["ruby"] = True
        self.assertTrue(decode_run_keys(valid, "keys").ruby)

    def card_mapping(self) -> dict[str, object]:
        card = asdict(State.new("81").decision().observation.context.deck[0])
        card["content_key"] = str(card["content_key"])
        return card

    def test_card_cache_snapshots_all_fields_and_retains_no_mutable_input(self) -> None:
        _cached_card.cache_clear()
        raw = self.card_mapping()
        card = decode_card(raw, "public.deck[0]")
        self.assertIs(card, decode_card(copy.deepcopy(raw), "public.deck[0]"))
        self.assertEqual(_cached_card.cache_info().hits, 1)
        self.assertEqual(_cached_card.cache_info().maxsize, 4096)
        raw["cost"] = 99
        raw["dynamic"] = {"rampage_damage_bonus": 100}
        changed = decode_card(raw, "public.deck[0]")
        self.assertEqual(changed, _decode_card(raw, "public.deck[0]"))
        self.assertNotEqual(card.cost, changed.cost)
        self.assertEqual(changed.dynamic.rampage_damage_bonus, 100)
        self.assertNotEqual(card.dynamic.rampage_damage_bonus, 100)
        with self.assertRaises(FrozenInstanceError):
            setattr(changed, "cost", 1)  # noqa: B010 — exercise the runtime frozen guard
        # Ordinary Mapping implementations remain supported without caching them.
        self.assertEqual(decode_card(UserDict(raw), "custom"), changed)

    def test_card_cache_preserves_decoder_errors_and_type_distinctions(self) -> None:
        raw = self.card_mapping()
        _cached_card.cache_clear()
        decode_card(raw, "public.deck[3]")
        # Warm the potential bool/int equality aliases before each malformed case.
        for field in (
            "cost",
            "cost_is_modified",
            "cost_resets_next_turn",
            "upgrade_level",
            "bottled",
            "temporary",
        ):
            valid = copy.deepcopy(raw)
            valid[field] = False if isinstance(raw[field], bool) else 0
            decode_card(valid, "public.deck[3]")
            for invalid in (True, False, 0, 1, 1.0, -0.0, None, "0", [], {}):
                candidate = {**valid, field: invalid}
                with self.subTest(field=field, invalid=invalid):
                    self.assert_decode_equal(candidate)
        for field in (
            "rampage_damage_bonus",
            "ritual_dagger_damage_bonus",
            "windmill_retain_damage",
            "steam_barrier_block_reduction",
            "combat_cost_under_turn_override",
        ):
            for value in (None, 0, 1, True, False, 1.0, -0.0, "0", [], {}):
                self.assert_decode_equal({**raw, "dynamic": {field: value}})
        for candidate in (
            {**raw, "hidden": 1},
            {k: v for k, v in raw.items() if k != "cost"},
            {**raw, "dynamic": {"hidden": 0}},
            {**raw, "content_key": "not_a_card"},
            {**raw, 1: 0},
        ):
            self.assert_decode_equal(candidate)

    def assert_decode_equal(self, raw: object) -> None:
        try:
            expected = _decode_card(raw, "public.deck[3]")
        except (TypeError, ValueError) as error:
            with self.assertRaises(type(error)) as actual:
                decode_card(raw, "public.deck[3]")
            self.assertEqual(str(actual.exception), str(error))
        else:
            self.assertEqual(decode_card(raw, "public.deck[3]"), expected)

    def test_mapping_fast_path_preserves_copy_and_key_validation(self) -> None:
        value = {"key": 1}
        decoded = _mapping(value, "public", required=frozenset({"key"}))
        self.assertEqual(decoded, value)
        self.assertIsNot(decoded, value)
        value["key"] = 2
        self.assertEqual(decoded["key"], 1)

        class StringAlias:
            def __hash__(self) -> int:
                return hash("key")

            def __eq__(self, other: object) -> bool:
                return other == "key"

        # A non-string key can compare equal to the expected schema. It must
        # still fail the key-type check, on both dict and generic Mapping paths.
        for mapping in ({StringAlias(): 1}, UserDict({StringAlias(): 1})):
            with self.assertRaises(TypeError):
                _mapping(mapping, "public", required=frozenset({"key"}))

    def test_unchanged_map_is_reused_and_changed_map_is_decoded(self) -> None:
        raw = State.new("713")._native.observation()._to_mapping()
        first = decode_observation(raw)
        assert first.context.map is not None
        same = decode_observation(copy.deepcopy(raw), previous=(raw, first))
        self.assertIs(same.context.map, first.context.map)
        changed = copy.deepcopy(raw)
        changed["context"]["map"]["current_node"] = first.context.map.current_node + 1
        decoded = decode_observation(changed, previous=(raw, first))
        assert decoded.context.map is not None
        self.assertIsNot(decoded.context.map, first.context.map)
        self.assertEqual(decoded, decode_observation(changed))
        self.assertEqual(decoded.context.map.current_node, first.context.map.current_node + 1)

    def test_state_reuse_matches_fresh_decode_through_combat(self) -> None:
        state = State.new("713", training_rng_seed=7)
        decision = state.decision()
        previous = decision.observation
        shared = {"map": 0, "draw_pile": 0}
        combat_steps = 0
        for step in range(300):
            fresh = decode_observation(state._native.observation()._to_mapping())
            self.assertEqual(decision.observation, fresh)
            current = decision.observation
            shared["map"] += current.context.map is previous.context.map is not None
            if current.kind == "combat" and previous.kind == "combat":
                combat_steps += 1
                shared["draw_pile"] += current.screen.draw_pile is previous.screen.draw_pile
            if not decision.actions:
                break
            previous = current
            decision = state.step(decision.actions[step % len(decision.actions)])
        self.assertGreater(combat_steps, 0)
        self.assertGreater(shared["map"], 0)
        self.assertGreater(shared["draw_pile"], 0)

    def test_schema_cache_is_bounded_and_class_specific(self) -> None:
        @dataclass
        class Other:
            value: int

        self.assertEqual(_field_names(Other), frozenset({"value"}))
        self.assertNotEqual(_field_names(Other), _field_names(RunKeys))
        self.assertEqual(_field_names.cache_info().maxsize, 128)
        with self.assertRaises(TypeError):
            _field_names(int)


if __name__ == "__main__":
    unittest.main()
