use crate::{
    automation::{
        bind_plan_step_to_live_action, blocked as automation_blocked, plan_action_with_warm_start,
    },
    bridge::BridgeManager,
    fidelity::FidelityChecker,
    model::{
        ActionId, AutomationConfig, AutomationJobSnapshot, AutomationState, BlockedState, BridgeId,
        BrokenSlayTheDataRun, FidelityKind, FidelityStatus, LegalAction, LegalActionKind,
        LiveError, LivePhase, LiveResult, LiveState, RunConfig, SessionId, SessionLifecycle,
        SessionListItem, SessionSnapshot, SlayTheDataRunSummary, SlayTheDataSearchFilters,
        TraceRecord,
    },
    operator_actions::{request_state_action, start_run_action},
    session_blocking::record_blocked,
    session_recovery,
    session_response::append_bridge_response_and_state,
    session_state::{lifecycle_for_fidelity, metadata_record, FidelityCache, SessionData},
    slaythedata::{
        is_new_act_entry_map, is_unsettled_neow_map_state, AttachedSlayTheDataRun, SlayTheDataIndex,
    },
    trace_writer::{read_records, TraceWriter},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use sts_verify::{
    import_communication_mod_trace, serialize_communication_mod_trace,
    verify_seed_start_communication_mod_trace, TraceLine, TraceMetadata,
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

    pub fn trace_root(&self) -> &Path {
        &self.trace_root
    }

    pub fn slaythedata_db_path(&self) -> &Path {
        self.slaythedata_index.db_path()
    }

    pub(crate) fn slaythedata_index(&self) -> &SlayTheDataIndex {
        &self.slaythedata_index
    }

    pub fn list_bridges(&self) -> LiveResult<Vec<crate::model::BridgeStatus>> {
        self.bridge.list_bridges()
    }

    pub fn request_bridge_state(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.bridge.request_state(bridge_id)
    }

    pub fn abandon_bridge_run(&mut self, bridge_id: &BridgeId) -> LiveResult<LiveState> {
        self.bridge.abandon_run(bridge_id)
    }

    pub fn send_bridge_command(
        &mut self,
        bridge_id: &BridgeId,
        state: &LiveState,
        command: &str,
        kind: LegalActionKind,
        label: &str,
    ) -> LiveResult<LiveState> {
        let action = LegalAction {
            id: ActionId(label.to_ascii_lowercase().replace(' ', "-")),
            kind,
            label: label.to_owned(),
            enabled: true,
            command: json!({
                "transport": "communication_mod",
                "command": command,
                "source_state_id": state_state_id(state),
            }),
            disabled_reason: None,
        };
        self.bridge.send_action(bridge_id, &action)
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

    pub fn observe_existing_session_ids(&mut self) -> LiveResult<()> {
        for path in session_recovery::trace_paths(&self.trace_root)? {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            session_recovery::bump_next_session(
                &mut self.next_session,
                &SessionId(stem.to_owned()),
            );
        }
        Ok(())
    }

    pub fn recover_session(&mut self, path: impl AsRef<Path>) -> LiveResult<SessionSnapshot> {
        let path = path.as_ref();
        let session = session_recovery::recover_session(path)?;
        Ok(self.insert_recovered_session(session))
    }

    pub(crate) fn insert_recovered_session(&mut self, session: SessionData) -> SessionSnapshot {
        if let Some(existing) = self.sessions.get(&session.id) {
            return existing.snapshot();
        }
        session_recovery::bump_next_session(&mut self.next_session, &session.id);
        let snapshot = session.snapshot();
        self.sessions.insert(session.id.clone(), session);
        snapshot
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

    pub fn list_session_items(&self) -> Vec<SessionListItem> {
        let mut items = self
            .sessions
            .values()
            .map(|session| SessionListItem {
                session_id: session.id.clone(),
                lifecycle: session.lifecycle.clone(),
            })
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            session_number(&left.session_id)
                .cmp(&session_number(&right.session_id))
                .then_with(|| left.session_id.0.cmp(&right.session_id.0))
        });
        items
    }

    pub fn clear_other_traces(&mut self, current_session_id: &SessionId) -> LiveResult<usize> {
        self.session(current_session_id)?;
        let current_trace_path = self
            .session(current_session_id)?
            .trace_writer
            .path()
            .to_path_buf();
        let mut deleted = 0;
        for path in session_recovery::trace_paths(&self.trace_root)? {
            if path == current_trace_path {
                continue;
            }
            fs::remove_file(&path)?;
            deleted += 1;
        }
        self.sessions.retain(|id, _| id == current_session_id);
        Ok(deleted)
    }

    pub fn copy_trace_to_permanent_corpus(
        &self,
        session_id: &SessionId,
        permanent_root: &Path,
    ) -> LiveResult<PathBuf> {
        let source = self.session(session_id)?.trace_writer.path();
        let file_name = source.file_name().ok_or_else(|| {
            LiveError::Blocked(format!("trace path has no file name: {}", source.display()))
        })?;
        fs::create_dir_all(permanent_root)?;
        let destination = permanent_root.join(format!("trace-{}", file_name.to_string_lossy()));
        if destination.exists() {
            return Ok(destination);
        }
        fs::copy(source, &destination)?;
        Ok(destination)
    }

    pub fn copy_verified_trace_prefix_to_permanent_corpus(
        &self,
        session_id: &SessionId,
        permanent_root: &Path,
    ) -> LiveResult<PathBuf> {
        let source = self.session(session_id)?.trace_writer.path();
        let file_name = source.file_name().ok_or_else(|| {
            LiveError::Blocked(format!("trace path has no file name: {}", source.display()))
        })?;
        fs::create_dir_all(permanent_root)?;
        let content = fs::read_to_string(source)?;
        let report = verify_seed_start_communication_mod_trace(&content).map_err(|error| {
            LiveError::Blocked(format!(
                "cannot verify trace before adding it to the permanent corpus: {error}"
            ))
        })?;
        let first_failing_step = report
            .unexpected_diffs
            .iter()
            .map(|diff| diff.action_step)
            .chain(
                report
                    .unsupported
                    .iter()
                    .map(|transition| transition.action_step),
            )
            .min();
        if first_failing_step.is_none()
            && (report.ignored_tail_actions != 0
                || report
                    .seed_start
                    .as_ref()
                    .is_some_and(|seed_start| seed_start.failed))
        {
            return Err(LiveError::Blocked(
                "trace has an unverified tail action or failed strict seed-start boundary"
                    .to_owned(),
            ));
        }

        let (destination, retained_content) = if let Some(failing_step) = first_failing_step {
            let trace = import_communication_mod_trace(&content).map_err(|error| {
                LiveError::Blocked(format!(
                    "cannot retain the verified trace prefix before step {failing_step}: {error}"
                ))
            })?;
            let failure_index = trace
                .lines
                .iter()
                .position(
                    |line| matches!(line, TraceLine::Action(action) if action.step == failing_step),
                )
                .ok_or_else(|| {
                    LiveError::Blocked(format!(
                        "cannot locate failing action step {failing_step} in the imported trace"
                    ))
                })?;
            let retained_lines = trace.lines[..failure_index]
                .iter()
                .filter(|line| !matches!(line, TraceLine::Metadata(_)))
                .cloned()
                .collect::<Vec<_>>();
            let retained_step = retained_lines
                .iter()
                .filter_map(|line| match line {
                    TraceLine::Action(action) => Some(action.step),
                    _ => None,
                })
                .max()
                .ok_or_else(|| {
                    LiveError::Blocked(
                        "trace has no verified action before its first failure".to_owned(),
                    )
                })?;
            let mut metadata = trace.metadata.unwrap_or(TraceMetadata {
                schema: 1,
                source: "communication_mod".to_owned(),
                client: None,
                mode: None,
                started_at: None,
                ended_at: None,
                event: None,
            });
            metadata.event = Some(format!(
                "retained_verified_prefix_through_step={retained_step}; excluded_failure_step={failing_step}"
            ));
            let retained_content = serialize_communication_mod_trace(&metadata, &retained_lines);
            let retained_report = verify_seed_start_communication_mod_trace(&retained_content)
                .map_err(|error| {
                    LiveError::Blocked(format!(
                        "cannot verify retained corpus prefix through step {retained_step}: {error}"
                    ))
                })?;
            if !retained_report.unexpected_diffs.is_empty()
                || !retained_report.unsupported.is_empty()
                || retained_report.ignored_tail_actions != 0
                || retained_report
                    .seed_start
                    .as_ref()
                    .is_some_and(|seed_start| seed_start.failed)
            {
                return Err(LiveError::Blocked(format!(
                    "retained corpus prefix through step {retained_step} is not fidelity-clean"
                )));
            }
            let source_stem = source
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("session");
            (
                permanent_root.join(format!(
                    "trace-{source_stem}.retained.step{retained_step}.jsonl"
                )),
                Some(retained_content),
            )
        } else {
            (
                permanent_root.join(format!("trace-{}", file_name.to_string_lossy())),
                None,
            )
        };
        persist_verified_trace(source, &destination, retained_content.as_deref())?;
        Ok(destination)
    }

    pub fn attached_slaythedata_run_id(&self, session_id: &SessionId) -> LiveResult<Option<i64>> {
        if let Some(run_id) = self
            .session(session_id)?
            .slaythedata
            .as_ref()
            .map(|run| run.summary.id)
        {
            return Ok(Some(run_id));
        }
        self.recorded_slaythedata_run_id(session_id)
    }

    pub fn ensure_slaythedata_attachment(
        &mut self,
        session_id: &SessionId,
    ) -> LiveResult<Option<SessionSnapshot>> {
        if self.session(session_id)?.slaythedata.is_some() {
            return Ok(Some(self.session(session_id)?.snapshot()));
        }
        let Some(run_id) = self.recorded_slaythedata_run_id(session_id)? else {
            return Ok(None);
        };
        self.attach_slaythedata_run(session_id, run_id).map(Some)
    }

    fn recorded_slaythedata_run_id(&self, session_id: &SessionId) -> LiveResult<Option<i64>> {
        let records = read_records(self.session(session_id)?.trace_writer.path())?;
        Ok(records.iter().rev().find_map(|record| {
            let TraceRecord::SlayTheData { event, details, .. } = record else {
                return None;
            };
            match event.as_str() {
                "attach_run" => details.pointer("/run/id").and_then(Value::as_i64),
                "sent_action" | "skip_guidance" | "skip_shop" => details
                    .pointer("/attached_run/id")
                    .or_else(|| details.pointer("/slaythedata/attached_run/id"))
                    .and_then(Value::as_i64),
                _ => None,
            }
        }))
    }

    pub fn mark_slaythedata_run_in_corpus(&self, run_id: i64, trace_path: &Path) -> LiveResult<()> {
        self.slaythedata_index.mark_in_corpus(run_id, trace_path)
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
        self.send_action_with_fidelity(session_id, action_id, true, true)
    }

    fn send_action_with_fidelity(
        &mut self,
        session_id: &SessionId,
        action_id: &ActionId,
        refresh_fidelity: bool,
        advance_manual_plan_after_success: bool,
    ) -> LiveResult<SessionSnapshot> {
        let (mut action, mut source_state) = {
            let session = self.session_mut(session_id)?;
            if matches!(session.lifecycle, SessionLifecycle::Blocked) {
                return Err(LiveError::Blocked("session is blocked".to_owned()));
            }
            let state = session
                .latest_state
                .as_ref()
                .ok_or_else(|| LiveError::Blocked("session has no live state".to_owned()))?;
            let action = state
                .legal_actions
                .iter()
                .find(|action| &action.id == action_id)
                .cloned()
                .ok_or_else(|| {
                    LiveError::InvalidAction(format!("unknown action {}", action_id.0))
                })?;
            (action, state.clone())
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
        let first_send = self.bridge.send_action(&bridge_id, &action);
        let state = match first_send {
            Ok(state) => state,
            Err(LiveError::Bridge(message)) if message == "stale bridge action rejected" => {
                let refreshed = self.bridge.request_state(&bridge_id)?;
                let refreshed_action = refreshed_equivalent_action(&refreshed, &action)
                    .cloned()
                    .ok_or_else(|| {
                        LiveError::Bridge(
                            "stale bridge action changed after refreshing live state".to_owned(),
                        )
                    })?;
                action = refreshed_action;
                source_state = refreshed;
                self.bridge.send_action(&bridge_id, &action)?
            }
            Err(LiveError::Bridge(message))
                if message == "timed out waiting for observed state update" =>
            {
                // The control bridge can time out after CommunicationMod has
                // already accepted and applied the command. Re-read the live
                // state before declaring failure so the accepted action is not
                // omitted from the trace and accidentally sent a second time.
                let refreshed = self.bridge.request_state(&bridge_id)?;
                let Some(source_state_id) = action_source_state_id(&action) else {
                    self.block_session(session_id, "bridge_error", &message)?;
                    return Err(LiveError::Bridge(message));
                };
                if state_matches_source_state_id(&refreshed, source_state_id)
                    || same_observed_game_state(&source_state, &refreshed)
                {
                    self.block_session(session_id, "bridge_error", &message)?;
                    return Err(LiveError::Bridge(message));
                }
                refreshed
            }
            Err(err) => {
                let message = err.to_string();
                self.block_session(session_id, "bridge_error", &message)?;
                return Err(err);
            }
        };
        let state = self.wait_for_fresh_action_state(&bridge_id, &action, &source_state, state)?;
        let post_action_state = state.clone();
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
            let skipped_pending_card_reward = self
                .session_mut(session_id)?
                .slaythedata
                .as_mut()
                .and_then(|attached| {
                    attached.skip_unavailable_pending_card_reward(&post_action_state)
                });
            if let Some(step_index) = skipped_pending_card_reward {
                let snapshot = self.session(session_id)?.snapshot();
                self.append_slaythedata_trace(
                    session_id,
                    "skip_guidance",
                    json!({
                        "step_index": step_index,
                        "reason": "manual card reward resolution advanced beyond an unavailable recorded reward",
                        "slaythedata": snapshot.slaythedata,
                    }),
                )?;
            }
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
        source_state: &LiveState,
        initial: LiveState,
    ) -> LiveResult<LiveState> {
        let Some(source_state_id) = action_source_state_id(action) else {
            return Ok(initial);
        };
        if !state_matches_source_state_id(&initial, source_state_id)
            && !same_observed_game_state(source_state, &initial)
            && transition_state_is_ready(source_state, &initial)
            && !is_unsettled_action_transition(action, &initial)
        {
            return Ok(initial);
        }

        let deadline = Instant::now() + ACTION_STATE_POLL_TIMEOUT;
        loop {
            let refreshed = self.bridge.request_state(bridge_id)?;
            if !state_matches_source_state_id(&refreshed, source_state_id)
                && !same_observed_game_state(source_state, &refreshed)
                && transition_state_is_ready(source_state, &refreshed)
                && !is_unsettled_action_transition(action, &refreshed)
            {
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

    pub fn slaythedata_run_json(&self, run_id: i64) -> LiveResult<serde_json::Value> {
        let (_, raw_run_json) = self.slaythedata_index.load_or_materialize_run(run_id)?;
        Ok(serde_json::from_str(&raw_run_json)?)
    }

    pub fn mark_slaythedata_run_broken(
        &self,
        run_id: i64,
        reason: Option<&str>,
    ) -> LiveResult<BrokenSlayTheDataRun> {
        self.slaythedata_index.mark_broken(run_id, reason)
    }

    pub fn unmark_slaythedata_run_broken(&self, run_id: i64) -> LiveResult<bool> {
        self.slaythedata_index.unmark_broken(run_id)
    }

    pub fn attach_slaythedata_run(
        &mut self,
        session_id: &SessionId,
        run_id: i64,
    ) -> LiveResult<SessionSnapshot> {
        let trace_path = self.session(session_id)?.trace_writer.path().to_path_buf();
        let latest_state = self.session(session_id)?.latest_state.clone();
        let (summary, raw_run_json) = self.slaythedata_index.load_or_materialize_run(run_id)?;
        let mut attached = AttachedSlayTheDataRun::from_raw(summary, &raw_run_json)?;
        restore_slaythedata_progress(&mut attached, &trace_path, latest_state.as_ref())?;
        let details = json!({
            "run": attached.summary,
            "preflight_steps": attached.report.steps.len(),
            "route_fully_checked": attached.report.route_fully_checked,
            "diagnostics": attached.report.diagnostics,
            "aligned_guidance_step_index": attached.next_step_index,
        });
        let session = self.session_mut(session_id)?;
        session.slaythedata = Some(attached);
        let snapshot = session.snapshot();
        self.append_slaythedata_trace(session_id, "attach_run", details)?;
        Ok(snapshot)
    }

    pub fn slaythedata_send_next(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        if let Some(attached) = self.session_mut(session_id)?.slaythedata.as_mut() {
            attached.blocked = None;
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
        if let Some(action) = auto_close_slaythedata_overlay_action(&state) {
            let action_id = action.id.clone();
            self.append_slaythedata_trace(
                session_id,
                "auto_close_overlay",
                json!({"action": action}),
            )?;
            if let Err(error) = self.send_action_with_fidelity(session_id, &action_id, false, false)
            {
                let message = error.to_string();
                return self.block_slaythedata(session_id, "slaythedata_send_failed", &message);
            }
            self.refresh_fidelity(session_id)?;
            if self.session(session_id)?.fidelity.kind != FidelityKind::Ok {
                return self.block_slaythedata(
                    session_id,
                    "slaythedata_transition_not_ok",
                    "SlayTheData automatic overlay close did not verify with fidelity ok",
                );
            }
            return Ok(self.session(session_id)?.snapshot());
        }
        if is_unsettled_neow_map_state(&state) {
            let session = self.session_mut(session_id)?;
            if let Some(attached) = session.slaythedata.as_mut() {
                attached.last_message = Some(
                    "Waiting for the Neow-to-map transition to settle before choosing a map node"
                        .to_owned(),
                );
            }
            return Ok(session.snapshot());
        }
        if state.phase == LivePhase::Combat {
            let session = self.session_mut(session_id)?;
            if let Some(attached) = session.slaythedata.as_mut() {
                attached.last_message =
                    Some("SlayTheData guidance paused in combat; use the combat agent".to_owned());
            }
            return Ok(session.snapshot());
        }
        if state.phase == LivePhase::Reward && offered_relic_changes_card_reward_schedule(&state) {
            return self.block_slaythedata(
                session_id,
                "reward_rng_critical_relic_mismatch",
                "An offered relic changes future card-reward RNG and does not match the simulator; resolve this reward manually before continuing",
            );
        }
        if let Some(action) = auto_take_slaythedata_reward_action(&state) {
            let action_id = action.id.clone();
            self.append_slaythedata_trace(
                session_id,
                "auto_take_reward",
                json!({"action": action}),
            )?;
            if let Err(error) = self.send_action_with_fidelity(session_id, &action_id, false, false)
            {
                let message = error.to_string();
                return self.block_slaythedata(session_id, "slaythedata_send_failed", &message);
            }
            self.refresh_fidelity(session_id)?;
            if self.session(session_id)?.fidelity.kind != FidelityKind::Ok {
                return self.block_slaythedata(
                    session_id,
                    "slaythedata_transition_not_ok",
                    "SlayTheData automatic reward transition did not verify with fidelity ok",
                );
            }
            return Ok(self.session(session_id)?.snapshot());
        }
        {
            let session = self.session_mut(session_id)?;
            if let Some(attached) = session.slaythedata.as_mut() {
                attached.skip_completed_route_on_live_map(&state);
                attached.rewind_future_card_reward_to_live_map(&state);
                attached.rewind_future_unmatched_route_to_live_map(&state);
            }
        }
        let skipped_pending_card_reward = {
            let session = self.session_mut(session_id)?;
            session
                .slaythedata
                .as_mut()
                .and_then(|attached| attached.skip_unavailable_pending_card_reward(&state))
        };
        if let Some(step_index) = skipped_pending_card_reward {
            let snapshot = self.session(session_id)?.snapshot();
            self.append_slaythedata_trace(
                session_id,
                "skip_guidance",
                json!({
                    "step_index": step_index,
                    "reason": "pending card reward is unavailable outside a reward screen",
                    "slaythedata": snapshot.slaythedata,
                }),
            )?;
            return Ok(snapshot);
        }
        let aligned_past_completed_guidance = {
            let session = self.session_mut(session_id)?;
            session
                .slaythedata
                .as_mut()
                .and_then(|attached| attached.align_past_completed_non_shop_guidance(&state))
        };
        if let Some((skipped_step_index, aligned_step_index, skipped_code)) =
            aligned_past_completed_guidance
        {
            let snapshot = self.session(session_id)?.snapshot();
            self.append_slaythedata_trace(
                session_id,
                "skip_guidance",
                json!({
                    "step_index": skipped_step_index,
                    "aligned_step_index": aligned_step_index,
                    "reason": format!(
                        "live map floor is past unavailable {skipped_code} guidance"
                    ),
                    "slaythedata": snapshot.slaythedata,
                }),
            )?;
            return Ok(snapshot);
        }
        let unavailable_shop_purge = {
            let session = self.session_mut(session_id)?;
            session
                .slaythedata
                .as_mut()
                .and_then(|attached| attached.skip_unavailable_shop_purge(&state))
        };
        if let Some((step_index, target)) = unavailable_shop_purge {
            let snapshot = self.session(session_id)?.snapshot();
            self.append_slaythedata_trace(
                session_id,
                "skip_guidance",
                json!({
                    "step_index": step_index,
                    "reason": "shop purge target is absent from the live deck",
                    "target": target,
                    "slaythedata": snapshot.slaythedata,
                }),
            )?;
            return Ok(snapshot);
        }
        let unavailable_shop_purchase = {
            let session = self.session(session_id)?;
            session
                .slaythedata
                .as_ref()
                .and_then(|attached| attached.unavailable_shop_purchase(&state))
        };
        if let Some((step_index, item)) = unavailable_shop_purchase {
            let rng_critical = is_rng_critical_shop_purchase(&item);
            let blocked = BlockedState {
                reason_code: if rng_critical {
                    "shop_purchase_rng_critical_unavailable"
                } else {
                    "shop_purchase_unavailable"
                }
                .to_owned(),
                message: if rng_critical {
                    format!(
                        "recorded SlayTheData shop purchase {item:?} is unavailable; continuing would desynchronize card-reward RNG"
                    )
                } else {
                    format!(
                        "recorded SlayTheData shop purchase {item:?} is unavailable in the live shop; use Skip shop to continue"
                    )
                },
            };
            let session = self.session_mut(session_id)?;
            session
                .slaythedata
                .as_mut()
                .expect("attached run was checked above")
                .mark_blocked(blocked);
            let snapshot = session.snapshot();
            self.append_slaythedata_trace(
                session_id,
                "blocked",
                json!({
                    "step_index": step_index,
                    "item": item,
                    "slaythedata": snapshot.slaythedata,
                }),
            )?;
            return Ok(snapshot);
        }
        let (step_index, step_code, action) = {
            let session = self.session_mut(session_id)?;
            let Some(attached) = session.slaythedata.as_mut() else {
                return Err(LiveError::Blocked(
                    "no SlayTheData run is attached to this session".to_owned(),
                ));
            };
            match attached.ready_action(&state) {
                Ok((index, action)) => {
                    let step_code = attached
                        .report
                        .steps
                        .get(index)
                        .map(|step| step.code.clone())
                        .unwrap_or_default();
                    (index, step_code, action)
                }
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
            if slaythedata_step_advances(&step_code, &action, session.latest_state.as_ref()) {
                attached.mark_sent_after_action(step_index, &action);
            }
        }
        let snapshot = session.snapshot();
        self.append_slaythedata_trace(session_id, "sent_action", json!(snapshot.slaythedata))?;
        Ok(snapshot)
    }

    pub fn slaythedata_auto_play(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let limit = 10_000;
        for _ in 0..limit {
            if let Some(snapshot) = self.slaythedata_auto_play_combat_handoff(session_id)? {
                return Ok(snapshot);
            }
            let after = self.slaythedata_send_next(session_id)?;
            if after.slaythedata.blocked.is_some() {
                return Ok(after);
            }
            if after
                .latest_state
                .as_ref()
                .is_some_and(slaythedata_state_is_temporarily_actionless)
            {
                self.request_state_with_fidelity(session_id, false)?;
                continue;
            }
            let after_sent_into_combat = after
                .latest_state
                .as_ref()
                .is_some_and(|state| state.phase == LivePhase::Combat);
            if let Some(snapshot) = self.slaythedata_auto_play_combat_handoff(session_id)? {
                return Ok(snapshot);
            }
            if after_sent_into_combat {
                continue;
            }
            if after
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

    pub fn slaythedata_start_auto_play(
        &mut self,
        session_id: &SessionId,
    ) -> LiveResult<SessionSnapshot> {
        let session = self.session_mut(session_id)?;
        let attached = session.slaythedata.as_mut().ok_or_else(|| {
            LiveError::Blocked("no SlayTheData run is attached to this session".to_owned())
        })?;
        attached.auto_play_paused = false;
        attached.last_message = Some("SlayTheData auto-play started".to_owned());
        let snapshot = session.snapshot();
        self.append_slaythedata_trace(
            session_id,
            "auto_play_started",
            json!(snapshot.slaythedata),
        )?;
        Ok(snapshot)
    }

    pub fn slaythedata_auto_play_tick(
        &mut self,
        session_id: &SessionId,
    ) -> LiveResult<(SessionSnapshot, bool)> {
        let paused = self
            .session(session_id)?
            .slaythedata
            .as_ref()
            .is_some_and(|attached| attached.auto_play_paused);
        if paused {
            return Ok((self.session_snapshot(session_id)?, false));
        }

        if self
            .session(session_id)?
            .latest_state
            .as_ref()
            .is_some_and(|state| state.phase == LivePhase::GameOver)
        {
            let message = "SlayTheData auto-play ended because the live run reached game over before the target run completed";
            let blocked = self.block_slaythedata(session_id, "game_over_before_target", message)?;
            return Ok((blocked, false));
        }

        let in_combat = self
            .session(session_id)?
            .latest_state
            .as_ref()
            .is_some_and(|state| state.phase == LivePhase::Combat);
        let snapshot = if in_combat {
            let automation_is_blocked = self.session(session_id)?.automation.blocked.is_some()
                || matches!(
                    self.session(session_id)?.automation.state,
                    AutomationState::Blocked | AutomationState::Failed
                );
            if automation_is_blocked {
                self.session_snapshot(session_id)?
            } else {
                if self.session(session_id)?.automation.state != AutomationState::AutoPlaying {
                    self.automation_start_auto_play(session_id)?;
                }
                let actions_sent = self.session(session_id)?.automation.executed_actions.len();
                self.automation_auto_play_tick(session_id, actions_sent)?.0
            }
        } else {
            self.slaythedata_send_next(session_id)?
        };
        if in_combat {
            if let Some(automation_blocked) = snapshot.automation.blocked.as_ref() {
                let message = format!(
                    "SlayTheData combat automation stopped: {} ({})",
                    automation_blocked.message, automation_blocked.reason_code
                );
                let blocked = self.block_slaythedata(
                    session_id,
                    "slaythedata_combat_automation_blocked",
                    &message,
                )?;
                return Ok((blocked, false));
            }
        }
        // The combat tick can finish the fight and return the first reward/event
        // state. Its attached advisor still describes the guidance that was
        // pending while combat was active; let the next tick reconcile that
        // guidance against the new phase instead of reporting a false stop.
        if in_combat
            && snapshot
                .latest_state
                .as_ref()
                .is_some_and(|state| state.phase != LivePhase::Combat)
        {
            return Ok((snapshot, true));
        }
        if snapshot
            .latest_state
            .as_ref()
            .is_some_and(slaythedata_state_is_temporarily_actionless)
        {
            let refreshed = self.request_state_with_fidelity(session_id, false)?;
            return Ok((refreshed, true));
        }
        // Entering the merchant changes the live phase after the shop-entry
        // action, while the snapshot's advisor still reflects the pre-entry
        // high-level step. Give the next tick a chance to bind the concrete
        // shop item or classify it as unavailable (which enables Skip shop)
        // instead of prematurely converting the stale advisor into the generic
        // no-live-action error.
        let entered_shop_with_stale_guidance = snapshot.slaythedata.blocked.is_none()
            && snapshot
                .latest_state
                .as_ref()
                .is_some_and(|state| state.phase == LivePhase::Shop)
            && snapshot
                .slaythedata
                .advisor
                .as_ref()
                .is_some_and(|advisor| {
                    matches!(
                        advisor.code.as_str(),
                        "guided_shop_purchase" | "guided_shop_purge"
                    ) && advisor.action_id.is_none()
                });
        if entered_shop_with_stale_guidance {
            return Ok((snapshot, true));
        }
        let pending_reward_binding = snapshot.slaythedata.blocked.is_none()
            && snapshot.latest_state.as_ref().is_some_and(|state| {
                slaythedata_reward_binding_is_pending(
                    state,
                    snapshot
                        .slaythedata
                        .advisor
                        .as_ref()
                        .map(|advisor| advisor.code.as_str()),
                    snapshot
                        .slaythedata
                        .advisor
                        .as_ref()
                        .is_some_and(|advisor| advisor.action_id.is_some()),
                )
            });
        if pending_reward_binding {
            return Ok((snapshot, true));
        }
        let stale_completed_route = snapshot
            .slaythedata
            .blocked
            .as_ref()
            .is_some_and(|blocked| blocked.reason_code == "slaythedata_no_live_action")
            && snapshot
                .slaythedata
                .advisor
                .as_ref()
                .is_some_and(|advisor| advisor.code == "pending_room_resolution")
            && snapshot
                .latest_state
                .as_ref()
                .is_some_and(|state| state.phase == LivePhase::Map);
        if stale_completed_route {
            let state = snapshot
                .latest_state
                .as_ref()
                .expect("map state checked above")
                .clone();
            let realigned = self
                .session_mut(session_id)?
                .slaythedata
                .as_mut()
                .is_some_and(|attached| attached.align_progress_to_live_state(&state));
            if realigned {
                return Ok((self.session_snapshot(session_id)?, true));
            }
        }
        let should_continue = snapshot.slaythedata.blocked.is_none()
            && !snapshot.slaythedata.auto_play_paused
            && (snapshot
                .latest_state
                .as_ref()
                .is_some_and(|state| state.phase == LivePhase::Combat)
                || snapshot
                    .slaythedata
                    .advisor
                    .as_ref()
                    .is_some_and(|advisor| advisor.action_id.is_some()));
        if should_continue || snapshot.slaythedata.blocked.is_some() {
            return Ok((snapshot, should_continue));
        }
        let stale_pending_reward_state = snapshot.latest_state.as_ref().filter(|state| {
            !matches!(state.phase, LivePhase::Combat | LivePhase::Reward)
                && snapshot
                    .slaythedata
                    .advisor
                    .as_ref()
                    .is_some_and(|advisor| advisor.code == "pending_card_reward")
        });
        if let Some(state) = stale_pending_reward_state.cloned() {
            let skipped = self
                .session_mut(session_id)?
                .slaythedata
                .as_mut()
                .and_then(|attached| attached.skip_unavailable_pending_card_reward(&state));
            if skipped.is_some() {
                return Ok((self.session_snapshot(session_id)?, true));
            }
        }
        if let Some(advisor) = snapshot.slaythedata.advisor.as_ref() {
            let message = format!(
                "SlayTheData auto-play stopped at {} (floor {}, step {}): {}. No enabled live action was bound.",
                advisor.code, advisor.floor, advisor.ordinal, advisor.message
            );
            let blocked =
                self.block_slaythedata(session_id, "slaythedata_no_live_action", &message)?;
            return Ok((blocked, false));
        }
        let session = self.session_mut(session_id)?;
        if let Some(attached) = session.slaythedata.as_mut() {
            attached.last_message =
                Some("SlayTheData auto-play completed: no guided steps remain".to_owned());
        }
        let snapshot = session.snapshot();
        self.append_slaythedata_trace(
            session_id,
            "auto_play_completed",
            json!(snapshot.slaythedata),
        )?;
        Ok((snapshot, false))
    }

    pub fn slaythedata_fail_auto_play(
        &mut self,
        session_id: &SessionId,
        reason_code: &str,
        message: &str,
    ) -> LiveResult<SessionSnapshot> {
        self.block_slaythedata(session_id, reason_code, message)
    }

    pub fn slaythedata_pause(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let session = self.session_mut(session_id)?;
        let attached = session.slaythedata.as_mut().ok_or_else(|| {
            LiveError::Blocked("no SlayTheData run is attached to this session".to_owned())
        })?;
        attached.auto_play_paused = true;
        attached.last_message = Some("SlayTheData auto-play paused".to_owned());
        if session.automation.state == AutomationState::AutoPlaying {
            session.automation.state = AutomationState::Paused;
            session.automation.last_message = Some("automation paused".to_owned());
        }
        let snapshot = session.snapshot();
        self.append_slaythedata_trace(session_id, "auto_play_paused", json!(snapshot.slaythedata))?;
        Ok(snapshot)
    }

    pub fn slaythedata_skip_shop(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let (live_floor, trace_path) = {
            let session = self.session(session_id)?;
            let state = session
                .latest_state
                .as_ref()
                .filter(|state| state.phase == LivePhase::Shop);
            let Some(state) = state else {
                return Err(LiveError::Blocked(
                    "Skip shop is available only while the live shop is open".to_owned(),
                ));
            };
            (
                live_state_floor(state),
                session.trace_writer.path().to_path_buf(),
            )
        };
        let Some(live_floor) = live_floor else {
            return Err(LiveError::Blocked(
                "Skip shop could not identify the current live floor".to_owned(),
            ));
        };
        let records = read_records(&trace_path)?;
        let completed_manual_purge = trace_has_completed_shop_purge(&records, live_floor);
        let blocked_for_shop = self
            .session(session_id)?
            .slaythedata
            .as_ref()
            .and_then(|attached| attached.blocked.as_ref())
            .is_some_and(|blocked| blocked.reason_code == "shop_purchase_unavailable");
        if !blocked_for_shop {
            return Err(LiveError::Blocked(
                "Skip shop requires an unavailable SlayTheData shop purchase".to_owned(),
            ));
        }

        let (skipped, completed_purge_step) = {
            let attached = self
                .session_mut(session_id)?
                .slaythedata
                .as_mut()
                .expect("attached SlayTheData run was checked above");
            let skipped = attached.skip_current_shop_purchases();
            let completed_purge_step = completed_manual_purge
                .then(|| attached.skip_completed_shop_purge(live_floor))
                .flatten();
            (skipped, completed_purge_step)
        };
        if skipped.is_empty() {
            return Err(LiveError::Blocked(
                "SlayTheData has no remaining shop purchases to skip".to_owned(),
            ));
        }
        let snapshot = self.session(session_id)?.snapshot();
        self.append_slaythedata_trace(
            session_id,
            "skip_shop",
            json!({
                "purchases": skipped,
                "completed_manual_purge_step": completed_purge_step,
                "slaythedata": snapshot.slaythedata,
            }),
        )?;
        if completed_purge_step.is_some() {
            self.append_slaythedata_trace(
                session_id,
                "skip_guidance",
                json!(snapshot.slaythedata),
            )?;
        }

        // The next pending-room step owns the explicit Leave shop action.
        self.slaythedata_send_next(session_id)
    }

    fn slaythedata_auto_play_combat_handoff(
        &mut self,
        session_id: &SessionId,
    ) -> LiveResult<Option<SessionSnapshot>> {
        let in_combat = self
            .session(session_id)?
            .latest_state
            .as_ref()
            .is_some_and(|state| state.phase == LivePhase::Combat);
        if !in_combat {
            return Ok(None);
        }

        let snapshot = self.automation_auto_play(session_id)?;
        if snapshot.automation.blocked.is_some()
            || snapshot
                .latest_state
                .as_ref()
                .is_some_and(|state| state.phase == LivePhase::Combat)
        {
            return Ok(Some(snapshot));
        }
        Ok(None)
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
            self.refresh_fidelity_uncached(session_id)?;
        }
        if let Some(blocked) = self.block_if_automation_fidelity_not_ok(
            session_id,
            "automation requires fidelity ok before planning",
        )? {
            return Ok(blocked);
        }
        self.set_automation_state(session_id, AutomationState::Planning)?;
        let (config, state, warm_steps) = {
            let session = self.session(session_id)?;
            let state = session.latest_state.clone().ok_or_else(|| {
                LiveError::Blocked("automation cannot plan without a live state".to_owned())
            })?;
            (
                session.automation.config.clone(),
                state,
                cached_turn_suffix(&session.automation),
            )
        };

        match plan_action_with_warm_start(&config, &state, &warm_steps) {
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
            self.refresh_fidelity_uncached(session_id)?;
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
        if refreshed
            .latest_state
            .as_ref()
            .is_some_and(|state| !combat_state_is_actionable(state))
        {
            let session = self.session_mut(session_id)?;
            session.automation.state = AutomationState::AutoPlaying;
            session.automation.last_message =
                Some("waiting for the live combat state to become actionable".to_owned());
            return Ok((session.snapshot(), true));
        }
        self.refresh_fidelity_uncached(session_id)?;
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
        if plan_needs_turn_boundary_replan(plan) {
            session.automation.planned_action = None;
            return Ok(false);
        }
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
        plan.cache_hits += 1;
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
        self.refresh_fidelity_inner(session_id, true)
    }

    fn refresh_fidelity_uncached(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        self.refresh_fidelity_inner(session_id, false)
    }

    fn refresh_fidelity_inner(
        &mut self,
        session_id: &SessionId,
        allow_cache: bool,
    ) -> LiveResult<SessionSnapshot> {
        let path = self.session(session_id)?.trace_writer.path().to_path_buf();
        let trace_len = std::fs::metadata(&path)?.len();
        let previous_cache_trace_len = self
            .session(session_id)?
            .fidelity_cache
            .as_ref()
            .map(|cache| cache.trace_len);
        if allow_cache {
            if let Some(cached) = self.reusable_fidelity_cache(session_id, &path, trace_len)? {
                let clear_stale_sim_state = cached.sim_run_state.is_none()
                    && previous_cache_trace_len
                        .is_some_and(|previous| previous != cached.trace_len);
                self.apply_fidelity_result(
                    session_id,
                    cached.status.clone(),
                    cached.sim_run_state.clone(),
                    Some(cached),
                )?;
                if clear_stale_sim_state {
                    self.clear_latest_sim_run_state(session_id)?;
                }
                return Ok(self.session(session_id)?.snapshot());
            }
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
        Ok(Some(FidelityCache {
            trace_len,
            sim_run_state: None,
            ..cache
        }))
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
        if let Some(state) = session.latest_state.as_mut() {
            if let Some(raw) = state.raw.as_object_mut() {
                if let Some(sim_run_state) = sim_run_state {
                    raw.insert(
                        "sim_run_state".to_owned(),
                        serde_json::to_value(sim_run_state)?,
                    );
                }
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

    fn clear_latest_sim_run_state(&mut self, session_id: &SessionId) -> LiveResult<()> {
        if let Some(state) = self.session_mut(session_id)?.latest_state.as_mut() {
            if let Some(raw) = state.raw.as_object_mut() {
                raw.remove("sim_run_state");
            }
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
        let Some(plan) = session.automation.plan.as_ref() else {
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

        if let Some(plan) = session.automation.plan.as_mut() {
            plan.played_actions = (plan.played_actions + 1).min(plan.actions.len());
        }
        if prepare_plan_for_turn_boundary(&mut session.automation, sent_action, clear_on_mismatch) {
            // The next actionable state contains a newly drawn hand and newly
            // resolved enemy intents/powers.  A principal variation chosen on
            // the previous turn is not a decision for this turn, so force the
            // next automation tick through a fresh search instead of binding
            // its stale suffix.
            return Ok(());
        }
        let next_step = session.automation.plan.as_ref().and_then(|plan| {
            plan.actions
                .get(plan.played_actions)
                .cloned()
                .map(|step| (plan.played_actions, step))
        });
        let next_planned_action = match (latest_state.as_ref(), next_step.as_ref()) {
            (Some(state), Some((index, step))) => {
                if let Some(live_step) = bind_plan_step_to_live_action(state, step) {
                    if let Some(plan_step) = session
                        .automation
                        .plan
                        .as_mut()
                        .and_then(|plan| plan.actions.get_mut(*index))
                    {
                        *plan_step = live_step.clone();
                    }
                    Some(live_step)
                } else {
                    None
                }
            }
            _ => None,
        };
        session.automation.planned_action = next_planned_action;
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

    #[cfg(test)]
    pub(super) fn set_latest_state_for_test(
        &mut self,
        session_id: &SessionId,
        state: LiveState,
    ) -> LiveResult<()> {
        self.session_mut(session_id)?.latest_state = Some(state);
        Ok(())
    }
}

pub(crate) fn persist_verified_trace(
    source: &Path,
    destination: &Path,
    retained_content: Option<&str>,
) -> LiveResult<()> {
    if let Some(retained_content) = retained_content {
        // Retained prefixes are immutable snapshots named for their last verified
        // step, so an existing snapshot is already the requested artifact.
        if !destination.exists() {
            fs::write(destination, retained_content)?;
        }
    } else {
        // A clean session keeps growing after each recovery. Refresh the stable
        // corpus path so repeated promotions never leave an older short trace in
        // place under the same session name.
        fs::copy(source, destination)?;
    }
    Ok(())
}

fn prepare_plan_for_turn_boundary(
    automation: &mut AutomationJobSnapshot,
    sent_action: &LegalAction,
    manual_send: bool,
) -> bool {
    if sent_action.kind != LegalActionKind::EndTurn {
        return false;
    }
    automation.planned_action = None;
    if manual_send {
        automation.state = AutomationState::Done;
        automation.blocked = None;
        automation.last_message =
            Some("turn ended; fresh planning required for the next turn".to_owned());
    }
    true
}

fn plan_needs_turn_boundary_replan(plan: &crate::model::AutomationPlanSnapshot) -> bool {
    plan.played_actions > 0
        && plan
            .actions
            .get(plan.played_actions - 1)
            .is_some_and(|action| action.kind == LegalActionKind::EndTurn)
}

fn cached_turn_suffix(
    automation: &AutomationJobSnapshot,
) -> Vec<crate::model::AutomationPlannedAction> {
    automation
        .plan
        .as_ref()
        .filter(|plan| plan_needs_turn_boundary_replan(plan))
        .map(|plan| plan.actions[plan.played_actions..].to_vec())
        .unwrap_or_default()
}

pub(crate) fn refreshed_equivalent_action<'a>(
    state: &'a LiveState,
    stale_action: &LegalAction,
) -> Option<&'a LegalAction> {
    let stale_command = stale_action
        .command
        .get("command")
        .and_then(Value::as_str)?;
    state.legal_actions.iter().find(|action| {
        action.enabled
            && action.id == stale_action.id
            && action.kind == stale_action.kind
            && action.command.get("command").and_then(Value::as_str) == Some(stale_command)
    })
}

fn fidelity_status_reusable_after_append(status: &FidelityStatus) -> bool {
    status.kind == FidelityKind::Unknown
        && status
            .message
            .as_deref()
            .is_some_and(|message| message.starts_with("seed-start replay reached boundary "))
}

pub(crate) fn slaythedata_state_is_temporarily_actionless(state: &LiveState) -> bool {
    is_unsettled_neow_map_state(state)
}

pub(crate) fn slaythedata_reward_binding_is_pending(
    state: &LiveState,
    advisor_code: Option<&str>,
    advisor_has_action: bool,
) -> bool {
    state.phase == LivePhase::Reward
        && advisor_code == Some("pending_card_reward")
        && !advisor_has_action
}

fn live_state_floor(state: &LiveState) -> Option<u32> {
    state
        .raw
        .pointer("/summary/floor")
        .or_else(|| state.raw.pointer("/current_state/message/game_state/floor"))
        .and_then(Value::as_u64)
        .and_then(|floor| u32::try_from(floor).ok())
}

fn state_is_shop_purge_grid(state: &LiveState) -> bool {
    state
        .raw
        .pointer("/summary/screen_state/for_purge")
        .or_else(|| {
            state
                .raw
                .pointer("/current_state/message/game_state/screen_state/for_purge")
        })
        .and_then(Value::as_bool)
        == Some(true)
}

pub(crate) fn trace_has_completed_shop_purge(records: &[TraceRecord], floor: u32) -> bool {
    let mut latest_state = None;
    let mut confirmed_purge_floor = None;
    for record in records {
        match record {
            TraceRecord::State { state, .. } => {
                if confirmed_purge_floor == Some(floor)
                    && state.phase == LivePhase::Shop
                    && live_state_floor(state) == Some(floor)
                {
                    return true;
                }
                latest_state = Some(state);
            }
            TraceRecord::Action { action, .. }
                if action.kind == LegalActionKind::Confirm
                    && action.label.eq_ignore_ascii_case("confirm")
                    && latest_state.is_some_and(state_is_shop_purge_grid) =>
            {
                confirmed_purge_floor = latest_state.and_then(live_state_floor);
            }
            _ => {}
        }
    }
    false
}

fn restore_slaythedata_progress(
    attached: &mut AttachedSlayTheDataRun,
    trace_path: &Path,
    latest_state: Option<&LiveState>,
) -> LiveResult<()> {
    let records = read_records(trace_path)?;
    restore_slaythedata_progress_from_records(attached, &records, latest_state);
    Ok(())
}

fn restore_slaythedata_progress_from_records(
    attached: &mut AttachedSlayTheDataRun,
    records: &[TraceRecord],
    latest_state: Option<&LiveState>,
) {
    let progress_checkpoint = latest_slaythedata_progress_checkpoint(records, attached.summary.id);
    let restored_persisted_progress =
        if let Some((record_index, next_step_index)) = progress_checkpoint {
            // Persisted SlayTheData progress also covers dynamic steps without bridge commands.
            attached.restore_next_step_index(next_step_index);
            // Reconcile only actions recorded after the checkpoint.  This
            // catches an action that reached the game but failed fidelity
            // verification before sent_action could persist the new cursor.
            // Stop at a later attachment so actions from another guidance
            // lifetime cannot leak into this one.
            for record in records.iter().skip(record_index.saturating_add(1)) {
                if matches!(
                    record,
                    TraceRecord::SlayTheData { event, .. } if event == "attach_run"
                ) {
                    break;
                }
                if let TraceRecord::Action { action, .. } = record {
                    attached.restore_progress_from_recorded_action(action);
                }
            }
            true
        } else {
            for record in records {
                if let TraceRecord::Action { action, .. } = record {
                    attached.restore_progress_from_recorded_action(action);
                }
            }
            false
        };
    if let Some(state) = latest_state {
        attached.skip_manually_resolved_unavailable_neow(state);
        let live_shop_floor = (state.phase == LivePhase::Shop)
            .then(|| state.raw.pointer("/summary/floor").and_then(Value::as_u64))
            .flatten()
            .and_then(|floor| u32::try_from(floor).ok());
        let rewound_shop = live_shop_floor.is_some_and(|floor| {
            attached.rewind_to_unresolved_shop_purchase(
                floor,
                &explicitly_skipped_shop_steps(records, attached.summary.id),
            )
        });
        if !rewound_shop && !restored_persisted_progress {
            attached.align_progress_to_live_state(state);
        } else if restored_persisted_progress && !rewound_shop {
            attached.last_message = Some(
                "SlayTheData progress restored from recorded guidance; simulator state unchanged"
                    .to_owned(),
            );
        }
    }
}

fn explicitly_skipped_shop_steps(
    records: &[TraceRecord],
    run_id: i64,
) -> std::collections::HashSet<usize> {
    records
        .iter()
        .filter_map(|record| {
            let TraceRecord::SlayTheData { event, details, .. } = record else {
                return None;
            };
            (event == "skip_shop"
                && details
                    .pointer("/slaythedata/attached_run/id")
                    .and_then(Value::as_i64)
                    == Some(run_id))
            .then_some(details)
        })
        .flat_map(|details| {
            details
                .get("purchases")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|purchase| {
            purchase
                .as_array()
                .and_then(|pair| pair.first())
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
        })
        .collect()
}

fn latest_slaythedata_progress_checkpoint(
    records: &[TraceRecord],
    run_id: i64,
) -> Option<(usize, usize)> {
    records
        .iter()
        .enumerate()
        .filter_map(|(record_index, record)| {
            let TraceRecord::SlayTheData { event, details, .. } = record else {
                return None;
            };
            if event != "sent_action" && event != "skip_guidance" {
                return None;
            }
            let attached_run_id = details.pointer("/attached_run/id")?.as_i64()?;
            if attached_run_id != run_id {
                return None;
            }
            details
                .get("next_step_index")?
                .as_u64()
                .map(|index| (record_index, index as usize))
        })
        .next_back()
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

fn same_observed_game_state(source: &LiveState, candidate: &LiveState) -> bool {
    source
        .raw
        .pointer("/current_state/message")
        .zip(candidate.raw.pointer("/current_state/message"))
        .is_some_and(|(source, candidate)| source == candidate)
}

fn transition_state_is_ready(source: &LiveState, candidate: &LiveState) -> bool {
    let source_reports_readiness = state_ready_for_command(source).is_some();
    !source_reports_readiness || state_ready_for_command(candidate) == Some(true)
}

fn state_ready_for_command(state: &LiveState) -> Option<bool> {
    state
        .raw
        .pointer("/summary/ready_for_command")
        .or_else(|| {
            state
                .raw
                .pointer("/current_state/message/ready_for_command")
        })
        .and_then(serde_json::Value::as_bool)
}

pub(crate) fn is_unsettled_action_transition(action: &LegalAction, state: &LiveState) -> bool {
    let hidden_map_transition = action.kind == LegalActionKind::ChooseMapNode
        && state.phase == LivePhase::Neow
        && state
            .raw
            .pointer("/summary/is_screen_up")
            .and_then(serde_json::Value::as_bool)
            == Some(false);

    // Selecting a map node always enters a room. CommunicationMod may publish
    // the old choices and then a blank map while the room initializes; neither
    // is the settled result of the selection.
    let stale_map_choice =
        action.kind == LegalActionKind::ChooseMapNode && state.phase == LivePhase::Map;

    // CommunicationMod can acknowledge a Neow choice before the event has
    // replaced the selected option with its next screen. A different state id
    // only proves that a poll completed; it does not prove the game transition
    // settled.
    let stale_neow_choice = action.kind == LegalActionKind::ChooseNeow
        && state.phase == LivePhase::Neow
        && state.legal_actions.iter().any(|candidate| {
            candidate.kind == LegalActionKind::ChooseNeow
                && candidate.label.eq_ignore_ascii_case(&action.label)
        });

    let stale_card_reward_choice = action.kind == LegalActionKind::ChooseReward
        && state.phase == LivePhase::Reward
        && state
            .raw
            .pointer("/summary/screen_type")
            .or_else(|| {
                state
                    .raw
                    .pointer("/current_state/message/game_state/screen_type")
            })
            .and_then(serde_json::Value::as_str)
            == Some("CARD_REWARD")
        && state.legal_actions.iter().any(|candidate| {
            candidate.kind == LegalActionKind::ChooseReward
                && candidate.label.eq_ignore_ascii_case(&action.label)
        });

    // SmokeBomb.use starts the target's 2.5-second player escape timer. The
    // potion disappears immediately, so CommunicationMod can publish a fresh,
    // command-ready combat state before the timer moves the room to rewards.
    // Treat that intermediate combat state as unsettled instead of checking
    // fidelity against the simulator's completed escape transition.
    let smoke_bomb_escape_pending = action.kind == LegalActionKind::UsePotion
        && action.label.starts_with("Use Smoke Bomb")
        && state.phase == LivePhase::Combat;

    // The Nest queues Ritual Dagger through ShowCardAndObtainEffect. The event
    // exposes its Leave button before that visual effect updates the master
    // deck, so CommunicationMod can report a command-ready intermediate state.
    // Keep polling instead of recording that transient state (or sending
    // Leave, which can discard the queued reward when the map opens).
    let nest_ritual_dagger_pending = action.kind == LegalActionKind::EventChoice
        && action.label.eq_ignore_ascii_case("stay in line")
        && state.phase == LivePhase::Event
        && state_event_id(state).is_some_and(|event| event.eq_ignore_ascii_case("Nest"))
        && state_deck_has_card(state, &["RitualDagger", "Ritual Dagger"]) == Some(false);

    hidden_map_transition
        || stale_map_choice
        || stale_neow_choice
        || stale_card_reward_choice
        || smoke_bomb_escape_pending
        || nest_ritual_dagger_pending
}

fn state_event_id(state: &LiveState) -> Option<&str> {
    state
        .raw
        .pointer("/summary/screen_state/event_id")
        .or_else(|| {
            state
                .raw
                .pointer("/current_state/message/game_state/screen_state/event_id")
        })
        .and_then(Value::as_str)
}

fn state_deck_has_card(state: &LiveState, aliases: &[&str]) -> Option<bool> {
    let deck = state
        .raw
        .pointer("/summary/deck")
        .or_else(|| state.raw.pointer("/current_state/message/game_state/deck"))?
        .as_array()?;
    Some(deck.iter().any(|card| {
        card.get("id")
            .or_else(|| card.get("name"))
            .and_then(Value::as_str)
            .is_some_and(|card| aliases.iter().any(|alias| card.eq_ignore_ascii_case(alias)))
    }))
}

pub(crate) fn combat_state_is_actionable(state: &LiveState) -> bool {
    state.phase == LivePhase::Combat
        && state.legal_actions.iter().any(|action| {
            action.enabled
                && matches!(
                    action.kind,
                    LegalActionKind::PlayCard
                        | LegalActionKind::UsePotion
                        | LegalActionKind::EndTurn
                        | LegalActionKind::Confirm
                )
        })
}

fn is_rng_critical_shop_purchase(item: &str) -> bool {
    item.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect::<String>()
        == "dreamcatcher"
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

pub(crate) fn slaythedata_step_advances(
    step_code: &str,
    action: &LegalAction,
    post_state: Option<&LiveState>,
) -> bool {
    if step_code == "guided_event_sequence" {
        return true;
    }
    if step_code == "pending_room_resolution" {
        return action.kind == LegalActionKind::ChooseMapNode
            || (action.kind == LegalActionKind::Confirm
                && action.label.eq_ignore_ascii_case("Leave shop"))
            || (action.kind == LegalActionKind::Confirm
                && action.label.eq_ignore_ascii_case("Proceed")
                && post_state.is_some_and(is_new_act_entry_map));
    }
    !(step_code == "pending_neow_followup"
        && matches!(
            action.kind,
            LegalActionKind::ChooseReward | LegalActionKind::ChooseNeow
        )
        && post_state
            .is_none_or(|state| state.phase == LivePhase::Reward || state_is_grid_screen(state)))
        && !(step_code == "legal_neow_leave"
            && !(action.kind == LegalActionKind::ChooseNeow
                && action.label.eq_ignore_ascii_case("leave")))
        && !(matches!(step_code, "pending_card_reward" | "guided_card_reward")
            && action.kind == LegalActionKind::ChooseReward
            && action.label.eq_ignore_ascii_case("card")
            && post_state.is_some_and(state_is_card_reward_screen))
        && !(matches!(step_code, "pending_card_reward" | "guided_card_reward")
            && action.kind == LegalActionKind::Confirm)
        && !(step_code == "guided_shop_purchase"
            && action.kind == LegalActionKind::Confirm
            && action.label.eq_ignore_ascii_case("shop"))
        && !(step_code == "guided_shop_purge"
            && action.kind == LegalActionKind::Confirm
            && action.label.eq_ignore_ascii_case("shop"))
        && !(step_code == "guided_shop_purge" && post_state.is_some_and(state_is_grid_screen))
        && !(step_code == "guided_campfire" && post_state.is_some_and(state_is_grid_screen))
        && !(step_code == "guided_event_choice" && post_state.is_some_and(state_is_event_screen))
        && !(step_code == "guided_event_choice" && post_state.is_some_and(state_is_grid_screen))
        && !(step_code == "guided_event_choice"
            && post_state.is_some_and(state_is_single_followup_event_screen))
}

fn state_is_event_screen(state: &LiveState) -> bool {
    state.phase == LivePhase::Event && !state_is_big_fish_followup_event(state)
}

fn state_is_grid_screen(state: &LiveState) -> bool {
    state
        .raw
        .pointer("/summary/screen_type")
        .or_else(|| state.raw.pointer("/summary/screen_name"))
        .and_then(serde_json::Value::as_str)
        == Some("GRID")
}

fn state_is_card_reward_screen(state: &LiveState) -> bool {
    if state
        .raw
        .pointer("/current_state/message/game_state/combat_state")
        .is_some()
    {
        return false;
    }
    state
        .raw
        .pointer("/summary/screen_type")
        .or_else(|| state.raw.pointer("/summary/screen_name"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|screen| screen == "CARD_REWARD" || screen == "GRID")
}

fn state_is_single_followup_event_screen(state: &LiveState) -> bool {
    if state.phase != LivePhase::Event {
        return false;
    }
    if state_is_big_fish_followup_event(state) {
        return false;
    }
    let mut event_choices = state
        .legal_actions
        .iter()
        .filter(|action| action.enabled && action.kind == LegalActionKind::EventChoice);
    let Some(action) = event_choices.next() else {
        return false;
    };
    (action.label.eq_ignore_ascii_case("continue") || action.label.eq_ignore_ascii_case("leave"))
        && event_choices.next().is_none()
}

fn state_is_big_fish_followup_event(state: &LiveState) -> bool {
    state
        .raw
        .pointer("/summary/screen_state/event_name")
        .and_then(serde_json::Value::as_str)
        == Some("Big Fish")
        && state
            .legal_actions
            .iter()
            .filter(|action| action.enabled && action.kind == LegalActionKind::EventChoice)
            .all(|action| action.label.eq_ignore_ascii_case("leave"))
}

fn auto_take_slaythedata_reward_action(state: &LiveState) -> Option<LegalAction> {
    if state.phase != LivePhase::Reward {
        return None;
    }
    if state_is_bottled_card_grid(state) {
        return state
            .legal_actions
            .iter()
            .find(|action| action.enabled && action.kind == LegalActionKind::ChooseReward)
            .cloned();
    }
    let mut choices = state.legal_actions.iter().filter(|action| {
        action.enabled && action.kind == crate::model::LegalActionKind::ChooseReward
    });
    choices
        .find(|action| {
            action.label.trim().to_ascii_lowercase().contains("relic")
                && !offered_relic_changes_card_reward_schedule(state)
        })
        .or_else(|| {
            state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled && action.kind == crate::model::LegalActionKind::ChooseReward
                })
                .find(|action| action.label.trim().to_ascii_lowercase().contains("gold"))
        })
        .or_else(|| {
            state
                .legal_actions
                .iter()
                .filter(|action| {
                    action.enabled && action.kind == crate::model::LegalActionKind::ChooseReward
                })
                .find(|action| {
                    let label = action.label.trim().to_ascii_lowercase();
                    label == "potion" || label.ends_with(" potion")
                })
        })
        .cloned()
}

fn auto_close_slaythedata_overlay_action(state: &LiveState) -> Option<LegalAction> {
    state
        .legal_actions
        .iter()
        .find(|action| action.enabled && action.id.0 == "close-master-deck-view")
        .cloned()
}

fn state_is_bottled_card_grid(state: &LiveState) -> bool {
    let game_state = state.raw.pointer("/current_state/message/game_state");
    let screen_state = game_state.and_then(|state| state.get("screen_state"));
    let is_plain_single_card_grid = game_state
        .and_then(|state| state.get("screen_type"))
        .and_then(Value::as_str)
        == Some("GRID")
        && screen_state
            .and_then(|state| state.get("num_cards"))
            .and_then(Value::as_u64)
            == Some(1)
        && ["for_purge", "for_transform", "for_upgrade"]
            .into_iter()
            .all(|field| {
                screen_state
                    .and_then(|state| state.get(field))
                    .and_then(Value::as_bool)
                    == Some(false)
            });
    if !is_plain_single_card_grid {
        return false;
    }
    game_state
        .and_then(|state| state.get("relics"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|relic| relic.get("id").or_else(|| relic.get("name")))
        .filter_map(Value::as_str)
        .map(|name| {
            name.chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .any(|name| {
            matches!(
                name.as_str(),
                "bottledflame" | "bottledlightning" | "bottledtornado"
            )
        })
}

fn offered_relic_changes_card_reward_schedule(state: &LiveState) -> bool {
    let relic = state
        .raw
        .pointer("/current_state/message/game_state/screen_state/rewards")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("RELIC"))
        })
        .and_then(|reward| reward.get("relic"));
    let identity = relic
        .and_then(|relic| relic.get("id").or_else(|| relic.get("name")))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let disruptive = matches!(
        identity.as_str(),
        "dreamcatcher" | "questioncard" | "prayerwheel" | "prismaticshard"
    );
    if !disruptive {
        return false;
    }
    let expected = state
        .raw
        .pointer("/sim_run_state/reward/relic_offer")
        .or_else(|| state.raw.pointer("/sim_run_state/reward/relic_key_offer"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    expected != identity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ActionId, AutomationPlanSnapshot, AutomationPlannedAction, LegalAction, LegalActionKind,
        LivePhase, LiveState, SlayTheDataRunOutcome,
    };
    use serde_json::json;
    use sts_verify::{
        SlayTheDataPreflightReport, SlayTheDataPreflightStatus, SlayTheDataPreflightStep,
        SlayTheDataSource, SlayTheDataSourceKind,
    };

    fn action(kind: LegalActionKind, label: &str) -> LegalAction {
        LegalAction {
            id: ActionId(label.to_owned()),
            kind,
            label: label.to_owned(),
            enabled: true,
            command: json!({}),
            disabled_reason: None,
        }
    }

    #[test]
    fn reattach_reconciles_map_action_recorded_after_last_guidance_checkpoint() {
        let mut attached = AttachedSlayTheDataRun {
            summary: SlayTheDataRunSummary {
                id: 6453,
                seed_played: Some("-547381600895504017".to_owned()),
                build_version: Some("2020-07-30".to_owned()),
                ascension_level: Some(0),
                floor_reached: Some(51),
                victory: true,
                run_outcome: SlayTheDataRunOutcome::Win,
                path_length: None,
                card_choice_count: None,
                event_choice_count: None,
                shop_purchase_count: None,
                potion_usage_count: None,
                neow_bonus: None,
                neow_cost: None,
                guided_score: 0,
                materialized: true,
            },
            report: SlayTheDataPreflightReport {
                schema: 1,
                source: SlayTheDataSource {
                    kind: SlayTheDataSourceKind::RawRun,
                    run_id: Some(6453),
                    play_id: None,
                    source_file: None,
                    source_run_ordinal: None,
                },
                run_start: None,
                numeric_seed: None,
                start_phase: None,
                route_fully_checked: false,
                diagnostics: Vec::new(),
                steps: vec![
                    SlayTheDataPreflightStep {
                        floor: 27,
                        ordinal: 71,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "pending_room_resolution".to_owned(),
                        message: "route symbol \"?\" waits for the map".to_owned(),
                        bridge_command: None,
                    },
                    SlayTheDataPreflightStep {
                        floor: 27,
                        ordinal: 72,
                        status: SlayTheDataPreflightStatus::Guided,
                        code: "guided_event_choice".to_owned(),
                        message: "Augmenter event choice".to_owned(),
                        bridge_command: None,
                    },
                ],
            },
            next_step_index: 0,
            blocked: None,
            last_message: None,
            auto_play_paused: false,
        };
        let records = vec![
            TraceRecord::SlayTheData {
                sequence: 1041,
                event: "sent_action".to_owned(),
                details: json!({
                    "attached_run": {"id": 6453},
                    "next_step_index": 0
                }),
            },
            TraceRecord::Action {
                sequence: 1041,
                action: LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ChooseMapNode,
                    label: "x=6".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                },
            },
        ];

        restore_slaythedata_progress_from_records(&mut attached, &records, None);

        assert_eq!(attached.next_step_index, 1);
        assert_eq!(
            attached.report.steps[attached.next_step_index].code,
            "guided_event_choice"
        );
    }

    #[test]
    fn end_turn_requires_fresh_search_but_retains_a_warm_suffix() {
        let planned = AutomationPlannedAction {
            action_id: ActionId("end".to_owned()),
            kind: LegalActionKind::EndTurn,
            label: "End turn".to_owned(),
            source_sequence: 7,
            command: Some("END".to_owned()),
            planner_action: "end_turn".to_owned(),
        };
        let mut automation = AutomationJobSnapshot {
            state: AutomationState::ReadyToSend,
            planned_action: Some(planned.clone()),
            plan: Some(AutomationPlanSnapshot {
                actions: vec![planned],
                played_actions: 0,
                predicted_final_hp: Some(50),
                predicted_monster_hp: Some(20),
                value: Some(1.0),
                nodes: 10,
                terminal_reason: None,
                ..AutomationPlanSnapshot::default()
            }),
            ..AutomationJobSnapshot::default()
        };

        assert!(prepare_plan_for_turn_boundary(
            &mut automation,
            &action(LegalActionKind::EndTurn, "End turn"),
            false,
        ));
        assert!(
            automation.plan.is_some(),
            "the suffix remains as a warm start"
        );
        assert!(automation.planned_action.is_none());
        assert_eq!(automation.state, AutomationState::ReadyToSend);
    }

    #[test]
    fn within_turn_action_keeps_the_existing_plan() {
        let planned = AutomationPlannedAction {
            action_id: ActionId("play".to_owned()),
            kind: LegalActionKind::PlayCard,
            label: "Strike".to_owned(),
            source_sequence: 7,
            command: Some("PLAY 1 0".to_owned()),
            planner_action: "play_card:1:0".to_owned(),
        };
        let mut automation = AutomationJobSnapshot {
            planned_action: Some(planned.clone()),
            plan: Some(AutomationPlanSnapshot {
                actions: vec![planned],
                played_actions: 0,
                predicted_final_hp: None,
                predicted_monster_hp: None,
                value: None,
                nodes: 1,
                terminal_reason: None,
                ..AutomationPlanSnapshot::default()
            }),
            ..AutomationJobSnapshot::default()
        };

        assert!(!prepare_plan_for_turn_boundary(
            &mut automation,
            &action(LegalActionKind::PlayCard, "Strike"),
            false,
        ));
        assert!(automation.plan.is_some());
        assert!(automation.planned_action.is_some());
    }

    #[test]
    fn auto_take_reward_prioritizes_relic_over_gold_and_potion() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Reward,
            legal_actions: vec![
                action(LegalActionKind::ChooseReward, "gold"),
                action(LegalActionKind::ChooseReward, "potion"),
                action(LegalActionKind::ChooseReward, "relic"),
            ],
            raw: json!({}),
        };

        let selected = auto_take_slaythedata_reward_action(&state).unwrap();

        assert_eq!(selected.label, "relic");
    }

    #[test]
    fn auto_close_overlay_selects_master_deck_cancel_action() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Unknown,
            legal_actions: vec![
                action(LegalActionKind::RequestState, "request-state"),
                action(LegalActionKind::Confirm, "close-master-deck-view"),
            ],
            raw: json!({}),
        };

        let selected = auto_close_slaythedata_overlay_action(&state).unwrap();

        assert_eq!(selected.id.0, "close-master-deck-view");
    }

    #[test]
    fn auto_take_reward_resolves_bottled_relic_card_grid() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Reward,
            legal_actions: vec![
                action(LegalActionKind::RequestState, "Request state"),
                action(LegalActionKind::ChooseReward, "Shrug It Off"),
                action(LegalActionKind::ChooseReward, "Defend"),
            ],
            raw: json!({
                "current_state": {"message": {"game_state": {
                    "screen_type": "GRID",
                    "relics": [{"id": "Bottled Lightning"}],
                    "screen_state": {
                        "num_cards": 1,
                        "for_purge": false,
                        "for_transform": false,
                        "for_upgrade": false
                    }
                }}}
            }),
        };

        let selected = auto_take_slaythedata_reward_action(&state).unwrap();

        assert_eq!(selected.label, "Shrug It Off");
    }

    #[test]
    fn auto_take_reward_skips_replacement_dream_catcher_to_preserve_card_rng() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Reward,
            legal_actions: vec![
                action(LegalActionKind::ChooseReward, "gold"),
                action(LegalActionKind::ChooseReward, "relic"),
                action(LegalActionKind::ChooseReward, "card"),
            ],
            raw: json!({
                "current_state": {"message": {"game_state": {"screen_state": {"rewards": [
                    {"reward_type": "GOLD", "gold": 29},
                    {"reward_type": "RELIC", "relic": {"id": "Dream Catcher", "name": "Dream Catcher"}},
                    {"reward_type": "CARD"}
                ]}}}}
            }),
        };

        let action = auto_take_slaythedata_reward_action(&state).unwrap();

        assert_eq!(action.label, "gold");
    }

    #[test]
    fn replacement_card_reward_relics_are_card_rng_disruptive() {
        for relic in ["Question Card", "Prayer Wheel", "PrismaticShard"] {
            let state = LiveState {
                sequence: 1,
                phase: LivePhase::Reward,
                legal_actions: vec![action(LegalActionKind::ChooseReward, "relic")],
                raw: json!({
                    "current_state": {"message": {"game_state": {"screen_state": {"rewards": [
                        {"reward_type": "RELIC", "relic": {"id": relic, "name": relic}}
                    ]}}}}
                }),
            };

            assert!(
                auto_take_slaythedata_reward_action(&state).is_none(),
                "{relic}"
            );
        }
    }

    #[test]
    fn matching_simulator_dream_catcher_is_safe_to_take() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Reward,
            legal_actions: vec![action(LegalActionKind::ChooseReward, "relic")],
            raw: json!({
                "sim_run_state": {"reward": {"relic_offer": "DreamCatcher"}},
                "current_state": {"message": {"game_state": {"screen_state": {"rewards": [
                    {"reward_type": "RELIC", "relic": {"id": "Dream Catcher"}}
                ]}}}}
            }),
        };

        assert_eq!(
            auto_take_slaythedata_reward_action(&state)
                .as_ref()
                .map(|action| action.label.as_str()),
            Some("relic")
        );
    }

    #[test]
    fn neow_map_transition_is_not_ready_for_a_map_choice() {
        let mut state = state(LivePhase::Map);
        state.raw = serde_json::json!({
            "summary": {
                "screen_type": "MAP",
                "room_type": "NeowRoom"
            }
        });

        assert!(is_unsettled_neow_map_state(&state));
    }

    #[test]
    fn only_dream_catcher_is_rng_critical_for_missing_shop_purchase_policy() {
        assert!(is_rng_critical_shop_purchase("Dream Catcher"));
        assert!(is_rng_critical_shop_purchase("DreamCatcher"));
        assert!(!is_rng_critical_shop_purchase("Lantern"));
        assert!(!is_rng_critical_shop_purchase("Membership Card"));
    }

    #[test]
    fn ordinary_map_state_is_ready_for_a_map_choice() {
        let mut state = state(LivePhase::Map);
        state.raw = serde_json::json!({
            "summary": {
                "screen_type": "MAP",
                "room_type": "MonsterRoom"
            }
        });

        assert!(!is_unsettled_neow_map_state(&state));
    }

    fn state(phase: LivePhase) -> LiveState {
        LiveState {
            sequence: 1,
            phase,
            legal_actions: Vec::new(),
            raw: json!({}),
        }
    }

    fn grid_state(phase: LivePhase) -> LiveState {
        LiveState {
            sequence: 1,
            phase,
            legal_actions: Vec::new(),
            raw: json!({"summary": {"screen_type": "GRID"}}),
        }
    }

    #[test]
    fn pending_neow_followup_does_not_advance_while_grid_remains_open() {
        let sent = action(LegalActionKind::ChooseReward, "Strike_R");
        let post_state = grid_state(LivePhase::Reward);

        assert!(!slaythedata_step_advances(
            "pending_neow_followup",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn pending_neow_followup_advances_when_grid_returns_to_neow_leave() {
        let sent = action(LegalActionKind::ChooseReward, "Strike_R");
        let post_state = state(LivePhase::Neow);

        assert!(slaythedata_step_advances(
            "pending_neow_followup",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn legal_neow_leave_does_not_advance_for_grid_card_choice() {
        let sent = action(LegalActionKind::ChooseReward, "strike");
        let post_state = state(LivePhase::Neow);

        assert!(!slaythedata_step_advances(
            "legal_neow_leave",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn pending_room_resolution_does_not_advance_after_event_closes_to_map() {
        let sent = action(LegalActionKind::EventChoice, "continue");
        let post_state = state(LivePhase::Map);

        assert!(!slaythedata_step_advances(
            "pending_room_resolution",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn pending_room_resolution_does_not_advance_after_neow_leave_closes_to_map() {
        let sent = action(LegalActionKind::ChooseNeow, "leave");
        let post_state = state(LivePhase::Map);

        assert!(!slaythedata_step_advances(
            "pending_room_resolution",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn pending_room_resolution_advances_when_map_node_enters_room() {
        for post_state in [state(LivePhase::Combat), state(LivePhase::Event)] {
            let sent = action(LegalActionKind::ChooseMapNode, "x=0");

            assert!(slaythedata_step_advances(
                "pending_room_resolution",
                &sent,
                Some(&post_state)
            ));
        }
    }

    #[test]
    fn pending_room_resolution_does_not_advance_when_proceed_returns_to_map() {
        let sent = action(LegalActionKind::Confirm, "Proceed");
        let post_state = state(LivePhase::Map);

        assert!(!slaythedata_step_advances(
            "pending_room_resolution",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn pending_room_resolution_advances_when_proceed_opens_the_next_act_map() {
        let sent = action(LegalActionKind::Confirm, "Proceed");
        let post_state = LiveState {
            sequence: 18,
            phase: LivePhase::Map,
            legal_actions: Vec::new(),
            raw: json!({
                "current_state": {"message": {"game_state": {
                    "act": 2,
                    "floor": 17,
                    "screen_state": {
                        "first_node_chosen": false,
                        "current_node": {"x": 0, "y": -1}
                    }
                }}}
            }),
        };

        assert!(slaythedata_step_advances(
            "pending_room_resolution",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn pending_room_resolution_does_not_advance_when_proceed_stays_in_reward() {
        let sent = action(LegalActionKind::Confirm, "Proceed");
        let post_state = state(LivePhase::Reward);

        assert!(!slaythedata_step_advances(
            "pending_room_resolution",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn pending_room_resolution_does_not_advance_for_reward_cleanup() {
        for sent in [
            action(LegalActionKind::ChooseReward, "gold"),
            action(LegalActionKind::ChooseReward, "potion"),
            action(LegalActionKind::SkipReward, "skip"),
        ] {
            assert!(!slaythedata_step_advances(
                "pending_room_resolution",
                &sent,
                Some(&state(LivePhase::Map))
            ));
        }
    }

    #[test]
    fn pending_shop_route_advances_when_leaving_shop() {
        let sent = action(LegalActionKind::Confirm, "Leave shop");
        let post_state = state(LivePhase::Unknown);

        assert!(slaythedata_step_advances(
            "pending_room_resolution",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn guided_event_choice_does_not_advance_while_event_screen_remains_open() {
        let sent = action(LegalActionKind::EventChoice, "continue");
        let post_state = state(LivePhase::Event);

        assert!(!slaythedata_step_advances(
            "guided_event_choice",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn guided_event_choice_advances_after_event_closes() {
        let sent = action(LegalActionKind::EventChoice, "ignore");
        let post_state = state(LivePhase::Map);

        assert!(slaythedata_step_advances(
            "guided_event_choice",
            &sent,
            Some(&post_state)
        ));
    }

    #[test]
    fn guided_event_sequence_advances_for_each_click_while_event_remains_open() {
        let sent = action(LegalActionKind::EventChoice, "riches");
        let post_state = state(LivePhase::Event);

        assert!(slaythedata_step_advances(
            "guided_event_sequence",
            &sent,
            Some(&post_state)
        ));
    }
}
