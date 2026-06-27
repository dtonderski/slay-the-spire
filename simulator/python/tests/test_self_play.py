import json
import tempfile
import unittest
from pathlib import Path

from sts import omni
from sts.search import CombatSearchConfig
from sts.search_lab import SearchCandidate
from sts.self_play import (
    evaluate_self_play_corpus,
    real_trace_root_report,
    replay_real_trace_guided,
    run_self_play,
    run_self_play_batch,
    verify_self_play_trace,
)


class SelfPlayTests(unittest.TestCase):
    def test_map_fixture_self_play_writes_replayable_trace(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "selfplay.jsonl"

            result = run_self_play(
                output=trace_path,
                start="map_fixture",
                random_seed=7,
                max_steps=12,
            )

            self.assertEqual(result.trace_path, trace_path)
            self.assertGreater(result.steps, 0)
            self.assertTrue(result.verified)

            verification = verify_self_play_trace(trace_path)
            self.assertTrue(verification["ok"])
            self.assertEqual(verification["steps"], result.steps)

            records = self._read_jsonl(trace_path)
            self.assertEqual(records[0]["source"], "sim_selfplay")
            self.assertIn("initial_snapshot_json", records[0])
            self.assertTrue(any(record.get("type") == "step" for record in records))
            self.assertTrue(
                all("potions" in record["before_summary"] for record in records[1:])
            )

    def test_seed_start_writes_replayable_placeholder_trace(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "seed.jsonl"

            result = run_self_play(
                output=trace_path,
                start="seed",
                seed="TEST",
                max_steps=4,
            )

            self.assertTrue(result.verified)
            self.assertGreater(result.steps, 0)

            records = self._read_jsonl(trace_path)
            self.assertEqual(records[0]["source"], "sim_selfplay")
            self.assertEqual(records[0]["start"], "seed")
            self.assertEqual(records[0]["seed"], "TEST")
            self.assertIn("initial_snapshot_json", records[0])

    def test_seed_start_can_record_potion_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "seed-potion.jsonl"

            result = run_self_play(
                output=trace_path,
                start="seed",
                seed="3",
                random_seed=4,
                max_steps=40,
            )

            self.assertTrue(result.verified)
            records = self._read_jsonl(trace_path)
            self.assertTrue(
                any((record.get("after_summary") or {}).get("potions") for record in records[1:])
            )

    def test_batch_writes_verified_non_parity_index(self):
        with tempfile.TemporaryDirectory() as directory:
            output_dir = Path(directory) / "corpus"

            result = run_self_play_batch(
                output_dir=output_dir,
                seeds=["TEST", "3"],
                random_seed=11,
                max_steps=6,
            )

            self.assertEqual(result.trace_count, 2)
            self.assertEqual(result.verified_count, 2)
            self.assertTrue(result.index_path.exists())

            index = json.loads(result.index_path.read_text(encoding="utf-8"))
            self.assertEqual(index["source"], "sim_selfplay_corpus")
            self.assertEqual(index["parity"], "non_parity_simulator_only")
            self.assertEqual(index["trace_count"], 2)
            self.assertEqual(index["verified_count"], 2)
            self.assertEqual(len(index["traces"]), 2)
            self.assertTrue(all(Path(row["path"]).exists() for row in index["traces"]))

    def test_trace_eval_uses_recorded_combat_roots(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "combat.jsonl"
            env = self._combat_env_with_fire_potion()
            before_hash = env.snapshot_hash()
            before_snapshot = env.snapshot_json()
            actions = env.exact_legal_actions()
            use_potion = next(action for action in actions if action.kind() == "use_potion")
            result = env.step(use_potion)
            self._write_jsonl(
                trace_path,
                [
                    {
                        "type": "metadata",
                        "schema": 1,
                        "source": "sim_selfplay",
                        "parity": "non_parity_simulator_only",
                        "initial_snapshot_json": before_snapshot,
                        "initial_state_id": before_hash,
                        "stop_reason": "test_fixture",
                        "steps": 1,
                    },
                    {
                        "type": "step",
                        "step": 0,
                        "before_hash": before_hash,
                        "before_snapshot_json": before_snapshot,
                        "before_summary": {"phase": "combat", "potions": ["Fire"]},
                        "legal_actions": [
                            {"family": action.family(), "kind": action.kind(), "json": action.json()}
                            for action in actions
                        ],
                        "action_family": use_potion.family(),
                        "action_kind": use_potion.kind(),
                        "action_json": use_potion.json(),
                        "policy": "test_fixture",
                        "policy_diagnostics": {},
                        "after_hash": result.snapshot_hash,
                        "after_snapshot_json": result.snapshot_json,
                        "after_summary": {"phase": result.phase, "potions": []},
                        "transition": None,
                        "unsupported_reason": result.unsupported_reason,
                        "error": None,
                    },
                ],
            )

            report = evaluate_self_play_corpus(
                traces=[trace_path],
                max_roots=2,
                max_actions=2,
                candidates=[
                    SearchCandidate(
                        "tiny_greedy",
                        CombatSearchConfig(
                            max_depth=1,
                            objective="survive_then_damage",
                            algorithm="greedy",
                        ),
                    )
                ],
            )

            self.assertEqual(report["source"], "sim_selfplay_trace_eval")
            self.assertEqual(report["parity"], "non_parity_simulator_only")
            self.assertGreater(report["roots"], 0)
            self.assertGreater(report["potion_action_roots"], 0)
            self.assertGreater(report["allowed_potion_roots"], 0)
            self.assertGreater(report["groups"]["potion"]["roots"], 0)
            self.assertGreater(report["groups"]["allowed_potion"]["roots"], 0)
            self.assertEqual(len(report["ranking"]), 1)
            self.assertEqual(report["ranking"][0]["candidate"], "tiny_greedy")
            self.assertTrue(report["episodes"])
            self.assertEqual(report["episodes"][0]["trace_path"], str(trace_path))
            self.assertIn("legal_action_kinds", report["episodes"][0])
            self.assertTrue(report["episodes"][0]["has_potion_actions"])
            self.assertTrue(report["episodes"][0]["has_allowed_potion_actions"])

            no_potions_report = evaluate_self_play_corpus(
                traces=[trace_path],
                max_roots=2,
                max_actions=2,
                allowed_potions=(),
                candidates=[
                    SearchCandidate(
                        "tiny_greedy",
                        CombatSearchConfig(
                            max_depth=1,
                            objective="survive_then_damage",
                            algorithm="greedy",
                        ),
                    )
                ],
            )
            self.assertEqual(no_potions_report["allowed_potions"], ())
            self.assertEqual(no_potions_report["allowed_potion_roots"], 0)
            self.assertFalse(no_potions_report["episodes"][0]["has_allowed_potion_actions"])

    def test_real_trace_report_explains_missing_simulator_snapshots(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "communication.jsonl"
            self._write_jsonl(
                trace_path,
                [
                    {"type": "metadata", "schema": 1, "source": "communication_mod"},
                    {
                        "type": "state",
                        "step": 1,
                        "message": {
                            "game_state": {
                                "screen_type": "COMBAT",
                                "room_phase": "COMBAT",
                                "hand": [{"name": "Strike"}],
                                "monsters": [{"name": "Cultist", "current_hp": 48}],
                                "potions": [
                                    {
                                        "name": "Fire Potion",
                                        "id": "Fire Potion",
                                        "can_use": True,
                                    }
                                ],
                            }
                        },
                    },
                ],
            )

            report = real_trace_root_report([trace_path])

            self.assertEqual(report["extractable_roots"], 0)
            self.assertEqual(report["observed_combat_states"], 1)
            self.assertEqual(report["observed_potion_combat_states"], 1)
            self.assertIn("simulator snapshots", report["blocked_traces"][0]["block_reason"])

    def test_trace_guided_replay_writes_verified_prefix_until_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "communication.jsonl"
            output_path = Path(directory) / "replayed.jsonl"
            report_path = Path(directory) / "report.json"
            env = omni.OmniRunEnv.new_ironclad(seed="TEST", ascension=0)
            records = [
                {"type": "metadata", "schema": 1, "source": "communication_mod"},
                {"type": "action", "step": 0, "command": "START IRONCLAD 0 TEST"},
                {"type": "state", "step": 1, "message": {"game_state": self._observed_state_from_env(env)}},
            ]
            step = 2
            map_steps = 0
            while env.exact_legal_actions() and env.phase() != "combat":
                records.append({"type": "action", "step": step, "command": "CHOOSE 0"})
                env.step(env.exact_legal_actions()[0])
                records.append(
                    {
                        "type": "state",
                        "step": step + 1,
                        "message": {"game_state": self._observed_state_from_env(env)},
                    }
                )
                step += 2
                map_steps += 1
            records.append({"type": "action", "step": step, "command": "CHOOSE 0"})
            self._write_jsonl(
                trace_path,
                records,
            )

            result = replay_real_trace_guided(
                trace=trace_path,
                output=output_path,
                report_output=report_path,
            )

            self.assertEqual(result.stop_reason, "unsupported_command_mapping")
            self.assertEqual(result.steps, map_steps)
            self.assertEqual(result.combat_roots, 0)
            self.assertTrue(result.verified)
            self.assertTrue(report_path.exists())
            verification = verify_self_play_trace(output_path)
            self.assertTrue(verification["ok"])
            records = self._read_jsonl(output_path)
            self.assertEqual(records[0]["blocker"]["category"], "unsupported_command_mapping")

    def test_trace_guided_replay_reports_neow_divergence(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "communication.jsonl"
            output_path = Path(directory) / "replayed.jsonl"
            self._write_jsonl(
                trace_path,
                [
                    {"type": "metadata", "schema": 1, "source": "communication_mod"},
                    {"type": "action", "step": 0, "command": "START IRONCLAD 0 MANUAL01"},
                    {
                        "type": "state",
                        "step": 1,
                        "message": {
                            "game_state": {
                                "screen_type": "EVENT",
                                "current_hp": 80,
                                "max_hp": 80,
                                "gold": 99,
                                "potions": [],
                            }
                        },
                    },
                    {"type": "action", "step": 2, "command": "CHOOSE 0"},
                ],
            )

            result = replay_real_trace_guided(trace=trace_path, output=output_path)
            records = self._read_jsonl(output_path)

            self.assertEqual(result.stop_reason, "observed_simulator_divergence")
            self.assertEqual(result.steps, 0)
            self.assertEqual(result.combat_roots, 0)
            self.assertEqual(records[0]["blocker"]["category"], "observed_simulator_divergence")
            self.assertEqual(records[0]["blocker"]["diffs"][0]["field"], "phase")

    def test_verify_rejects_action_mismatch(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "selfplay.jsonl"
            run_self_play(
                output=trace_path,
                start="map_fixture",
                random_seed=1,
                max_steps=4,
            )

            records = self._read_jsonl(trace_path)
            step_record = next(record for record in records if record.get("type") == "step")
            step_record["action_json"] = '"not a legal action"'
            self._write_jsonl(trace_path, records)

            verification = verify_self_play_trace(trace_path)
            self.assertFalse(verification["ok"])
            self.assertEqual(verification["error"], "action not legal during replay")

    def _read_jsonl(self, path):
        with path.open("r", encoding="utf-8") as handle:
            return [json.loads(line) for line in handle if line.strip()]

    def _write_jsonl(self, path, records):
        with path.open("w", encoding="utf-8") as handle:
            for record in records:
                handle.write(json.dumps(record))
                handle.write("\n")

    def _combat_env_with_fire_potion(self):
        state = json.loads(omni.OmniRunEnv.combat_fixture().state_json())
        state["potions"] = ["Fire"]
        return omni.OmniRunEnv.from_state_json(json.dumps(state))

    def _observed_state_from_env(self, env):
        state = json.loads(env.state_json())
        combat = state.get("combat")
        if combat:
            hand = ((combat.get("piles") or {}).get("hand") or [])
            return {
                "screen_type": "NONE",
                "current_hp": state["player_hp"],
                "max_hp": state["player_max_hp"],
                "gold": state["gold"],
                "potions": state.get("potions") or [],
                "combat_state": {
                    "energy": ((combat.get("player") or {}).get("energy")),
                    "hand": [{} for _ in hand],
                    "monsters": [{} for _ in combat.get("monsters", [])],
                },
            }
        screen_type = "MAP" if env.phase() == "map" else env.phase().upper()
        return {
            "screen_type": screen_type,
            "current_hp": state["player_hp"],
            "max_hp": state["player_max_hp"],
            "gold": state["gold"],
            "potions": state.get("potions") or [],
        }


if __name__ == "__main__":
    unittest.main()
