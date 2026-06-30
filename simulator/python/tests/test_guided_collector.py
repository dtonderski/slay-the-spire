import unittest

from sts.guided_collector import GuidedCollector, send_guided_suggestion, suggest_guided_action
from sts.slaythedata_policy import build_guided_run_script


def sample_script():
    return build_guided_run_script(
        {
            "run_id": 42,
            "event": {
                "character_chosen": "IRONCLAD",
                "ascension_level": 0,
                "seed_played": "ABC",
                "path_per_floor": ["M", "?"],
                "card_choices": [{"floor": 1, "picked": "Inflame"}],
                "relics_obtained": [{"floor": 1, "key": "Oddly Smooth Stone"}],
                "items_purchased": ["Shrug It Off"],
                "item_purchase_floors": [3],
                "campfire_choices": [{"floor": 4, "key": "SMITH", "data": "Bash+"}],
                "event_choices": [
                    {"floor": 2, "event_name": "Golden Shrine", "player_choice": "Pray"}
                ],
                "boss_relics": [{"picked": "Black Blood", "not_picked": ["Snecko Eye"]}],
                "potions_floor_usage": [3],
            },
        }
    )


class GuidedCollectorTests(unittest.TestCase):
    def ready_event_bridge(self):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "bridge-state",
            "summary": {
                "floor": 2,
                "screen_type": "EVENT",
                "choices": ["Pray", "Leave"],
                "available_commands": ["choose"],
            },
        }

    def ready_combat_bridge(self):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "combat-bridge-state",
            "summary": {
                "floor": 3,
                "phase": "combat",
                "combat": {"monsters": []},
            },
        }

    def ready_boss_relic_bridge(self):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "boss-relic-bridge-state",
            "summary": {
                "floor": 17,
                "act": 1,
                "screen_type": "BOSS_RELIC_REWARD",
                "choices": ["Snecko Eye", "Black Blood", "Tiny House"],
                "available_commands": ["choose"],
            },
        }

    def ready_map_bridge(self):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "map-bridge-state",
            "current_state": {
                "message": {
                    "game_state": {
                        "floor": 1,
                        "act": 1,
                        "screen_type": "MAP",
                        "choice_list": ["x=1 ?", "x=3 $"],
                        "screen_state": {
                            "next_nodes": [
                                {"x": 1, "room_symbol": "?"},
                                {"x": 3, "room_symbol": "$"},
                            ]
                        },
                    }
                }
            },
            "summary": {
                "floor": 1,
                "act": 1,
                "screen_type": "MAP",
                "choices": ["x=1 ?", "x=3 $"],
                "available_commands": ["choose"],
            },
        }

    def ready_campfire_bridge(self):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "campfire-bridge-state",
            "summary": {
                "floor": 4,
                "screen_type": "REST",
                "choices": ["Rest", "Smith"],
                "available_commands": ["choose"],
            },
        }

    def ready_reward_bridge(self):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "reward-bridge-state",
            "summary": {
                "floor": 1,
                "screen_type": "COMBAT_REWARD",
                "choices": ["Gold", "Card", "Relic"],
                "available_commands": ["choose"],
            },
        }

    def ready_shop_bridge(self, choices=None):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "shop-bridge-state",
            "summary": {
                "floor": 3,
                "screen_type": "SHOP",
                "choices": choices or ["Strike", "Shrug It Off", "Leave"],
                "available_commands": ["choose", "leave"],
            },
        }

    def ready_grid_bridge(self):
        return {
            "connected": True,
            "exited": False,
            "pending_command": False,
            "ready_for_command": True,
            "state_id": "grid-bridge-state",
            "summary": {
                "floor": 4,
                "screen_type": "GRID",
                "choices": ["Strike", "Bash"],
                "available_commands": ["choose"],
            },
        }

    def test_suggest_guided_action_matches_visible_event_choice(self):
        result = suggest_guided_action(
            sample_script(),
            self.ready_event_bridge(),
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})
        self.assertEqual(result["category"], "event")

    def test_suggest_guided_action_matches_boss_relic_choice(self):
        result = suggest_guided_action(
            sample_script(),
            self.ready_boss_relic_bridge(),
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["category"], "boss_relic")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})

    def test_suggest_guided_action_matches_map_path_choice(self):
        result = suggest_guided_action(
            sample_script(),
            self.ready_map_bridge(),
        )

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["category"], "map")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 0})

    def test_suggest_guided_action_matches_campfire_then_grid_choice(self):
        campfire = suggest_guided_action(sample_script(), self.ready_campfire_bridge())
        grid = suggest_guided_action(sample_script(), self.ready_grid_bridge())

        self.assertEqual(campfire["status"], "matched")
        self.assertEqual(campfire["category"], "campfire")
        self.assertEqual(campfire["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(grid["status"], "matched")
        self.assertEqual(grid["category"], "grid")
        self.assertEqual(grid["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})

    def test_suggest_guided_action_matches_generic_reward_choice(self):
        result = suggest_guided_action(sample_script(), self.ready_reward_bridge())

        self.assertEqual(result["status"], "matched")
        self.assertEqual(result["category"], "reward")
        self.assertEqual(result["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 2})

    def test_suggest_guided_action_matches_shop_purchase_and_leave(self):
        buy = suggest_guided_action(sample_script(), self.ready_shop_bridge())
        leave = suggest_guided_action(
            sample_script(),
            self.ready_shop_bridge(["Leave"]),
            ordinal=1,
        )

        self.assertEqual(buy["status"], "matched")
        self.assertEqual(buy["category"], "shop")
        self.assertEqual(buy["descriptor"], {"kind": "ChooseVisibleOption", "option_slot": 1})
        self.assertEqual(leave["status"], "matched")
        self.assertEqual(leave["category"], "shop")
        self.assertEqual(leave["descriptor"], {"kind": "LeaveScreen"})

    def test_send_guided_suggestion_sends_matching_descriptor_with_source_state(self):
        calls = []

        def send_command(command, *, source_state_id=None):
            calls.append((command, source_state_id))
            return {"ok": True, "command_id": "cmd-1", "command": command}

        suggestion = suggest_guided_action(sample_script(), self.ready_event_bridge())

        result = send_guided_suggestion(
            suggestion,
            self.ready_event_bridge(),
            send_command=send_command,
        )

        self.assertEqual(result["status"], "sent")
        self.assertEqual(result["command"], "CHOOSE 0")
        self.assertEqual(result["source_state_id"], "bridge-state")
        self.assertEqual(calls, [("CHOOSE 0", "bridge-state")])

    def test_send_guided_suggestion_blocks_when_bridge_is_not_ready(self):
        bridge = self.ready_event_bridge()
        bridge["ready_for_command"] = False
        suggestion = suggest_guided_action(sample_script(), bridge)

        result = send_guided_suggestion(
            suggestion,
            bridge,
            send_command=lambda *_args, **_kwargs: {"ok": True},
        )

        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["reason"], "bridge_not_ready")

    def test_collector_tick_send_is_opt_in(self):
        collector = GuidedCollector()
        collector.start({"script": sample_script()})
        calls = []

        dry_run = collector.tick(
            self.ready_event_bridge(),
            send_command=lambda command, **kwargs: calls.append((command, kwargs)) or {"ok": True},
        )

        sent = collector.tick(
            self.ready_event_bridge(),
            {"send": True},
            send_command=lambda command, **kwargs: calls.append((command, kwargs)) or {
                "ok": True,
                "command_id": "cmd-2",
                "command": command,
            },
        )

        self.assertEqual(dry_run["suggestion"]["status"], "matched")
        self.assertEqual(sent["suggestion"]["status"], "sent")
        self.assertEqual(calls, [("CHOOSE 0", {"source_state_id": "bridge-state"})])

    def test_suggest_guided_action_reports_combat_potion_budget(self):
        result = suggest_guided_action(
            sample_script(),
            {
                "summary": {
                    "floor": 3,
                    "phase": "combat",
                    "combat": {"monsters": []},
                }
            },
        )

        self.assertEqual(result["status"], "combat")
        self.assertEqual(result["mode"], "combat_agent")
        self.assertEqual(result["potion_uses_allowed"], 1)

    def test_collector_tick_send_combat_delegates_to_combat_callback(self):
        collector = GuidedCollector()
        collector.start({"script": sample_script()})
        calls = []

        result = collector.tick(
            self.ready_combat_bridge(),
            {"send": True, "max_depth": 4},
            send_combat=lambda **kwargs: calls.append(kwargs)
            or {
                "predicted_state_id": "predicted-1",
                "source_state_id": "sim-1",
                "bridge_state_id": "combat-bridge-state",
                "bridge_step": 9,
                "send_result": {"command": "END"},
            },
        )

        self.assertEqual(result["suggestion"]["status"], "sent_combat")
        self.assertEqual(result["pending_prediction"]["predicted_state_id"], "predicted-1")
        self.assertEqual(calls[0]["payload"]["max_depth"], 4)
        self.assertEqual(calls[0]["suggestion"]["potion_uses_allowed"], 1)

    def test_collector_tick_send_non_combat_delegates_to_strict_callback(self):
        collector = GuidedCollector()
        collector.start({"script": sample_script()})
        calls = []

        result = collector.tick(
            self.ready_event_bridge(),
            {"send": True},
            send_non_combat=lambda **kwargs: calls.append(kwargs)
            or {
                "predicted_state_id": "predicted-event",
                "source_state_id": "sim-event",
                "bridge_state_id": "bridge-state",
                "bridge_step": 2,
                "send_result": {"command": "CHOOSE 0"},
            },
        )

        self.assertEqual(result["suggestion"]["status"], "sent_non_combat")
        self.assertEqual(result["pending_prediction"]["predicted_state_id"], "predicted-event")
        self.assertEqual(calls[0]["suggestion"]["category"], "event")

    def test_collector_tick_blocks_combat_send_without_callback(self):
        collector = GuidedCollector()
        collector.start({"script": sample_script()})

        result = collector.tick(self.ready_combat_bridge(), {"send": True})

        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["blocker"]["reason"], "missing_combat_sender")

    def test_collector_blocks_on_pending_prediction_mismatch(self):
        collector = GuidedCollector()
        collector.start({"script": sample_script()})
        collector.tick(
            self.ready_combat_bridge(),
            {"send": True},
            send_combat=lambda **_kwargs: {
                "predicted_state_id": "predicted-1",
                "source_state_id": "sim-1",
                "bridge_state_id": "combat-bridge-state",
                "send_result": {"command": "END"},
            },
        )

        result = collector.tick(
            self.ready_combat_bridge(),
            verify_prediction=lambda *_args, **_kwargs: {
                "status": "mismatch",
                "detail": "expected predicted-1, observed other",
            },
        )

        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["blocker"]["reason"], "prediction_mismatch")
        self.assertEqual(result["pending_prediction"]["predicted_state_id"], "predicted-1")

    def test_collector_waits_for_bridge_ack_before_prediction_check(self):
        collector = GuidedCollector()
        collector.start({"script": sample_script()})
        collector.tick(
            self.ready_combat_bridge(),
            {"send": True},
            send_combat=lambda **_kwargs: {
                "predicted_state_id": "predicted-1",
                "source_state_id": "sim-1",
                "bridge_state_id": "combat-bridge-state",
                "send_result": {"command": "END"},
            },
        )
        bridge = self.ready_combat_bridge()
        bridge["pending_command"] = True

        result = collector.tick(
            bridge,
            verify_prediction=lambda *_args, **_kwargs: self.fail("prediction should wait for bridge ack"),
        )

        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["blocker"]["reason"], "pending_command")
        self.assertEqual(result["pending_prediction"]["predicted_state_id"], "predicted-1")

    def test_collector_clears_matching_pending_prediction_before_next_tick(self):
        collector = GuidedCollector()
        collector.start({"script": sample_script()})
        collector.tick(
            self.ready_combat_bridge(),
            {"send": True},
            send_combat=lambda **_kwargs: {
                "predicted_state_id": "predicted-1",
                "source_state_id": "sim-1",
                "bridge_state_id": "combat-bridge-state",
                "send_result": {"command": "END"},
            },
        )

        result = collector.tick(
            self.ready_combat_bridge(),
            verify_prediction=lambda *_args, **_kwargs: {"status": "matched"},
        )

        self.assertEqual(result["suggestion"]["status"], "combat")
        self.assertIsNone(result["pending_prediction"])

    def test_collector_tracks_blockers_and_status(self):
        collector = GuidedCollector()
        started = collector.start({"script": sample_script()})
        self.assertTrue(started["active"])
        self.assertEqual(started["status"], "ready")

        tick = collector.tick(
            {
                "summary": {
                    "floor": 2,
                    "screen_type": "EVENT",
                    "choices": ["Leave"],
                }
            }
        )

        self.assertEqual(tick["status"], "blocked")
        self.assertEqual(tick["blocker"]["reason"], "target_not_visible")
        self.assertEqual(tick["history_count"], 1)

        stopped = collector.stop()
        self.assertEqual(stopped["status"], "stopped")
        self.assertFalse(collector.status()["active"])


if __name__ == "__main__":
    unittest.main()
