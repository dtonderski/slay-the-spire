use crate::{
    bridge::{BridgeManager, FakeBridgeManager},
    fidelity::{FidelityChecker, TraceFidelityChecker},
    model::{
        ActionId, AutomationConfig, AutomationPolicy, BridgeId, BridgeStatus, Character,
        FidelityKind, FidelityStatus, LegalAction, LegalActionKind, LiveError, LivePhase,
        LiveResult, LiveState, RunConfig, RunSeed, SessionId, SessionLifecycle, TraceRecord,
    },
    session::{
        combat_state_is_actionable, is_cursed_key_chest_curse_pending,
        is_hand_selection_confirm_still_pending, is_play_card_still_pending,
        is_unsettled_action_transition, persist_verified_trace, refreshed_equivalent_action,
        slaythedata_reward_binding_is_pending, slaythedata_state_is_temporarily_actionless,
        slaythedata_step_advances, trace_has_completed_shop_purge, SessionStore,
    },
    slaythedata::SlayTheDataIndex,
};
use rusqlite::Connection;
use serde_json::json;
use std::{
    cell::Cell,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    rc::Rc,
    time::SystemTime,
};

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
                profile: None,
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
fn start_run_records_communication_mod_profile_in_initial_metadata() {
    let root = temp_dir("profile-metadata");
    fs::create_dir_all(&root).unwrap();
    let mut store = SessionStore::new(ProfileStartBridge, AlwaysOkFidelity, &root);
    let snapshot = store
        .start_run(
            BridgeId("profile-bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("PROFILE01".to_owned()),
                profile: None,
            },
        )
        .unwrap();

    let profile = snapshot
        .run_config
        .as_ref()
        .and_then(|config| config.profile.as_ref())
        .expect("profile is attached to the recorded run config");
    assert_eq!(profile.note_card, "Twin Strike");
    assert_eq!(profile.note_upgrades, 1);

    let content = fs::read_to_string(&snapshot.trace_path).unwrap();
    let metadata: serde_json::Value =
        serde_json::from_str(content.lines().next().unwrap()).unwrap();
    assert_eq!(
        metadata.pointer("/run_config/profile/note_card"),
        Some(&json!("Twin Strike"))
    );
    assert_eq!(
        metadata.pointer("/run_config/profile/note_upgrades"),
        Some(&json!(1))
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn start_verification_run_records_exact_start_and_profile_metadata() {
    let root = temp_dir("verification-profile-metadata");
    fs::create_dir_all(&root).unwrap();
    let mut store = SessionStore::new(ProfileStartBridge, AlwaysOkFidelity, &root);
    let snapshot = store
        .start_verification_run(
            BridgeId("profile-bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("PROFILE01".to_owned()),
                profile: None,
            },
            10_000,
        )
        .unwrap();

    let profile = snapshot
        .run_config
        .as_ref()
        .and_then(|config| config.profile.as_ref())
        .expect("profile is attached to the recorded run config");
    assert_eq!(profile.note_card, "Twin Strike");
    assert_eq!(profile.note_upgrades, 1);

    let content = fs::read_to_string(&snapshot.trace_path).unwrap();
    let mut lines = content.lines();
    let metadata: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    let start: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    assert_eq!(
        metadata.pointer("/run_config/profile/note_card"),
        Some(&json!("Twin Strike"))
    );
    assert_eq!(
        start.pointer("/action/command/command"),
        Some(&json!("START_VERIFY IRONCLAD 0 PROFILE01 10000"))
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn stale_action_can_be_rebound_only_to_the_same_refreshed_live_action() {
    let stale = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseNeow,
        label: "talk".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0", "source_state_id": "old"}),
        disabled_reason: None,
    };
    let mut refreshed = stale.clone();
    refreshed.command = json!({"command": "CHOOSE 0", "source_state_id": "new"});
    let state = LiveState {
        sequence: 2,
        phase: LivePhase::Neow,
        legal_actions: vec![refreshed.clone()],
        raw: json!({}),
    };

    assert_eq!(
        refreshed_equivalent_action(&state, &stale),
        Some(&refreshed)
    );

    let mut changed = state;
    changed.legal_actions[0].command = json!({"command": "CHOOSE 1", "source_state_id": "new"});
    assert!(refreshed_equivalent_action(&changed, &stale).is_none());
}

#[test]
fn active_trace_can_be_snapshotted_to_permanent_corpus_without_overwrite() {
    let root = temp_dir("permanent-corpus-source");
    let permanent_root = temp_dir("permanent-corpus-destination");
    let mut store = fake_store(&root);
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();

    let destination = store
        .copy_trace_to_permanent_corpus(&snapshot.session_id, &permanent_root)
        .unwrap();

    assert_eq!(destination.file_name().unwrap(), "trace-session-1.jsonl");
    assert_eq!(
        fs::read_to_string(&destination).unwrap(),
        fs::read_to_string(&snapshot.trace_path).unwrap()
    );
    assert!(std::path::Path::new(&snapshot.trace_path).exists());
    assert_eq!(
        store
            .copy_trace_to_permanent_corpus(&snapshot.session_id, &permanent_root)
            .unwrap(),
        destination
    );

    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(permanent_root).ok();
}

#[test]
fn repeated_clean_trace_promotion_refreshes_the_stable_corpus_file() {
    let root = temp_dir("refresh-clean-promotion");
    fs::create_dir_all(&root).unwrap();
    let source = root.join("session-1.jsonl");
    let destination = root.join("trace-session-1.jsonl");
    fs::write(&source, "short trace\n").unwrap();
    fs::write(&destination, "stale corpus trace\n").unwrap();

    persist_verified_trace(&source, &destination, None).unwrap();
    assert_eq!(fs::read_to_string(&destination).unwrap(), "short trace\n");

    fs::write(&source, "longer clean trace\n").unwrap();
    persist_verified_trace(&source, &destination, None).unwrap();
    assert_eq!(
        fs::read_to_string(&destination).unwrap(),
        "longer clean trace\n"
    );

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
                profile: None,
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
                profile: None,
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
                profile: None,
            },
        )
        .unwrap();

    let updated = store
        .send_action(&snapshot.session_id, &ActionId("talk".to_owned()))
        .unwrap();

    assert_eq!(updated.lifecycle, SessionLifecycle::FidelityLost);
    assert_eq!(updated.fidelity.kind, FidelityKind::Lost);
    assert!(updated.blocked.is_none());
    let trace = fs::read_to_string(&updated.trace_path).unwrap();
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
fn refresh_fidelity_reuses_cached_result_when_trace_is_unchanged() {
    let root = temp_dir("fidelity-cache-unchanged");
    let calls = Rc::new(Cell::new(0));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        CountingBoundaryFidelity::new(calls.clone()),
        &root,
    );
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
                profile: None,
            },
        )
        .unwrap();
    let calls_after_start = calls.get();

    store.refresh_fidelity(&snapshot.session_id).unwrap();
    assert_eq!(calls.get(), calls_after_start + 1);

    store.refresh_fidelity(&snapshot.session_id).unwrap();
    assert_eq!(calls.get(), calls_after_start + 1);

    fs::remove_dir_all(root).ok();
}

