use crate::{
    model::{
        AutomationJobSnapshot, BlockedState, BridgeId, FidelityKind, FidelityStatus, LiveState,
        RunConfig, SessionId, SessionLifecycle, SessionSnapshot, TraceRecord,
    },
    slaythedata::AttachedSlayTheDataRun,
    trace_writer::TraceWriter,
};
use sts_core::RunState;

#[derive(Clone)]
pub(super) struct FidelityCache {
    pub(super) trace_len: u64,
    pub(super) status: FidelityStatus,
    pub(super) sim_run_state: Option<RunState>,
    pub(super) reusable_after_append: bool,
}

pub(super) struct SessionData {
    pub(super) id: SessionId,
    pub(super) bridge_id: BridgeId,
    pub(super) lifecycle: SessionLifecycle,
    pub(super) trace_writer: TraceWriter,
    pub(super) run_config: Option<RunConfig>,
    pub(super) latest_state: Option<LiveState>,
    pub(super) fidelity: FidelityStatus,
    pub(super) fidelity_cache: Option<FidelityCache>,
    pub(super) blocked: Option<BlockedState>,
    pub(super) automation: AutomationJobSnapshot,
    pub(super) slaythedata: Option<AttachedSlayTheDataRun>,
}

impl SessionData {
    pub(super) fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            session_id: self.id.clone(),
            bridge_id: self.bridge_id.clone(),
            lifecycle: self.lifecycle.clone(),
            trace_path: self.trace_writer.path().display().to_string(),
            run_config: self.run_config.clone(),
            latest_state: self.latest_state.clone(),
            fidelity: self.fidelity.clone(),
            blocked: self.blocked.clone(),
            automation: self.automation.clone(),
            slaythedata: self
                .slaythedata
                .as_ref()
                .map(|run| run.snapshot(self.latest_state.as_ref()))
                .unwrap_or_default(),
        }
    }
}

pub(super) fn metadata_record(session: &SessionData, source: &str) -> TraceRecord {
    TraceRecord::Metadata {
        schema: 1,
        source: source.to_owned(),
        session_id: session.id.clone(),
        bridge_id: session.bridge_id.clone(),
        run_config: session.run_config.clone(),
    }
}

pub(super) fn lifecycle_for_fidelity(status: &FidelityStatus) -> SessionLifecycle {
    match status.kind {
        FidelityKind::Unknown | FidelityKind::UnverifiedStart => SessionLifecycle::Recording,
        FidelityKind::Ok => SessionLifecycle::FidelityOk,
        FidelityKind::Lost => SessionLifecycle::FidelityLost,
    }
}
