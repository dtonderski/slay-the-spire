import unittest

from sts.slaythedata_policy import (
    build_guided_run_script,
    floor_decision,
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

    def test_identity_blocker_rejects_visible_character_or_ascension_mismatch(self):
        script = build_guided_run_script(
            {
                "event": {
                    "character_chosen": "IRONCLAD",
                    "ascension_level": 0,
                }
            }
        )

        character = identity_blocker(script, {"class": "THE_SILENT", "ascension_level": 0})
        ascension = identity_blocker(script, {"class": "IRONCLAD", "ascension_level": 1})
        matching = identity_blocker(script, {"class": "IRONCLAD", "ascension_level": 0})

        self.assertEqual(character["reason"], "run_identity_mismatch")
        self.assertEqual(ascension["reason"], "run_identity_mismatch")
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


if __name__ == "__main__":
    unittest.main()
