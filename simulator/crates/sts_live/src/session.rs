use crate::{
    automation::{bind_plan_step_to_live_action, blocked as automation_blocked, plan_action},
    bridge::BridgeManager,
    fidelity::FidelityChecker,
    model::{
        ActionId, AutomationConfig, AutomationJobSnapshot, AutomationState, BlockedState, BridgeId,
        FidelityKind, FidelityStatus, LegalAction, LiveError, LivePhase, LiveResult, LiveState,
        RunConfig, SessionId, SessionLifecycle, SessionSnapshot, SlayTheDataRunSummary,
        SlayTheDataSearchFilters, TraceRecord,
    },
    operator_actions::{request_state_action, start_run_action},
    session_blocking::record_blocked,
    session_recovery,
    session_response::append_bridge_response_and_state,
    session_state::{lifecycle_for_fidelity, metadata_record, FidelityCache, SessionData},
    slaythedata::{AttachedSlayTheDataRun, SlayTheDataIndex},
    trace_writer::TraceWriter,
};
use serde_json::json;
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

const ACTION_STATE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const ACTION_STATE_POLL_TIMEOUT: Duration = Duration::from_secs(15);

pub struct SessionStore<B, F> {
    bridge: B,
    fidelity: F,
    slaythedata_index: SlayTheDataIndex,
    trace_root: PathBuf,
    sessions: HashMap<SessionId, SessionData>,
    next_session: u64,
}

