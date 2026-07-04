use crate::{
    fidelity::FidelityChecker,
    model::{
        BlockedState, FidelityKind, FidelityStatus, LiveError, LiveResult, RunConfig, SessionId,
        SessionLifecycle, TraceRecord,
    },
    session_state::{lifecycle_for_fidelity, SessionData},
    trace_writer::{read_records, TraceWriter},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn trace_paths(trace_root: &Path) -> LiveResult<Vec<PathBuf>> {
    if !trace_root.exists() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(trace_root)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

pub(super) fn recover_session<F>(path: &Path, fidelity: &F) -> LiveResult<SessionData>
where
    F: FidelityChecker,
{
    let records = read_records(path)?;
    let recovered = recovered_session_data(&records)?;
    let (trace_writer, _) = TraceWriter::recover_existing(path)?;
    let fidelity = fidelity.check_trace(path)?;
    let lifecycle = recovered_lifecycle(&records, &fidelity, recovered.blocked.as_ref());
    Ok(SessionData {
        id: recovered.id,
        bridge_id: recovered.bridge_id,
        lifecycle,
        trace_writer,
        run_config: recovered.run_config,
        latest_state: recovered.latest_state,
        fidelity,
        blocked: recovered.blocked,
    })
}

pub(super) fn bump_next_session(next_session: &mut u64, id: &SessionId) {
    if let Some(number) =
        id.0.strip_prefix("session-")
            .and_then(|n| n.parse::<u64>().ok())
    {
        *next_session = (*next_session).max(number + 1);
    }
}

struct RecoveredSessionData {
    id: SessionId,
    bridge_id: crate::model::BridgeId,
    run_config: Option<RunConfig>,
    latest_state: Option<crate::model::LiveState>,
    blocked: Option<BlockedState>,
}

fn recovered_session_data(records: &[TraceRecord]) -> LiveResult<RecoveredSessionData> {
    let mut metadata = None;
    let mut latest_state = None;
    let mut blocked = None;
    for record in records {
        match record {
            TraceRecord::Metadata {
                session_id,
                bridge_id,
                run_config,
                ..
            } => metadata = Some((session_id.clone(), bridge_id.clone(), run_config.clone())),
            TraceRecord::State { state, .. } => {
                latest_state = Some(state.clone());
                blocked = None;
            }
            TraceRecord::Error {
                reason_code,
                message,
                ..
            } if reason_code != "fidelity_lost" => {
                blocked = Some(BlockedState {
                    reason_code: reason_code.clone(),
                    message: message.clone(),
                });
            }
            _ => {}
        }
    }
    let (id, bridge_id, run_config) = metadata.ok_or_else(|| {
        LiveError::Blocked("cannot recover trace without metadata record".to_owned())
    })?;
    Ok(RecoveredSessionData {
        id,
        bridge_id,
        run_config,
        latest_state,
        blocked,
    })
}

fn recovered_lifecycle(
    records: &[TraceRecord],
    fidelity: &FidelityStatus,
    blocked: Option<&BlockedState>,
) -> SessionLifecycle {
    if records
        .iter()
        .any(|record| matches!(record, TraceRecord::RunAbandoned { .. }))
    {
        return SessionLifecycle::Ended;
    }
    if blocked.is_some() && fidelity.kind != FidelityKind::Lost {
        return SessionLifecycle::Blocked;
    }
    lifecycle_for_fidelity(fidelity)
}
