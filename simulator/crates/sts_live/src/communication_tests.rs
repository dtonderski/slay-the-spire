use crate::{
    bridge::BridgeManager,
    communication::{CommunicationBridgeConfig, CommunicationModBridgeManager},
    model::{
        ActionId, BridgeId, Character, LegalAction, LegalActionKind, LiveError, LivePhase,
        RunConfig, RunSeed,
    },
};
use serde_json::json;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn communication_bridge_lists_session_files() {
    let root = temp_dir("list");
    write_bridge_files(&root, neow_summary());
    let bridge = bridge(&root, false);

    let bridges = bridge.list_bridges().unwrap();

    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].id.0, "communication-mod");
    assert_eq!(bridges[0].process_id, Some(1234));
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_marks_exited_session_unconnected() {
    let root = temp_dir("exited");
    let summary = neow_summary();
    write_bridge_files(&root, summary.clone());
    fs::write(
        root.join("status.json"),
        serde_json::to_vec(&json!({
            "step": summary["step"],
            "client_pid": 4321,
            "status": "exited",
            "trace_path": "trace.jsonl",
            "allow_file_commands": false,
            "summary": summary,
        }))
        .unwrap(),
    )
    .unwrap();
    let bridge = bridge(&root, false);

    let bridges = bridge.list_bridges().unwrap();

    assert_eq!(bridges.len(), 1);
    assert!(!bridges[0].connected);
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_uses_tcp_reachability_for_connected_status() {
    let root = temp_dir("tcp-reachable-list");
    write_bridge_files(&root, menu_summary());
    thread::sleep(Duration::from_millis(25));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let _ = listener.accept().unwrap();
    });
    fs::write(
        root.join("status.json"),
        serde_json::to_vec(&json!({
            "step": 0,
            "client_pid": 1234,
            "status": "ready",
            "trace_path": "trace.jsonl",
            "control": {
                "host": "127.0.0.1",
                "port": port,
                "protocol": "tcp-jsonl"
            },
        }))
        .unwrap(),
    )
    .unwrap();
    let bridge = bridge_with_stale_after(&root, false, Duration::from_millis(10));

    let bridges = bridge.list_bridges().unwrap();

    server.join().unwrap();
    assert_eq!(bridges.len(), 1);
    assert!(bridges[0].connected);
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_maps_visible_choices_to_typed_actions() {
    let root = temp_dir("actions");
    write_bridge_files(&root, neow_summary());
    let mut bridge = bridge(&root, false);

    let state = bridge
        .request_state(&BridgeId("communication-mod".to_owned()))
        .unwrap();

    assert_eq!(state.phase, LivePhase::Neow);
    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("choose-0".to_owned())
            && action.kind == LegalActionKind::ChooseNeow
            && action.command["command"] == "CHOOSE 0"
            && action.command["source_state_id"] == "state-1"
    }));
    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("abandon-run".to_owned())
            && action.kind == LegalActionKind::AbandonRun
            && action.command["command"] == "ABANDON"
    }));
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_hides_full_belt_potion_reward_choice() {
    let root = temp_dir("full-potion-reward");
    write_bridge_files(&root, full_belt_reward_summary());
    let mut bridge = bridge(&root, false);

    let state = bridge
        .request_state(&BridgeId("communication-mod".to_owned()))
        .unwrap();

    assert_eq!(state.phase, LivePhase::Reward);
    assert!(!state
        .legal_actions
        .iter()
        .any(|action| action.id == ActionId("choose-0".to_owned())));
    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("choose-1".to_owned())
            && action.kind == LegalActionKind::ChooseReward
            && action.label == "card"
            && action.command["command"] == "CHOOSE 1"
    }));
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_treats_room_combat_with_none_screen_as_combat_phase() {
    let root = temp_dir("combat-none-screen");
    write_bridge_files(&root, combat_summary_with_none_screen());
    let mut bridge = bridge(&root, false);

    let state = bridge
        .request_state(&BridgeId("communication-mod".to_owned()))
        .unwrap();

    assert_eq!(state.phase, LivePhase::Combat);
    assert!(state
        .legal_actions
        .iter()
        .any(|action| action.id.0 == "play-1-0"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_maps_usable_combat_potions() {
    let root = temp_dir("combat-potions");
    write_bridge_files(&root, combat_summary_with_usable_potions());
    let mut bridge = bridge(&root, false);

    let state = bridge
        .request_state(&BridgeId("communication-mod".to_owned()))
        .unwrap();

    assert_eq!(state.phase, LivePhase::Combat);
    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("potion-0".to_owned())
            && action.kind == LegalActionKind::UsePotion
            && action.label == "Use Ancient Potion"
            && action.command["command"] == "POTION USE 0"
            && action.command["source_state_id"] == "combat-potion-state"
    }));
    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("potion-1-0".to_owned())
            && action.kind == LegalActionKind::UsePotion
            && action.label == "Use Fire Potion -> Lagavulin"
            && action.command["command"] == "POTION USE 1 0"
            && action.command["source_state_id"] == "combat-potion-state"
    }));
    assert!(!state
        .legal_actions
        .iter()
        .any(|action| action.id == ActionId("potion-2".to_owned())));
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_uses_raw_potion_belt_slots_when_summary_is_compacted() {
    let root = temp_dir("combat-potions-raw-slots");
    let summary = combat_summary_with_compacted_potion_slots();
    let current_state = json!({
        "step": summary["step"],
        "state_seq": summary["state_seq"],
        "state_id": summary["state_id"],
        "client_pid": 1234,
        "message": {
            "available_commands": summary["available_commands"],
            "ready_for_command": true,
            "game_state": {
                "screen_type": summary["screen_type"],
                "room_phase": summary["room_phase"],
                "room_type": summary["room_type"],
                "potions": [
                    {
                        "id": "Ancient Potion",
                        "name": "Ancient Potion",
                        "can_use": true,
                        "can_discard": true,
                        "requires_target": false
                    },
                    {
                        "id": "Potion Slot",
                        "name": "Potion Slot",
                        "can_use": false,
                        "can_discard": false,
                        "requires_target": false
                    },
                    {
                        "id": "Swift Potion",
                        "name": "Swift Potion",
                        "can_use": true,
                        "can_discard": true,
                        "requires_target": false
                    }
                ]
            }
        }
    });
    write_bridge_files_with_current_state(&root, summary, current_state);
    let mut bridge = bridge(&root, false);

    let state = bridge
        .request_state(&BridgeId("communication-mod".to_owned()))
        .unwrap();

    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("potion-2".to_owned())
            && action.kind == LegalActionKind::UsePotion
            && action.label == "Use Swift Potion"
            && action.command["command"] == "POTION USE 2"
    }));
    assert!(!state
        .legal_actions
        .iter()
        .any(|action| action.id == ActionId("potion-1".to_owned())));
    assert_eq!(state.raw["summary"]["potions"][1]["index"], 2);
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_maps_shop_screen_leave_action() {
    let root = temp_dir("shop-leave");
    write_bridge_files(&root, shop_summary());
    let mut bridge = bridge(&root, false);

    let state = bridge
        .request_state(&BridgeId("communication-mod".to_owned()))
        .unwrap();

    assert_eq!(state.phase, LivePhase::Shop);
    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("leave".to_owned())
            && action.kind == LegalActionKind::Confirm
            && action.label == "Leave shop"
            && action.command["command"] == "LEAVE"
            && action.command["source_state_id"] == "shop-state"
    }));
    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("choose-0".to_owned())
            && action.kind == LegalActionKind::ShopBuy
            && action.command["command"] == "CHOOSE 0"
    }));
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_attach_falls_back_when_no_observed_state_exists_yet() {
    let root = temp_dir("no-observed-state");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        serde_json::to_writer(
            &mut stream,
            &json!({"ok": false, "error": "no observed state is available"}),
        )
        .unwrap();
        stream.write_all(b"\n").unwrap();
    });
    write_bridge_files(&root, menu_summary());
    let status = json!({
        "step": 0,
        "client_pid": 1234,
        "status": "ready",
        "trace_path": "trace.jsonl",
        "control": {
            "host": "127.0.0.1",
            "port": port,
            "protocol": "tcp-jsonl"
        },
    });
    fs::write(
        root.join("status.json"),
        serde_json::to_vec(&status).unwrap(),
    )
    .unwrap();
    let mut bridge = bridge(&root, false);

    let state = bridge
        .request_state(&BridgeId("communication-mod".to_owned()))
        .unwrap();

    server.join().unwrap();
    assert_eq!(state.phase, LivePhase::Menu);
    assert!(state
        .legal_actions
        .iter()
        .any(|action| action.id.0 == "request-state"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_start_sends_tcp_command_without_state_preflight() {
    let root = temp_dir("stale-summary-start");
    write_bridge_files(&root, menu_summary());
    thread::sleep(Duration::from_millis(25));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let server_received = Arc::clone(&received);
    let server = thread::spawn(move || {
        for response in [
            json!({"ok": true, "owner_token": "owner-1"}),
            json!({"ok": true}),
            json!({"ok": true}),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            server_received
                .lock()
                .unwrap()
                .push(serde_json::from_str::<serde_json::Value>(&line).unwrap());
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.write_all(b"\n").unwrap();
        }
    });
    fs::write(
        root.join("status.json"),
        serde_json::to_vec(&json!({
            "step": 0,
            "client_pid": 4321,
            "status": "ready",
            "trace_path": "trace.jsonl",
            "control": {
                "host": "127.0.0.1",
                "port": port,
                "protocol": "tcp-jsonl"
            },
        }))
        .unwrap(),
    )
    .unwrap();
    let mut bridge = bridge_with_stale_after(&root, false, Duration::from_millis(10));

    bridge
        .start_run(
            &BridgeId("communication-mod".to_owned()),
            &RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
            },
        )
        .unwrap();

    server.join().unwrap();
    let requests = received.lock().unwrap();
    assert!(!requests.iter().any(|request| request["type"] == "state"));
    let command = requests
        .iter()
        .find(|request| request["type"] == "command")
        .expect("missing command request");
    assert_eq!(command["command"], "START IRONCLAD 0 CODEX04");
    assert_eq!(command["expected_state_id"], serde_json::Value::Null);
    assert_eq!(command["expected_state_seq"], serde_json::Value::Null);
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_tcp_command_result_survives_release_failure() {
    let root = temp_dir("tcp-release-failure");
    write_bridge_files(&root, neow_summary());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let server_received = Arc::clone(&received);
    let server = thread::spawn(move || {
        let responses = [
            Some(json!({"ok": true, "owner_token": "owner-1"})),
            Some(json!({
                "ok": true,
                "observed_update": {
                    "state": {
                        "status": {"status": "ready"},
                        "summary": menu_summary(),
                    }
                }
            })),
            None,
        ];
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            server_received
                .lock()
                .unwrap()
                .push(serde_json::from_str::<serde_json::Value>(&line).unwrap());
            if let Some(response) = response {
                serde_json::to_writer(&mut stream, &response).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        }
    });
    fs::write(
        root.join("status.json"),
        serde_json::to_vec(&json!({
            "step": 1,
            "client_pid": 1234,
            "status": "ready",
            "trace_path": "trace.jsonl",
            "control": {
                "host": "127.0.0.1",
                "port": port,
                "protocol": "tcp-jsonl"
            },
        }))
        .unwrap(),
    )
    .unwrap();
    let mut bridge = bridge(&root, false);
    let action = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseNeow,
        label: "Talk".to_owned(),
        enabled: true,
        command: json!({
            "transport": "communication_mod",
            "command": "CHOOSE 0",
            "source_state_id": "state-1",
        }),
        disabled_reason: None,
    };

    let state = bridge
        .send_action(&BridgeId("communication-mod".to_owned()), &action)
        .unwrap();

    server.join().unwrap();
    assert_eq!(state.phase, LivePhase::Menu);
    let requests = received.lock().unwrap();
    assert!(!requests.iter().any(|request| request["type"] == "state"));
    assert_eq!(requests[0]["type"], "acquire");
    assert_eq!(requests[1]["type"], "command");
    assert_eq!(requests[1]["command"], "CHOOSE 0");
    assert_eq!(requests[2]["type"], "release");
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_rejects_observed_update_timeout_instead_of_returning_stale_state() {
    let root = temp_dir("tcp-observed-timeout");
    write_bridge_files(&root, neow_summary());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let server_received = Arc::clone(&received);
    let server = thread::spawn(move || {
        let responses = [
            json!({"ok": true, "owner_token": "owner-1"}),
            json!({
                "ok": true,
                "observed_update": {
                    "ok": false,
                    "error": "timed out waiting for observed state update",
                    "accepted_state_id": "state-1",
                    "accepted_state_seq": 3,
                    "observed_changed": false,
                    "application_status": "timeout",
                    "step": 2
                }
            }),
            json!({"ok": true, "released": true}),
        ];
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            server_received
                .lock()
                .unwrap()
                .push(serde_json::from_str::<serde_json::Value>(&line).unwrap());
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.write_all(b"\n").unwrap();
        }
    });
    fs::write(
        root.join("status.json"),
        serde_json::to_vec(&json!({
            "step": 1,
            "client_pid": 1234,
            "status": "ready",
            "trace_path": "trace.jsonl",
            "control": {
                "host": "127.0.0.1",
                "port": port,
                "protocol": "tcp-jsonl"
            },
        }))
        .unwrap(),
    )
    .unwrap();
    let mut bridge = bridge(&root, false);
    let action = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseNeow,
        label: "Talk".to_owned(),
        enabled: true,
        command: json!({
            "transport": "communication_mod",
            "command": "CHOOSE 0",
            "source_state_id": "state-1",
        }),
        disabled_reason: None,
    };

    let err = bridge
        .send_action(&BridgeId("communication-mod".to_owned()), &action)
        .unwrap_err();

    server.join().unwrap();
    assert!(matches!(err, LiveError::Bridge(message) if message.contains("observed state update")));
    let requests = received.lock().unwrap();
    assert_eq!(requests[0]["type"], "acquire");
    assert_eq!(requests[1]["type"], "command");
    assert_eq!(requests[1]["update_timeout_ms"], 15_000);
    assert_eq!(requests[2]["type"], "release");
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_file_command_fallback_is_opt_in() {
    let root = temp_dir("file-fallback");
    write_bridge_files(&root, menu_summary());
    let mut strict = bridge(&root, false);
    let err = strict
        .start_run(
            &BridgeId("communication-mod".to_owned()),
            &RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
            },
        )
        .unwrap_err();
    assert!(matches!(err, LiveError::Bridge(_)));

    let mut fallback = bridge(&root, true);
    fallback
        .start_run(
            &BridgeId("communication-mod".to_owned()),
            &RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
            },
        )
        .unwrap();

    let command = fs::read_to_string(root.join("next_command.txt")).unwrap();
    assert_eq!(command, "START IRONCLAD 0 CODEX04\n");
    let meta = fs::read_to_string(root.join("next_command.json")).unwrap();
    assert!(meta.contains("legacy-file"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_rejects_stale_action_source_id() {
    let root = temp_dir("stale-action");
    write_bridge_files(&root, neow_summary());
    let mut read_bridge = bridge(&root, false);
    let state = read_bridge
        .request_state(&BridgeId("communication-mod".to_owned()))
        .unwrap();
    let mut action = state
        .legal_actions
        .into_iter()
        .find(|action| action.id.0 == "choose-0")
        .unwrap();
    action.command["source_state_id"] = json!("older-state");

    let mut bridge = bridge(&root, true);
    let err = bridge
        .send_action(&BridgeId("communication-mod".to_owned()), &action)
        .unwrap_err();

    assert!(matches!(err, LiveError::Bridge(_)));
    assert!(!root.join("next_command.txt").exists());
    fs::remove_dir_all(root).ok();
}

#[test]
fn communication_bridge_abandon_uses_operator_command() {
    let root = temp_dir("abandon");
    write_bridge_files(&root, neow_summary());
    let mut bridge = bridge(&root, true);

    bridge
        .abandon_run(&BridgeId("communication-mod".to_owned()))
        .unwrap();

    let command = fs::read_to_string(root.join("next_command.txt")).unwrap();
    assert_eq!(command, "ABANDON\n");
    let meta = fs::read_to_string(root.join("next_command.json")).unwrap();
    assert!(meta.contains("ABANDON"));
    fs::remove_dir_all(root).ok();
}

fn bridge(root: &Path, allow_file_commands: bool) -> CommunicationModBridgeManager {
    let mut config = CommunicationBridgeConfig::new(root);
    config.allow_file_commands = allow_file_commands;
    config.discover_local_processes = false;
    CommunicationModBridgeManager::new(config)
}

fn bridge_with_stale_after(
    root: &Path,
    allow_file_commands: bool,
    stale_after: Duration,
) -> CommunicationModBridgeManager {
    let mut config = CommunicationBridgeConfig::new(root);
    config.allow_file_commands = allow_file_commands;
    config.discover_local_processes = false;
    config.stale_after = stale_after;
    CommunicationModBridgeManager::new(config)
}

fn write_bridge_files(root: &Path, summary: serde_json::Value) {
    let current_state = json!({
        "step": summary["step"],
        "state_seq": summary["state_seq"],
        "state_id": summary["state_id"],
        "client_pid": 1234,
        "message": {"game_state": {"screen_type": summary["screen_type"]}},
    });
    write_bridge_files_with_current_state(root, summary, current_state);
}

fn write_bridge_files_with_current_state(
    root: &Path,
    summary: serde_json::Value,
    current_state: serde_json::Value,
) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("status.json"),
        serde_json::to_vec(&json!({
            "step": summary["step"],
            "client_pid": 1234,
            "status": "waiting",
            "trace_path": "trace.jsonl",
            "allow_file_commands": false,
            "summary": summary,
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("summary.json"),
        serde_json::to_vec(&summary).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("current_state.json"),
        serde_json::to_vec(&current_state).unwrap(),
    )
    .unwrap();
}

fn menu_summary() -> serde_json::Value {
    json!({
        "step": 0,
        "client_pid": 1234,
        "state_seq": 1,
        "state_id": "menu-state",
        "available_commands": ["start", "state"],
        "ready_for_command": true,
        "in_game": false,
        "screen_type": "MENU",
    })
}

fn neow_summary() -> serde_json::Value {
    json!({
        "step": 2,
        "client_pid": 1234,
        "state_seq": 3,
        "state_id": "state-1",
        "available_commands": ["choose", "state"],
        "ready_for_command": true,
        "in_game": true,
        "screen_type": "EVENT",
        "room_type": "NeowRoom",
        "choices": [
            "obtain a random rare card",
            "enemies in your next three combats have 1 hp"
        ],
    })
}

fn combat_summary_with_none_screen() -> serde_json::Value {
    json!({
        "step": 10,
        "client_pid": 1234,
        "state_seq": 11,
        "state_id": "combat-state",
        "available_commands": ["play", "end", "state"],
        "ready_for_command": true,
        "in_game": true,
        "screen_type": "NONE",
        "screen_name": "NONE",
        "room_phase": "COMBAT",
        "room_type": "MonsterRoom",
        "combat": {
            "hand": [
                {
                    "index": 1,
                    "id": "Strike_R",
                    "name": "Strike",
                    "cost": 1,
                    "playable": true,
                    "type": "ATTACK",
                    "has_target": true
                }
            ],
            "monsters": [
                {
                    "index": 0,
                    "id": "Cultist",
                    "name": "Cultist",
                    "hp": 20,
                    "max_hp": 54,
                    "gone": false
                }
            ]
        }
    })
}

fn combat_summary_with_usable_potions() -> serde_json::Value {
    json!({
        "step": 12,
        "client_pid": 1234,
        "state_seq": 13,
        "state_id": "combat-potion-state",
        "available_commands": ["play", "potion", "end", "state", "abandon"],
        "ready_for_command": true,
        "in_game": true,
        "screen_type": "NONE",
        "screen_name": "NONE",
        "room_phase": "COMBAT",
        "room_type": "MonsterRoomElite",
        "combat": {
            "hand": [],
            "monsters": [
                {
                    "index": 0,
                    "id": "Lagavulin",
                    "name": "Lagavulin",
                    "hp": 105,
                    "max_hp": 110,
                    "gone": false,
                    "half_dead": false
                }
            ]
        },
        "potions": [
            {
                "index": 0,
                "id": "Ancient Potion",
                "name": "Ancient Potion",
                "can_use": true,
                "requires_target": false
            },
            {
                "index": 1,
                "id": "Fire Potion",
                "name": "Fire Potion",
                "can_use": true,
                "requires_target": true
            },
            {
                "index": 2,
                "id": "Swift Potion",
                "name": "Swift Potion",
                "can_use": false,
                "requires_target": false
            }
        ],
    })
}

fn combat_summary_with_compacted_potion_slots() -> serde_json::Value {
    json!({
        "step": 14,
        "client_pid": 1234,
        "state_seq": 15,
        "state_id": "combat-potion-gap-state",
        "available_commands": ["play", "potion", "end", "state", "abandon"],
        "ready_for_command": true,
        "in_game": true,
        "screen_type": "NONE",
        "screen_name": "NONE",
        "room_phase": "COMBAT",
        "room_type": "MonsterRoomElite",
        "combat": {
            "hand": [],
            "monsters": [
                {
                    "index": 0,
                    "id": "Lagavulin",
                    "name": "Lagavulin",
                    "hp": 19,
                    "max_hp": 110,
                    "gone": false,
                    "half_dead": false
                }
            ]
        },
        "potions": [
            {
                "index": 0,
                "id": "Ancient Potion",
                "name": "Ancient Potion",
                "can_use": true,
                "requires_target": false
            },
            {
                "index": 1,
                "id": "Swift Potion",
                "name": "Swift Potion",
                "can_use": true,
                "requires_target": false
            }
        ],
    })
}

fn full_belt_reward_summary() -> serde_json::Value {
    json!({
        "step": 83,
        "client_pid": 1234,
        "state_seq": 84,
        "state_id": "full-belt-reward",
        "available_commands": ["choose", "potion", "proceed", "state", "abandon"],
        "ready_for_command": true,
        "in_game": true,
        "screen_type": "COMBAT_REWARD",
        "screen_name": "COMBAT_REWARD",
        "room_phase": "COMPLETE",
        "room_type": "MonsterRoom",
        "choices": ["potion", "card"],
        "open_potion_slots": 0,
        "potion_capacity": 3,
        "potions": [
            {"index": 0, "id": "Ancient Potion", "name": "Ancient Potion"},
            {"index": 1, "id": "Fire Potion", "name": "Fire Potion"},
            {"index": 2, "id": "Swift Potion", "name": "Swift Potion"}
        ],
    })
}

fn shop_summary() -> serde_json::Value {
    json!({
        "step": 83,
        "client_pid": 1234,
        "state_seq": 84,
        "state_id": "shop-state",
        "available_commands": ["choose", "potion", "leave", "state", "abandon"],
        "ready_for_command": true,
        "in_game": true,
        "screen_type": "SHOP_SCREEN",
        "screen_name": "SHOP",
        "room_phase": "COMPLETE",
        "room_type": "ShopRoom",
        "choices": ["purge", "hemokinesis", "disarm", "swift potion", "strength potion"],
    })
}

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sts-live-communication-{name}-{nonce}"))
}
