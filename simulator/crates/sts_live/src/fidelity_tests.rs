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
fn checker_waits_for_action_before_claiming_replay_ok() {
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
    assert!(status.message.unwrap().contains("waiting"));
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
    assert!(status.message.unwrap().contains("recorded live action"));
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
    assert_eq!(status.kind, FidelityKind::Unknown);
    assert!(status.message.unwrap().contains("recorded live action"));
    assert!(status.compact_diff.is_empty());
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
    assert_eq!(status.kind, FidelityKind::Ok);
    assert_eq!(
        status.message.as_deref(),
        Some("observed-state replay matched supported transitions")
    );
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
    assert_eq!(status.kind, FidelityKind::Ok);
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
    assert_eq!(
        status.message.as_deref(),
        Some("observed-state replay matched supported transitions")
    );
    fs::remove_file(path).ok();
}

#[test]
fn checker_does_not_surface_neow_choose_as_observed_state_unsupported() {
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
