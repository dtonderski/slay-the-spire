use crate::{
    bridge::FakeBridgeManager,
    fidelity::TraceFidelityChecker,
    http::{LiveHttpApp, UI_INDEX_HTML, UI_MAIN_TS, UI_STYLES_CSS},
    session::SessionStore,
};
use serde_json::json;
use std::{fs, time::SystemTime};

#[test]
fn static_ui_exposes_stage_1_manual_controls() {
    for id in [
        "bridge",
        "session",
        "load-session",
        "character",
        "ascension",
        "seed",
        "start",
        "request",
        "abandon",
        "kill-selected",
        "kill-all",
        "actions",
        "notifications",
        "trace",
        "fidelity",
    ] {
        assert!(
            UI_INDEX_HTML.contains(&format!("id=\"{id}\"")),
            "missing #{id}"
        );
    }

    for text in [
        "Open Trace",
        "Recorded run",
        "Start New Run",
        "Running Game",
        "Bridge Manager",
    ] {
        assert!(UI_INDEX_HTML.contains(text), "missing UI text {text:?}");
    }

    assert!(!UI_INDEX_HTML.contains("Restart trace"));
    assert!(!UI_INDEX_HTML.contains("id=\"attach\""));

    assert!(
        !UI_INDEX_HTML.contains(">Attach<"),
        "primary connect action should not use backend jargon"
    );

    assert!(UI_STYLES_CSS.contains("max-width: 1760px"));
    assert!(UI_STYLES_CSS.contains("@media (min-width: 1350px)"));
    assert!(UI_STYLES_CSS.contains(".control-group:not(:first-child)"));
    assert!(UI_STYLES_CSS.contains("border-left: 1px solid #34404b"));
}

#[test]
fn static_ui_sends_backend_action_ids() {
    assert!(UI_MAIN_TS.contains("/actions/${action.id}"));
    assert!(!UI_MAIN_TS.contains("/actions/${action.label}"));
    assert!(UI_MAIN_TS.contains("api<SessionsResponse>(\"/sessions\")"));
    assert!(UI_MAIN_TS.contains("/sessions/${sessionId}"));
    assert!(!UI_MAIN_TS.contains("/sessions/restart"));
    assert!(!UI_MAIN_TS.contains("/sessions/attach"));
    assert!(!UI_MAIN_TS.contains("session_id?:"));
    assert!(UI_MAIN_TS.contains("payload.error?.message"));
    assert!(UI_MAIN_TS.contains("/bridges/${bridgeId}/kill"));
    assert!(UI_MAIN_TS.contains("/bridges/kill-all"));
    assert!(UI_MAIN_TS.contains("bridgeStatuses"));
    assert!(UI_MAIN_TS.contains("(kill only)"));
    assert!(UI_MAIN_TS.contains("(not connected)"));
    assert!(UI_MAIN_TS.contains("selectedBridge"));
    assert!(UI_MAIN_TS.contains("No active bridges"));
    assert!(UI_MAIN_TS.contains("BRIDGE_REFRESH_MS"));
    assert!(UI_MAIN_TS.contains("setInterval"));
    assert!(UI_MAIN_TS.contains("GROUP_LABELS"));
    assert!(UI_MAIN_TS.contains("dataset.groupKey"));
    assert!(UI_MAIN_TS.contains("notifyError"));
    assert!(UI_MAIN_TS.contains("const sessionId = currentSession.session_id"));
    assert!(UI_MAIN_TS.contains("currentSession?.session_id === sessionId"));
    assert!(!UI_MAIN_TS.contains("catch(alert)"));
    assert!(!UI_MAIN_TS.contains("alert("));
}

#[test]
fn http_actions_contract_returns_typed_legal_actions() {
    let root = temp_dir("actions-contract");
    let app = LiveHttpApp::new(SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        TraceFidelityChecker,
        &root,
    ));
    app.handle(
        "POST",
        "/sessions/start",
        &json!({
            "bridge_id": "fake-bridge-1",
            "config": {
                "character": "ironclad",
                "ascension": 0,
                "seed": {"external": "CODEX04"}
            }
        })
        .to_string(),
    )
    .unwrap();

    let actions = app
        .handle("GET", "/sessions/session-1/actions", "")
        .unwrap()["legal_actions"]
        .as_array()
        .unwrap()
        .clone();
    let talk = actions
        .iter()
        .find(|action| action["id"] == "talk")
        .expect("fake neow action should be addressable by id");

    assert_eq!(talk["kind"], "choose_neow");
    assert_eq!(talk["enabled"], true);
    assert!(talk["label"].as_str().unwrap().contains("Talk"));
    assert!(talk["command"].is_object());
    fs::remove_dir_all(root).ok();
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sts-live-ui-{name}-{nonce}"))
}
