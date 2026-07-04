use crate::{
    bridge::BridgeManager,
    fidelity::FidelityChecker,
    model::{
        ActionId, BridgeId, FidelityStatus, LiveError, LiveResult, RunConfig, SessionId,
        SessionLifecycle, SessionSnapshot, TraceRecord,
    },
    operator_actions::{request_state_action, start_run_action},
    session_blocking::record_blocked,
    session_recovery,
    session_response::append_bridge_response_and_state,
    session_state::{lifecycle_for_fidelity, metadata_record, SessionData},
    trace_writer::TraceWriter,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct SessionStore<B, F> {
    bridge: B,
    fidelity: F,
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
            trace_root: trace_root.as_ref().to_path_buf(),
            sessions: HashMap::new(),
            next_session: 1,
        }
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
        snapshots.sort_by(|left, right| left.session_id.0.cmp(&right.session_id.0));
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
        self.refresh_fidelity(session_id)
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
        let session = self.session_mut(session_id)?;
        session.trace_writer.append(&TraceRecord::Action {
            sequence: state.sequence.saturating_sub(1),
            action,
        })?;
        append_bridge_response_and_state(session, "send_action", &state)?;
        session.latest_state = Some(state);
        self.refresh_fidelity(session_id)
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

    pub fn refresh_fidelity(&mut self, session_id: &SessionId) -> LiveResult<SessionSnapshot> {
        let path = self.session(session_id)?.trace_writer.path().to_path_buf();
        let fidelity = self.fidelity.check_trace(&path)?;
        let session = self.session_mut(session_id)?;
        session.fidelity = fidelity;
        if !matches!(
            session.lifecycle,
            SessionLifecycle::Blocked | SessionLifecycle::Ended
        ) {
            session.lifecycle = lifecycle_for_fidelity(&session.fidelity);
        }
        Ok(session.snapshot())
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
            blocked: None,
        })
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
