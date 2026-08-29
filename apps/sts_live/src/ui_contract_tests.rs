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
        "add-to-permanent-corpus",
        "character",
        "ascension",
        "seed",
        "start",
        "request",
        "abandon",
        "kill-selected",
        "kill-all",
        "automation-policy",
        "automation-depth",
        "automation-width",
        "automation-limit",
        "automation-potions",
        "automation-plan",
        "automation-run-one",
        "automation-auto-play",
        "automation-pause",
        "automation-resume",
        "automation-cancel",
        "automation-summary",
        "slaythedata-run-id",
        "slaythedata-ascension",
        "slaythedata-min-floor",
        "slaythedata-outcome",
        "slaythedata-neow-bonus",
        "slaythedata-corpus-runs",
        "slaythedata-limit",
        "slaythedata-search",
        "slaythedata-send-next",
        "slaythedata-auto-play",
        "slaythedata-pause",
        "slaythedata-skip-shop",
        "slaythedata-results",
        "slaythedata-advisor",
        "actions",
        "command-status",
        "session-alert",
        "notifications",
        "confirm-dialog",
        "confirm-dialog-message",
        "lifecycle",
        "trace",
        "fidelity",
        "reason",
        "first-divergent",
        "state-freshness",
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
        "Combat Agent",
        "SlayTheData",
        "Session Health",
    ] {
        assert!(UI_INDEX_HTML.contains(text), "missing UI text {text:?}");
    }

    assert!(!UI_INDEX_HTML.contains("Restart trace"));
    assert!(!UI_INDEX_HTML.contains("id=\"attach\""));

    assert!(
        !UI_INDEX_HTML.contains(">Attach<"),
        "primary connect action should not use backend jargon"
    );

    assert!(UI_STYLES_CSS.contains("max-width: none"));
    assert!(UI_STYLES_CSS.contains(".workspace"));
    assert!(UI_STYLES_CSS.contains(".empty-state"));
    assert!(UI_STYLES_CSS.contains(".command-status"));
    assert!(UI_STYLES_CSS.contains(".fidelity-chip"));
    assert!(UI_STYLES_CSS.contains("minmax(500px, 1fr) minmax(330px, 390px)"));
    assert!(UI_INDEX_HTML.contains("class=\"workspace\""));
    assert!(UI_INDEX_HTML.contains("class=\"health-card\""));
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
    assert!(UI_MAIN_TS.contains("blocked?: BlockedState"));
    assert!(UI_MAIN_TS.contains("first_divergent_step?: number"));
    assert!(UI_MAIN_TS.contains("renderFidelityChip"));
    assert!(UI_MAIN_TS.contains("fidelityClass"));
    assert!(UI_MAIN_TS.contains("fidelity-ok"));
    assert!(UI_MAIN_TS.contains("fidelity-lost"));
    assert!(UI_MAIN_TS.contains("fidelity-unverified"));
    assert!(UI_MAIN_TS.contains("window.confirm"));
    assert!(UI_MAIN_TS.contains("stateFreshness"));
    assert!(UI_MAIN_TS.contains("sessionStatusLabel"));
    assert!(UI_MAIN_TS.contains("renderSessionAlert"));
    assert!(UI_MAIN_TS.contains("No legal actions were reported"));
    assert!(UI_MAIN_TS.contains("healthReason"));
    assert!(UI_MAIN_TS.contains("/bridges/${bridgeId}/kill"));
    assert!(UI_MAIN_TS.contains("/bridges/kill-all"));
    assert!(UI_MAIN_TS.contains("/automation/configure"));
    assert!(UI_MAIN_TS.contains("/automation/${command}"));
    assert!(UI_MAIN_TS.contains("automationCommand(\"plan\")"));
    assert!(UI_MAIN_TS.contains("automationCommand(\"run-one\")"));
    assert!(UI_MAIN_TS.contains("automationAutoPlay"));
    assert!(UI_MAIN_TS.contains("/automation/auto-play"));
    assert!(UI_MAIN_TS.contains("/slaythedata/search"));
    assert!(UI_MAIN_TS.contains("/slaythedata/attach"));
    assert!(UI_MAIN_TS.contains("/slaythedata/${command}"));
    assert!(UI_MAIN_TS.contains("renderSlayTheData"));
    assert!(UI_MAIN_TS.contains("renderSlayTheDataResults"));
    assert!(UI_MAIN_TS.contains("neow_bonus"));
    assert!(UI_MAIN_TS.contains("run_id: runId === \"\" ? null : Number(runId)"));
    assert!(UI_MAIN_TS.contains("SlayTheDataSessionSnapshot"));
    assert!(UI_MAIN_TS.contains("not materialized"));
    assert!(UI_MAIN_TS.contains("runSummary.materialized"));
    assert!(UI_MAIN_TS.contains("automationControlCommand(\"cancel\")"));
    assert!(UI_MAIN_TS.contains("planned-action"));
    assert!(UI_MAIN_TS.contains("allowed_potion_slots"));
    assert!(UI_MAIN_TS.contains("played_actions"));
    assert!(UI_MAIN_TS.contains("executed_actions"));
    assert!(UI_MAIN_TS.contains("renderPlanList"));
    assert!(UI_MAIN_TS.contains("planScrollKey"));
    assert!(UI_MAIN_TS.contains("previousPlanScrollTop"));
    assert!(UI_MAIN_TS.contains("PendingCommand"));
    assert!(UI_MAIN_TS.contains("setPendingCommand"));
    assert!(UI_MAIN_TS.contains("clearPendingCommand"));
    assert!(UI_MAIN_TS.contains("actionBlockedByPendingCommand"));
    assert!(UI_MAIN_TS.contains("COMMAND_PENDING_TIMEOUT_MS"));
    assert!(UI_MAIN_TS.contains("Still waiting for the game state"));
    assert!(UI_MAIN_TS.contains("automationDrafts"));
    assert!(UI_MAIN_TS.contains("rememberAutomationDraft"));
    assert!(UI_MAIN_TS.contains("renderAutomationSummary"));
    assert!(UI_MAIN_TS.contains("currentPlannedActionId"));
    assert!(UI_MAIN_TS.contains("bridgeStatuses"));
    assert!(UI_MAIN_TS.contains("(kill only)"));
    assert!(UI_MAIN_TS.contains("(not connected)"));
    assert!(UI_MAIN_TS.contains("selectedBridge"));
    assert!(UI_MAIN_TS.contains("No active bridges"));
    assert!(UI_MAIN_TS.contains("BRIDGE_REFRESH_MS"));
    assert!(UI_MAIN_TS.contains("setInterval"));
    assert!(UI_MAIN_TS.contains("GROUP_LABELS"));
    assert!(UI_MAIN_TS.contains("dataset.groupKey"));
    assert!(UI_MAIN_TS.contains("compareSessionsNewestFirst"));
    assert!(UI_MAIN_TS.contains("sessionNumber"));
    assert!(UI_MAIN_TS.contains("loadLatestSessionOnStartup"));
    assert!(UI_MAIN_TS.contains(".then(loadLatestSessionOnStartup)"));
    assert!(UI_MAIN_TS.contains("notifyError"));
    assert!(UI_MAIN_TS.contains("confirmAction"));
    assert!(UI_INDEX_HTML.contains("confirm-dialog"));
    assert!(UI_MAIN_TS.contains("lastSlayTheDataRuns = [runSummary]"));
    assert!(UI_MAIN_TS
        .contains("run(() => runSlayTheDataTask(() => startAndAttachSlayTheDataRun(runSummary)))"));
    assert!(UI_MAIN_TS
        .contains("run(() => runSlayTheDataTask(() => attachSlayTheDataRun(runSummary)))"));
    assert!(UI_MAIN_TS.contains("const sessionId = currentSession.session_id"));
    assert!(UI_MAIN_TS.contains("currentSession?.session_id === sessionId"));
    assert!(!UI_MAIN_TS.contains("catch(alert)"));
    assert!(!UI_MAIN_TS.contains("alert("));
}