#[test]
fn refresh_fidelity_reuses_unsupported_boundary_cache_after_trace_append() {
    let root = temp_dir("fidelity-cache-append");
    let calls = Rc::new(Cell::new(0));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        CountingBoundaryFidelity::new(calls.clone()),
        &root,
    );
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
                profile: None,
            },
        )
        .unwrap();
    let calls_after_start = calls.get();

    let boundary = store.refresh_fidelity(&snapshot.session_id).unwrap();
    assert_eq!(boundary.fidelity.kind, FidelityKind::Unknown);
    assert_eq!(calls.get(), calls_after_start + 1);
    assert!(boundary
        .latest_state
        .as_ref()
        .is_some_and(|state| state.raw.get("sim_run_state").is_some()));

    append_trace_record(
        &snapshot.trace_path,
        &TraceRecord::Automation {
            sequence: 999,
            event: "test-noop".to_owned(),
            details: json!({}),
        },
    );
    let refreshed = store.refresh_fidelity(&snapshot.session_id).unwrap();
    assert_eq!(refreshed.fidelity.kind, FidelityKind::Unknown);
    assert_eq!(calls.get(), calls_after_start + 1);
    assert!(refreshed
        .latest_state
        .as_ref()
        .is_some_and(|state| state.raw.get("sim_run_state").is_none()));

    append_trace_record(
        &snapshot.trace_path,
        &TraceRecord::Error {
            sequence: 1000,
            reason_code: "fidelity_lost".to_owned(),
            message: "tail fidelity lost".to_owned(),
        },
    );
    let lost = store.refresh_fidelity(&snapshot.session_id).unwrap();
    assert_eq!(lost.fidelity.kind, FidelityKind::Lost);
    assert_eq!(lost.fidelity.message.as_deref(), Some("tail fidelity lost"));
    assert_eq!(calls.get(), calls_after_start + 1);

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
                profile: None,
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
                profile: None,
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
fn send_action_waits_for_state_newer_than_action_source() {
    let root = temp_dir("send-action-fresh-state");
    let mut store = SessionStore::new(StaleActionBridge::default(), TraceFidelityChecker, &root);
    let snapshot = store
        .start_run(
            BridgeId("bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
                profile: None,
            },
        )
        .unwrap();

    let next = store
        .send_action(&snapshot.session_id, &ActionId("choose-0".to_owned()))
        .unwrap();

    let raw = &next.latest_state.as_ref().unwrap().raw;
    assert_eq!(
        raw.pointer("/summary/state_id")
            .and_then(serde_json::Value::as_str),
        Some("state-2")
    );
    assert_eq!(next.latest_state.as_ref().unwrap().sequence, 2);
    fs::remove_dir_all(root).ok();
}

