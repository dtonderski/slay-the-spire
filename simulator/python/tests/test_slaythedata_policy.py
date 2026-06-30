import unittest

from sts.slaythedata_policy import (
    build_guided_run_script,
    floor_decision,
    guided_script_support_blocker,
    identity_blocker,
    match_map_choice,
    match_visible_choice,
    potion_uses_allowed_on_floor,
)


class SlayTheDataPolicyTests(unittest.TestCase):
    def test_build_guided_script_groups_floor_decisions(self):
        script = build_guided_run_script(
            {
                "run_id": 123,
                "event": {
                    "play_id": "play-1",
                    "character_chosen": "IRONCLAD",
                    "ascension_level": 0,
                    "seed_played": "ABC",
                    "path_per_floor": ["M", "?", "$"],
                    "path_taken": ["M", "?", "$"],
                    "card_choices": [
                        {"floor": 1, "picked": "Inflame+", "not_picked": ["Strike", "Clash+"]}
                    ],
                    "relics_obtained": [{"floor": 1, "key": "Oddly Smooth Stone"}],
                    "event_choices": [
                        {
                            "floor": 2,
                            "event_name": "World of Goop",
                            "player_choice": "Gather Gold",
                            "damage_taken": 11,
                            "gold_gain": 75,
                        }
                    ],
                    "items_purchased": ["Shrug It Off", "Membership Card"],
                    "item_purchase_floors": [3, 3],
                    "campfire_choices": [{"floor": 4, "key": "SMITH", "data": "Bash+"}],
                    "potions_floor_usage": [1, 3, 3],
                    "potions_obtained": [{"floor": 2, "key": "Fire Potion"}],
                    "boss_relics": [{"picked": "Black Blood", "not_picked": ["Snecko Eye"]}],
                    "master_deck": ["Bash+", "Inflame+"],
                    "relics": ["Burning Blood"],
                    "gold": 99,
                    "floor_reached": 4,
                    "victory": False,
                },
            }
        )

        self.assertEqual(script["schema"], 1)
        self.assertEqual(script["source"]["run_id"], 123)
        self.assertEqual(script["config"]["character"], "IRONCLAD")
        self.assertFalse(script["replay_policy"]["exact_combat_actions"])
        self.assertEqual(potion_uses_allowed_on_floor(script, 3), 2)

        floor_1 = floor_decision(script, 1)
        self.assertEqual(floor_1["route"], "M")
        self.assertEqual(floor_1["card_rewards"][0]["picked"], "Inflame")
        self.assertEqual(floor_1["card_rewards"][0]["not_picked"], ["Strike", "Clash"])
        self.assertEqual(floor_1["relics_obtained"][0]["key"], "Oddly Smooth Stone")

        floor_2 = floor_decision(script, 2)
        self.assertEqual(floor_2["events"][0]["event_name"], "World of Goop")
        self.assertEqual(floor_2["events"][0]["player_choice"], "Gather Gold")
        self.assertEqual(floor_2["potions"]["obtained"][0]["key"], "Fire Potion")

        floor_3 = floor_decision(script, 3)
        self.assertEqual([item["item"] for item in floor_3["shop_purchases"]], ["Shrug It Off", "Membership Card"])
        self.assertEqual(script["boss_relic_choices"][0]["act"], 1)
        self.assertEqual(script["final_observed"]["master_deck"], ["Bash", "Inflame"])

    def test_match_visible_choice_finds_single_textual_card_reward(self):
        script = build_guided_run_script(
            {
                "event": {
                    "card_choices": [
                        {"floor": 1, "picked": "Inflame", "not_picked": ["Clash", "Flex"]}
                    ]
                }
            }
        )

        result = match_visible_choice(
            script,
            floor=1,
            choice_labels=["Clash", "Inflame+", "Skip"],
            category="card_reward",
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(result["target"], "Inflame")

    def test_match_visible_choice_preserves_floor_zero_neow_card_reward(self):
        script = build_guided_run_script(
            {
                "event": {
                    "neow_bonus": "THREE_CARDS",
                    "neow_cost": "NONE",
                    "card_choices": [
                        {"floor": 0, "picked": "True Grit", "not_picked": ["Flex", "Anger"]}
                    ],
                }
            }
        )

        result = match_visible_choice(
            script,
            floor=0,
            choice_labels=["Flex", "True Grit", "Anger"],
            category="card_reward",
        )

        self.assertEqual(floor_decision(script, 0)["card_rewards"][0]["picked"], "True Grit")
        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})

    def test_match_visible_choice_skips_card_reward_when_script_skipped(self):
        script = build_guided_run_script(
            {
                "event": {
                    "card_choices": [
                        {"floor": 1, "picked": "SKIP", "not_picked": ["Clash", "Flex", "Anger"]}
                    ]
                }
            }
        )

        result = match_visible_choice(
            script,
            floor=1,
            choice_labels=["Clash", "Flex", "Anger"],
            category="card_reward",
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "SkipVisibleReward"})
        self.assertEqual(result["target"], "SKIP")

    def test_match_visible_choice_handles_neow_talk_bonus_and_leave(self):
        script = build_guided_run_script(
            {
                "event": {
                    "neow_bonus": "THREE_ENEMY_KILL",
                    "neow_cost": "NONE",
                }
            }
        )

        talk = match_visible_choice(
            script,
            floor=0,
            choice_labels=["talk"],
            category="neow",
        )
        bonus = match_visible_choice(
            script,
            floor=0,
            choice_labels=[
                "obtain a random rare card",
                "enemies in your next three combats have 1 hp",
                "obtain a curse choose a rare colorless card to obtain",
            ],
            category="neow",
        )
        leave = match_visible_choice(
            script,
            floor=0,
            choice_labels=["leave"],
            category="neow",
        )

        self.assertEqual(talk["status"], "matched")
        self.assertEqual(talk["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})
        self.assertEqual(bonus["status"], "matched")
        self.assertEqual(bonus["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(leave["status"], "matched")
        self.assertEqual(leave["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})

    def test_match_visible_choice_handles_neow_cost_bonus_phrase(self):
        script = build_guided_run_script(
            {
                "event": {
                    "neow_bonus": "RANDOM_COLORLESS_2",
                    "neow_cost": "CURSE",
                }
            }
        )

        result = match_visible_choice(
            script,
            floor=0,
            choice_labels=["obtain a curse choose a rare colorless card to obtain"],
            category="neow",
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})

    def test_match_visible_choice_handles_guided_safe_neow_bonus_aliases(self):
        cases = [
            ("ONE_RANDOM_RARE_CARD", "NONE", "obtain a random rare card"),
            ("ONE_RARE_RELIC", "CURSE", "obtain a curse obtain a random rare relic"),
            ("TWO_FIFTY_GOLD", "NONE", "obtain 250 gold"),
            ("TWENTY_PERCENT_HP_BONUS", "PERCENT_DAMAGE", "take 18 damage gain 16 max hp"),
            ("THREE_SMALL_POTIONS", "NONE", "obtain 3 random potions"),
            ("BOSS_RELIC", "NO_GOLD", "lose all gold obtain a random boss relic"),
        ]
        for bonus, cost, label in cases:
            with self.subTest(bonus=bonus, cost=cost):
                script = build_guided_run_script(
                    {
                        "event": {
                            "neow_bonus": bonus,
                            "neow_cost": cost,
                        }
                    }
                )

                result = match_visible_choice(
                    script,
                    floor=0,
                    choice_labels=["skip this option", label],
                    category="neow",
                )

                self.assertEqual(result["status"], "matched")
                self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})

    def test_guided_script_support_blocks_unrecorded_neow_grids(self):
        for bonus in ("REMOVE_CARD", "REMOVE_TWO", "TRANSFORM_CARD", "TRANSFORM_TWO_CARDS", "UPGRADE_CARD"):
            with self.subTest(bonus=bonus):
                script = build_guided_run_script({"event": {"neow_bonus": bonus, "neow_cost": "NONE"}})

                blocker = guided_script_support_blocker(script)

                self.assertIsNotNone(blocker)
                self.assertEqual(blocker["reason"], "unsupported_neow_followup")

    def test_guided_script_support_requires_floor_zero_neow_card_choice(self):
        missing = build_guided_run_script({"event": {"neow_bonus": "THREE_CARDS", "neow_cost": "NONE"}})
        present = build_guided_run_script(
            {
                "event": {
                    "neow_bonus": "THREE_CARDS",
                    "neow_cost": "NONE",
                    "card_choices": [{"floor": 0, "picked": "Bash", "not_picked": ["Strike"]}],
                }
            }
        )

        self.assertEqual(guided_script_support_blocker(missing)["reason"], "missing_neow_card_reward")
        self.assertIsNone(guided_script_support_blocker(present))

    def test_identity_blocker_rejects_visible_character_or_ascension_mismatch(self):
        script = build_guided_run_script(
            {
                "event": {
                    "character_chosen": "IRONCLAD",
                    "ascension_level": 0,
                    "seed_played": "LIVE01",
                }
            }
        )

        character = identity_blocker(script, {"class": "THE_SILENT", "ascension_level": 0})
        ascension = identity_blocker(script, {"class": "IRONCLAD", "ascension_level": 1})
        seed = identity_blocker(script, {"class": "IRONCLAD", "ascension_level": 0, "seed": "OTHER"})
        matching = identity_blocker(script, {"class": "IRONCLAD", "ascension_level": 0, "seed": "LIVE01"})

        self.assertEqual(character["reason"], "run_identity_mismatch")
        self.assertEqual(ascension["reason"], "run_identity_mismatch")
        self.assertEqual(seed["reason"], "run_identity_mismatch")
        self.assertIsNone(matching)

    def test_match_visible_choice_blocks_missing_and_ambiguous_targets(self):
        script = build_guided_run_script(
            {
                "event": {
                    "event_choices": [
                        {"floor": 2, "event_name": "Golden Shrine", "player_choice": "Pray"}
                    ]
                }
            }
        )

        missing = match_visible_choice(
            script,
            floor=2,
            choice_labels=["Leave"],
            category="event",
        )
        self.assertEqual(missing["status"], "blocked")
        self.assertEqual(missing["reason"], "target_not_visible")

        ambiguous = match_visible_choice(
            script,
            floor=2,
            choice_labels=["Pray", "Pray again"],
            category="event",
        )
        self.assertEqual(ambiguous["status"], "blocked")
        self.assertEqual(ambiguous["reason"], "ambiguous_target")

    def test_match_visible_choice_finds_boss_relic_by_act(self):
        script = build_guided_run_script(
            {
                "event": {
                    "boss_relics": [
                        {"picked": "Black Blood", "not_picked": ["Snecko Eye"]},
                        {"picked": "Runic Pyramid", "not_picked": ["Tiny House"]},
                    ]
                }
            }
        )

        result = match_visible_choice(
            script,
            floor=34,
            act=2,
            choice_labels=["Tiny House", "Runic Pyramid", "Velvet Choker"],
            category="boss_relic",
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(result["target"], "Runic Pyramid")

    def test_match_visible_choice_uses_campfire_key_before_grid_target(self):
        script = build_guided_run_script(
            {
                "event": {
                    "campfire_choices": [{"floor": 4, "key": "SMITH", "data": "Bash+"}],
                }
            }
        )

        campfire = match_visible_choice(
            script,
            floor=4,
            choice_labels=["Rest", "Smith"],
            category="campfire",
        )
        grid = match_visible_choice(
            script,
            floor=4,
            choice_labels=["Strike", "Bash"],
            category="grid",
        )

        self.assertEqual(campfire["status"], "matched")
        self.assertEqual(campfire["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(campfire["target"], "SMITH")
        self.assertEqual(grid["status"], "matched")
        self.assertEqual(grid["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(grid["target"], "Bash")

    def test_match_visible_choice_selects_remaining_reward_from_script_evidence(self):
        script = build_guided_run_script(
            {
                "event": {
                    "card_choices": [{"floor": 1, "picked": "Inflame"}],
                    "relics_obtained": [{"floor": 1, "key": "Oddly Smooth Stone"}],
                    "potions_obtained": [{"floor": 1, "key": "Fire Potion"}],
                }
            }
        )

        relic = match_visible_choice(
            script,
            floor=1,
            choice_labels=["Gold", "Card", "Relic", "Potion"],
            category="reward",
        )
        card = match_visible_choice(
            script,
            floor=1,
            choice_labels=["Gold", "Card"],
            category="reward",
        )
        gold = match_visible_choice(
            script,
            floor=1,
            choice_labels=["Gold"],
            category="reward",
        )

        self.assertEqual(relic["status"], "matched")
        self.assertEqual(relic["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 2})
        self.assertEqual(card["status"], "matched")
        self.assertEqual(card["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(gold["status"], "matched")
        self.assertEqual(gold["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})

    def test_match_visible_choice_prefers_named_reward_identity(self):
        script = build_guided_run_script(
            {
                "event": {
                    "relics_obtained": [{"floor": 1, "key": "Oddly Smooth Stone"}],
                    "potions_obtained": [{"floor": 2, "key": "Fire Potion"}],
                }
            }
        )

        relic = match_visible_choice(
            script,
            floor=1,
            choice_labels=["Gold", "Oddly Smooth Stone", "Card"],
            category="reward",
        )
        potion = match_visible_choice(
            script,
            floor=2,
            choice_labels=["Gold", "Fire Potion"],
            category="reward",
        )

        self.assertEqual(relic["status"], "matched")
        self.assertEqual(relic["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(relic["target"], "Oddly Smooth Stone")
        self.assertEqual(potion["status"], "matched")
        self.assertEqual(potion["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(potion["target"], "Fire Potion")

    def test_match_visible_choice_selects_shop_purchase_then_leave(self):
        script = build_guided_run_script(
            {
                "event": {
                    "items_purchased": ["Shrug It Off"],
                    "item_purchase_floors": [3],
                }
            }
        )

        buy = match_visible_choice(
            script,
            floor=3,
            choice_labels=["Strike", "Shrug It Off", "Leave"],
            category="shop",
        )
        leave = match_visible_choice(
            script,
            floor=3,
            choice_labels=["Leave"],
            category="shop",
            ordinal=1,
        )

        self.assertEqual(buy["status"], "matched")
        self.assertEqual(buy["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(leave["status"], "matched")
        self.assertEqual(leave["descriptor"], {"kind": "LeaveScreen"})

    def test_match_visible_choice_opens_shop_purge_then_selects_removed_card(self):
        script = build_guided_run_script(
            {
                "event": {
                    "event_choices": [
                        {"floor": 3, "event_name": "Shop", "player_choice": "Purge", "cards_removed": ["Strike"]}
                    ],
                }
            }
        )

        purge = match_visible_choice(
            script,
            floor=3,
            choice_labels=["Remove Card", "Leave"],
            category="shop",
        )
        grid = match_visible_choice(
            script,
            floor=3,
            choice_labels=["Defend", "Strike"],
            category="grid",
        )

        self.assertEqual(purge["status"], "matched")
        self.assertEqual(purge["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})
        self.assertEqual(grid["status"], "matched")
        self.assertEqual(grid["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})

    def test_match_map_choice_finds_next_path_room(self):
        script = build_guided_run_script(
            {
                "event": {
                    "path_per_floor": ["M", "?", "$"],
                }
            }
        )

        result = match_map_choice(
            script,
            floor=1,
            choice_labels=["x=2 ?", "x=4 $"],
            next_nodes=[
                {"x": 2, "room_symbol": "?"},
                {"x": 4, "room_symbol": "$"},
            ],
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})
        self.assertEqual(result["target"], "?")

    def test_match_map_choice_blocks_ambiguous_route(self):
        script = build_guided_run_script({"event": {"path_per_floor": ["M"]}})

        result = match_map_choice(
            script,
            floor=0,
            choice_labels=["left monster", "right monster"],
            next_nodes=[
                {"choice_index": 0, "room_symbol": "M"},
                {"choice_index": 1, "room_symbol": "monster"},
            ],
        )

        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reason"], "ambiguous_target")

    def test_match_map_choice_disambiguates_ambiguous_route_with_map_lookahead(self):
        script = build_guided_run_script({"event": {"path_per_floor": ["M", "?", "$"]}})

        result = match_map_choice(
            script,
            floor=0,
            choice_labels=["x=1", "x=2"],
            next_nodes=[
                {"x": 1, "y": 0, "symbol": "M"},
                {"x": 2, "y": 0, "symbol": "M"},
            ],
            map_nodes=[
                {"x": 1, "y": 0, "symbol": "M", "children": [{"x": 1, "y": 1}]},
                {"x": 2, "y": 0, "symbol": "M", "children": [{"x": 2, "y": 1}]},
                {"x": 1, "y": 1, "symbol": "?", "children": [{"x": 1, "y": 2}]},
                {"x": 2, "y": 1, "symbol": "$", "children": [{"x": 2, "y": 2}]},
                {"x": 1, "y": 2, "symbol": "$", "children": []},
                {"x": 2, "y": 2, "symbol": "?", "children": []},
            ],
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})
        self.assertEqual(result["match_evidence"], "map_topology_lookahead")

    def test_match_map_choice_still_blocks_when_map_lookahead_is_ambiguous(self):
        script = build_guided_run_script({"event": {"path_per_floor": ["M", "?", "$"]}})

        result = match_map_choice(
            script,
            floor=0,
            choice_labels=["x=1", "x=2"],
            next_nodes=[
                {"x": 1, "y": 0, "symbol": "M"},
                {"x": 2, "y": 0, "symbol": "M"},
            ],
            map_nodes=[
                {"x": 1, "y": 0, "symbol": "M", "children": [{"x": 1, "y": 1}]},
                {"x": 2, "y": 0, "symbol": "M", "children": [{"x": 2, "y": 1}]},
                {"x": 1, "y": 1, "symbol": "?", "children": [{"x": 1, "y": 2}]},
                {"x": 2, "y": 1, "symbol": "?", "children": [{"x": 2, "y": 2}]},
                {"x": 1, "y": 2, "symbol": "$", "children": []},
                {"x": 2, "y": 2, "symbol": "$", "children": []},
            ],
        )

        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reason"], "ambiguous_target")


if __name__ == "__main__":
    unittest.main()