#[test]
fn static_ui_exposes_stage_2_combat_agent_controls() {
    assert!(UI_STYLES_CSS.contains(".automation-card"));
    assert!(UI_STYLES_CSS.contains(".automation-buttons-row"));
    assert!(UI_STYLES_CSS.contains(".potion-slots"));
    assert!(UI_STYLES_CSS.contains(".actions button.planned-action"));
    assert!(UI_STYLES_CSS.contains(".actions button.sending-action"));
    assert!(UI_STYLES_CSS.contains(".plan-list"));
    assert!(UI_STYLES_CSS.contains(".plan-step"));
    assert!(UI_STYLES_CSS.contains(".played-plan-step"));
    assert!(UI_STYLES_CSS.contains("max-height: 178px"));
    assert!(UI_STYLES_CSS.contains("overflow-y: auto"));
    assert!(!UI_STYLES_CSS.contains(".plan-arrow"));
    assert!(!UI_STYLES_CSS.contains("overflow-x: auto"));
    assert!(UI_INDEX_HTML.contains("Beam search"));
    assert!(UI_INDEX_HTML.contains("Greedy search"));
    assert!(UI_INDEX_HTML.contains("Fake first card"));
    assert!(UI_STYLES_CSS.contains(".slaythedata-card"));
    assert!(UI_STYLES_CSS.contains(".slaythedata-results"));
    assert!(UI_STYLES_CSS.contains(".slaythedata-advisor"));
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
