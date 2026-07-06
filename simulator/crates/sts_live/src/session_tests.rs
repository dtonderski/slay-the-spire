use crate::{
    bridge::{BridgeManager, FakeBridgeManager},
    fidelity::{FidelityChecker, TraceFidelityChecker},
    model::{
        ActionId, BridgeId, BridgeStatus, Character, FidelityKind, FidelityStatus, LegalAction,
        LegalActionKind, LiveError, LivePhase, LiveResult, LiveState, RunConfig, RunSeed,
        SessionId, SessionLifecycle, TraceRecord,
    },
    session::SessionStore,
};
use serde_json::json;
use std::{cell::Cell, fs, path::PathBuf, time::SystemTime};

#[test]
fn start_run_creates_recording_session_and_trace() {
    let root = temp_dir("start");
    let mut store = fake_store(&root);
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
            },
        )
        .unwrap();

    assert_eq!(snapshot.lifecycle, SessionLifecycle::Recording);
    assert_eq!(snapshot.fidelity.kind, FidelityKind::Unknown);
    assert!(snapshot.trace_path.ends_with("session-1.jsonl"));
    assert!(std::path::Path::new(&snapshot.trace_path).exists());
    let trace = fs::read_to_string(&snapshot.trace_path).unwrap();
    assert!(trace.contains("\"type\":\"action\""));
    assert!(trace.contains("\"type\":\"response\""));
    assert!(trace.contains("\"command\":\"start_run\""));
    assert!(trace.contains("START IRONCLAD 0 CODEX04"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn sends_only_current_legal_actions() {
    let root = temp_dir("send");
    let mut store = fake_store(&root);
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
            },
        )
        .unwrap();

    let next = store
        .send_action(&snapshot.session_id, &ActionId("talk".to_owned()))
        .unwrap();
    assert_eq!(next.latest_state.as_ref().unwrap().phase, LivePhase::Combat);

    let err = store
        .send_action(&snapshot.session_id, &ActionId("missing".to_owned()))
        .unwrap_err();
    assert!(matches!(err, LiveError::InvalidAction(_)));
    fs::remove_dir_all(root).ok();
}

#[test]
fn bridge_send_error_blocks_session_and_records_trace_error() {
    let root = temp_dir("bridge-error");
    let mut store = fake_store(&root);
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
            },
        )
        .unwrap();
    store
        .kill_bridge(&BridgeId("fake-bridge-1".to_owned()))
        .unwrap();

    let err = store
        .send_action(&snapshot.session_id, &ActionId("talk".to_owned()))
        .unwrap_err();
    assert!(matches!(err, LiveError::NotFound(_)));

    let blocked = store.session_snapshot(&snapshot.session_id).unwrap();
    assert_eq!(blocked.lifecycle, SessionLifecycle::Blocked);
    assert_eq!(
        blocked.blocked.as_ref().unwrap().reason_code,
        "bridge_error"
    );
    let trace = fs::read_to_string(blocked.trace_path).unwrap();
    assert!(trace.contains("\"type\":\"error\""));
    assert!(trace.contains("\"reason_code\":\"bridge_error\""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn fidelity_loss_after_action_stays_visible_without_blocking_manual_collection() {
    let root = temp_dir("fidelity-lost");
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        FlipToLostFidelity::default(),
        &root,
    );
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
            },
        )
        .unwrap();

    let lost = store
        .send_action(&snapshot.session_id, &ActionId("talk".to_owned()))
        .unwrap();

    assert_eq!(lost.lifecycle, SessionLifecycle::FidelityLost);
    assert_eq!(lost.fidelity.kind, FidelityKind::Lost);
    assert!(lost.blocked.is_none());
    let trace = fs::read_to_string(&lost.trace_path).unwrap();
    assert!(!trace.contains("\"reason_code\":\"fidelity_lost\""));

    let continued = store
        .send_action(
            &snapshot.session_id,
            &ActionId("strike-jaw-worm".to_owned()),
        )
        .unwrap();
    assert_eq!(continued.fidelity.kind, FidelityKind::Lost);
    assert!(continued.blocked.is_none());

    fs::remove_dir_all(root).ok();
}

