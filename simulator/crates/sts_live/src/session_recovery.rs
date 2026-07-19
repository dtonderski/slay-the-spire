use crate::{
    communication::{live_state_from_files, BridgeFiles},
    model::{
        AutomationJobSnapshot, BlockedState, FidelityKind, FidelityStatus, LiveError, LiveResult,
        LiveState, RunConfig, SessionId, SessionLifecycle, TraceRecord,
    },
    session_state::{lifecycle_for_fidelity, SessionData},
    trace_writer::{read_records, TraceWriter},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn trace_paths(trace_root: &Path) -> LiveResult<Vec<PathBuf>> {
    if !trace_root.exists() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(trace_root)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| is_session_trace_path(path))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn is_session_trace_path(path: &Path) -> bool {
    if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
        return false;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.strip_prefix("session-"))
        .is_some_and(|number| !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()))
}

pub(crate) fn recover_session(path: &Path) -> LiveResult<SessionData> {
    let records = read_records(path)?;
    let recovered = recovered_session_data(&records)?;
    let (trace_writer, _) = TraceWriter::recover_existing(path)?;
    let fidelity = recovered_fidelity(&records);
    let lifecycle = recovered_lifecycle(&records, &fidelity, recovered.blocked.as_ref());
    Ok(SessionData {
        id: recovered.id,
        bridge_id: recovered.bridge_id,
        lifecycle,
        trace_writer,
        run_config: recovered.run_config,
        latest_state: recovered.latest_state,
        fidelity,
        fidelity_cache: None,
        blocked: recovered.blocked,
        automation: AutomationJobSnapshot::default(),
        slaythedata: None,
    })
}

fn recovered_fidelity(records: &[TraceRecord]) -> FidelityStatus {
    for (index, record) in records.iter().enumerate() {
        if let TraceRecord::Error {
            reason_code,
            message,
            ..
        } = record
        {
            if reason_code == "fidelity_lost" {
                return FidelityStatus {
                    kind: FidelityKind::Lost,
                    first_divergent_step: Some(index as u64),
                    compact_diff: vec![message.clone()],
                    message: Some(message.clone()),
                };
            }
        }
    }
    FidelityStatus {
        kind: FidelityKind::Unknown,
        first_divergent_step: None,
        compact_diff: Vec::new(),
        message: Some(
            "recovered session fidelity is stale; refresh before taking guided actions".to_owned(),
        ),
    }
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
    latest_state: Option<LiveState>,
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
                latest_state = Some(recomputed_state_from_raw_summary(state));
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

fn recomputed_state_from_raw_summary(state: &LiveState) -> LiveState {
    let Some(summary) = state.raw.get("summary") else {
        return state.clone();
    };
    let files = BridgeFiles {
        status: state.raw.get("status").cloned().unwrap_or_default(),
        summary: summary.clone(),
        current_state: state.raw.get("current_state").cloned().unwrap_or_default(),
        status_age: None,
        summary_age: None,
    };
    live_state_from_files(&files)
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
