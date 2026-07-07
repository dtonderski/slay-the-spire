use crate::{
    bridge::{BridgeManager, FakeBridgeManager},
    cli::run_cli,
    fidelity::FidelityChecker,
    http::LiveHttpApp,
    model::{
        ActionId, AutomationState, BridgeId, BridgeStatus, Character, FidelityKind, FidelityStatus,
        LegalAction, LegalActionKind, LiveError, LivePhase, LiveResult, LiveState, RunConfig,
        RunSeed,
    },
    session::SessionStore,
};
use serde_json::json;
use std::{cell::Cell, fs, path::PathBuf, rc::Rc, time::SystemTime};
use sts_core::{
    apply_combat_action_on_run, content::cards::get_card_definition, legal_combat_actions,
    CombatAction, ContentId, RunPhase, RunState,
};

#[test]
fn automation_step_sends_one_fake_card_when_fidelity_is_ok() {
    let root = temp_dir("step-ok");
    let mut store = ok_fake_store(&root);
    let started = store
        .start_run(BridgeId("fake-bridge-1".to_owned()), run_config())
        .unwrap();
    let combat = store
        .send_action(&started.session_id, &ActionId("talk".to_owned()))
        .unwrap();
    configure_fake_policy(&mut store, &started.session_id);
    assert_eq!(
        combat.latest_state.as_ref().unwrap().phase,
        LivePhase::Combat
    );

    let stepped = store.automation_step(&started.session_id).unwrap();

    assert_eq!(stepped.automation.state, AutomationState::Done);
    assert!(stepped.automation.planned_action.is_none());
    assert_eq!(stepped.automation.plan.as_ref().unwrap().played_actions, 1);
    assert_eq!(
        stepped.latest_state.as_ref().unwrap().raw["last_action"]["kind"],
        "play_card"
    );
    let trace = fs::read_to_string(stepped.trace_path).unwrap();
    assert!(trace.contains("request-state"));
    assert!(trace.contains("strike-jaw-worm"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn automation_blocks_when_fidelity_is_not_ok() {
    let root = temp_dir("fidelity-not-ok");
    let mut store = SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysUnknownFidelity,
        &root,
    );
    let started = store
        .start_run(BridgeId("fake-bridge-1".to_owned()), run_config())
        .unwrap();
    store
        .send_action(&started.session_id, &ActionId("talk".to_owned()))
        .unwrap();
    configure_fake_policy(&mut store, &started.session_id);

    let stepped = store.automation_step(&started.session_id).unwrap();

    assert_eq!(stepped.automation.state, AutomationState::Blocked);
    assert_eq!(
        stepped.automation.blocked.unwrap().reason_code,
        "automation_fidelity_not_ok"
    );
    assert_eq!(stepped.lifecycle, crate::model::SessionLifecycle::Recording);
    fs::remove_dir_all(root).ok();
}

#[test]
fn automation_blocks_ambiguous_card_matches() {
    let root = temp_dir("ambiguous");
    let mut store = SessionStore::new(AmbiguousCardBridge::default(), AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();
    configure_fake_policy(&mut store, &started.session_id);

    let stepped = store.automation_step(&started.session_id).unwrap();

    assert_eq!(stepped.automation.state, AutomationState::Blocked);
    assert_eq!(
        stepped.automation.blocked.unwrap().reason_code,
        "automation_ambiguous_action"
    );
    assert!(stepped
        .latest_state
        .as_ref()
        .unwrap()
        .legal_actions
        .iter()
        .any(|action| action.id.0 == "strike-a"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn automation_send_ready_blocks_stale_plans() {
    let root = temp_dir("stale");
    let mut store = ok_fake_store(&root);
    let started = store
        .start_run(BridgeId("fake-bridge-1".to_owned()), run_config())
        .unwrap();
    store
        .send_action(&started.session_id, &ActionId("talk".to_owned()))
        .unwrap();
    configure_fake_policy(&mut store, &started.session_id);
    let planned = store.automation_plan(&started.session_id).unwrap();
    assert_eq!(planned.automation.state, AutomationState::ReadyToSend);
    store.request_state(&started.session_id).unwrap();

    let sent = store.automation_send_ready(&started.session_id).unwrap();

    assert_eq!(sent.automation.state, AutomationState::Blocked);
    assert_eq!(
        sent.automation.blocked.unwrap().reason_code,
        "automation_stale_state"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn automation_send_ready_blocks_desynced_actions() {
    let root = temp_dir("desynced");
    let mut store = SessionStore::new(DesyncSameSequenceBridge, AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();
    configure_fake_policy(&mut store, &started.session_id);
    let planned = store.automation_plan(&started.session_id).unwrap();
    assert_eq!(planned.automation.state, AutomationState::ReadyToSend);
    store.request_state(&started.session_id).unwrap();

    let sent = store.automation_send_ready(&started.session_id).unwrap();

    assert_eq!(sent.automation.state, AutomationState::Blocked);
    assert_eq!(
        sent.automation.blocked.unwrap().reason_code,
        "automation_desynced_action"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn cli_exposes_automation_status_and_step() {
    let root = temp_dir("cli");
    let mut store = ok_fake_store(&root);
    run_cli(
        &mut store,
        strings(["sessions", "start", "--seed", "123", "--ascension", "0"]),
    )
    .unwrap();
    run_cli(
        &mut store,
        strings(["actions", "send", "session-1", "talk"]),
    )
    .unwrap();
    run_cli(
        &mut store,
        strings([
            "automation",
            "configure",
            "session-1",
            "--policy",
            "fake_play_first_card",
        ]),
    )
    .unwrap();

    let status = run_cli(&mut store, strings(["automation", "status", "session-1"])).unwrap();
    assert_eq!(status["state"], "idle");

    let stepped = run_cli(&mut store, strings(["automation", "step", "session-1"])).unwrap();
    assert_eq!(stepped["automation"]["state"], "done");
    assert_eq!(
        stepped["latest_state"]["raw"]["last_action"]["kind"],
        "play_card"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn http_exposes_automation_status_and_step() {
    let root = temp_dir("http");
    let app = LiveHttpApp::new(ok_fake_store(&root));
    app.handle(
        "POST",
        "/sessions/start",
        r#"{"bridge_id":"fake-bridge-1","config":{"character":"ironclad","ascension":0,"seed":{"numeric":123}}}"#,
    )
    .unwrap();
    app.handle("POST", "/sessions/session-1/actions/talk", "{}")
        .unwrap();
    app.handle(
        "POST",
        "/sessions/session-1/automation/configure",
        r#"{"policy":"fake_play_first_card","depth":12,"width":24,"allowed_potion_slots":[],"auto_action_limit":80}"#,
    )
    .unwrap();

    let status = app
        .handle("GET", "/sessions/session-1/automation", "")
        .unwrap();
    assert_eq!(status["state"], "idle");

    let stepped = app
        .handle("POST", "/sessions/session-1/automation/step", "{}")
        .unwrap();
    assert_eq!(stepped["automation"]["state"], "done");
    assert_eq!(
        stepped["latest_state"]["raw"]["last_action"]["kind"],
        "play_card"
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn real_planner_maps_next_action_and_records_plan_trace() {
    let root = temp_dir("real-plan");
    let mut store = SessionStore::new(SimCombatBridge::default(), AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();

    let planned = store.automation_plan(&started.session_id).unwrap();

    assert_eq!(planned.automation.state, AutomationState::ReadyToSend);
    let action = planned.automation.planned_action.as_ref().unwrap();
    assert!(
        action.command.as_deref().unwrap().starts_with("PLAY")
            || action.command == Some("END".to_owned())
    );
    let plan = planned.automation.plan.as_ref().unwrap();
    assert_eq!(plan.played_actions, 0);
    assert!(plan.nodes > 1);
    assert!(!plan.actions.iter().any(|action| action
        .label
        .contains("future action cannot be mapped from the current live state")));
    assert!(!plan
        .actions
        .iter()
        .any(|action| action.label.contains("ContentId(")));
    if let Some(future) = plan.actions.get(1) {
        assert_eq!(future.action_id.0, "future");
        assert!(future.command.is_some());
    }
    let live_actions = planned.latest_state.as_ref().unwrap().legal_actions.clone();
    assert_eq!(
        live_actions
            .iter()
            .filter(|live| live.id == action.action_id)
            .count(),
        1
    );
    let trace = fs::read_to_string(planned.trace_path).unwrap();
    assert!(trace.contains("\"event\":\"plan_ready\""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn manual_matching_action_advances_ready_plan() {
    let root = temp_dir("manual-advance");
    let mut store = SessionStore::new(SimCombatBridge::default(), AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();
    let planned = store.automation_plan(&started.session_id).unwrap();
    let planned_action = planned.automation.planned_action.as_ref().unwrap().clone();

    let sent = store
        .send_action(&started.session_id, &planned_action.action_id)
        .unwrap();

    let plan = sent.automation.plan.as_ref().unwrap();
    assert_eq!(plan.played_actions, 1);
    let next_action = sent
        .automation
        .planned_action
        .as_ref()
        .expect("next plan step should bind to a live legal action");
    assert_ne!(next_action.action_id, planned_action.action_id);
    assert_ne!(next_action.action_id.0, "future");
    assert_eq!(plan.actions[1].action_id, next_action.action_id);
    assert_eq!(plan.actions[1].label, next_action.label);
    assert_eq!(
        sent.automation.last_message.as_deref(),
        Some("manual action matched plan")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn manual_different_action_clears_ready_plan() {
    let root = temp_dir("manual-clear");
    let mut store = SessionStore::new(SimCombatBridge::default(), AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();
    let planned = store.automation_plan(&started.session_id).unwrap();
    let planned_action = planned.automation.planned_action.as_ref().unwrap();
    let different_action = planned
        .latest_state
        .as_ref()
        .unwrap()
        .legal_actions
        .iter()
        .find(|action| action.id != planned_action.action_id)
        .expect("fixture should expose a non-planned legal action")
        .id
        .clone();

    let sent = store
        .send_action(&started.session_id, &different_action)
        .unwrap();

    assert!(sent.automation.plan.is_none());
    assert!(sent.automation.planned_action.is_none());
    assert_eq!(
        sent.automation.last_message.as_deref(),
        Some("manual action differed from plan; plan cleared")
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn real_planner_maps_live_card_indices_instead_of_simulator_positions() {
    let root = temp_dir("real-plan-live-indices");
    let mut bridge = SimCombatBridge::lethal_fixture();
    bridge.live_index_offset = 1;
    let mut store = SessionStore::new(bridge, AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();

    let planned = store.automation_plan(&started.session_id).unwrap();

    assert_eq!(planned.automation.state, AutomationState::ReadyToSend);
    let action = planned.automation.planned_action.as_ref().unwrap();
    let command = action.command.as_deref().unwrap();
    assert!(command.starts_with("PLAY "));
    assert!(!command.starts_with("PLAY 0"));
    let live = planned
        .latest_state
        .as_ref()
        .unwrap()
        .legal_actions
        .iter()
        .find(|live| live.id == action.action_id)
        .unwrap();
    assert_eq!(live.command["command"], command);
    fs::remove_dir_all(root).ok();
}

#[test]
fn real_planner_auto_plays_current_combat_under_fake_bridge() {
    let root = temp_dir("real-auto");
    let mut store = SessionStore::new(SimCombatBridge::lethal_fixture(), AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();

    let finished = store.automation_auto_play(&started.session_id).unwrap();

    assert_eq!(finished.automation.state, AutomationState::Done);
    assert_ne!(
        finished.latest_state.as_ref().unwrap().phase,
        LivePhase::Combat
    );
    let trace = fs::read_to_string(finished.trace_path).unwrap();
    assert!(trace.contains("\"event\":\"sent_action\""));
    assert!(trace.contains("\"event\":\"auto_play_done\""));
    fs::remove_dir_all(root).ok();
}

#[test]
fn backend_auto_play_start_marks_running_without_sending_immediately() {
    let root = temp_dir("auto-start");
    let mut store = SessionStore::new(SimCombatBridge::lethal_fixture(), AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();
    let starting_sequence = started.latest_state.as_ref().unwrap().sequence;

    let (running, _, started_job) = store
        .automation_start_auto_play(&started.session_id)
        .unwrap();

    assert!(started_job);
    assert_eq!(running.automation.state, AutomationState::AutoPlaying);
    assert_eq!(
        running.latest_state.as_ref().unwrap().sequence,
        starting_sequence
    );

    let (ticked, _) = store
        .automation_auto_play_tick(&started.session_id, 0)
        .unwrap();
    assert!(ticked.latest_state.as_ref().unwrap().sequence > starting_sequence);
    assert_eq!(ticked.automation.executed_actions.len(), 1);
    fs::remove_dir_all(root).ok();
}

#[test]
fn backend_auto_play_tick_checks_fidelity_before_planning_and_after_finish() {
    let root = temp_dir("auto-fidelity-count");
    let checks = Rc::new(Cell::new(0));
    let mut store = SessionStore::new(
        SimCombatBridge::lethal_fixture(),
        CountingOkFidelity {
            checks: Rc::clone(&checks),
        },
        &root,
    );
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();
    checks.set(0);
    store
        .automation_start_auto_play(&started.session_id)
        .unwrap();

    let (ticked, _) = store
        .automation_auto_play_tick(&started.session_id, 0)
        .unwrap();

    assert_eq!(checks.get(), 2);
    assert_eq!(ticked.automation.executed_actions.len(), 1);
    fs::remove_dir_all(root).ok();
}

#[test]
fn backend_auto_play_tick_uses_sim_state_from_fidelity_replay() {
    let root = temp_dir("auto-fidelity-sim-state");
    let mut store = SessionStore::new(
        SimCombatBridge::default().without_embedded_sim_state(),
        SimStateFidelity {
            run: RunState::combat_fixture(),
        },
        &root,
    );
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();
    assert!(started
        .latest_state
        .as_ref()
        .is_some_and(|state| state.raw.get("sim_run_state").is_none()));
    store
        .automation_start_auto_play(&started.session_id)
        .unwrap();

    let (ticked, _) = store
        .automation_auto_play_tick(&started.session_id, 0)
        .unwrap();

    assert_ne!(ticked.automation.state, AutomationState::Blocked);
    assert_eq!(ticked.automation.executed_actions.len(), 1);
    fs::remove_dir_all(root).ok();
}

#[test]
fn backend_auto_play_history_keeps_all_executed_actions() {
    let root = temp_dir("auto-history");
    let mut store = SessionStore::new(SimCombatBridge::default(), AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();
    store
        .automation_start_auto_play(&started.session_id)
        .unwrap();

    let (first, keep_going) = store
        .automation_auto_play_tick(&started.session_id, 0)
        .unwrap();
    assert!(keep_going);
    let first_plan = first
        .automation
        .plan
        .as_ref()
        .expect("first tick should keep an active plan");
    let first_plan_nodes = first_plan.nodes;
    assert_eq!(first_plan.played_actions, 1);
    assert!(
        first_plan.actions.len() > first_plan.played_actions,
        "fixture should leave a second planned action to continue"
    );

    let (second, _) = store
        .automation_auto_play_tick(&started.session_id, 1)
        .unwrap();

    assert_eq!(second.automation.executed_actions.len(), 2);
    let second_plan = second
        .automation
        .plan
        .as_ref()
        .expect("second tick should continue the same plan");
    assert_eq!(second_plan.nodes, first_plan_nodes);
    assert_eq!(second_plan.played_actions, 2);
    assert!(second
        .automation
        .executed_actions
        .iter()
        .all(|action| action.command.is_some()));
    fs::remove_dir_all(root).ok();
}

#[test]
fn automation_pause_resume_and_cancel_are_session_scoped() {
    let root = temp_dir("pause-resume-cancel");
    let mut store = SessionStore::new(SimCombatBridge::default(), AlwaysOkFidelity, &root);
    let started = store
        .start_run(BridgeId("bridge".to_owned()), run_config())
        .unwrap();

    let paused = store.automation_pause(&started.session_id).unwrap();
    assert_eq!(paused.automation.state, AutomationState::Paused);
    assert!(store.automation_step(&started.session_id).is_err());

    let resumed = store.automation_resume(&started.session_id).unwrap();
    assert_eq!(resumed.automation.state, AutomationState::Idle);

    let canceled = store.automation_cancel(&started.session_id).unwrap();
    assert_eq!(canceled.automation.state, AutomationState::Done);
    assert_eq!(
        canceled.automation.last_message.as_deref(),
        Some("automation canceled")
    );
    fs::remove_dir_all(root).ok();
}

fn ok_fake_store(root: &std::path::Path) -> SessionStore<FakeBridgeManager, AlwaysOkFidelity> {
    SessionStore::new(
        FakeBridgeManager::with_default_bridge(),
        AlwaysOkFidelity,
        root,
    )
}

fn configure_fake_policy<B, F>(store: &mut SessionStore<B, F>, session_id: &crate::model::SessionId)
where
    B: BridgeManager,
    F: FidelityChecker,
{
    store
        .configure_automation(
            session_id,
            crate::model::AutomationConfig {
                policy: crate::model::AutomationPolicy::FakePlayFirstCard,
                ..crate::model::AutomationConfig::default()
            },
        )
        .unwrap();
}

fn run_config() -> RunConfig {
    RunConfig {
        character: Character::Ironclad,
        ascension: 0,
        seed: RunSeed::Numeric(123),
    }
}

#[derive(Clone, Copy)]
struct AlwaysOkFidelity;

impl FidelityChecker for AlwaysOkFidelity {
    fn check_trace(&self, _path: &std::path::Path) -> LiveResult<FidelityStatus> {
        Ok(FidelityStatus {
            kind: FidelityKind::Ok,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: Some("test fidelity ok".to_owned()),
        })
    }
}

#[derive(Clone, Copy)]
struct AlwaysUnknownFidelity;

impl FidelityChecker for AlwaysUnknownFidelity {
    fn check_trace(&self, _path: &std::path::Path) -> LiveResult<FidelityStatus> {
        Ok(FidelityStatus::unknown())
    }
}

struct CountingOkFidelity {
    checks: Rc<Cell<usize>>,
}

impl FidelityChecker for CountingOkFidelity {
    fn check_trace(&self, _path: &std::path::Path) -> LiveResult<FidelityStatus> {
        self.checks.set(self.checks.get() + 1);
        Ok(FidelityStatus {
            kind: FidelityKind::Ok,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: Some("test fidelity ok".to_owned()),
        })
    }
}

struct SimStateFidelity {
    run: RunState,
}

impl FidelityChecker for SimStateFidelity {
    fn check_trace(&self, _path: &std::path::Path) -> LiveResult<FidelityStatus> {
        Ok(FidelityStatus {
            kind: FidelityKind::Ok,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: Some("test fidelity ok".to_owned()),
        })
    }

    fn check_trace_with_sim_state(
        &self,
        path: &std::path::Path,
    ) -> LiveResult<(FidelityStatus, Option<RunState>)> {
        self.check_trace(path)
            .map(|status| (status, Some(self.run.clone())))
    }
}

#[derive(Default)]
struct AmbiguousCardBridge {
    state: Option<LiveState>,
}

impl BridgeManager for AmbiguousCardBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(vec![bridge_status()])
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        let state = combat_state(
            1,
            vec![
                play_card("strike-a", "Strike -> Jaw Worm"),
                play_card("strike-b", "Strike -> Jaw Worm"),
            ],
        );
        self.state = Some(state.clone());
        Ok(state)
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(menu_state(2))
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(self.state.clone().unwrap())
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        Err(LiveError::Bridge(
            "ambiguous test should not send".to_owned(),
        ))
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(1)
    }
}

#[derive(Default)]
struct DesyncSameSequenceBridge;

impl BridgeManager for DesyncSameSequenceBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(vec![bridge_status()])
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        Ok(combat_state(
            1,
            vec![play_card("strike-a", "Strike -> Jaw Worm")],
        ))
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(menu_state(2))
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        Ok(combat_state(1, Vec::new()))
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        _action: &LegalAction,
    ) -> LiveResult<LiveState> {
        Err(LiveError::Bridge("desync test should not send".to_owned()))
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(1)
    }
}

struct SimCombatBridge {
    run: RunState,
    sequence: u64,
    live_index_offset: usize,
    include_sim_state: bool,
}

impl Default for SimCombatBridge {
    fn default() -> Self {
        Self {
            run: RunState::combat_fixture(),
            sequence: 1,
            live_index_offset: 0,
            include_sim_state: true,
        }
    }
}

impl SimCombatBridge {
    fn lethal_fixture() -> Self {
        let mut run = RunState::combat_fixture();
        if let Some(combat) = run.combat.as_mut() {
            if let Some(monster) = combat.monsters.iter_mut().find(|monster| monster.alive) {
                monster.hp = 6;
                monster.max_hp = monster.max_hp.max(6);
            }
        }
        Self {
            run,
            sequence: 1,
            live_index_offset: 0,
            include_sim_state: true,
        }
    }

    fn without_embedded_sim_state(mut self) -> Self {
        self.include_sim_state = false;
        self
    }

    fn live_state(&self) -> LiveState {
        let phase = if self.run.phase == RunPhase::Combat {
            LivePhase::Combat
        } else {
            LivePhase::Reward
        };
        let mut raw = json!({
            "screen": if phase == LivePhase::Combat { "combat" } else { "reward" },
            "summary": self.live_summary(),
        });
        if self.include_sim_state {
            raw["sim_run_state"] = json!(self.run);
        }
        LiveState {
            sequence: self.sequence,
            phase: phase.clone(),
            legal_actions: self.legal_actions(),
            raw,
        }
    }

    fn legal_actions(&self) -> Vec<LegalAction> {
        let Some(combat) = self.run.combat.as_ref() else {
            return Vec::new();
        };
        if self.run.phase != RunPhase::Combat {
            return Vec::new();
        }
        legal_combat_actions(combat)
            .into_iter()
            .filter_map(|action| {
                live_action_for_combat_action(&self.run, action, self.live_index_offset)
            })
            .collect()
    }

    fn live_summary(&self) -> serde_json::Value {
        let Some(combat) = self.run.combat.as_ref() else {
            return json!({});
        };
        json!({
            "combat": {
                "hand": combat
                    .piles
                    .hand
                    .iter()
                    .enumerate()
                    .map(|(index, card)| {
                        json!({
                            "index": index + self.live_index_offset,
                            "id": format!("{:?}", card.content_id),
                        })
                    })
                    .collect::<Vec<_>>(),
                "monsters": combat
                    .monsters
                    .iter()
                    .enumerate()
                    .map(|(index, monster)| {
                        json!({
                            "index": index + self.live_index_offset,
                            "id": monster.id.get(),
                        })
                    })
                    .collect::<Vec<_>>(),
            }
        })
    }
}

impl BridgeManager for SimCombatBridge {
    fn list_bridges(&self) -> LiveResult<Vec<BridgeStatus>> {
        Ok(vec![bridge_status()])
    }

    fn start_run(&mut self, _bridge_id: &BridgeId, _config: &RunConfig) -> LiveResult<LiveState> {
        Ok(self.live_state())
    }

    fn abandon_run(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.sequence += 1;
        Ok(menu_state(self.sequence))
    }

    fn request_state(&mut self, _bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.sequence += 1;
        Ok(self.live_state())
    }

    fn send_action(
        &mut self,
        _bridge_id: &BridgeId,
        action: &LegalAction,
    ) -> LiveResult<LiveState> {
        let command = action
            .command
            .get("command")
            .and_then(|value| value.as_str())
            .ok_or_else(|| LiveError::InvalidAction("missing command".to_owned()))?;
        let combat_action = combat_action_for_command(&self.run, command)
            .ok_or_else(|| LiveError::InvalidAction(format!("unknown command {command}")))?;
        self.run = apply_combat_action_on_run(&self.run, combat_action)
            .map_err(|err| LiveError::InvalidAction(format!("{err:?}")))?;
        self.sequence += 1;
        Ok(self.live_state())
    }

    fn kill_bridge(&mut self, _bridge_id: &BridgeId) -> LiveResult<()> {
        Ok(())
    }

    fn kill_all(&mut self) -> LiveResult<usize> {
        Ok(1)
    }
}

fn live_action_for_combat_action(
    run: &RunState,
    action: CombatAction,
    live_index_offset: usize,
) -> Option<LegalAction> {
    let combat = run.combat.as_ref()?;
    let (id, label, command) = match action {
        CombatAction::EndTurn => ("end".to_owned(), "End turn".to_owned(), "END".to_owned()),
        CombatAction::PlayCard { card_id, target } => {
            let hand_position = combat
                .piles
                .hand
                .iter()
                .position(|card| card.id == card_id)?;
            let hand_slot = hand_position + live_index_offset;
            let card = combat.piles.hand.get(hand_position)?;
            let card_label = card_display_name(card.content_id);
            match target {
                Some(target) => {
                    let target_position = combat
                        .monsters
                        .iter()
                        .position(|monster| monster.id == target)?;
                    let target_slot = target_position + live_index_offset;
                    (
                        format!("play-{hand_slot}-{target_slot}"),
                        format!("Play {card_label} -> target {target_slot}"),
                        format!("PLAY {hand_slot} {target_slot}"),
                    )
                }
                None => (
                    format!("play-{hand_slot}"),
                    format!("Play {card_label}"),
                    format!("PLAY {hand_slot}"),
                ),
            }
        }
    };
    Some(LegalAction {
        id: ActionId(id),
        kind: match action {
            CombatAction::EndTurn => LegalActionKind::EndTurn,
            CombatAction::PlayCard { .. } => LegalActionKind::PlayCard,
        },
        label,
        enabled: true,
        command: json!({
            "transport": "communication_mod",
            "command": command,
            "source_state_id": format!("sim-state-{}", run.player_hp),
        }),
        disabled_reason: None,
    })
}

fn card_display_name(content_id: ContentId) -> String {
    get_card_definition(content_id)
        .map(|definition| definition.name.to_owned())
        .unwrap_or_else(|| format!("card content {}", content_id.get()))
}

fn combat_action_for_command(run: &RunState, command: &str) -> Option<CombatAction> {
    if command.eq_ignore_ascii_case("END") {
        return Some(CombatAction::EndTurn);
    }
    let parts = command.split_whitespace().collect::<Vec<_>>();
    if parts
        .first()
        .is_none_or(|verb| !verb.eq_ignore_ascii_case("PLAY"))
    {
        return None;
    }
    let hand_slot = parts.get(1)?.parse::<usize>().ok()?;
    let combat = run.combat.as_ref()?;
    let card_id = combat.piles.hand.get(hand_slot)?.id;
    let target = if let Some(target_slot) = parts.get(2) {
        let target_slot = target_slot.parse::<usize>().ok()?;
        Some(combat.monsters.get(target_slot)?.id)
    } else {
        None
    };
    Some(CombatAction::PlayCard { card_id, target })
}

fn bridge_status() -> BridgeStatus {
    BridgeStatus {
        id: BridgeId("bridge".to_owned()),
        process_id: Some(1),
        client_id: Some("automation-test".to_owned()),
        connected: true,
        last_heartbeat_ms: None,
    }
}

fn combat_state(sequence: u64, legal_actions: Vec<LegalAction>) -> LiveState {
    LiveState {
        sequence,
        phase: LivePhase::Combat,
        legal_actions,
        raw: json!({"screen": "combat"}),
    }
}

fn menu_state(sequence: u64) -> LiveState {
    LiveState {
        sequence,
        phase: LivePhase::Menu,
        legal_actions: Vec::new(),
        raw: json!({"screen": "menu"}),
    }
}

fn play_card(id: &str, label: &str) -> LegalAction {
    LegalAction {
        id: ActionId(id.to_owned()),
        kind: LegalActionKind::PlayCard,
        label: label.to_owned(),
        enabled: true,
        command: json!({"kind": "play_card"}),
        disabled_reason: None,
    }
}

fn strings<const N: usize>(items: [&str; N]) -> Vec<String> {
    items.into_iter().map(str::to_owned).collect()
}

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("sts-live-automation-{name}-{nonce}"))
}