impl<B, F> SessionStore<B, F>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    pub fn new(bridge: B, fidelity: F, trace_root: impl AsRef<Path>) -> Self {
        Self {
            bridge,
            fidelity,
            slaythedata_index: SlayTheDataIndex::default_local(),
            trace_root: trace_root.as_ref().to_path_buf(),
            sessions: HashMap::new(),
            next_session: 1,
        }
    }

    pub fn with_slaythedata_index(mut self, index: SlayTheDataIndex) -> Self {
        self.slaythedata_index = index;
        self
    }

    pub fn list_bridges(&self) -> LiveResult<Vec<crate::model::BridgeStatus>> {
        self.bridge.list_bridges()
    }

    pub fn kill_bridge(&mut self, bridge_id: &BridgeId) -> LiveResult<()> {
        self.bridge.kill_bridge(bridge_id)
    }

    pub fn kill_all_bridges(&mut self) -> LiveResult<usize> {
        self.bridge.kill_all()
    }

    pub fn recover_existing_sessions(&mut self) -> LiveResult<Vec<SessionSnapshot>> {
        let mut snapshots = Vec::new();
        for path in session_recovery::trace_paths(&self.trace_root)? {
            snapshots.push(self.recover_session(path)?);
        }
        Ok(snapshots)
    }

    pub fn recover_session(&mut self, path: impl AsRef<Path>) -> LiveResult<SessionSnapshot> {
        let path = path.as_ref();
        let session = session_recovery::recover_session(path, &self.fidelity)?;
        if let Some(existing) = self.sessions.get(&session.id) {
            return Ok(existing.snapshot());
        }
        session_recovery::bump_next_session(&mut self.next_session, &session.id);
        let snapshot = session.snapshot();
        self.sessions.insert(session.id.clone(), session);
        Ok(snapshot)
    }

    pub fn list_sessions(&self) -> Vec<SessionSnapshot> {
        let mut snapshots = self
            .sessions
            .values()
            .map(SessionData::snapshot)
            .collect::<Vec<_>>();
        snapshots.sort_by(|left, right| {
            session_number(&left.session_id)
                .cmp(&session_number(&right.session_id))
                .then_with(|| left.session_id.0.cmp(&right.session_id.0))
        });
        snapshots
    }

    pub fn start_run(
        &mut self,
        bridge_id: BridgeId,
        config: RunConfig,
    ) -> LiveResult<SessionSnapshot> {
        let state = self.bridge.start_run(&bridge_id, &config)?;
        let id = self.next_session_id();
        let start_action = start_run_action(&config);
        let mut session = self.new_session(id.clone(), bridge_id, Some(config))?;
        session.lifecycle = SessionLifecycle::Recording;
        session
            .trace_writer
            .append(&metadata_record(&session, "live_trace"))?;
        session.trace_writer.append(&TraceRecord::Action {
            sequence: state.sequence.saturating_sub(1),
            action: start_action,
        })?;
        append_bridge_response_and_state(&mut session, "start_run", &state)?;
        session.latest_state = Some(state);
        session.fidelity = self.fidelity.check_trace(session.trace_writer.path())?;
        session.lifecycle = lifecycle_for_fidelity(&session.fidelity);
        let snapshot = session.snapshot();
        self.sessions.insert(id, session);
        Ok(snapshot)
    }

    pub fn request_state(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        self.request_state_with_fidelity(session_id, true)
    }

    fn request_state_with_fidelity(
        &mut self,
        session_id: &SessionId,
        refresh_fidelity: bool,
    ) -> LiveResult<SessionSnapshot> {
        let bridge_id = self.session(session_id)?.bridge_id.clone();
        let state = match self.bridge.request_state(&bridge_id) {
            Ok(state) => state,
            Err(err) => {
                let message = err.to_string();
                self.block_session(session_id, "bridge_error", &message)?;
                return Err(err);
            }
        };
        let session = self.session_mut(session_id)?;
        session.trace_writer.append(&TraceRecord::Action {
            sequence: state.sequence.saturating_sub(1),
            action: request_state_action(),
        })?;
        append_bridge_response_and_state(session, "request_state", &state)?;
        session.latest_state = Some(state);
        session.blocked = None;
        if matches!(session.lifecycle, SessionLifecycle::Blocked) {
            session.lifecycle = lifecycle_for_fidelity(&session.fidelity);
        }
        if refresh_fidelity {
            self.refresh_fidelity(session_id)
        } else {
            Ok(self.session(session_id)?.snapshot())
        }
    }

    pub fn abandon_run(
        &mut self,
        session_id: &SessionId,
        reason: &str,
    ) -> LiveResult<SessionSnapshot> {
        let bridge_id = self.session(session_id)?.bridge_id.clone();
        let state = self.bridge.abandon_run(&bridge_id)?;
        let trace_path = {
            let session = self.session_mut(session_id)?;
            session.trace_writer.append(&TraceRecord::RunAbandoned {
                sequence: state.sequence.saturating_sub(1),
                reason: reason.to_owned(),
            })?;
            append_bridge_response_and_state(session, "abandon_run", &state)?;
            session.latest_state = Some(state);
            session.lifecycle = SessionLifecycle::Ended;
            session.trace_writer.path().to_path_buf()
        };
        let fidelity = self.fidelity.check_trace(&trace_path)?;
        let session = self.session_mut(session_id)?;
        session.fidelity = fidelity;
        Ok(session.snapshot())
    }

    pub fn send_action(
        &mut self,
        session_id: &SessionId,
        action_id: &ActionId,
    ) -> LiveResult<SessionSnapshot> {
        self.send_action_with_fidelity(session_id, action_id, false, true)
    }

    fn send_action_with_fidelity(
        &mut self,
        session_id: &SessionId,
        action_id: &ActionId,
        refresh_fidelity: bool,
        advance_manual_plan_after_success: bool,
    ) -> LiveResult<SessionSnapshot> {
        let action = {
            let session = self.session_mut(session_id)?;
            if matches!(session.lifecycle, SessionLifecycle::Blocked) {
                return Err(LiveError::Blocked("session is blocked".to_owned()));
            }
            let state = session
                .latest_state
                .as_ref()
                .ok_or_else(|| LiveError::Blocked("session has no live state".to_owned()))?;
            state
                .legal_actions
                .iter()
                .find(|action| &action.id == action_id)
                .cloned()
                .ok_or_else(|| {
                    LiveError::InvalidAction(format!("unknown action {}", action_id.0))
                })?
        };

        if !action.enabled {
            let message = action
                .disabled_reason
                .clone()
                .unwrap_or_else(|| "action is disabled".to_owned());
            self.block_session(session_id, "disabled_action", &message)?;
            return Err(LiveError::InvalidAction(message));
        }

        let bridge_id = self.session(session_id)?.bridge_id.clone();
        let state = match self.bridge.send_action(&bridge_id, &action) {
            Ok(state) => state,
            Err(err) => {
                let message = err.to_string();
                self.block_session(session_id, "bridge_error", &message)?;
                return Err(err);
            }
        };
        let state = self.wait_for_fresh_action_state(&bridge_id, &action, state)?;
        let advance_manual_plan = {
            let session = self.session_mut(session_id)?;
            let advance_manual_plan =
                !matches!(session.automation.state, AutomationState::SendingAction);
            session.trace_writer.append(&TraceRecord::Action {
                sequence: state.sequence.saturating_sub(1),
                action: action.clone(),
            })?;
            append_bridge_response_and_state(session, "send_action", &state)?;
            session.latest_state = Some(state);
            advance_manual_plan && advance_manual_plan_after_success
        };
        if advance_manual_plan {
            self.advance_automation_plan_after_action(session_id, &action, true)?;
        }
        if refresh_fidelity {
            self.refresh_fidelity(session_id)
        } else {
            Ok(self.session(session_id)?.snapshot())
        }
    }

    fn wait_for_fresh_action_state(
        &mut self,
        bridge_id: &BridgeId,
        action: &LegalAction,
        initial: LiveState,
    ) -> LiveResult<LiveState> {
        let Some(source_state_id) = action_source_state_id(action) else {
            return Ok(initial);
        };
        if !state_matches_source_state_id(&initial, source_state_id) {
            return Ok(initial);
        }

        let deadline = Instant::now() + ACTION_STATE_POLL_TIMEOUT;
        loop {
            let refreshed = self.bridge.request_state(bridge_id)?;
            if !state_matches_source_state_id(&refreshed, source_state_id) {
                return Ok(refreshed);
            }
            if Instant::now() >= deadline {
                return Err(LiveError::Bridge(
                    "timed out waiting for the next live state after action".to_owned(),
                ));
            }
            thread::sleep(ACTION_STATE_POLL_INTERVAL);
        }
    }

    pub fn session_snapshot(&self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        Ok(self.session(session_id)?.snapshot())
    }

    pub fn actions(&self, session_id: &SessionId) -> LiveResult<Vec<crate::model::LegalAction>> {
        Ok(self
            .session(session_id)?
            .latest_state
            .as_ref()
            .map(|state| state.legal_actions.clone())
            .unwrap_or_default())
    }

    pub fn search_slaythedata_runs(
        &self,
        filters: SlayTheDataSearchFilters,
    ) -> LiveResult<Vec<SlayTheDataRunSummary>> {
        self.slaythedata_index.search(&filters)
    }

    pub fn attach_slaythedata_run(
        &mut self,
        session_id: &SessionId,
        run_id: i64,
    ) -> LiveResult<SessionSnapshot> {
        self.session(session_id)?;
        let (summary, raw_run_json) = self.slaythedata_index.load_materialized_run(run_id)?;
        let attached = AttachedSlayTheDataRun::from_raw(summary, &raw_run_json)?;
        let details = json!({
            "run": attached.summary,
            "preflight_steps": attached.report.steps.len(),
            "route_fully_checked": attached.report.route_fully_checked,
            "diagnostics": attached.report.diagnostics,
        });
        let session = self.session_mut(session_id)?;
        session.slaythedata = Some(attached);
        let snapshot = session.snapshot();
        self.append_slaythedata_trace(session_id, "attach_run", details)?;
        Ok(snapshot)
    }

    pub fn slaythedata_send_next(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        if self
            .session(session_id)?
            .slaythedata
            .as_ref()
            .and_then(|attached| attached.blocked.as_ref())
            .is_some()
        {
            return Ok(self.session(session_id)?.snapshot());
        }
        self.refresh_fidelity(session_id)?;
        if self.session(session_id)?.fidelity.kind != FidelityKind::Ok {
            return self.block_slaythedata(
                session_id,
                "slaythedata_fidelity_not_ok",
                "SlayTheData guided action requires fidelity ok before sending",
            );
        }
        let state = self
            .session(session_id)?
            .latest_state
            .clone()
            .ok_or_else(|| {
                LiveError::Blocked("SlayTheData guidance needs a live state".to_owned())
            })?;
        if state.phase == LivePhase::Combat {
            let session = self.session_mut(session_id)?;
            if let Some(attached) = session.slaythedata.as_mut() {
                attached.last_message =
                    Some("SlayTheData guidance paused in combat; use the combat agent".to_owned());
            }
            return Ok(session.snapshot());
        }
        let (step_index, action) = {
            let session = self.session_mut(session_id)?;
            let Some(attached) = session.slaythedata.as_mut() else {
                return Err(LiveError::Blocked(
                    "no SlayTheData run is attached to this session".to_owned(),
                ));
            };
            match attached.ready_action(&state) {
                Ok((index, action)) => (index, action),
                Err(blocked) => {
                    attached.mark_blocked(blocked.clone());
                    let snapshot = session.snapshot();
                    self.append_slaythedata_trace(
                        session_id,
                        "blocked",
                        json!(snapshot.slaythedata),
                    )?;
                    return Ok(snapshot);
                }
            }
        };

        let action_id = action.id.clone();
        self.append_slaythedata_trace(
            session_id,
            "send_action",
            json!({"step_index": step_index, "action": action}),
        )?;
        if let Err(error) = self.send_action_with_fidelity(session_id, &action_id, false, false) {
            let message = error.to_string();
            return self.block_slaythedata(session_id, "slaythedata_send_failed", &message);
        }
        self.refresh_fidelity(session_id)?;
        if self.session(session_id)?.fidelity.kind != FidelityKind::Ok {
            return self.block_slaythedata(
                session_id,
                "slaythedata_transition_not_ok",
                "SlayTheData guided transition did not verify with fidelity ok",
            );
        }
        let session = self.session_mut(session_id)?;
        if let Some(attached) = session.slaythedata.as_mut() {
            attached.mark_sent(step_index);
        }
        let snapshot = session.snapshot();
        self.append_slaythedata_trace(session_id, "sent_action", json!(snapshot.slaythedata))?;
        Ok(snapshot)
    }

    pub fn slaythedata_auto_play(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let limit = 50;
        for _ in 0..limit {
            let before = self.session(session_id)?.snapshot();
            if before
                .latest_state
                .as_ref()
                .is_some_and(|state| state.phase == LivePhase::Combat)
            {
                return Ok(before);
            }
            let after = self.slaythedata_send_next(session_id)?;
            if after.slaythedata.blocked.is_some()
                || after
                    .latest_state
                    .as_ref()
                    .is_some_and(|state| state.phase == LivePhase::Combat)
                || after
                    .slaythedata
                    .advisor
                    .as_ref()
                    .is_none_or(|advisor| advisor.action_id.is_none())
            {
                return Ok(after);
            }
        }
        self.block_slaythedata(
            session_id,
            "slaythedata_auto_action_limit",
            "SlayTheData guided auto-play reached its action limit",
        )
    }

    pub fn automation_status(&self, session_id: &SessionId) -> LiveResult<AutomationJobSnapshot> {
        Ok(self.session(session_id)?.automation.clone())
    }

    pub fn configure_automation(
        &mut self,
        session_id: &SessionId,
        config: AutomationConfig,
    ) -> LiveResult<AutomationJobSnapshot> {
        let session = self.session_mut(session_id)?;
        let policy = config.policy.clone();
        session.automation = AutomationJobSnapshot {
            policy,
            config,
            ..AutomationJobSnapshot::default()
        };
        let automation = session.automation.clone();
        let details = json!(automation);
        self.append_automation_trace(session_id, "configure", details)?;
        Ok(automation)
    }

    pub fn automation_plan(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        self.automation_plan_with_fidelity_check(session_id, true)
    }

    fn automation_plan_with_fidelity_check(
        &mut self,
        session_id: &SessionId,
        refresh_fidelity: bool,
    ) -> LiveResult<SessionSnapshot> {
        if refresh_fidelity {
            self.set_automation_state(session_id, AutomationState::WaitingForFidelity)?;
            self.refresh_fidelity(session_id)?;
        }
        if let Some(blocked) = self.block_if_automation_fidelity_not_ok(
            session_id,
            "automation requires fidelity ok before planning",
        )? {
            return Ok(blocked);
        }
        self.set_automation_state(session_id, AutomationState::Planning)?;
        let (config, state) = {
            let session = self.session(session_id)?;
            let state = session.latest_state.clone().ok_or_else(|| {
                LiveError::Blocked("automation cannot plan without a live state".to_owned())
            })?;
            (session.automation.config.clone(), state)
        };

        match plan_action(&config, &state) {
            Ok((planned_action, plan)) => {
                let session = self.session_mut(session_id)?;
                session.automation.state = AutomationState::ReadyToSend;
                session.automation.planned_action = Some(planned_action);
                session.automation.plan = Some(plan);
                session.automation.blocked = None;
                session.automation.last_message = Some("automation plan is ready".to_owned());
                let snapshot = session.snapshot();
                self.append_automation_trace(session_id, "plan_ready", json!(snapshot.automation))?;
                Ok(snapshot)
            }
            Err(blocked) => self.block_automation_with_state(session_id, blocked),
        }
    }

    pub fn automation_send_ready(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        self.automation_send_ready_with_fidelity_checks(session_id, true, true)
    }

    fn automation_send_ready_with_fidelity_checks(
        &mut self,
        session_id: &SessionId,
        refresh_before_send: bool,
        refresh_after_send: bool,
    ) -> LiveResult<SessionSnapshot> {
        if refresh_before_send {
            self.set_automation_state(session_id, AutomationState::WaitingForFidelity)?;
            self.refresh_fidelity(session_id)?;
        }
        if let Some(blocked) = self.block_if_automation_fidelity_not_ok(
            session_id,
            "automation requires fidelity ok before sending",
        )? {
            return Ok(blocked);
        }
        let planned = match self.session(session_id)?.automation.planned_action.clone() {
            Some(planned) => planned,
            None => {
                return self.block_automation(
                    session_id,
                    "automation_no_ready_action",
                    "automation has no ready planned action",
                )
            }
        };

        let current_state = self
            .session(session_id)?
            .latest_state
            .clone()
            .ok_or_else(|| {
                LiveError::Blocked("automation cannot send without a live state".to_owned())
            })?;

        if current_state.sequence != planned.source_sequence {
            return self.block_automation(
                session_id,
                "automation_stale_state",
                "planned action was based on an older live state",
            );
        }

        let Some(action) = current_state
            .legal_actions
            .iter()
            .find(|action| action.id == planned.action_id)
        else {
            return self.block_automation(
                session_id,
                "automation_desynced_action",
                "planned action is no longer present in the live legal actions",
            );
        };
        let action = action.clone();

        if action.kind != planned.kind || action.label != planned.label {
            return self.block_automation(
                session_id,
                "automation_desynced_action",
                "planned action identity no longer matches the live legal action",
            );
        }

        if let Some(expected_command) = planned.command.as_deref() {
            let actual_command = action
                .command
                .get("command")
                .and_then(|value| value.as_str());
            if actual_command.is_none_or(|command| !command.eq_ignore_ascii_case(expected_command))
            {
                return self.block_automation(
                    session_id,
                    "automation_desynced_action",
                    "planned command no longer matches the live legal action",
                );
            }
        }

        if !action.enabled {
            return self.block_automation(
                session_id,
                "automation_disabled_action",
                action
                    .disabled_reason
                    .as_deref()
                    .unwrap_or("planned action is disabled"),
            );
        }

        self.set_automation_state(session_id, AutomationState::SendingAction)?;
        let action_id = planned.action_id.clone();
        let result = self.send_action_with_fidelity(session_id, &action_id, false, true);
        if let Err(err) = result {
            if let Ok(session) = self.session_mut(session_id) {
                session.automation.state = AutomationState::Failed;
                session.automation.blocked = Some(automation_blocked(
                    "automation_send_failed",
                    &err.to_string(),
                ));
                session.automation.last_message = Some(err.to_string());
            }
            return Err(err);
        }

        self.set_automation_state(session_id, AutomationState::VerifyingTransition)?;
        if refresh_after_send {
            self.refresh_fidelity(session_id)?;
        }
        if let Some(blocked) = self.block_if_automation_fidelity_not_ok(
            session_id,
            "automation transition did not verify with fidelity ok",
        )? {
            return Ok(blocked);
        }

        self.advance_automation_plan_after_action(session_id, &action, false)?;
        let session = self.session_mut(session_id)?;
        session.automation.executed_actions.push(planned);
        session.automation.state = AutomationState::Done;
        session.automation.blocked = None;
        session.automation.last_message = Some("automation sent one action".to_owned());
        let snapshot = session.snapshot();
        self.append_automation_trace(session_id, "sent_action", json!(snapshot.automation))?;
        Ok(snapshot)
    }

    pub fn automation_step(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        self.automation_resume_if_idle_or_done(session_id)?;
        self.set_automation_state(session_id, AutomationState::WaitingForLiveState)?;
        self.request_state(session_id)?;
        let planned = self.automation_plan(session_id)?;
        if planned.automation.state != AutomationState::ReadyToSend {
            return Ok(planned);
        }
        self.automation_send_ready(session_id)
    }

    pub fn automation_auto_play(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let (_, limit, _) = self.automation_start_auto_play(session_id)?;
        for actions_sent in 0..limit {
            let (snapshot, keep_going) =
                self.automation_auto_play_tick(session_id, actions_sent)?;
            if !keep_going {
                return Ok(snapshot);
            }
        }
        self.block_automation(
            session_id,
            "automation_auto_action_limit",
            "automation reached the configured auto-play action limit",
        )
    }

    pub fn automation_start_auto_play(
        &mut self,
        session_id: &SessionId,
    ) -> LiveResult<(SessionSnapshot, usize, bool)> {
        self.automation_resume_if_idle_or_done(session_id)?;
        let limit = self
            .session(session_id)?
            .automation
            .config
            .auto_action_limit
            .max(1);
        let session = self.session_mut(session_id)?;
        let started = session.automation.state != AutomationState::AutoPlaying;
        if started {
            session.automation.state = AutomationState::AutoPlaying;
            session.automation.blocked = None;
            session.automation.last_message = Some("automation auto-play started".to_owned());
        }
        let snapshot = session.snapshot();
        if started {
            self.append_automation_trace(
                session_id,
                "auto_play_started",
                json!(snapshot.automation),
            )?;
        }
        Ok((snapshot, limit, started))
    }

    pub fn automation_auto_play_tick(
        &mut self,
        session_id: &SessionId,
        actions_sent: usize,
    ) -> LiveResult<(SessionSnapshot, bool)> {
        if self.session(session_id)?.automation.state != AutomationState::AutoPlaying {
            return Ok((self.session(session_id)?.snapshot(), false));
        }

        self.set_automation_state(session_id, AutomationState::WaitingForLiveState)?;
        let refreshed = self.request_state_with_fidelity(session_id, false)?;
        if refreshed
            .latest_state
            .as_ref()
            .is_none_or(|state| state.phase != crate::model::LivePhase::Combat)
        {
            return Ok((self.finish_auto_play(session_id, actions_sent)?, false));
        }
        self.refresh_fidelity(session_id)?;
        if let Some(blocked) = self.block_if_automation_fidelity_not_ok(
            session_id,
            "automation requires fidelity ok before auto-play planning",
        )? {
            return Ok((blocked, false));
        }

        let planned = if self.bind_existing_automation_plan_to_latest_state(session_id)? {
            self.set_automation_state(session_id, AutomationState::ReadyToSend)?;
            self.session(session_id)?.snapshot()
        } else {
            self.automation_plan_with_fidelity_check(session_id, false)?
        };
        if planned.automation.state != AutomationState::ReadyToSend {
            return Ok((planned, false));
        }
        let sent = self.automation_send_ready_with_fidelity_checks(session_id, false, false)?;
        if sent
            .latest_state
            .as_ref()
            .is_none_or(|state| state.phase != crate::model::LivePhase::Combat)
        {
            return Ok((self.finish_auto_play(session_id, actions_sent + 1)?, false));
        }

        let session = self.session_mut(session_id)?;
        session.automation.state = AutomationState::AutoPlaying;
        session.automation.blocked = None;
        session.automation.last_message = Some(format!(
            "automation auto-play sent {} actions",
            actions_sent + 1
        ));
        let snapshot = session.snapshot();
        self.append_automation_trace(session_id, "auto_play_progress", json!(snapshot.automation))?;
        Ok((snapshot, true))
    }

    fn bind_existing_automation_plan_to_latest_state(
        &mut self,
        session_id: &SessionId,
    ) -> LiveResult<bool> {
        let Some(state) = self.session(session_id)?.latest_state.clone() else {
            return Ok(false);
        };
        let session = self.session_mut(session_id)?;
        let Some(plan) = session.automation.plan.as_mut() else {
            return Ok(false);
        };
        let Some(step) = plan.actions.get(plan.played_actions).cloned() else {
            session.automation.planned_action = None;
            return Ok(false);
        };
        let Some(live_step) = bind_plan_step_to_live_action(&state, &step) else {
            return Ok(false);
        };
        if let Some(plan_step) = plan.actions.get_mut(plan.played_actions) {
            *plan_step = live_step.clone();
        }
        session.automation.planned_action = Some(live_step);
        Ok(true)
    }

    pub fn automation_fail_auto_play(
        &mut self,
        session_id: &SessionId,
        message: &str,
    ) -> LiveResult<SessionSnapshot> {
        let session = self.session_mut(session_id)?;
        session.automation.state = AutomationState::Failed;
        session.automation.blocked =
            Some(automation_blocked("automation_auto_play_failed", message));
        session.automation.last_message = Some(message.to_owned());
        let snapshot = session.snapshot();
        self.append_automation_trace(session_id, "auto_play_failed", json!(snapshot.automation))?;
        Ok(snapshot)
    }

    pub fn automation_pause(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let session = self.session_mut(session_id)?;
        session.automation.state = AutomationState::Paused;
        session.automation.last_message = Some("automation paused".to_owned());
        let snapshot = session.snapshot();
        self.append_automation_trace(session_id, "pause", json!(snapshot.automation))?;
        Ok(snapshot)
    }

    pub fn automation_resume(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let session = self.session_mut(session_id)?;
        if session.automation.state == AutomationState::Paused {
            session.automation.state = AutomationState::Idle;
            session.automation.last_message = Some("automation resumed".to_owned());
        }
        let snapshot = session.snapshot();
        self.append_automation_trace(session_id, "resume", json!(snapshot.automation))?;
        Ok(snapshot)
    }

    pub fn automation_cancel(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let session = self.session_mut(session_id)?;
        session.automation.state = AutomationState::Done;
        session.automation.planned_action = None;
        session.automation.plan = None;
        session.automation.last_message = Some("automation canceled".to_owned());
        let snapshot = session.snapshot();
        self.append_automation_trace(session_id, "cancel", json!(snapshot.automation))?;
        Ok(snapshot)
    }

    pub fn refresh_fidelity(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let path = self.session(session_id)?.trace_writer.path().to_path_buf();
        let trace_len = std::fs::metadata(&path)?.len();
        if let Some(cached) = self.reusable_fidelity_cache(session_id, &path, trace_len)? {
            self.apply_fidelity_result(
                session_id,
                cached.status.clone(),
                cached.sim_run_state.clone(),
                Some(cached),
            )?;
            return Ok(self.session(session_id)?.snapshot());
        }

        let (fidelity, sim_run_state) = self.fidelity.check_trace_with_sim_state(&path)?;
        let cache = FidelityCache {
            trace_len,
            reusable_after_append: fidelity_status_reusable_after_append(&fidelity),
            status: fidelity.clone(),
            sim_run_state: sim_run_state.clone(),
        };
        self.apply_fidelity_result(session_id, fidelity, sim_run_state, Some(cache))?;
        Ok(self.session(session_id)?.snapshot())
    }

    fn reusable_fidelity_cache(
        &self,
        session_id: &SessionId,
        path: &Path,
        trace_len: u64,
    ) -> LiveResult<Option<FidelityCache>> {
        let Some(cache) = self.session(session_id)?.fidelity_cache.clone() else {
            return Ok(None);
        };
        if cache.trace_len == trace_len {
            return Ok(Some(cache));
        }
        if trace_len < cache.trace_len || !cache.reusable_after_append {
            return Ok(None);
        }
        if let Some(status) = fidelity_loss_status_in_tail(path, cache.trace_len)? {
            return Ok(Some(FidelityCache {
                trace_len,
                status,
                sim_run_state: None,
                reusable_after_append: false,
            }));
        }
        Ok(Some(FidelityCache { trace_len, ..cache }))
    }

    fn apply_fidelity_result(
        &mut self,
        session_id: &SessionId,
        fidelity: FidelityStatus,
        sim_run_state: Option<sts_core::RunState>,
        cache: Option<FidelityCache>,
    ) -> LiveResult<()> {
        let session = self.session_mut(session_id)?;
        session.fidelity = fidelity;
        if let (Some(state), Some(sim_run_state)) = (session.latest_state.as_mut(), sim_run_state) {
            if let Some(raw) = state.raw.as_object_mut() {
                raw.insert(
                    "sim_run_state".to_owned(),
                    serde_json::to_value(sim_run_state)?,
                );
            }
        }
        session.fidelity_cache = cache;
        if !matches!(
            session.lifecycle,
            SessionLifecycle::Blocked | SessionLifecycle::Ended
        ) {
            session.lifecycle = lifecycle_for_fidelity(&session.fidelity);
        }
        Ok(())
    }

    fn block_session(
        &mut self,
        session_id: &SessionId,
        reason_code: &str,
        message: &str,
    ) -> LiveResult<()> {
        let session = self.session_mut(session_id)?;
        record_blocked(session, reason_code, message)
    }

    fn new_session(
        &self,
        id: SessionId,
        bridge_id: BridgeId,
        run_config: Option<RunConfig>,
    ) -> LiveResult<SessionData> {
        let trace_path = self.trace_root.join(format!("{}.jsonl", id.0));
        Ok(SessionData {
            id,
            bridge_id,
            lifecycle: SessionLifecycle::Attached,
            trace_writer: TraceWriter::create_new(trace_path)?,
            run_config,
            latest_state: None,
            fidelity: FidelityStatus::unknown(),
            fidelity_cache: None,
            blocked: None,
            automation: AutomationJobSnapshot::default(),
            slaythedata: None,
        })
    }

    fn set_automation_state(
        &mut self,
        session_id: &SessionId,
        state: AutomationState,
    ) -> LiveResult<()> {
        let session = self.session_mut(session_id)?;
        session.automation.state = state;
        session.automation.last_message = None;
        Ok(())
    }

    fn block_automation(
        &mut self,
        session_id: &SessionId,
        reason_code: &str,
        message: &str,
    ) -> LiveResult<SessionSnapshot> {
        self.block_automation_with_state(session_id, automation_blocked(reason_code, message))
    }

    fn block_if_automation_fidelity_not_ok(
        &mut self,
        session_id: &SessionId,
        message: &str,
    ) -> LiveResult<Option<SessionSnapshot>> {
        if self.session(session_id)?.fidelity.kind == FidelityKind::Ok {
            return Ok(None);
        }
        self.block_automation(session_id, "automation_fidelity_not_ok", message)
            .map(Some)
    }

    fn block_automation_with_state(
        &mut self,
        session_id: &SessionId,
        blocked: BlockedState,
    ) -> LiveResult<SessionSnapshot> {
        let session = self.session_mut(session_id)?;
        session.automation.state = AutomationState::Blocked;
        session.automation.planned_action = None;
        session.automation.last_message = Some(blocked.message.clone());
        session.automation.blocked = Some(blocked);
        let snapshot = session.snapshot();
        self.append_automation_trace(session_id, "blocked", json!(snapshot.automation))?;
        Ok(snapshot)
    }

    fn finish_auto_play(
        &mut self,
        session_id: &SessionId,
        actions_sent: usize,
    ) -> LiveResult<SessionSnapshot> {
        self.refresh_fidelity(session_id)?;
        if let Some(blocked) = self.block_if_automation_fidelity_not_ok(
            session_id,
            "automation auto-play trace did not verify with fidelity ok",
        )? {
            return Ok(blocked);
        }
        let session = self.session_mut(session_id)?;
        session.automation.state = AutomationState::Done;
        session.automation.planned_action = None;
        session.automation.last_message = Some(format!(
            "automation finished combat after {actions_sent} actions"
        ));
        let snapshot = session.snapshot();
        self.append_automation_trace(session_id, "auto_play_done", json!(snapshot.automation))?;
        Ok(snapshot)
    }

    fn append_automation_trace(
        &mut self,
        session_id: &SessionId,
        event: &str,
        details: serde_json::Value,
    ) -> LiveResult<()> {
        let sequence = self
            .session(session_id)?
            .latest_state
            .as_ref()
            .map(|state| state.sequence)
            .unwrap_or_default();
        let session = self.session_mut(session_id)?;
        session.trace_writer.append(&TraceRecord::Automation {
            sequence,
            event: event.to_owned(),
            details,
        })?;
        Ok(())
    }

    fn append_slaythedata_trace(
        &mut self,
        session_id: &SessionId,
        event: &str,
        details: serde_json::Value,
    ) -> LiveResult<()> {
        let sequence = self
            .session(session_id)?
            .latest_state
            .as_ref()
            .map(|state| state.sequence)
            .unwrap_or_default();
        let session = self.session_mut(session_id)?;
        session.trace_writer.append(&TraceRecord::SlayTheData {
            sequence,
            event: event.to_owned(),
            details,
        })?;
        Ok(())
    }

    fn block_slaythedata(
        &mut self,
        session_id: &SessionId,
        reason_code: &str,
        message: &str,
    ) -> LiveResult<SessionSnapshot> {
        let session = self.session_mut(session_id)?;
        let blocked = BlockedState {
            reason_code: reason_code.to_owned(),
            message: message.to_owned(),
        };
        if let Some(attached) = session.slaythedata.as_mut() {
            attached.mark_blocked(blocked);
        }
        let snapshot = session.snapshot();
        self.append_slaythedata_trace(session_id, "blocked", json!(snapshot.slaythedata))?;
        Ok(snapshot)
    }

    fn advance_automation_plan_after_action(
        &mut self,
        session_id: &SessionId,
        sent_action: &crate::model::LegalAction,
        clear_on_mismatch: bool,
    ) -> LiveResult<()> {
        let latest_state = self.session(session_id)?.latest_state.clone();
        let session = self.session_mut(session_id)?;
        let Some(plan) = session.automation.plan.as_mut() else {
            return Ok(());
        };
        let next_index = plan.played_actions;
        let Some(expected) = plan.actions.get(next_index).cloned() else {
            session.automation.planned_action = None;
            return Ok(());
        };

        if !planned_step_matches_action(&expected, sent_action) {
            if clear_on_mismatch {
                session.automation.state = AutomationState::Done;
                session.automation.planned_action = None;
                session.automation.plan = None;
                session.automation.blocked = None;
                session.automation.last_message =
                    Some("manual action differed from plan; plan cleared".to_owned());
            }
            return Ok(());
        }

        plan.played_actions = (plan.played_actions + 1).min(plan.actions.len());
        session.automation.planned_action = latest_state.as_ref().and_then(|state| {
            plan.actions
                .get(plan.played_actions)
                .cloned()
                .and_then(|step| {
                    let live_step = bind_plan_step_to_live_action(state, &step)?;
                    if let Some(plan_step) = plan.actions.get_mut(plan.played_actions) {
                        *plan_step = live_step.clone();
                    }
                    Some(live_step)
                })
        });
        if clear_on_mismatch {
            session.automation.state = if session.automation.planned_action.is_some() {
                AutomationState::ReadyToSend
            } else {
                AutomationState::Done
            };
            session.automation.blocked = None;
            session.automation.last_message = Some("manual action matched plan".to_owned());
        }
        Ok(())
    }

    fn automation_resume_if_idle_or_done(&mut self, session_id: &SessionId) -> LiveResult<()> {
        if self.session(session_id)?.automation.state == AutomationState::Paused {
            return Err(LiveError::Blocked("automation is paused".to_owned()));
        }
        Ok(())
    }

    fn next_session_id(&mut self) -> SessionId {
        let id = SessionId(format!("session-{}", self.next_session));
        self.next_session += 1;
        id
    }

    fn session(&self, session_id: &SessionId) -> LiveResult<&SessionData> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| LiveError::NotFound(format!("session {}", session_id.0)))
    }

    fn session_mut(&mut self, session_id: &SessionId) -> LiveResult<&mut SessionData> {
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| LiveError::NotFound(format!("session {}", session_id.0)))
    }
}