#[test]
fn send_action_records_late_state_after_bridge_observation_timeout() {
    let root = temp_dir("send-action-late-observation");
    let mut store = SessionStore::new(
        LateObservedActionBridge::default(),
        TraceFidelityChecker,
        &root,
    );
    let snapshot = store
        .start_run(
            BridgeId("bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
                profile: None,
            },
        )
        .unwrap();

    let next = store
        .send_action(&snapshot.session_id, &ActionId("choose-0".to_owned()))
        .unwrap();

    assert_eq!(next.latest_state.as_ref().unwrap().sequence, 2);
    assert_ne!(next.lifecycle, SessionLifecycle::Blocked);
    let trace = fs::read_to_string(next.trace_path).unwrap();
    assert!(trace.contains("\"type\":\"action\""));
    assert!(trace.contains("\"command\":\"CHOOSE 0\""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn send_neow_talk_waits_past_fresh_but_semantically_stale_state() {
    let root = temp_dir("send-neow-talk-settled-state");
    let mut store = SessionStore::new(
        TransientNeowTalkBridge::default(),
        TraceFidelityChecker,
        &root,
    );
    let snapshot = store
        .start_run(
            BridgeId("bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
                profile: None,
            },
        )
        .unwrap();

    let next = store
        .send_action(&snapshot.session_id, &ActionId("choose-0".to_owned()))
        .unwrap();

    let state = next.latest_state.as_ref().unwrap();
    assert_eq!(state.sequence, 3);
    assert_eq!(state.legal_actions.len(), 4);
    assert_eq!(state.legal_actions[0].label, "choose a card to obtain");
    let trace = fs::read_to_string(next.trace_path).unwrap();
    assert!(!trace.contains("state-2"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn send_neow_bonus_waits_until_selected_option_disappears() {
    let root = temp_dir("send-neow-bonus-settled-state");
    let mut store = SessionStore::new(
        TransientNeowBonusBridge::default(),
        TraceFidelityChecker,
        &root,
    );
    let snapshot = store
        .start_run(
            BridgeId("bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
                profile: None,
            },
        )
        .unwrap();

    let next = store
        .send_action(&snapshot.session_id, &ActionId("choose-0".to_owned()))
        .unwrap();

    let state = next.latest_state.as_ref().unwrap();
    assert_eq!(state.sequence, 3);
    assert_eq!(state.phase, LivePhase::Reward);
    assert_eq!(state.legal_actions[0].label, "inflame");
    let trace = fs::read_to_string(next.trace_path).unwrap();
    assert!(!trace.contains("state-2"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn selected_card_reward_remaining_visible_is_an_unsettled_transition() {
    let action = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseReward,
        label: "inflame".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let state = LiveState {
        sequence: 2,
        phase: LivePhase::Reward,
        legal_actions: vec![action.clone()],
        raw: json!({
            "summary": {
                "state_id": "state-2",
                "screen_type": "CARD_REWARD",
            }
        }),
    };

    assert!(is_unsettled_action_transition(&action, &state));
}

#[test]
fn selected_map_node_remaining_visible_is_an_unsettled_transition() {
    let action = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseMapNode,
        label: "x=1".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let state = LiveState {
        sequence: 2,
        phase: LivePhase::Map,
        legal_actions: vec![action.clone()],
        raw: json!({"summary": {"state_id": "state-2", "screen_type": "MAP"}}),
    };

    assert!(is_unsettled_action_transition(&action, &state));
}

#[test]
fn selected_event_option_remaining_visible_is_an_unsettled_transition() {
    let action = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "leave".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let state = LiveState {
        sequence: 2,
        phase: LivePhase::Event,
        legal_actions: vec![action.clone()],
        raw: json!({"summary": {"state_id": "state-2", "ready_for_command": true}}),
    };

    assert!(is_unsettled_action_transition(&action, &state));
}

#[test]
fn repeatable_non_leave_event_option_can_remain_visible() {
    let action = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "deeper".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let state = LiveState {
        sequence: 2,
        phase: LivePhase::Event,
        legal_actions: vec![action.clone()],
        raw: json!({"summary": {"state_id": "state-2", "ready_for_command": true}}),
    };

    assert!(!is_unsettled_action_transition(&action, &state));
}

#[test]
fn event_leave_advancing_to_a_different_leave_command_is_settled() {
    let action = LegalAction {
        id: ActionId("choose-1".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "leave".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 1", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let confirmation = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "leave".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0", "source_state_id": "state-2"}),
        disabled_reason: None,
    };
    let state = LiveState {
        sequence: 2,
        phase: LivePhase::Event,
        legal_actions: vec![confirmation],
        raw: json!({"summary": {"state_id": "state-2", "ready_for_command": true}}),
    };

    assert!(!is_unsettled_action_transition(&action, &state));
}

#[test]
fn played_card_is_pending_while_its_source_uuid_remains_in_hand() {
    let action = LegalAction {
        id: ActionId("play-1".to_owned()),
        kind: LegalActionKind::PlayCard,
        label: "Play Defend".to_owned(),
        enabled: true,
        command: json!({"command": "PLAY 1", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let state = |state_id: &str, hand: serde_json::Value| LiveState {
        sequence: 2,
        phase: LivePhase::Combat,
        legal_actions: Vec::new(),
        raw: json!({
            "current_state": {
                "message": {
                    "game_state": {"combat_state": {"hand": hand}}
                }
            },
            "summary": {"state_id": state_id, "ready_for_command": true}
        }),
    };
    let source = state(
        "state-1",
        json!([
            {"id": "Defend_R", "uuid": "played-card"},
            {"id": "Defend_R", "uuid": "other-defend"}
        ]),
    );
    let pending = state(
        "state-2",
        json!([
            {"id": "Defend_R", "uuid": "played-card"},
            {"id": "Defend_R", "uuid": "other-defend"}
        ]),
    );
    let settled = state(
        "state-3",
        json!([{"id": "Defend_R", "uuid": "other-defend"}]),
    );

    assert!(is_play_card_still_pending(&action, &source, &pending));
    assert!(!is_play_card_still_pending(&action, &source, &settled));
}

#[test]
fn hand_selection_confirm_is_pending_until_selected_card_reaches_a_combat_pile() {
    let action = LegalAction {
        id: ActionId("confirm".to_owned()),
        kind: LegalActionKind::Confirm,
        label: "Confirm".to_owned(),
        enabled: true,
        command: json!({"command": "CONFIRM", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let source = LiveState {
        sequence: 1,
        phase: LivePhase::Combat,
        legal_actions: vec![action.clone()],
        raw: json!({
            "current_state": {"message": {"game_state": {
                "screen_type": "HAND_SELECT",
                "screen_state": {"selected": [{"id": "Defend_R", "uuid": "selected-card"}]},
                "combat_state": {
                    "hand": [],
                    "draw_pile": [],
                    "discard_pile": [],
                    "exhaust_pile": [],
                    "limbo": []
                }
            }}},
            "summary": {"state_id": "state-1", "ready_for_command": true}
        }),
    };
    let candidate = |exhaust_pile: serde_json::Value| LiveState {
        sequence: 2,
        phase: LivePhase::Combat,
        legal_actions: Vec::new(),
        raw: json!({
            "current_state": {"message": {"game_state": {"combat_state": {
                "hand": [],
                "draw_pile": [],
                "discard_pile": [],
                "exhaust_pile": exhaust_pile,
                "limbo": []
            }}}},
            "summary": {"state_id": "state-2", "ready_for_command": true}
        }),
    };
    let pending = candidate(json!([]));
    let settled = candidate(json!([{"id": "Defend_R", "uuid": "selected-card"}]));

    assert!(is_hand_selection_confirm_still_pending(
        &action, &source, &pending
    ));
    assert!(!is_hand_selection_confirm_still_pending(
        &action, &source, &settled
    ));
}

#[test]
fn smoke_bomb_combat_state_is_unsettled_until_escape_timer_finishes() {
    let action = LegalAction {
        id: ActionId("potion-0-0".to_owned()),
        kind: LegalActionKind::UsePotion,
        label: "Use Smoke Bomb -> Giant Head".to_owned(),
        enabled: true,
        command: json!({"command": "POTION USE 0 0", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let still_in_combat = LiveState {
        sequence: 2,
        phase: LivePhase::Combat,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"state_id": "state-2", "ready_for_command": true}}),
    };
    let escaped_to_reward = LiveState {
        sequence: 3,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"state_id": "state-3", "ready_for_command": true}}),
    };

    assert!(is_unsettled_action_transition(&action, &still_in_combat));
    assert!(!is_unsettled_action_transition(&action, &escaped_to_reward));
}

#[test]
fn nest_ritual_dagger_effect_is_unsettled_until_card_enters_deck() {
    let action = LegalAction {
        id: ActionId("choose-1".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "stay in line".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 1", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let state = |state_id: &str, deck: serde_json::Value| LiveState {
        sequence: 2,
        phase: LivePhase::Event,
        legal_actions: Vec::new(),
        raw: json!({
            "current_state": {
                "state_id": state_id,
                "message": {
                    "game_state": {
                        "deck": deck,
                        "screen_state": {"event_id": "Nest"}
                    }
                }
            },
            "summary": {"state_id": state_id, "ready_for_command": true}
        }),
    };
    let pending = state("state-2", json!([{"id": "Warcry"}]));
    let settled = state("state-3", json!([{"id": "Warcry"}, {"id": "RitualDagger"}]));

    assert!(is_unsettled_action_transition(&action, &pending));
    assert!(!is_unsettled_action_transition(&action, &settled));
}

#[test]
fn cursed_key_chest_is_unsettled_until_queued_curse_enters_deck() {
    let action = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::Confirm,
        label: "open".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let state = |state_id: &str, screen_type: &str, room_type: &str, deck, relics| LiveState {
        sequence: 2,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({
            "current_state": {
                "state_id": state_id,
                "message": {
                    "game_state": {
                        "deck": deck,
                        "relics": relics,
                        "room_type": room_type,
                        "screen_type": screen_type
                    }
                }
            },
            "summary": {"state_id": state_id, "ready_for_command": true}
        }),
    };
    let cursed_key = json!([{"id": "Cursed Key", "counter": -1}]);
    let source = state(
        "state-1",
        "CHEST",
        "TreasureRoom",
        json!([{"id": "Strike_R"}]),
        cursed_key.clone(),
    );
    let pending = state(
        "state-2",
        "COMBAT_REWARD",
        "TreasureRoom",
        json!([{"id": "Strike_R"}]),
        cursed_key.clone(),
    );
    let settled = state(
        "state-3",
        "COMBAT_REWARD",
        "TreasureRoom",
        json!([{"id": "Strike_R"}, {"id": "Writhe"}]),
        cursed_key,
    );

    assert!(is_cursed_key_chest_curse_pending(
        &action, &source, &pending
    ));
    assert!(!is_cursed_key_chest_curse_pending(
        &action, &source, &settled
    ));
}

#[test]
fn cursed_key_chest_wait_is_disabled_for_boss_chests_and_active_omamori() {
    let action = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::Confirm,
        label: "open".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0", "source_state_id": "state-1"}),
        disabled_reason: None,
    };
    let state = |state_id: &str, screen_type: &str, room_type: &str, relics| LiveState {
        sequence: 2,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({
            "current_state": {
                "state_id": state_id,
                "message": {"game_state": {
                    "deck": [{"id": "Strike_R"}],
                    "relics": relics,
                    "room_type": room_type,
                    "screen_type": screen_type
                }}
            }
        }),
    };
    let pending = state(
        "state-2",
        "COMBAT_REWARD",
        "TreasureRoom",
        json!([{"id": "Cursed Key", "counter": -1}]),
    );
    let boss_source = state(
        "state-1",
        "CHEST",
        "TreasureRoomBoss",
        json!([{"id": "Cursed Key", "counter": -1}]),
    );
    let omamori_source = state(
        "state-1",
        "CHEST",
        "TreasureRoom",
        json!([
            {"id": "Cursed Key", "counter": -1},
            {"id": "Omamori", "counter": 2}
        ]),
    );

    assert!(!is_cursed_key_chest_curse_pending(
        &action,
        &boss_source,
        &pending
    ));
    assert!(!is_cursed_key_chest_curse_pending(
        &action,
        &omamori_source,
        &pending
    ));
}

#[test]
fn hidden_neow_to_map_state_is_temporary_not_plan_completion() {
    let state = LiveState {
        sequence: 2,
        phase: LivePhase::Map,
        legal_actions: Vec::new(),
        raw: json!({
            "summary": {
                "room_type": "NeowRoom",
                "screen_type": "MAP",
            }
        }),
    };

    assert!(slaythedata_state_is_temporarily_actionless(&state));
}

#[test]
fn neow_room_map_with_live_nodes_is_settled() {
    let state = LiveState {
        sequence: 3,
        phase: LivePhase::Map,
        legal_actions: vec![LegalAction {
            id: ActionId("choose-0".to_owned()),
            kind: LegalActionKind::ChooseMapNode,
            label: "x=0".to_owned(),
            enabled: true,
            command: json!({"command": "CHOOSE 0"}),
            disabled_reason: None,
        }],
        raw: json!({
            "summary": {
                "room_type": "NeowRoom",
                "screen_type": "MAP",
            }
        }),
    };

    assert!(!slaythedata_state_is_temporarily_actionless(&state));
}

#[test]
fn combat_state_is_not_actionable_while_all_gameplay_actions_are_disabled() {
    let mut action = LegalAction {
        id: ActionId("play-0".to_owned()),
        kind: LegalActionKind::PlayCard,
        label: "Play Strike".to_owned(),
        enabled: false,
        command: json!({"command": "PLAY 0 0"}),
        disabled_reason: Some("game is still initializing combat".to_owned()),
    };
    let mut state = LiveState {
        sequence: 1,
        phase: LivePhase::Combat,
        legal_actions: vec![action.clone()],
        raw: json!({}),
    };

    assert!(!combat_state_is_actionable(&state));
    action.enabled = true;
    state.legal_actions = vec![action];
    assert!(combat_state_is_actionable(&state));
}

#[test]
fn combat_state_is_actionable_when_a_combat_prompt_has_an_enabled_confirm() {
    let state = LiveState {
        sequence: 1,
        phase: LivePhase::Combat,
        legal_actions: vec![LegalAction {
            id: ActionId("choose-1".to_owned()),
            kind: LegalActionKind::Confirm,
            label: "Run action".to_owned(),
            enabled: true,
            command: json!({"command": "CHOOSE 1"}),
            disabled_reason: None,
        }],
        raw: json!({"summary": {"screen_type": "GRID"}}),
    };

    assert!(combat_state_is_actionable(&state));
}

#[test]
fn send_map_action_waits_past_hidden_neow_transition_state() {
    let root = temp_dir("send-map-action-settled-state");
    let mut store = SessionStore::new(TransientMapBridge::default(), TraceFidelityChecker, &root);
    let snapshot = store
        .start_run(
            BridgeId("bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::Numeric(123),
                profile: None,
            },
        )
        .unwrap();

    let next = store
        .send_action(&snapshot.session_id, &ActionId("choose-0".to_owned()))
        .unwrap();

    assert_eq!(next.latest_state.as_ref().unwrap().phase, LivePhase::Combat);
    assert_eq!(
        next.latest_state
            .as_ref()
            .unwrap()
            .raw
            .pointer("/summary/state_id")
            .and_then(serde_json::Value::as_str),
        Some("state-3")
    );
    let trace = fs::read_to_string(next.trace_path).unwrap();
    assert!(!trace.contains("state-2"));
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
                    profile: None,
                },
            )
            .unwrap();
    }
    fs::write(
        root.join("slaythedata-collection.jsonl"),
        "{\"status\":\"blocked\",\"run_id\":123}\n",
    )
    .unwrap();

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
                profile: None,
            },
        )
        .unwrap();
    assert_eq!(second.session_id.0, "session-2");
    fs::remove_dir_all(root).ok();
}

#[test]
fn recovery_does_not_run_full_fidelity_check() {
    let root = temp_dir("recover-no-fidelity");
    {
        let mut store = fake_store(&root);
        store
            .start_run(
                BridgeId("fake-bridge-1".to_owned()),
                RunConfig {
                    character: Character::Ironclad,
                    ascension: 0,
                    seed: RunSeed::External("CODEX04".to_owned()),
                    profile: None,
                },
            )
            .unwrap();
    }

    let calls = Rc::new(Cell::new(0));
    let mut recovered_store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        CountingBoundaryFidelity::new(calls.clone()),
        &root,
    );
    let recovered = recovered_store.recover_existing_sessions().unwrap();

    assert_eq!(recovered.len(), 1);
    assert_eq!(calls.get(), 0);
    assert_eq!(recovered[0].fidelity.kind, FidelityKind::Unknown);
    assert!(recovered[0]
        .fidelity
        .message
        .as_deref()
        .is_some_and(|message| message.contains("stale")));
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
        profile: None,
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
fn recovery_treats_fidelity_recheck_as_a_new_verifier_boundary() {
    let root = temp_dir("recover-fidelity-recheck");
    fs::create_dir_all(&root).unwrap();
    let trace_path = root.join("session-1.jsonl");
    let records = [
        TraceRecord::Metadata {
            schema: 1,
            source: "live_trace".to_owned(),
            session_id: SessionId("session-1".to_owned()),
            bridge_id: BridgeId("fake-bridge-1".to_owned()),
            run_config: None,
        },
        TraceRecord::Error {
            sequence: 1,
            reason_code: "fidelity_lost".to_owned(),
            message: "old verifier diff".to_owned(),
        },
        TraceRecord::SlayTheData {
            sequence: 2,
            event: "fidelity_recheck".to_owned(),
            details: json!({"reason": "verified simulator repair"}),
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
    assert_eq!(recovered[0].fidelity.kind, FidelityKind::Unknown);
    assert_ne!(recovered[0].lifecycle, SessionLifecycle::FidelityLost);
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
                profile: None,
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
                profile: None,
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
                profile: None,
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
                profile: None,
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
                profile: None,
            },
        )
        .unwrap();

    assert_eq!(second.session_id.0, "session-2");
    assert_eq!(second.lifecycle, SessionLifecycle::Recording);
    assert_ne!(abandoned.trace_path, second.trace_path);

    let first_trace = fs::read_to_string(abandoned.trace_path).unwrap();
    assert_eq!(first_trace.matches("START IRONCLAD").count(), 1);
    assert!(first_trace.contains("START IRONCLAD 0 3I"));
    assert!(first_trace.contains("\"command\":\"abandon_run\""));

    let second_trace = fs::read_to_string(second.trace_path).unwrap();
    assert_eq!(second_trace.matches("START IRONCLAD").count(), 1);
    assert!(second_trace.contains("START IRONCLAD 0 CODEX04"));
    assert!(!second_trace.contains("live_trace_after_abandon"));
    assert!(!second_trace.contains("live_trace_restart"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_attach_advises_and_sends_next_non_combat_action() {
    let root = temp_dir("slaythedata-send-next");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();

    let attached = store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();

    let advisor = attached.slaythedata.advisor.as_ref().expect("advisor");
    assert_eq!(attached.slaythedata.attached_run.as_ref().unwrap().id, 7);
    assert_eq!(advisor.code, "legal_neow_talk");
    assert_eq!(advisor.command.as_deref(), Some("CHOOSE 0"));
    assert_eq!(advisor.action_id.as_ref().unwrap().0, "talk");

    let sent = store.slaythedata_send_next(&snapshot.session_id).unwrap();

    assert_eq!(sent.latest_state.as_ref().unwrap().phase, LivePhase::Combat);
    assert_eq!(sent.slaythedata.next_step_index, 1);
    assert!(sent.slaythedata.blocked.is_none());
    let trace = fs::read_to_string(sent.trace_path).unwrap();
    assert!(trace.contains("\"type\":\"slay_the_data\""));
    assert!(trace.contains("\"event\":\"send_action\""));
    assert!(trace.contains("\"event\":\"sent_action\""));
    assert!(trace.contains("\"id\":\"talk\""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_auto_play_hands_combat_to_the_combat_agent() {
    let root = temp_dir("slaythedata-auto-combat-agent");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();
    store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();
    let combat = store.slaythedata_send_next(&snapshot.session_id).unwrap();
    assert_eq!(
        combat.latest_state.as_ref().unwrap().phase,
        LivePhase::Combat
    );
    store
        .configure_automation(
            &snapshot.session_id,
            AutomationConfig {
                policy: AutomationPolicy::FakePlayFirstCard,
                ..AutomationConfig::default()
            },
        )
        .unwrap();

    let handed_off = store.slaythedata_auto_play(&snapshot.session_id).unwrap();

    assert_eq!(
        handed_off.latest_state.as_ref().unwrap().phase,
        LivePhase::Combat
    );
    assert_eq!(
        handed_off.automation.executed_actions.len(),
        1,
        "SlayTheData auto-play should invoke the combat agent"
    );
    assert_eq!(
        handed_off.automation.state,
        crate::model::AutomationState::Blocked
    );
    let trace = fs::read_to_string(handed_off.trace_path).unwrap();
    assert!(trace.contains("auto_play_started"));
    assert!(trace.contains("\"event\":\"sent_action\""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_background_combat_reuses_the_existing_plan() {
    let root = temp_dir("slaythedata-background-plan-reuse");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();
    store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();
    store.slaythedata_send_next(&snapshot.session_id).unwrap();
    store
        .configure_automation(
            &snapshot.session_id,
            AutomationConfig {
                policy: AutomationPolicy::FakePlayFirstCard,
                ..AutomationConfig::default()
            },
        )
        .unwrap();
    store
        .slaythedata_start_auto_play(&snapshot.session_id)
        .unwrap();

    let _ = store
        .slaythedata_auto_play_tick(&snapshot.session_id)
        .unwrap();
    let _ = store
        .slaythedata_auto_play_tick(&snapshot.session_id)
        .unwrap();

    let trace = fs::read_to_string(
        store
            .session_snapshot(&snapshot.session_id)
            .unwrap()
            .trace_path,
    )
    .unwrap();
    assert_eq!(
        trace.matches("\"event\":\"plan_ready\"").count(),
        1,
        "background SlayTheData combat should consume one plan instead of replanning each action"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_background_stops_when_combat_automation_blocks() {
    let root = temp_dir("slaythedata-background-blocked-combat");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();
    store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();
    store.slaythedata_send_next(&snapshot.session_id).unwrap();
    store
        .configure_automation(
            &snapshot.session_id,
            AutomationConfig {
                policy: AutomationPolicy::FakePlayFirstCard,
                ..AutomationConfig::default()
            },
        )
        .unwrap();
    store
        .slaythedata_start_auto_play(&snapshot.session_id)
        .unwrap();
    store
        .automation_fail_auto_play(&snapshot.session_id, "fixture planner failure")
        .unwrap();

    let (stopped, should_continue) = store
        .slaythedata_auto_play_tick(&snapshot.session_id)
        .unwrap();
    assert!(!should_continue);
    assert_eq!(
        stopped.slaythedata.blocked.as_ref().unwrap().reason_code,
        "slaythedata_combat_automation_blocked"
    );
    assert_eq!(
        stopped.automation.state,
        crate::model::AutomationState::Failed
    );
    assert_eq!(
        stopped.automation.blocked.as_ref().unwrap().reason_code,
        "automation_auto_play_failed"
    );
    let trace = fs::read_to_string(stopped.trace_path).unwrap();
    assert!(!trace.lines().any(|line| {
        line.contains("\"type\":\"automation\"") && line.contains("\"event\":\"auto_play_started\"")
    }));
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_pause_stops_background_ticks_without_losing_progress() {
    let root = temp_dir("slaythedata-pause");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();
    store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();

    let started = store
        .slaythedata_start_auto_play(&snapshot.session_id)
        .unwrap();
    assert!(!started.slaythedata.auto_play_paused);
    let paused = store.slaythedata_pause(&snapshot.session_id).unwrap();
    assert!(paused.slaythedata.auto_play_paused);

    let (after_tick, should_continue) = store
        .slaythedata_auto_play_tick(&snapshot.session_id)
        .unwrap();
    assert!(!should_continue);
    assert_eq!(after_tick.slaythedata.next_step_index, 0);
    assert!(after_tick.slaythedata.auto_play_paused);
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_game_over_reports_terminal_loss_instead_of_stale_guidance() {
    let root = temp_dir("slaythedata-game-over");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();
    store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();
    store
        .slaythedata_start_auto_play(&snapshot.session_id)
        .unwrap();
    store
        .set_latest_state_for_test(
            &snapshot.session_id,
            LiveState {
                sequence: 99,
                phase: LivePhase::GameOver,
                legal_actions: Vec::new(),
                raw: json!({"summary": {"floor": 8}}),
            },
        )
        .unwrap();

    let (after_tick, should_continue) = store
        .slaythedata_auto_play_tick(&snapshot.session_id)
        .unwrap();

    assert!(!should_continue);
    let blocked = after_tick.slaythedata.blocked.unwrap();
    assert_eq!(blocked.reason_code, "game_over_before_target");
    assert!(blocked.message.contains("game over"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_reattach_restores_sent_progress_from_trace() {
    let root = temp_dir("slaythedata-reattach-progress");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();
    store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();
    let sent = store.slaythedata_send_next(&snapshot.session_id).unwrap();
    assert_eq!(sent.slaythedata.next_step_index, 1);
    append_trace_record(
        &sent.trace_path,
        &TraceRecord::SlayTheData {
            sequence: 99,
            event: "sent_action".to_owned(),
            details: json!({
                "attached_run": {"id": 7},
                "next_step_index": 3
            }),
        },
    );

    let reattached = store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();

    assert_eq!(reattached.slaythedata.next_step_index, 3);
    assert_eq!(
        reattached.slaythedata.last_message.as_deref(),
        Some("SlayTheData progress restored from recorded guidance; simulator state unchanged")
    );
    assert_ne!(
        reattached
            .slaythedata
            .advisor
            .as_ref()
            .map(|advisor| advisor.code.as_str()),
        Some("legal_neow_talk")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn recovered_cli_session_reattaches_guidance_from_records_without_hydrating_simulator_state() {
    let root = temp_dir("slaythedata-recovered-cli-attachment");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let session_id = {
        let mut store = SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            AlwaysOkFidelity,
            &root,
        )
        .with_slaythedata_index(SlayTheDataIndex::new(&db));
        let snapshot = store
            .start_run(
                BridgeId("fake-bridge-1".to_owned()),
                RunConfig {
                    character: Character::Ironclad,
                    ascension: 0,
                    seed: RunSeed::External("CODEX04".to_owned()),
                    profile: None,
                },
            )
            .unwrap();
        store
            .attach_slaythedata_run(&snapshot.session_id, 7)
            .unwrap();
        snapshot.session_id
    };

    let mut recovered = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    recovered.recover_existing_sessions().unwrap();
    assert_eq!(
        recovered.attached_slaythedata_run_id(&session_id).unwrap(),
        Some(7)
    );

    let restored = recovered
        .ensure_slaythedata_attachment(&session_id)
        .unwrap()
        .expect("recorded attachment");
    assert_eq!(restored.slaythedata.attached_run.unwrap().id, 7);
    assert!(restored
        .latest_state
        .as_ref()
        .and_then(|state| state.raw.get("sim_run_state"))
        .is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_blocks_guided_send_when_fidelity_is_not_ok() {
    let root = temp_dir("slaythedata-fidelity-block");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        TraceFidelityChecker,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();
    store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();

    let blocked = store.slaythedata_send_next(&snapshot.session_id).unwrap();

    assert_eq!(
        blocked.slaythedata.blocked.as_ref().unwrap().reason_code,
        "slaythedata_fidelity_not_ok"
    );
    assert_eq!(
        blocked.latest_state.as_ref().unwrap().phase,
        LivePhase::Neow
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_rechecks_same_command_on_wrong_live_phase_without_sending() {
    let root = temp_dir("slaythedata-wrong-phase");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(PendingCommandBridge::default(), AlwaysOkFidelity, &root)
        .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("bridge".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();

    let attached = store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();
    assert_eq!(
        attached
            .slaythedata
            .advisor
            .as_ref()
            .unwrap()
            .command
            .as_deref(),
        Some("CHOOSE 0")
    );
    assert!(attached
        .slaythedata
        .advisor
        .as_ref()
        .unwrap()
        .action_id
        .is_none());

    let blocked = store.slaythedata_send_next(&snapshot.session_id).unwrap();
    assert_eq!(
        blocked.slaythedata.blocked.as_ref().unwrap().reason_code,
        "slaythedata_action_mismatch"
    );
    assert_eq!(
        blocked.latest_state.as_ref().unwrap().phase,
        LivePhase::Reward
    );

    let still_blocked = store.slaythedata_send_next(&snapshot.session_id).unwrap();
    assert_eq!(still_blocked.slaythedata.next_step_index, 0);
    assert_eq!(
        still_blocked
            .slaythedata
            .blocked
            .as_ref()
            .unwrap()
            .reason_code,
        "slaythedata_action_mismatch"
    );
    let trace = fs::read_to_string(still_blocked.trace_path).unwrap();
    assert_eq!(trace.matches("\"event\":\"blocked\"").count(), 2);
    assert!(!trace.contains("\"event\":\"sent_action\""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_bridge_send_failure_records_slaythedata_block() {
    let root = temp_dir("slaythedata-bridge-failure");
    let db = root.join("slaythedata.sqlite3");
    write_slaythedata_db(&db, slaythedata_raw_run_json("CODEX04"));
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        &root,
    )
    .with_slaythedata_index(SlayTheDataIndex::new(&db));
    let snapshot = store
        .start_run(
            BridgeId("fake-bridge-1".to_owned()),
            RunConfig {
                character: Character::Ironclad,
                ascension: 0,
                seed: RunSeed::External("CODEX04".to_owned()),
                profile: None,
            },
        )
        .unwrap();
    store
        .attach_slaythedata_run(&snapshot.session_id, 7)
        .unwrap();
    store
        .kill_bridge(&BridgeId("fake-bridge-1".to_owned()))
        .unwrap();

    let blocked = store.slaythedata_send_next(&snapshot.session_id).unwrap();

    assert_eq!(blocked.lifecycle, SessionLifecycle::Blocked);
    assert_eq!(
        blocked.slaythedata.blocked.as_ref().unwrap().reason_code,
        "slaythedata_send_failed"
    );
    let trace = fs::read_to_string(blocked.trace_path).unwrap();
    assert!(trace.contains("\"event\":\"send_action\""));
    assert!(trace.contains("\"event\":\"blocked\""));
    assert!(trace.contains("slaythedata_send_failed"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn slaythedata_pending_room_event_choices_do_not_advance_step() {
    let leave_event = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "leave".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };
    let outrun_event = LegalAction {
        id: ActionId("choose-1".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "outrun".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 1"}),
        disabled_reason: None,
    };
    let map_choice = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseMapNode,
        label: "x=0".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };

    assert!(!slaythedata_step_advances(
        "pending_room_resolution",
        &leave_event,
        None
    ));
    assert!(!slaythedata_step_advances(
        "pending_room_resolution",
        &outrun_event,
        None
    ));
    assert!(slaythedata_step_advances(
        "pending_room_resolution",
        &map_choice,
        None
    ));
}

#[test]
fn slaythedata_shop_entry_does_not_advance_purchase_or_purge_step() {
    let shop_entry = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::Confirm,
        label: "shop".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };
    let purchase = LegalAction {
        id: ActionId("choose-1".to_owned()),
        kind: LegalActionKind::ShopBuy,
        label: "Whirlwind - 112 gold".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 1"}),
        disabled_reason: None,
    };

    assert!(!slaythedata_step_advances(
        "guided_shop_purchase",
        &shop_entry,
        None
    ));
    assert!(!slaythedata_step_advances(
        "guided_shop_purge",
        &shop_entry,
        None
    ));
    assert!(slaythedata_step_advances(
        "guided_shop_purchase",
        &purchase,
        None
    ));
}

#[test]
fn completed_shop_purge_requires_confirmed_purge_grid_and_return_to_same_shop() {
    let purge_grid = LiveState {
        sequence: 10,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({
            "summary": {"floor": 5, "screen_type": "GRID"},
            "current_state": {"message": {"game_state": {
                "floor": 5,
                "screen_state": {"for_purge": true}
            }}}
        }),
    };
    let confirm = LegalAction {
        id: ActionId("confirm".to_owned()),
        kind: LegalActionKind::Confirm,
        label: "Confirm".to_owned(),
        enabled: true,
        command: json!({"command": "CONFIRM"}),
        disabled_reason: None,
    };
    let returned_shop = LiveState {
        sequence: 11,
        phase: LivePhase::Shop,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"floor": 5, "screen_type": "SHOP_SCREEN"}}),
    };
    let records = vec![
        TraceRecord::State {
            sequence: 10,
            state: purge_grid.clone(),
        },
        TraceRecord::Action {
            sequence: 10,
            action: confirm.clone(),
        },
        TraceRecord::State {
            sequence: 11,
            state: returned_shop,
        },
    ];

    assert!(trace_has_completed_shop_purge(&records, 5));
    assert!(!trace_has_completed_shop_purge(&records, 6));
    assert!(!trace_has_completed_shop_purge(
        &[
            TraceRecord::State {
                sequence: 10,
                state: purge_grid,
            },
            TraceRecord::Action {
                sequence: 10,
                action: confirm,
            },
        ],
        5
    ));
}

#[test]
fn slaythedata_shop_room_proceed_does_not_advance_room_step() {
    let proceed = LegalAction {
        id: ActionId("proceed".to_owned()),
        kind: LegalActionKind::Confirm,
        label: "Proceed".to_owned(),
        enabled: true,
        command: json!({"command": "PROCEED"}),
        disabled_reason: None,
    };

    assert!(!slaythedata_step_advances(
        "pending_room_resolution",
        &proceed,
        None
    ));
}

#[test]
fn slaythedata_neow_followup_reward_does_not_advance_leave_step() {
    let reward_choice = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseReward,
        label: "shrug it off".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };

    assert!(!slaythedata_step_advances(
        "pending_neow_followup",
        &reward_choice,
        None
    ));
}

#[test]
fn slaythedata_card_reward_opening_grid_does_not_advance_step() {
    let card_reward = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseReward,
        label: "card".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };
    let grid_state = LiveState {
        sequence: 10,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"screen_type": "CARD_REWARD"}}),
    };

    assert!(!slaythedata_step_advances(
        "pending_card_reward",
        &card_reward,
        Some(&grid_state)
    ));
}

#[test]
fn slaythedata_pending_card_reward_retries_after_reward_screen_opens() {
    let reward_state = LiveState {
        sequence: 10,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"screen_type": "COMBAT_REWARD"}}),
    };

    assert!(slaythedata_reward_binding_is_pending(
        &reward_state,
        Some("pending_card_reward"),
        false
    ));
    assert!(!slaythedata_reward_binding_is_pending(
        &reward_state,
        Some("guided_card_reward"),
        false
    ));
    assert!(!slaythedata_reward_binding_is_pending(
        &reward_state,
        Some("pending_card_reward"),
        true
    ));
}

#[test]
fn slaythedata_prerequisite_grid_confirm_does_not_advance_card_reward_step() {
    let confirm = LegalAction {
        id: ActionId("confirm".to_owned()),
        kind: LegalActionKind::Confirm,
        label: "Confirm".to_owned(),
        enabled: true,
        command: json!({"command": "CONFIRM"}),
        disabled_reason: None,
    };

    assert!(!slaythedata_step_advances(
        "guided_card_reward",
        &confirm,
        None
    ));
}

#[test]
fn slaythedata_guided_card_reward_opening_does_not_advance_step() {
    let card_reward = LegalAction {
        id: ActionId("choose-card".to_owned()),
        kind: LegalActionKind::ChooseReward,
        label: "card".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };
    let card_screen = LiveState {
        sequence: 10,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"screen_type": "CARD_REWARD"}}),
    };

    assert!(!slaythedata_step_advances(
        "guided_card_reward",
        &card_reward,
        Some(&card_screen)
    ));
}

#[test]
fn slaythedata_event_choice_opening_grid_does_not_advance_step() {
    let event_choice = LegalAction {
        id: ActionId("choose-1".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "purify".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 1"}),
        disabled_reason: None,
    };
    let grid_state = LiveState {
        sequence: 10,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"screen_type": "GRID"}}),
    };

    assert!(!slaythedata_step_advances(
        "guided_event_choice",
        &event_choice,
        Some(&grid_state)
    ));
}

#[test]
fn slaythedata_campfire_smith_opening_grid_does_not_advance_step() {
    let smith = LegalAction {
        id: ActionId("choose-1".to_owned()),
        kind: LegalActionKind::RestSite,
        label: "smith".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 1"}),
        disabled_reason: None,
    };
    let grid_state = LiveState {
        sequence: 10,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"screen_type": "GRID"}}),
    };

    assert!(!slaythedata_step_advances(
        "guided_campfire",
        &smith,
        Some(&grid_state)
    ));
}

#[test]
fn slaythedata_event_choice_opening_continue_does_not_advance_step() {
    let event_choice = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "pray".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };
    let continue_state = LiveState {
        sequence: 10,
        phase: LivePhase::Event,
        legal_actions: vec![LegalAction {
            id: ActionId("choose-0".to_owned()),
            kind: LegalActionKind::EventChoice,
            label: "continue".to_owned(),
            enabled: true,
            command: json!({"command": "CHOOSE 0"}),
            disabled_reason: None,
        }],
        raw: json!({}),
    };

    assert!(!slaythedata_step_advances(
        "guided_event_choice",
        &event_choice,
        Some(&continue_state)
    ));
}

#[test]
fn slaythedata_event_choice_opening_single_leave_does_not_advance_step() {
    let event_choice = LegalAction {
        id: ActionId("choose-2".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "leave".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 2"}),
        disabled_reason: None,
    };
    let leave_state = LiveState {
        sequence: 10,
        phase: LivePhase::Event,
        legal_actions: vec![LegalAction {
            id: ActionId("choose-0".to_owned()),
            kind: LegalActionKind::EventChoice,
            label: "leave".to_owned(),
            enabled: true,
            command: json!({"command": "CHOOSE 0"}),
            disabled_reason: None,
        }],
        raw: json!({}),
    };

    assert!(!slaythedata_step_advances(
        "guided_event_choice",
        &event_choice,
        Some(&leave_state)
    ));
}

#[test]
fn slaythedata_big_fish_box_followup_leave_advances_step() {
    let event_choice = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::EventChoice,
        label: "leave".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };
    let leave_state = LiveState {
        sequence: 10,
        phase: LivePhase::Event,
        legal_actions: vec![event_choice.clone()],
        raw: json!({
            "summary": {
                "screen_type": "EVENT",
                "screen_state": {"event_name": "Big Fish"}
            }
        }),
    };

    assert!(slaythedata_step_advances(
        "guided_event_choice",
        &event_choice,
        Some(&leave_state)
    ));
}

#[test]
fn slaythedata_event_grid_card_selection_does_not_advance_step_until_confirm() {
    let grid_choice = LegalAction {
        id: ActionId("choose-0".to_owned()),
        kind: LegalActionKind::ChooseReward,
        label: "strike".to_owned(),
        enabled: true,
        command: json!({"command": "CHOOSE 0"}),
        disabled_reason: None,
    };
    let grid_state = LiveState {
        sequence: 11,
        phase: LivePhase::Reward,
        legal_actions: Vec::new(),
        raw: json!({"summary": {"screen_type": "GRID", "screen_state": {"confirm_up": true}}}),
    };

    assert!(!slaythedata_step_advances(
        "guided_event_choice",
        &grid_choice,
        Some(&grid_state)
    ));
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

fn append_trace_record(path: &str, record: &TraceRecord) {
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    writeln!(file, "{}", serde_json::to_string(record).unwrap()).unwrap();
}

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sts-live-session-{name}-{nonce}"))
}

struct CountingBoundaryFidelity {
    calls: Rc<Cell<u32>>,
}

impl CountingBoundaryFidelity {
    fn new(calls: Rc<Cell<u32>>) -> Self {
        Self { calls }
    }
}

impl FidelityChecker for CountingBoundaryFidelity {
    fn check_trace(&self, path: &std::path::Path) -> LiveResult<FidelityStatus> {
        self.check_trace_with_sim_state(path)
            .map(|(status, _)| status)
    }

    fn check_trace_with_sim_state(
        &self,
        _path: &std::path::Path,
    ) -> LiveResult<(FidelityStatus, Option<sts_core::RunState>)> {
        self.calls.set(self.calls.get() + 1);
        Ok((
            FidelityStatus {
                kind: FidelityKind::Unknown,
                first_divergent_step: None,
                compact_diff: vec!["unsupported verifier boundary".to_owned()],
                message: Some(
                    "seed-start replay reached boundary unsupported_event_card_grid_rng_divergence: test"
                        .to_owned(),
                ),
            },
            Some(sts_core::RunState::map_fixture()),
        ))
    }
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

struct AlwaysOkFidelity;

impl FidelityChecker for AlwaysOkFidelity {
    fn check_trace(&self, _path: &std::path::Path) -> LiveResult<FidelityStatus> {
        Ok(FidelityStatus {
            kind: FidelityKind::Ok,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: None,
        })
    }
}

struct ProfileStartBridge;

impl BridgeManager for ProfileStartBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(Vec::new())
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        Ok(LiveState {
            sequence: 1,
            phase: LivePhase::Neow,
            legal_actions: vec![request_state_action()],
            raw: json!({
                "current_state": {
                    "message": {
                        "game_state": {
                            "seed": 123,
                            "profile": {
                                "note_card": "Twin Strike",
                                "note_upgrades": 1
                            }
                        }
                    }
                }
            }),
        })
    }

    fn start_verification_run(
        &mut self,
        bridge_id: &BridgeId,
        config: &RunConfig,
        _starting_hp: i32,
    ) -> LiveResult<LiveState> {
        self.start_run(bridge_id, config)
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Err(LiveError::Bridge("not used by profile test".to_owned()))
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Err(LiveError::Bridge("not used by profile test".to_owned()))
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        Err(LiveError::Bridge("not used by profile test".to_owned()))
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(0)
    }
}

fn write_slaythedata_db(path: &std::path::Path, raw_run_json: String) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE runs (
            id INTEGER PRIMARY KEY,
            character_chosen TEXT,
            ascension_level INTEGER,
            floor_reached INTEGER,
            is_daily INTEGER,
            is_endless INTEGER,
            is_trial INTEGER,
            unsupported_any INTEGER,
            seed_played TEXT,
            build_version TEXT,
            victory INTEGER,
            path_length INTEGER,
            card_choice_count INTEGER,
            event_choice_count INTEGER,
            shop_purchase_count INTEGER,
            potion_usage_count INTEGER,
            neow_bonus TEXT,
            neow_cost TEXT
        );
        CREATE TABLE chunk_runs (run_id INTEGER PRIMARY KEY);
        CREATE TABLE run_materialized_json (
            run_id INTEGER PRIMARY KEY,
            raw_event_json TEXT NOT NULL
        );
        "#,
    )
    .unwrap();
    conn.execute(
        "INSERT INTO runs VALUES (7, 'IRONCLAD', 0, 1, 0, 0, 0, 0, 'CODEX04', '2020-07-30', 0, 1, 0, 0, 0, 0, 'TEN_PERCENT_HP_BONUS', 'NONE')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO chunk_runs VALUES (7)", [])
        .unwrap();
    conn.execute(
        "INSERT INTO run_materialized_json VALUES (7, ?)",
        [&raw_run_json],
    )
    .unwrap();
}

fn slaythedata_raw_run_json(seed: &str) -> String {
    json!({
        "character_chosen": "IRONCLAD",
        "ascension_level": 0,
        "seed_played": seed,
        "build_version": "2022-12-18",
        "neow_bonus": "TEN_PERCENT_HP_BONUS",
        "neow_cost": "NONE",
        "path_taken": [],
        "path_per_floor": [],
        "floor_reached": 1,
        "victory": false
    })
    .to_string()
}

#[derive(Default)]
struct StaleActionBridge {
    state: Option<LiveState>,
}

impl StaleActionBridge {
    fn state_with_id(state_id: &str, sequence: u64, actions: Vec<LegalAction>) -> LiveState {
        LiveState {
            sequence,
            phase: LivePhase::Event,
            legal_actions: actions,
            raw: json!({
                "summary": {
                    "state_id": state_id,
                    "state_seq": sequence,
                    "available_commands": ["choose", "state"],
                    "in_game": true,
                    "screen_type": "EVENT",
                },
                "current_state": {
                    "state_id": state_id,
                    "state_seq": sequence,
                }
            }),
        }
    }

    fn choice_action() -> LegalAction {
        LegalAction {
            id: ActionId("choose-0".to_owned()),
            kind: LegalActionKind::EventChoice,
            label: "Choose".to_owned(),
            enabled: true,
            command: json!({
                "transport": "communication_mod",
                "command": "CHOOSE 0",
                "source_state_id": "state-1",
            }),
            disabled_reason: None,
        }
    }
}

impl BridgeManager for StaleActionBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(vec![BridgeStatus {
            id: BridgeId("bridge".to_owned()),
            process_id: Some(1234),
            client_id: Some("stale-action-test".to_owned()),
            connected: true,
            last_heartbeat_ms: None,
        }])
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        let state = Self::state_with_id("state-1", 1, vec![Self::choice_action()]);
        self.state = Some(state.clone());
        Ok(state)
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        let state = Self::state_with_id("state-2", 2, vec![request_state_action()]);
        self.state = Some(state.clone());
        Ok(state)
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(0)
    }
}

#[derive(Default)]
struct LateObservedActionBridge {
    state: Option<LiveState>,
}

impl BridgeManager for LateObservedActionBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(Vec::new())
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        let state = StaleActionBridge::state_with_id(
            "state-1",
            1,
            vec![StaleActionBridge::choice_action()],
        );
        self.state = Some(state.clone());
        Ok(state)
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        self.state = Some(StaleActionBridge::state_with_id(
            "state-2",
            2,
            vec![request_state_action()],
        ));
        Err(LiveError::Bridge(
            "timed out waiting for observed state update".to_owned(),
        ))
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(0)
    }
}

#[derive(Default)]
struct TransientNeowTalkBridge {
    state: Option<LiveState>,
    requests: u8,
}

impl TransientNeowTalkBridge {
    fn state(state_id: &str, sequence: u64, labels: &[&str]) -> LiveState {
        let actions = labels
            .iter()
            .enumerate()
            .map(|(index, label)| LegalAction {
                id: ActionId(format!("choose-{index}")),
                kind: LegalActionKind::ChooseNeow,
                label: (*label).to_owned(),
                enabled: true,
                command: json!({
                    "command": format!("CHOOSE {index}"),
                    "source_state_id": state_id,
                }),
                disabled_reason: None,
            })
            .collect();
        LiveState {
            sequence,
            phase: LivePhase::Neow,
            legal_actions: actions,
            raw: json!({
                "summary": {
                    "state_id": state_id,
                    "state_seq": sequence,
                    "screen_type": "EVENT",
                    "room_type": "NeowRoom",
                }
            }),
        }
    }
}

impl BridgeManager for TransientNeowTalkBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(Vec::new())
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        let state = Self::state("state-1", 1, &["talk"]);
        self.state = Some(state.clone());
        Ok(state)
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.requests += 1;
        let state = if self.requests == 1 {
            Self::state("state-2", 2, &["talk"])
        } else {
            Self::state(
                "state-3",
                3,
                &[
                    "choose a card to obtain",
                    "obtain 3 random potions",
                    "lose 8 max hp choose a rare colorless card to obtain",
                    "lose your starting relic obtain a random boss relic",
                ],
            )
        };
        self.state = Some(state.clone());
        Ok(state)
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(0)
    }
}

#[derive(Default)]
struct TransientNeowBonusBridge {
    state: Option<LiveState>,
    requests: u8,
}

impl TransientNeowBonusBridge {
    fn bonus_state(state_id: &str, sequence: u64) -> LiveState {
        TransientNeowTalkBridge::state(
            state_id,
            sequence,
            &[
                "choose a card to obtain",
                "obtain 3 random potions",
                "lose 8 max hp choose a rare colorless card to obtain",
                "lose your starting relic obtain a random boss relic",
            ],
        )
    }

    fn reward_state() -> LiveState {
        LiveState {
            sequence: 3,
            phase: LivePhase::Reward,
            legal_actions: ["inflame", "flex", "warcry"]
                .into_iter()
                .enumerate()
                .map(|(index, label)| LegalAction {
                    id: ActionId(format!("choose-{index}")),
                    kind: LegalActionKind::ChooseReward,
                    label: label.to_owned(),
                    enabled: true,
                    command: json!({
                        "command": format!("CHOOSE {index}"),
                        "source_state_id": "state-3",
                    }),
                    disabled_reason: None,
                })
                .collect(),
            raw: json!({
                "summary": {
                    "state_id": "state-3",
                    "state_seq": 3,
                    "screen_type": "CARD_REWARD",
                    "room_type": "NeowRoom",
                }
            }),
        }
    }
}

impl BridgeManager for TransientNeowBonusBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(Vec::new())
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        let state = Self::bonus_state("state-1", 1);
        self.state = Some(state.clone());
        Ok(state)
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.requests += 1;
        let state = if self.requests == 1 {
            Self::bonus_state("state-2", 2)
        } else {
            Self::reward_state()
        };
        self.state = Some(state.clone());
        Ok(state)
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(0)
    }
}