#[test]
fn request_state_records_operator_command() {
    let root = temp_dir("request-state");
    let mut store = fake_store(&root);
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
            },
        )
        .unwrap();

    let refreshed = store.request_state(&snapshot.session_id).unwrap();
    let trace = fs::read_to_string(refreshed.trace_path).unwrap();

    assert!(trace.contains("\"id\":\"request-state\""));
    assert!(trace.contains("\"command\":\"state\""));
    assert!(trace.contains("\"command\":\"request_state\""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn successful_request_state_unblocks_transient_bridge_error() {
    let root = temp_dir("request-state-unblocks");
    let mut store = SessionStore::new(PendingCommandBridge::default(), TraceFidelityChecker, &root);
    let snapshot = store
        .start_run(
            BridgeId("bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
            },
        )
        .unwrap();

    let err = store
        .send_action(&snapshot.session_id, &ActionId("reward-potion".to_owned()))
        .unwrap_err();
    assert!(matches!(err, LiveError::Bridge(_)));
    assert_eq!(
        store
            .session_snapshot(&snapshot.session_id)
            .unwrap()
            .lifecycle,
        SessionLifecycle::Blocked
    );

    let refreshed = store.request_state(&snapshot.session_id).unwrap();

    assert_ne!(refreshed.lifecycle, SessionLifecycle::Blocked);
    assert!(refreshed.blocked.is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn recovers_existing_sessions_from_trace_root() {
    let root = temp_dir("recover");
    {
        let mut store = fake_store(&root);
        store
            .start_run(
                BridgeId("fake-bridge-1".to_owned()),
                RunConfig {
                    character: Character::Ironclad,
                    ascension: 0,
                    seed: RunSeed::External("CODEX04".to_owned()),
                },
            )
            .unwrap();
    }

    let mut recovered_store = fake_store(&root);
    let recovered = recovered_store.recover_existing_sessions().unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].session_id.0, "session-1");
    assert!(recovered[0]
        .latest_state
        .as_ref()
        .unwrap()
        .legal_actions
        .iter()
        .any(|action| action.id.0 == "talk"));

    let next = recovered_store
        .send_action(
            &SessionId("session-1".to_owned()),
            &ActionId("talk".to_owned()),
        )
        .unwrap();
    assert_eq!(next.latest_state.as_ref().unwrap().phase, LivePhase::Combat);

    let second = recovered_store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(456),
            },
        )
        .unwrap();
    assert_eq!(second.session_id.0, "session-2");
    fs::remove_dir_all(root).ok();
}