fn fidelity_status_reusable_after_append(status: &FidelityStatus) -> bool {
    status.kind == FidelityKind::Unknown
        && status
            .message
            .as_deref()
            .is_some_and(|message| message.starts_with("seed-start replay reached boundary "))
}

fn fidelity_loss_status_in_tail(path: &Path, start: u64) -> LiveResult<Option<FidelityStatus>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<TraceRecord>(&line) else {
            continue;
        };
        if let TraceRecord::Error {
            reason_code,
            message,
            ..
        } = record
        {
            if reason_code == "fidelity_lost" {
                return Ok(Some(FidelityStatus {
                    kind: FidelityKind::Lost,
                    first_divergent_step: None,
                    compact_diff: vec![message.clone()],
                    message: Some(message),
                }));
            }
        }
    }
    Ok(None)
}

fn action_source_state_id(action: &LegalAction) -> Option<&str> {
    action
        .command
        .get("source_state_id")
        .and_then(serde_json::Value::as_str)
}

fn state_matches_source_state_id(state: &LiveState, source_state_id: &str) -> bool {
    state_state_id(state).is_some_and(|state_id| state_id == source_state_id)
}

fn state_state_id(state: &LiveState) -> Option<&str> {
    state
        .raw
        .pointer("/summary/state_id")
        .or_else(|| state.raw.pointer("/current_state/state_id"))
        .or_else(|| state.raw.pointer("/current_state/message/state_id"))
        .and_then(serde_json::Value::as_str)
}

fn session_number(session_id: &SessionId) -> Option<u64> {
    session_id
        .0
        .strip_prefix("session-")
        .and_then(|number| number.parse::<u64>().ok())
}

fn planned_step_matches_action(
    expected: &crate::model::AutomationPlannedAction,
    action: &crate::model::LegalAction,
) -> bool {
    if expected.kind != action.kind {
        return false;
    }
    if let Some(expected_command) = expected.command.as_deref() {
        return action
            .command
            .get("command")
            .and_then(|value| value.as_str())
            .is_some_and(|command| command.eq_ignore_ascii_case(expected_command));
    }
    expected.action_id == action.id && expected.label == action.label
}