#[derive(Default)]
struct TransientMapBridge {
    state: Option<LiveState>,
    requests: u8,
}

impl TransientMapBridge {
    fn map_state(state_id: &str, sequence: u64) -> LiveState {
        LiveState {
            sequence,
            phase: LivePhase::Map,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::ChooseMapNode,
                label: "x=1".to_owned(),
                enabled: true,
                command: json!({
                    "command": "CHOOSE 0",
                    "source_state_id": "state-1"
                }),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {
                    "state_id": state_id,
                    "state_seq": sequence,
                    "screen_type": "MAP",
                    "is_screen_up": true
                }
            }),
        }
    }

    fn hidden_neow_state() -> LiveState {
        LiveState {
            sequence: 2,
            phase: LivePhase::Neow,
            legal_actions: Vec::new(),
            raw: json!({
                "summary": {
                    "state_id": "state-2",
                    "state_seq": 2,
                    "screen_type": "EVENT",
                    "room_type": "NeowRoom",
                    "is_screen_up": false
                }
            }),
        }
    }

    fn combat_state() -> LiveState {
        LiveState {
            sequence: 3,
            phase: LivePhase::Combat,
            legal_actions: vec![request_state_action()],
            raw: json!({
                "summary": {
                    "state_id": "state-3",
                    "state_seq": 3,
                    "screen_type": "COMBAT"
                }
            }),
        }
    }
}

impl BridgeManager for TransientMapBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(vec![BridgeStatus {
            id: BridgeId("bridge".to_owned()),
            process_id: Some(1234),
            client_id: Some("transient-map-test".to_owned()),
            connected: true,
            last_heartbeat_ms: None,
        }])
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        let state = Self::map_state("state-1", 1);
        self.state = Some(state.clone());
        Ok(state)
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.requests += 1;
        let state = Self::combat_state();
        self.state = Some(state.clone());
        Ok(state)
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        let state = Self::hidden_neow_state();
        self.state = Some(state.clone());
        Ok(state)
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(0)
    }
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
                    // Keep a same-command action visible on the wrong phase
                    // without triggering SlayTheData's automatic potion pickup.
                    label: "card".to_owned(),
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
