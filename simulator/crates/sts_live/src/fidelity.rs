use crate::{
    fidelity_status::verify_seed_start_trace,
    model::{FidelityKind, FidelityStatus, LiveResult, TraceRecord},
};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};
use sts_core::RunState;

pub trait FidelityChecker {
    fn check_trace(&self, path: &Path) -> LiveResult<FidelityStatus>;

    fn check_trace_with_sim_state(
        &self,
        path: &Path,
    ) -> LiveResult<(FidelityStatus, Option<RunState>)> {
        self.check_trace(path).map(|status| (status, None))
    }
}

enum TraceMode {
    SeedStart,
}

#[derive(Debug, Default, Clone)]
pub struct TraceFidelityChecker;

impl FidelityChecker for TraceFidelityChecker {
    fn check_trace(&self, path: &Path) -> LiveResult<FidelityStatus> {
        self.check_trace_with_sim_state(path)
            .map(|(status, _)| status)
    }

    fn check_trace_with_sim_state(
        &self,
        path: &Path,
    ) -> LiveResult<(FidelityStatus, Option<RunState>)> {
        if !path.exists() {
            return Ok((FidelityStatus::unknown(), None));
        }

        let records = read_live_trace(path)?;
        if let Some(status) = explicit_live_status(&records) {
            return Ok((status, None));
        }

        let seed_start_trace = communication_mod_trace(&records, TraceMode::SeedStart)?;
        if has_run_config(&records) && seed_start_trace.has_start {
            if seed_start_trace.transitions == 0 {
                return Ok((FidelityStatus {
                    kind: FidelityKind::Unknown,
                    first_divergent_step: None,
                    compact_diff: Vec::new(),
                    message: Some(
                        "waiting for a recorded state-action-state transition before strict seed-start replay is meaningful"
                            .to_owned(),
                    ),
                }, None));
            }
            return verify_seed_start_trace(&seed_start_trace.jsonl);
        }

        if seed_start_trace.states == 0 {
            return Ok((FidelityStatus::unknown(), None));
        }
        if !has_run_config(&records) || !seed_start_trace.has_start {
            return Ok((
                FidelityStatus {
                    kind: FidelityKind::Unknown,
                    first_divergent_step: None,
                    compact_diff: Vec::new(),
                    message: Some(
                        "strict seed-start replay requires recorded run config and START command"
                            .to_owned(),
                    ),
                },
                None,
            ));
        }
        Ok((
            FidelityStatus {
                kind: FidelityKind::Unknown,
                first_divergent_step: None,
                compact_diff: Vec::new(),
                message: Some(
                    "waiting for strict seed-start replay to reach a supported verifier boundary"
                        .to_owned(),
                ),
            },
            None,
        ))
    }
}

struct CommunicationTrace {
    jsonl: String,
    states: usize,
    transitions: usize,
    has_start: bool,
}

fn read_live_trace(path: &Path) -> LiveResult<Vec<TraceRecord>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        records.push(serde_json::from_str(&line)?);
    }
    Ok(records)
}

fn explicit_live_status(records: &[TraceRecord]) -> Option<FidelityStatus> {
    for (index, record) in records.iter().enumerate() {
        match record {
            TraceRecord::Error {
                reason_code,
                message,
                ..
            } if reason_code == "fidelity_lost" => {
                return Some(FidelityStatus {
                    kind: FidelityKind::Lost,
                    first_divergent_step: Some(index as u64),
                    compact_diff: vec![message.clone()],
                    message: Some(message.clone()),
                });
            }
            _ => {}
        }
    }
    None
}

fn has_run_config(records: &[TraceRecord]) -> bool {
    records.iter().any(|record| {
        matches!(
            record,
            TraceRecord::Metadata {
                run_config: Some(_),
                ..
            }
        )
    })
}

fn communication_mod_trace(
    records: &[TraceRecord],
    mode: TraceMode,
) -> LiveResult<CommunicationTrace> {
    let mut jsonl = serde_json::to_string(&json!({
        "type": "metadata",
        "schema": 1,
        "source": "communication_mod",
        "client": "sts_live",
    }))?;
    jsonl.push('\n');
    let mut states = 0;
    let mut saw_state = false;
    let mut pending_action_after_state = false;
    let mut transitions = 0;
    let mut has_start = false;
    for record in records {
        match record {
            TraceRecord::RunAbandoned { .. } => break,
            TraceRecord::State { sequence, state } => {
                if let Some(message) = communication_message(&state.raw) {
                    if pending_action_after_state {
                        transitions += 1;
                        pending_action_after_state = false;
                    }
                    jsonl.push_str(&serde_json::to_string(&json!({
                        "type": "state",
                        "step": sequence,
                        "message": message,
                    }))?);
                    jsonl.push('\n');
                    states += 1;
                    saw_state = true;
                }
            }
            TraceRecord::Action { sequence, action } => {
                if let Some(command) = action.command.get("command").and_then(Value::as_str) {
                    if command.to_ascii_uppercase().starts_with("START ") {
                        has_start = true;
                        if matches!(mode, TraceMode::SeedStart) && !saw_state {
                            append_state_line(
                                &mut jsonl,
                                0,
                                json!({"game_state": {"screen_type": "MENU"}}),
                            )?;
                            states += 1;
                            saw_state = true;
                        }
                    }
                    jsonl.push_str(&serde_json::to_string(&json!({
                        "type": "action",
                        "step": sequence,
                        "command": command,
                    }))?);
                    jsonl.push('\n');
                    if saw_state {
                        pending_action_after_state = true;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(CommunicationTrace {
        jsonl,
        states,
        transitions,
        has_start,
    })
}

fn append_state_line(jsonl: &mut String, sequence: u64, message: Value) -> LiveResult<()> {
    jsonl.push_str(&serde_json::to_string(&json!({
        "type": "state",
        "step": sequence,
        "message": message,
    }))?);
    jsonl.push('\n');
    Ok(())
}

fn communication_message(raw: &Value) -> Option<Value> {
    raw.pointer("/current_state/message")
        .or_else(|| raw.pointer("/state/message"))
        .cloned()
}
