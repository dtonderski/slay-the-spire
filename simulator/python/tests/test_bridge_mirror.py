import json
import os
import socket
import tempfile
import threading
import unittest
from pathlib import Path

from sts.bridge import (
    BridgeMirror,
    bridge_actions_from_status,
    bridge_lifecycle_from_status,
    command_for_descriptor,
)


class BridgeMirrorTests(unittest.TestCase):
    def test_missing_session_reports_disconnected_and_stale(self):
        with tempfile.TemporaryDirectory() as directory:
            status = BridgeMirror(Path(directory)).status(now=1000.0)

        self.assertFalse(status["connected"])
        self.assertTrue(status["stale"])
        self.assertTrue(status["status"]["missing"])
        self.assertEqual(status["bridge_lifecycle"]["status"], "disconnected")

    def test_reads_active_bridge_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(
                json.dumps({"status": "waiting", "client_pid": 12, "trace_path": "trace.jsonl", "step": 4}),
                encoding="utf-8",
            )
            (root / "summary.json").write_text(
                json.dumps({"step": 4, "ready_for_command": True, "available_commands": ["play", "end", "state"]}),
                encoding="utf-8",
            )
            (root / "current_state.json").write_text(json.dumps({"step": 4}), encoding="utf-8")

            status = BridgeMirror(root, stale_after_seconds=9999).status()

        self.assertTrue(status["connected"])
        self.assertFalse(status["stale"])
        self.assertIn("state_id", status)
        self.assertEqual(status["client_pid"], 12)
        self.assertEqual(status["trace_path"], "trace.jsonl")
        self.assertEqual(status["available_commands"], ["play", "end", "state"])
        self.assertIn("bridge_actions", status)
        self.assertTrue(status["bridge_actions"])
        self.assertEqual(status["bridge_actions"][0]["source_state_id"], status["state_id"])
        self.assertEqual(status["bridge_lifecycle"]["status"], "ready")

    def test_status_prefers_bridge_advertised_state_id(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "summary.json").write_text(
                json.dumps(
                    {
                        "state_id": "bridge-protocol-state",
                        "ready_for_command": True,
                        "available_commands": ["state"],
                    }
                ),
                encoding="utf-8",
            )
            (root / "current_state.json").write_text(json.dumps({"step": 4}), encoding="utf-8")

            status = BridgeMirror(root, stale_after_seconds=9999).status()

        self.assertEqual(status["state_id"], "bridge-protocol-state")

    def test_status_treats_tcp_queued_command_as_pending(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(
                json.dumps(
                    {
                        "status": "waiting",
                        "pending_command": True,
                        "queued_command_meta": {
                            "command_id": "tcp-cmd-1",
                            "command": "CHOOSE 0",
                            "protocol": "tcp-jsonl",
                        },
                    }
                ),
                encoding="utf-8",
            )
            (root / "summary.json").write_text(
                json.dumps(
                    {
                        "state_id": "bridge-state",
                        "ready_for_command": True,
                        "available_commands": ["choose", "state"],
                    }
                ),
                encoding="utf-8",
            )

            status = BridgeMirror(root, stale_after_seconds=9999).status()
            preflight = BridgeMirror(root, stale_after_seconds=9999).preflight()

        self.assertTrue(status["pending_command"])
        self.assertEqual(status["command_id"], "tcp-cmd-1")
        self.assertEqual(status["pending_command_meta"]["protocol"], "tcp-jsonl")
        self.assertFalse(preflight["ok"])
        self.assertIn("bridge command already pending", preflight["problems"])
        self.assertEqual(preflight["pending_command"]["transport"], "tcp-jsonl")
        self.assertEqual(preflight["pending_command"]["command_id"], "tcp-cmd-1")

    def test_send_command_writes_pending_command(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")

            source_state_id = BridgeMirror(root).status()["state_id"]
            result = BridgeMirror(root).send_command("state", source_state_id=source_state_id)

            self.assertTrue(result["ok"])
            self.assertIn("command_id", result)
            self.assertEqual((root / "next_command.txt").read_text(encoding="utf-8"), "state\n")
            command_meta = json.loads((root / "next_command.json").read_text(encoding="utf-8"))
            self.assertEqual(command_meta["command_id"], result["command_id"])
            self.assertEqual(command_meta["source_state_id"], source_state_id)
            self.assertTrue(result["bridge_status"]["pending_command"])
            self.assertEqual(result["bridge_status"]["command_id"], result["command_id"])
            self.assertEqual(result["bridge_status"]["bridge_lifecycle"]["status"], "waiting_for_command_ack")

    def test_send_command_writes_optional_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")

            source_state_id = BridgeMirror(root).status()["state_id"]
            BridgeMirror(root).send_command(
                "state",
                source_state_id=source_state_id,
                metadata={"source": "guided_collector", "collector_id": "collector-1"},
            )

            command_meta = json.loads((root / "next_command.json").read_text(encoding="utf-8"))
            self.assertEqual(
                command_meta["metadata"],
                {"source": "guided_collector", "collector_id": "collector-1"},
            )

    def test_send_command_prefers_tcp_control_when_available(self):
        received = []

        def run_server(server):
            owner_token = "owner-token-1"
            for _ in range(2):
                conn, _addr = server.accept()
                with conn:
                    data = b""
                    while b"\n" not in data:
                        data += conn.recv(4096)
                    payload = json.loads(data.split(b"\n", 1)[0].decode("utf-8"))
                    received.append(payload)
                    if payload["type"] == "acquire":
                        response = {
                            "ok": True,
                            "owner_id": payload["owner_id"],
                            "owner_token": owner_token,
                            "state_id": "bridge-protocol-state",
                            "state_seq": 7,
                        }
                    else:
                        response = {
                            "ok": True,
                            "command_id": payload["command_id"],
                            "command": payload["command"],
                            "accepted_state_id": payload["expected_state_id"],
                            "accepted_state_seq": payload.get("expected_state_seq"),
                        }
                    conn.sendall((json.dumps(response) + "\n").encode("utf-8"))

        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
            server.bind(("127.0.0.1", 0))
            server.listen(1)
            port = server.getsockname()[1]
            thread = threading.Thread(target=run_server, args=(server,), daemon=True)
            thread.start()

            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "status.json").write_text(
                    json.dumps(
                        {
                            "status": "waiting",
                            "control": {"protocol": "tcp-jsonl", "host": "127.0.0.1", "port": port},
                        }
                    ),
                    encoding="utf-8",
                )
                (root / "summary.json").write_text(
                    json.dumps(
                        {
                            "state_id": "bridge-protocol-state",
                            "state_seq": 7,
                            "ready_for_command": True,
                            "available_commands": ["choose", "state"],
                        }
                    ),
                    encoding="utf-8",
                )

                result = BridgeMirror(root, stale_after_seconds=9999).send_command(
                    "CHOOSE 0",
                    source_state_id="bridge-protocol-state",
                    metadata={"source": "test"},
                )

                thread.join(timeout=2)

                self.assertTrue(result["ok"])
                self.assertEqual(result["transport"], "tcp-jsonl")
                self.assertEqual(result["accepted_state_id"], "bridge-protocol-state")
                self.assertEqual(result["accepted_state_seq"], 7)
                self.assertEqual(result["owner_id"], f"sts-python-ui-{os.getpid()}")
                self.assertFalse((root / "next_command.txt").exists())
                self.assertFalse((root / "next_command.json").exists())

        self.assertEqual(len(received), 2)
        self.assertEqual(received[0]["type"], "acquire")
        self.assertEqual(received[0]["owner_id"], f"sts-python-ui-{os.getpid()}")
        self.assertEqual(received[1]["type"], "command")
        self.assertEqual(received[1]["command"], "CHOOSE 0")
        self.assertEqual(received[1]["expected_state_id"], "bridge-protocol-state")
        self.assertEqual(received[1]["expected_state_seq"], 7)
        self.assertEqual(received[1]["owner_token"], "owner-token-1")
        self.assertEqual(received[1]["metadata"], {"source": "test"})

    def test_send_command_can_require_tcp_control(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "summary.json").write_text(
                json.dumps(
                    {
                        "state_id": "file-only-state",
                        "ready_for_command": True,
                        "available_commands": ["choose", "state"],
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "TCP bridge control is required"):
                BridgeMirror(root, stale_after_seconds=9999).send_command(
                    "CHOOSE 0",
                    source_state_id="file-only-state",
                    require_tcp_control=True,
                )

            self.assertFalse((root / "next_command.txt").exists())
            self.assertFalse((root / "next_command.json").exists())

    def test_send_command_can_wait_for_tcp_state_update(self):
        received = []

        def run_server(server):
            owner_token = "owner-token-1"
            for _ in range(2):
                conn, _addr = server.accept()
                with conn:
                    data = b""
                    while b"\n" not in data:
                        data += conn.recv(4096)
                    payload = json.loads(data.split(b"\n", 1)[0].decode("utf-8"))
                    received.append(payload)
                    if payload["type"] == "acquire":
                        response = {
                            "ok": True,
                            "owner_id": payload["owner_id"],
                            "owner_token": owner_token,
                            "state_id": "bridge-protocol-state",
                            "state_seq": 7,
                        }
                    else:
                        response = {
                            "ok": True,
                            "command_id": payload["command_id"],
                            "command": payload["command"],
                            "accepted_state_id": payload["expected_state_id"],
                            "accepted_state_seq": payload.get("expected_state_seq"),
                            "observed_update": {
                                "ok": True,
                                "state_id": "bridge-protocol-state-2",
                                "state_seq": 8,
                                "step": 3,
                                "state": {
                                    "ok": True,
                                    "protocol": "sts-bridge-jsonl-v1",
                                    "client_pid": 4321,
                                    "trace_path": "trace.jsonl",
                                    "step": 3,
                                    "state_seq": 8,
                                    "state_id": "bridge-protocol-state-2",
                                    "ready_for_command": True,
                                    "available_commands": ["choose", "state"],
                                    "pending_command": False,
                                    "summary": {
                                        "state_id": "bridge-protocol-state-2",
                                        "state_seq": 8,
                                        "ready_for_command": True,
                                        "available_commands": ["choose", "state"],
                                        "choices": ["Pray"],
                                        "floor": 2,
                                    },
                                    "state": {
                                        "state_id": "bridge-protocol-state-2",
                                        "state_seq": 8,
                                        "message": {
                                            "game_state": {
                                                "floor": 2,
                                                "choice_list": ["Pray"],
                                            }
                                        },
                                    },
                                    "status": {"status": "waiting", "step": 3},
                                },
                            },
                        }
                    conn.sendall((json.dumps(response) + "\n").encode("utf-8"))

        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
            server.bind(("127.0.0.1", 0))
            server.listen(1)
            port = server.getsockname()[1]
            thread = threading.Thread(target=run_server, args=(server,), daemon=True)
            thread.start()

            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "status.json").write_text(
                    json.dumps(
                        {
                            "status": "waiting",
                            "control": {"protocol": "tcp-jsonl", "host": "127.0.0.1", "port": port},
                        }
                    ),
                    encoding="utf-8",
                )
                (root / "summary.json").write_text(
                    json.dumps(
                        {
                            "state_id": "bridge-protocol-state",
                            "state_seq": 7,
                            "ready_for_command": True,
                            "available_commands": ["choose", "state"],
                        }
                    ),
                    encoding="utf-8",
                )

                result = BridgeMirror(root, stale_after_seconds=9999).send_command(
                    "CHOOSE 0",
                    source_state_id="bridge-protocol-state",
                    wait_for_state_update=True,
                    update_timeout_seconds=3,
                )

                thread.join(timeout=2)

        self.assertEqual(received[1]["wait_for_state_update"], True)
        self.assertEqual(received[1]["update_timeout_ms"], 3000)
        self.assertEqual(result["observed_update"]["state_id"], "bridge-protocol-state-2")
        self.assertEqual(result["observed_update"]["state_seq"], 8)
        observed_status = result["observed_update"]["bridge_status"]
        self.assertEqual(observed_status["state_id"], "bridge-protocol-state-2")
        self.assertEqual(observed_status["last_state_step"], 3)
        self.assertEqual(observed_status["current_state"]["message"]["game_state"]["floor"], 2)
        self.assertEqual(observed_status["bridge_actions"][0]["command"], "CHOOSE 0")
        self.assertEqual(observed_status["bridge_actions"][0]["source_state_id"], "bridge-protocol-state-2")

    def test_preflight_reports_orphan_command_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting", "step": 1}), encoding="utf-8")
            (root / "summary.json").write_text(
                json.dumps({"ready_for_command": True, "available_commands": ["state"], "step": 1}),
                encoding="utf-8",
            )
            (root / "next_command.json").write_text(json.dumps({"command_id": "orphan"}), encoding="utf-8")

            result = BridgeMirror(root, stale_after_seconds=9999).preflight()

            self.assertFalse(result["ok"])
            self.assertIn("next_command.json exists without next_command.txt", result["problems"])
            self.assertFalse(result["pending_command"]["present"])

    def test_preflight_reports_tcp_control_availability(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(
                json.dumps(
                    {
                        "status": "waiting",
                        "control": {"protocol": "tcp-jsonl", "host": "127.0.0.1", "port": 12345},
                    }
                ),
                encoding="utf-8",
            )
            (root / "summary.json").write_text(
                json.dumps({"ready_for_command": True, "available_commands": ["state"], "step": 1}),
                encoding="utf-8",
            )

            result = BridgeMirror(root, stale_after_seconds=9999).preflight()

            self.assertTrue(result["ok"])
            self.assertTrue(result["tcp_control_available"])
            self.assertEqual(result["control"]["protocol"], "tcp-jsonl")
            self.assertIn("status_age_seconds", result["ages"])
            self.assertIn("summary_age_seconds", result["ages"])
            self.assertFalse(result["pending_command"]["present"])

    def test_clear_orphan_command_metadata_removes_only_orphan_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "next_command.json").write_text(json.dumps({"command_id": "orphan"}), encoding="utf-8")

            result = BridgeMirror(root).clear_orphan_command_metadata()

            self.assertEqual(result, {"ok": True, "cleared": True})
            self.assertFalse((root / "next_command.json").exists())

    def test_clear_orphan_command_metadata_rejects_pending_command(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "next_command.txt").write_text("state\n", encoding="utf-8")
            (root / "next_command.json").write_text(json.dumps({"command_id": "pending"}), encoding="utf-8")

            with self.assertRaises(ValueError):
                BridgeMirror(root).clear_orphan_command_metadata()

            self.assertTrue((root / "next_command.json").exists())

    def test_send_command_rejects_stale_bridge_source_without_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "summary.json").write_text(
                json.dumps({"ready_for_command": True, "available_commands": ["end"], "step": 1}),
                encoding="utf-8",
            )
            mirror = BridgeMirror(root, stale_after_seconds=9999)
            old_source_state_id = mirror.status()["state_id"]
            (root / "summary.json").write_text(
                json.dumps({"ready_for_command": True, "available_commands": ["end"], "step": 2}),
                encoding="utf-8",
            )

            with self.assertRaises(ValueError):
                mirror.send_command("END", source_state_id=old_source_state_id)

            self.assertFalse((root / "next_command.txt").exists())
            self.assertFalse((root / "next_command.json").exists())

    def test_send_command_rejects_existing_pending_command(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "next_command.txt").write_text("state\n", encoding="utf-8")

            with self.assertRaises(ValueError):
                BridgeMirror(root).send_command("state")

    def test_send_command_rejects_unavailable_gameplay_command(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "summary.json").write_text(
                json.dumps({"ready_for_command": True, "available_commands": ["state"], "step": 1}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "not available"):
                BridgeMirror(root, stale_after_seconds=9999).send_command("END")

            self.assertFalse((root / "next_command.txt").exists())

    def test_send_command_rejects_stale_gameplay_but_allows_manual_state(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "summary.json").write_text(
                json.dumps({"ready_for_command": True, "available_commands": ["end", "state"], "step": 1}),
                encoding="utf-8",
            )
            mirror = BridgeMirror(root, stale_after_seconds=-1)

            with self.assertRaisesRegex(ValueError, "stale"):
                mirror.send_command("END")

            result = mirror.send_command("state")
            self.assertTrue(result["ok"])
            self.assertEqual((root / "next_command.txt").read_text(encoding="utf-8"), "state\n")

    def test_send_command_allows_stale_gameplay_with_matching_source_state(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "summary.json").write_text(
                json.dumps({"ready_for_command": True, "available_commands": ["choose", "state"], "step": 1}),
                encoding="utf-8",
            )
            mirror = BridgeMirror(root, stale_after_seconds=-1)
            source_state_id = mirror.status()["state_id"]

            result = mirror.send_command("CHOOSE 0", source_state_id=source_state_id)

            self.assertTrue(result["ok"])
            self.assertEqual((root / "next_command.txt").read_text(encoding="utf-8"), "CHOOSE 0\n")

    def test_send_command_allows_stale_start_from_main_menu(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "status.json").write_text(json.dumps({"status": "waiting"}), encoding="utf-8")
            (root / "summary.json").write_text(
                json.dumps(
                    {
                        "ready_for_command": True,
                        "available_commands": ["start", "state"],
                        "in_game": False,
                        "step": 1,
                    }
                ),
                encoding="utf-8",
            )
            mirror = BridgeMirror(root, stale_after_seconds=-1)

            result = mirror.send_command("START IRONCLAD 0 LIVE01")

            self.assertTrue(result["ok"])
            self.assertEqual(
                (root / "next_command.txt").read_text(encoding="utf-8"),
                "START IRONCLAD 0 LIVE01\n",
            )

    def test_clients_include_current_status_and_recent_trace_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            session = root / "session"
            trace_dir = root / "traces"
            session.mkdir()
            trace_dir.mkdir()
            trace_path = trace_dir / "trace-2026-06-30T00-00-00-000Z.jsonl"
            trace_path.write_text(
                json.dumps({"type": "metadata", "client_pid": 222, "started_at": "2026-06-30T00:00:00Z"})
                + "\n",
                encoding="utf-8",
            )
            (session / "status.json").write_text(
                json.dumps({"status": "waiting", "client_pid": 111, "trace_path": str(trace_path)}),
                encoding="utf-8",
            )
            (session / "summary.json").write_text(
                json.dumps({"ready_for_command": True, "available_commands": ["state"]}),
                encoding="utf-8",
            )

            result = BridgeMirror(session, stale_after_seconds=9999).clients(
                trace_dir=trace_dir,
                now=2000.0,
                process_info=lambda pid: {"alive": pid == 222, "name": "node.exe" if pid == 222 else f"proc-{pid}"},
            )

        clients = {client["pid"]: client for client in result["clients"]}
        self.assertEqual(set(clients), {111, 222})
        self.assertTrue(clients[111]["current"])
        self.assertFalse(clients[111]["alive"])
        self.assertFalse(clients[111]["killable"])
        self.assertFalse(clients[222]["current"])
        self.assertTrue(clients[222]["alive"])
        self.assertTrue(clients[222]["killable"])
        self.assertEqual(clients[222]["started_at"], "2026-06-30T00:00:00Z")

    def test_kill_client_rejects_ui_process(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "UI service"):
                BridgeMirror(Path(directory)).kill_client(__import__("os").getpid())

    def test_descriptor_translation_covers_known_command_families(self):
        cases = [
            ({"kind": "PlayHandSlot", "hand_slot": 1, "target_slot": 0}, "PLAY 1 0"),
            ({"kind": "EndTurn"}, "END"),
            ({"kind": "UsePotionSlot", "potion_slot": 0, "target_slot": 1}, "POTION USE 0 1"),
            ({"kind": "UsePotionSlot", "potion_slot": 0}, "POTION USE 0"),
            ({"kind": "DiscardPotionSlot", "potion_slot": 0}, "POTION 0 DISCARD"),
            ({"kind": "ChooseVisibleOption", "option_slot": 2}, "CHOOSE 2"),
            ({"kind": "ConfirmChoice"}, "CONFIRM"),
            ({"kind": "CancelChoice"}, "CANCEL"),
            ({"kind": "SkipVisibleReward"}, "SKIP"),
            ({"kind": "Proceed"}, "PROCEED"),
            ({"kind": "LeaveScreen"}, "LEAVE"),
            ({"kind": "ReturnToPreviousScreen"}, "RETURN"),
        ]

        for descriptor, command in cases:
            with self.subTest(descriptor=descriptor):
                self.assertEqual(command_for_descriptor(descriptor), command)

    def test_descriptor_translation_rejects_unknown_kind(self):
        with self.assertRaises(ValueError):
            command_for_descriptor({"kind": "Unknown"})

    def test_bridge_actions_cover_choices_and_simple_commands(self):
        actions = bridge_actions_from_status(
            {
                "ready_for_command": True,
                "available_commands": ["choose", "leave", "return", "state"],
                "choices": ["talk", "x=3"],
            },
            source_state_id="bridge-state",
        )

        labels = [action["label"] for action in actions]
        commands = [action["command"] for action in actions]
        self.assertEqual(labels[:2], ["talk", "x=3"])
        self.assertEqual(commands, ["CHOOSE 0", "CHOOSE 1", "LEAVE", "RETURN"])
        self.assertTrue(all(action["source_state_id"] == "bridge-state" for action in actions))
        self.assertTrue(all(action["enabled"] for action in actions))

    def test_bridge_actions_cover_play_end_and_disabled_state(self):
        actions = bridge_actions_from_status(
            {
                "ready_for_command": True,
                "available_commands": ["play", "end", "state"],
                "combat": {
                    "hand": [
                        {"index": 1, "name": "Strike", "playable": True, "has_target": True},
                        {"index": 2, "name": "Defend", "playable": True, "has_target": False},
                        {"index": 3, "name": "Wound", "playable": False, "has_target": False},
                    ],
                    "monsters": [
                        {"index": 0, "name": "Cultist", "gone": False},
                        {"index": 1, "name": "Slime", "gone": True},
                    ],
                },
            },
            pending_command=True,
        )

        self.assertEqual(
            [(action["label"], action["command"]) for action in actions],
            [
                ("Play Strike -> Cultist", "PLAY 1 0"),
                ("Play Defend", "PLAY 2"),
                ("End turn", "END"),
            ],
        )
        self.assertTrue(all(not action["enabled"] for action in actions))
        self.assertTrue(all(action["disabled_reason"] == "bridge command already pending" for action in actions))

    def test_bridge_actions_disable_when_age_stale(self):
        actions = bridge_actions_from_status(
            {
                "ready_for_command": True,
                "available_commands": ["choose", "state"],
                "choices": ["talk"],
            },
            source_state_id="bridge-state",
            stale=True,
        )

        self.assertEqual(actions[0]["command"], "CHOOSE 0")
        self.assertFalse(actions[0]["enabled"])
        self.assertEqual(actions[0]["disabled_reason"], "bridge state is stale")

    def test_bridge_lifecycle_names_core_states(self):
        cases = [
            (
                {"status": "exited", "reason": "stdin_closed"},
                {},
                {"connected": False, "stale": False, "exited": True, "pending_command": False},
                "exited",
            ),
            (
                {},
                {},
                {"connected": False, "stale": False, "exited": False, "pending_command": False},
                "disconnected",
            ),
            (
                {"status": "waiting"},
                {"ready_for_command": True},
                {"connected": True, "stale": True, "exited": False, "pending_command": False},
                "stale",
            ),
            (
                {"status": "waiting"},
                {"ready_for_command": True},
                {"connected": True, "stale": False, "exited": False, "pending_command": True},
                "waiting_for_command_ack",
            ),
            (
                {"status": "sent", "command": "END"},
                {"ready_for_command": False},
                {"connected": True, "stale": False, "exited": False, "pending_command": False},
                "waiting_for_next_state",
            ),
            (
                {"status": "ready"},
                {},
                {"connected": True, "stale": False, "exited": False, "pending_command": False},
                "waiting_for_observed_state",
            ),
            (
                {"status": "waiting"},
                {"ready_for_command": True},
                {"connected": True, "stale": False, "exited": False, "pending_command": False},
                "ready",
            ),
        ]

        for status, summary, flags, expected in cases:
            with self.subTest(expected=expected):
                lifecycle = bridge_lifecycle_from_status(status, summary, **flags)
                self.assertEqual(lifecycle["status"], expected)
                self.assertTrue(lifecycle["label"])


if __name__ == "__main__":
    unittest.main()