#[test]
fn recovery_recomputes_stale_shop_actions_from_raw_summary() {
    let root = temp_dir("recover-shop-actions");
    fs::create_dir_all(&root).unwrap();
    let trace_path = root.join("session-1.jsonl");
    let stale_state = LiveState {
        sequence: 84,
        phase: LivePhase::Unknown,
        legal_actions: vec![request_state_action()],
        raw: json!({
            "status": {},
            "summary": {
                "state_seq": 84,
                "state_id": "shop-state",
                "available_commands": ["choose", "potion", "leave", "state", "abandon"],
                "ready_for_command": true,
                "in_game": true,
                "screen_type": "SHOP_SCREEN",
                "screen_name": "SHOP",
                "room_phase": "COMPLETE",
                "room_type": "ShopRoom",
                "choices": ["purge", "hemokinesis", "disarm", "swift potion", "strength potion"]
            },
            "current_state": {
                "message": {
                    "available_commands": ["choose", "potion", "leave", "state", "abandon"],
                    "ready_for_command": true,
                    "game_state": {
                        "screen_type": "SHOP_SCREEN",
                        "room_type": "ShopRoom"
                    }
                }
            }
        }),
    };
    let records = [
        TraceRecord::Metadata {
            schema: 1,
            source: "live_trace".to_owned(),
            session_id: SessionId("session-1".to_owned()),
            bridge_id: BridgeId("fake-bridge-1".to_owned()),
            run_config: None,
        },
        TraceRecord::State {
            sequence: 84,
            state: stale_state,
        },
    ];
    fs::write(
        &trace_path,
        records
            .iter()
            .map(|record| serde_json::to_string(record).unwrap())
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();

    let mut store = fake_store(&root);
    let recovered = store.recover_existing_sessions().unwrap();
    let state = recovered[0].latest_state.as_ref().unwrap();

    assert_eq!(state.phase, LivePhase::Shop);
    assert!(state.legal_actions.iter().any(|action| {
        action.id == ActionId("leave".to_owned())
            && action.kind == LegalActionKind::Confirm
            && action.label == "Leave shop"
            && action.command["command"] == "LEAVE"
            && action.command["source_state_id"] == "shop-state"
    }));
    fs::remove_dir_all(root).ok();
}

#[test]
fn recovery_does_not_restore_old_fidelity_lost_blocks() {
    let root = temp_dir("recover-fidelity-lost");
    fs::create_dir_all(&root).unwrap();
    let trace_path = root.join("session-1.jsonl");
    let run_config = RunConfig {
        character: Character::Ironclad,
        ascension: 0,
        seed: RunSeed::Numeric(123),
    };
    let records = [
        TraceRecord::Metadata {
            schema: 1,
            source: "live_trace".to_owned(),
            session_id: SessionId("session-1".to_owned()),
            bridge_id: BridgeId("fake-bridge-1".to_owned()),
            run_config: Some(run_config),
        },
        TraceRecord::Error {
            sequence: 1,
            reason_code: "fidelity_lost".to_owned(),
            message: "old verifier diff".to_owned(),
        },
    ];
    let body = records
        .iter()
        .map(|record| serde_json::to_string(record).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&trace_path, format!("{body}\n")).unwrap();

    let mut store = fake_store(&root);
    let recovered = store.recover_existing_sessions().unwrap();

    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].lifecycle, SessionLifecycle::FidelityLost);
    assert_eq!(recovered[0].fidelity.kind, FidelityKind::Lost);
    assert!(recovered[0].blocked.is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn lists_sessions_in_stable_id_order() {
    let root = temp_dir("list");
    let mut store = fake_store(&root);
    store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
            },
        )
        .unwrap();
    store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(456),
            },
        )
        .unwrap();

    let sessions = store.list_sessions();
    assert_eq!(sessions[0].session_id.0, "session-1");
    assert_eq!(sessions[1].session_id.0, "session-2");
    fs::remove_dir_all(root).ok();
}

#[test]
fn lists_recovered_sessions_in_numeric_id_order() {
    let root = temp_dir("list-recovered-numeric");
    fs::create_dir_all(&root).unwrap();
    write_metadata_trace(&root, "session-10");
    write_metadata_trace(&root, "session-2");

    let mut store = fake_store(&root);
    store.recover_existing_sessions().unwrap();

    let sessions = store.list_sessions();
    assert_eq!(sessions[0].session_id.0, "session-2");
    assert_eq!(sessions[1].session_id.0, "session-10");
    fs::remove_dir_all(root).ok();
}

