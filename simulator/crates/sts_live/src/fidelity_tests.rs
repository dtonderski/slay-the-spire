use crate::{
    fidelity::{FidelityChecker, TraceFidelityChecker},
    model::{
        ActionId, BridgeId, Character, FidelityKind, LegalAction, LegalActionKind, LivePhase,
        LiveState, RunConfig, RunSeed, SessionId, TraceRecord,
    },
    trace_writer::TraceWriter,
};
use serde_json::json;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn checker_does_not_report_checkpoint_specific_error_for_non_start_trace() {
    let path = temp_trace_path("non-start");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::Metadata {
            schema: 1,
            source: "test".to_owned(),
            session_id: SessionId("s".to_owned()),
            bridge_id: BridgeId("b".to_owned()),
            run_config: None,
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(status.kind, FidelityKind::Unknown);
    assert_ne!(
        status.message.as_deref(),
        Some("trace starts from an attached checkpoint")
    );
    fs::remove_file(path).ok();
}

#[test]
fn checker_requires_seed_start_before_claiming_replay_ok() {
    let path = temp_trace_path("wait");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 0,
            state: live_state(json!({
                "ready_for_command": true,
                "available_commands": ["state"],
                "game_state": {"screen_type": "COMBAT"}
            })),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(status.kind, FidelityKind::Unknown);
    assert!(status.message.unwrap().contains("strict seed-start replay"));
    fs::remove_file(path).ok();
}

#[test]
fn checker_waits_for_verifiable_transition_before_claiming_replay_ok() {
    let path = temp_trace_path("wait-transition");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 0,
            action: LegalAction {
                id: ActionId("start-run".to_owned()),
                kind: LegalActionKind::StartRun,
                label: "Start run".to_owned(),
                enabled: true,
                command: json!({"command": "START IRONCLAD 0 CODEX04"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 1,
            state: live_state(combat_message()),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(status.kind, FidelityKind::Unknown);
    assert!(status.message.unwrap().contains("strict seed-start replay"));
    fs::remove_file(path).ok();
}

#[test]
fn checker_uses_seed_start_for_complete_start_traces() {
    let path = temp_trace_path("seed-start");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::Metadata {
            schema: 1,
            source: "test".to_owned(),
            session_id: SessionId("s".to_owned()),
            bridge_id: BridgeId("b".to_owned()),
            run_config: Some(RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
            }),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 0,
            action: LegalAction {
                id: ActionId("start-run".to_owned()),
                kind: LegalActionKind::StartRun,
                label: "Start run".to_owned(),
                enabled: true,
                command: json!({"command": "START IRONCLAD 0 CODEX04"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 1,
            state: live_state(neow_bootstrap_message()),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(
        status.kind,
        FidelityKind::Ok,
        "unexpected fidelity status: {:?}",
        status
    );
    assert!(status.message.unwrap().contains("matched"));
    assert!(status.compact_diff.is_empty());
    fs::remove_file(path).ok();
}

#[test]
fn checker_returns_simulator_state_for_supported_live_trace() {
    let corpus_path =
        sts_verify::corpus_path("communication_mod/trace-2026-07-06T16-59-52-285Z.jsonl");
    if !corpus_path.exists() {
        return;
    }

    let path = temp_trace_path("sim-state");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::Metadata {
            schema: 1,
            source: "test".to_owned(),
            session_id: SessionId("s".to_owned()),
            bridge_id: BridgeId("b".to_owned()),
            run_config: Some(RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX10".to_owned()),
            }),
        })
        .unwrap();

    let content = fs::read_to_string(corpus_path).unwrap();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let sequence = value
            .get("step")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("state") => {
                writer
                    .append(&TraceRecord::State {
                        sequence,
                        state: live_state(
                            value
                                .get("message")
                                .cloned()
                                .expect("state trace line has message"),
                        ),
                    })
                    .unwrap();
            }
            Some("action") => {
                let command = value
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                    .expect("action trace line has command");
                writer
                    .append(&TraceRecord::Action {
                        sequence,
                        action: LegalAction {
                            id: ActionId(format!("action-{sequence}")),
                            kind: LegalActionKind::RequestState,
                            label: command.to_owned(),
                            enabled: true,
                            command: json!({ "command": command }),
                            disabled_reason: None,
                        },
                    })
                    .unwrap();
            }
            _ => {}
        }
    }

    let (status, sim_run_state) = TraceFidelityChecker
        .check_trace_with_sim_state(&path)
        .unwrap();
    assert_eq!(
        status.kind,
        FidelityKind::Ok,
        "unexpected fidelity status: {:?}",
        status
    );
    let sim_run_state = sim_run_state.expect("strict replay should return simulator run state");
    assert!(
        matches!(
            sim_run_state.phase,
            sts_core::RunPhase::Idle | sts_core::RunPhase::Combat | sts_core::RunPhase::Reward
        ),
        "strict replay returned an unexpected run phase: {:?}",
        sim_run_state.phase
    );

    fs::remove_file(path).ok();
}

#[test]
fn checker_preserves_action_timer_when_pairing_settled_live_states() {
    let corpus_path = sts_verify::corpus_path(
        "fidelity_regressions/session-31-floor1-stale-combat-post-state.jsonl",
    );
    assert!(
        corpus_path.exists(),
        "session 31 action-settling regression must remain in the corpus"
    );

    let path = temp_trace_path("settled-action-timer");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::Metadata {
            schema: 1,
            source: "test".to_owned(),
            session_id: SessionId("s".to_owned()),
            bridge_id: BridgeId("b".to_owned()),
            run_config: Some(RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("354Y73L428LQ8".to_owned()),
            }),
        })
        .unwrap();

    let content = fs::read_to_string(corpus_path).unwrap();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let sequence = value
            .get("step")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("state") => {
                writer
                    .append(&TraceRecord::State {
                        sequence,
                        state: live_state(
                            value
                                .get("message")
                                .cloned()
                                .expect("state trace line has message"),
                        ),
                    })
                    .unwrap();
            }
            Some("action") => {
                let command = value
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                    .expect("action trace line has command");
                let mut action_command = json!({ "command": command });
                if let Some(playtime_seconds) = value.get("playtime_seconds") {
                    action_command["playtime_seconds"] = playtime_seconds.clone();
                }
                writer
                    .append(&TraceRecord::Action {
                        sequence,
                        action: LegalAction {
                            id: ActionId(format!("action-{sequence}")),
                            kind: LegalActionKind::RequestState,
                            label: command.to_owned(),
                            enabled: true,
                            command: action_command,
                            disabled_reason: None,
                        },
                    })
                    .unwrap();
            }
            _ => {}
        }
    }

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(
        status.kind,
        FidelityKind::Ok,
        "in-process fidelity must agree with direct strict replay: {status:?}"
    );
    fs::remove_file(path).ok();
}

