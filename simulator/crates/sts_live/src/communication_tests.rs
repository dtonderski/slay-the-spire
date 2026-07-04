use crate::{
    bridge::BridgeManager,
    communication::{CommunicationBridgeConfig, CommunicationModBridgeManager},
    model::{
        ActionId, BridgeId, Character, LegalActionKind, LiveError, LivePhase, RunConfig, RunSeed,
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
            "client_pid": 1234,
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
        serde_json::to_vec(&json!({
            "step": summary["step"],
            "state_seq": summary["state_seq"],
            "state_id": summary["state_id"],
            "client_pid": 1234,
            "message": {"game_state": {"screen_type": summary["screen_type"]}},
        }))
        .unwrap(),
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

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sts-live-communication-{name}-{nonce}"))
}