#[test]
fn abandon_run_marks_trace_and_ends_session() {
    let root = temp_dir("abandon");
    let mut store = fake_store(&root);
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
            },
        )
        .unwrap();

    let abandoned = store
        .abandon_run(&snapshot.session_id, "test_abandon")
        .unwrap();

    assert_eq!(abandoned.lifecycle, SessionLifecycle::Ended);
    assert_eq!(
        abandoned.latest_state.as_ref().unwrap().phase,
        LivePhase::Menu
    );
    let trace = fs::read_to_string(abandoned.trace_path).unwrap();
    assert!(trace.contains("\"type\":\"run_abandoned\""));
    assert!(trace.contains("\"command\":\"abandon_run\""));
    assert!(trace.contains("test_abandon"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn abandon_then_start_creates_two_start_rooted_traces() {
    let root = temp_dir("abandon-start");
    let mut store = fake_store(&root);
    let first = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
            },
        )
        .unwrap();

    let abandoned = store
        .abandon_run(&first.session_id, "operator_http")
        .unwrap();
    assert_eq!(abandoned.lifecycle, SessionLifecycle::Ended);

    let second = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
            },
        )
        .unwrap();

    assert_eq!(second.session_id.0, "session-2");
    assert_eq!(second.lifecycle, SessionLifecycle::Recording);
    assert_ne!(abandoned.trace_path, second.trace_path);

    let first_trace = fs::read_to_string(abandoned.trace_path).unwrap();
    assert_eq!(first_trace.matches("START IRONCLAD").count(), 1);
    assert!(first_trace.contains("START IRONCLAD 0 123"));
    assert!(first_trace.contains("\"command\":\"abandon_run\""));

    let second_trace = fs::read_to_string(second.trace_path).unwrap();
    assert_eq!(second_trace.matches("START IRONCLAD").count(), 1);
    assert!(second_trace.contains("START IRONCLAD 0 CODEX04"));
    assert!(!second_trace.contains("live_trace_after_abandon"));
    assert!(!second_trace.contains("live_trace_restart"));
    fs::remove_dir_all(root).ok();
}

fn fake_store(root: &std::path::Path) -> SessionStore<FakeBridgeManager, TraceFidelityChecker> {
    SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        TraceFidelityChecker,
        root,
    )
}

fn write_metadata_trace(root: &std::path::Path, session_id: &str) {
    let record = TraceRecord::Metadata {
        schema: 1,
        source: "live_trace".to_owned(),
        session_id: SessionId(session_id.to_owned()),
        bridge_id: BridgeId("fake-bridge-1".to_owned()),
        run_config: None,
    };
    fs::write(
        root.join(format!("{session_id}.jsonl")),
        format!("{}\n", serde_json::to_string(&record).unwrap()),
    )
    .unwrap();
}

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sts-live-session-{name}-{nonce}"))
}

#[derive(Default)]
struct FlipToLostFidelity {
    calls: Cell<u32>,
}

impl FidelityChecker for FlipToLostFidelity {
    fn check_trace(&self, _path: &std::path::Path) -> LiveResult<FidelityStatus> {
        let calls = self.calls.get() + 1;
        self.calls.set(calls);
        if calls == 1 {
            return Ok(FidelityStatus::unknown());
        }
        Ok(FidelityStatus {
            kind: FidelityKind::Lost,
            first_divergent_step: Some(2),
            compact_diff: vec!["hp mismatch".to_owned()],
            message: Some("simulator diverged after test action".to_owned()),
        })
    }
}

struct PendingCommandBridge {
    state: LiveState,
}

impl Default for PendingCommandBridge {
    fn default() -> Self {
        Self {
            state: LiveState {
                sequence: 0,
                phase: LivePhase::Menu,
                legal_actions: vec![request_state_action()],
                raw: json!({"screen": "menu"}),
            },
        }
    }
}

impl BridgeManager for PendingCommandBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(vec![BridgeStatus {
            id: BridgeId("bridge".to_owned()),
            process_id: Some(1234),
            client_id: Some("pending-command-test".to_owned()),
            connected: true,
            last_heartbeat_ms: None,
        }])
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        self.state = LiveState {
            sequence: 1,
            phase: LivePhase::Reward,
            legal_actions: vec![
                LegalAction {
                    id: ActionId("reward-potion".to_owned()),
                    kind: LegalActionKind::ChooseReward,
                    label: "potion".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 0"}),
                    disabled_reason: None,
                },
                request_state_action(),
            ],
            raw: json!({"screen": "reward"}),
        };
        Ok(self.state.clone())
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(self.state.clone())
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.state.sequence += 1;
        Ok(self.state.clone())
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        Err(LiveError::Bridge(
            "bridge command already pending".to_owned(),
        ))
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(0)
    }
}

fn request_state_action() -> LegalAction {
    LegalAction {
        id: ActionId("request-state".to_owned()),
        kind: LegalActionKind::RequestState,
        label: "Request state".to_owned(),
        enabled: true,
        command: json!({"kind": "request_state"}),
        disabled_reason: None,
    }
}