#[test]
fn checker_treats_operator_abandon_as_intentional_trace_end() {
    let path = temp_trace_path("abandon");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 0,
            state: live_state(combat_message()),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 1,
            action: LegalAction {
                id: ActionId("request-state".to_owned()),
                kind: LegalActionKind::RequestState,
                label: "Request state".to_owned(),
                enabled: true,
                command: json!({"command": "state"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 2,
            state: live_state(combat_message()),
        })
        .unwrap();
    writer
        .append(&TraceRecord::RunAbandoned {
            sequence: 3,
            reason: "operator_http".to_owned(),
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 4,
            state: live_state(json!({
                "ready_for_command": true,
                "available_commands": ["start", "state"],
                "game_state": {"screen_type": "MENU"}
            })),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(status.kind, FidelityKind::Unknown);
    assert!(status
        .message
        .as_deref()
        .is_some_and(|message| message.contains("strict seed-start replay")));
    assert!(status.compact_diff.is_empty());
    fs::remove_file(path).ok();
}

#[test]
fn checker_marks_supported_observed_trace_ok() {
    let path = temp_trace_path("verified-ok");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 0,
            state: live_state(combat_message()),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 1,
            action: LegalAction {
                id: ActionId("request-state".to_owned()),
                kind: LegalActionKind::RequestState,
                label: "Request state".to_owned(),
                enabled: true,
                command: json!({"command": "state"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 2,
            state: live_state(combat_message()),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(status.kind, FidelityKind::Unknown);
    fs::remove_file(path).ok();
}

#[test]
fn checker_ignores_start_before_first_state_when_waiting_for_seed_boundary() {
    let path = temp_trace_path("start-before-state");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::Metadata {
            schema: 1,
            source: "test".to_owned(),
            session_id: SessionId("s".to_owned()),
            bridge_id: BridgeId("b".to_owned()),
            run_config: Some(RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
            }),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 0,
            action: LegalAction {
                id: ActionId("start-run".to_owned()),
                kind: LegalActionKind::StartRun,
                label: "Start run".to_owned(),
                enabled: true,
                command: json!({"command": "START IRONCLAD 0 CODEX04"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 1,
            state: live_state(neow_bootstrap_message()),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 2,
            action: LegalAction {
                id: ActionId("request-state".to_owned()),
                kind: LegalActionKind::RequestState,
                label: "Request state".to_owned(),
                enabled: true,
                command: json!({"command": "state"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 3,
            state: live_state(neow_bootstrap_message()),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(status.kind, FidelityKind::Ok);
    assert!(status.message.unwrap().contains("matched"));
    fs::remove_file(path).ok();
}

#[test]
fn checker_requires_seed_start_for_neow_choose_without_run_config() {
    let path = temp_trace_path("neow-choose-unsupported");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 0,
            state: live_state(neow_bootstrap_message()),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 1,
            action: LegalAction {
                id: ActionId("choose-neow".to_owned()),
                kind: LegalActionKind::ChooseNeow,
                label: "Choose Neow reward".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 2,
            state: live_state(neow_bootstrap_message()),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_ne!(status.kind, FidelityKind::Lost);
    assert!(!status.message.unwrap().contains("unsupported"));
    fs::remove_file(path).ok();
}

#[test]
fn checker_requires_seed_start_for_shop_entry_without_run_config() {
    let path = temp_trace_path("shop-entry-unsupported");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 42,
            state: live_state(shop_message("SHOP_ROOM", ["shop"])),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 43,
            action: LegalAction {
                id: ActionId("shop".to_owned()),
                kind: LegalActionKind::ShopBuy,
                label: "shop".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 44,
            state: live_state(shop_message(
                "SHOP_SCREEN",
                ["offering", "wild strike", "purge", "leave"],
            )),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(status.kind, FidelityKind::Unknown);
    assert!(!status.message.unwrap().contains("unsupported"));
    fs::remove_file(path).ok();
}

#[test]
fn checker_requires_seed_start_for_shop_grid_without_run_config() {
    let path = temp_trace_path("shop-grid-unsupported");
    let mut writer = TraceWriter::create_new(&path).unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 46,
            state: live_state(shop_message("GRID", ["strike"])),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 47,
            action: LegalAction {
                id: ActionId("confirm".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Confirm".to_owned(),
                enabled: true,
                command: json!({"command": "CONFIRM"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 48,
            state: live_state(shop_message(
                "SHOP_SCREEN",
                ["hemokinesis", "purge", "leave"],
            )),
        })
        .unwrap();
    writer
        .append(&TraceRecord::Action {
            sequence: 49,
            action: LegalAction {
                id: ActionId("leave".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Leave shop".to_owned(),
                enabled: true,
                command: json!({"command": "LEAVE"}),
                disabled_reason: None,
            },
        })
        .unwrap();
    writer
        .append(&TraceRecord::State {
            sequence: 50,
            state: live_state(shop_message("SHOP_ROOM", ["shop"])),
        })
        .unwrap();

    let status = TraceFidelityChecker.check_trace(&path).unwrap();
    assert_eq!(status.kind, FidelityKind::Unknown);
    assert!(!status.message.unwrap().contains("unsupported"));
    fs::remove_file(path).ok();
}

fn live_state(message: serde_json::Value) -> LiveState {
    LiveState {
        sequence: 0,
        phase: LivePhase::Combat,
        legal_actions: Vec::new(),
        raw: json!({"current_state": {"message": message}}),
    }
}

fn combat_message() -> serde_json::Value {
    json!({
        "ready_for_command": true,
        "available_commands": ["state"],
        "game_state": {
            "screen_type": "COMBAT",
            "combat_state": {
                "player": {"current_hp": 80, "block": 0, "energy": 3},
                "hand": [],
                "monsters": [],
                "draw_pile": [],
                "discard_pile": [],
                "exhaust_pile": []
            }
        }
    })
}

fn shop_message<const N: usize>(screen_type: &str, choices: [&str; N]) -> serde_json::Value {
    let choices = choices.to_vec();
    json!({
        "ready_for_command": true,
        "available_commands": ["choose", "state"],
        "game_state": {
            "screen_type": screen_type,
            "room_type": "ShopRoom",
            "choice_list": choices,
            "screen_state": {}
        }
    })
}

fn neow_bootstrap_message() -> serde_json::Value {
    json!({
        "ready_for_command": true,
        "available_commands": ["choose 0"],
        "game_state": {
            "screen_type": "EVENT",
            "ascension_level": 0,
            "floor": 0,
            "gold": 99,
            "current_hp": 80,
            "max_hp": 80,
            "deck": [
                {"id": "Strike_R"},
                {"id": "Strike_R"},
                {"id": "Strike_R"},
                {"id": "Strike_R"},
                {"id": "Strike_R"},
                {"id": "Defend_R"},
                {"id": "Defend_R"},
                {"id": "Defend_R"},
                {"id": "Defend_R"},
                {"id": "Bash"}
            ],
            "relics": [{"name": "Burning Blood"}],
            "choice_list": ["talk"]
        }
    })
}

fn temp_trace_path(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sts-live-fidelity-{name}-{nonce}.jsonl"))
}
