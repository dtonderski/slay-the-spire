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
                        "strict seed-start replay requires recorded run config and START or START_VERIFY command"
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
    let recheck_start = records
        .iter()
        .rposition(|record| {
            matches!(
                record,
                TraceRecord::SlayTheData { event, .. } if event == "fidelity_recheck"
            )
        })
        .map_or(0, |index| index + 1);
    for (index, record) in records.iter().enumerate().skip(recheck_start) {
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
    let run_config = records.iter().find_map(|record| match record {
        TraceRecord::Metadata {
            run_config: Some(run_config),
            ..
        } => Some(run_config),
        _ => None,
    });
    let mut jsonl = serde_json::to_string(&json!({
        "type": "metadata",
        "schema": 1,
        "source": "communication_mod",
        "client": "sts_live",
        "run_config": run_config,
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
                    if is_start_command(command) {
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
                        "playtime_seconds": action.command.get("playtime_seconds"),
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

fn is_start_command(command: &str) -> bool {
    command.split_whitespace().next().is_some_and(|verb| {
        verb.eq_ignore_ascii_case("START") || verb.eq_ignore_ascii_case("START_VERIFY")
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

#[cfg(test)]
mod tests {
    use super::{explicit_live_status, is_start_command};
    use crate::model::TraceRecord;

    #[test]
    fn recognizes_normal_and_verification_start_commands_by_exact_verb() {
        assert!(is_start_command("START IRONCLAD 0 CODEX04"));
        assert!(is_start_command("start_verify ironclad 0 FIDL00055 10000"));
        assert!(!is_start_command("START_VERIFYING IRONCLAD 0 CODEX04"));
        assert!(!is_start_command("RESTART IRONCLAD 0 CODEX04"));
    }

    #[test]
    fn fidelity_recheck_supersedes_only_earlier_recorded_loss() {
        let records = vec![
            TraceRecord::Error {
                sequence: 1,
                reason_code: "fidelity_lost".to_owned(),
                message: "old verifier result".to_owned(),
            },
            TraceRecord::SlayTheData {
                sequence: 2,
                event: "fidelity_recheck".to_owned(),
                details: serde_json::json!({"reason": "verified simulator repair"}),
            },
        ];
        assert!(explicit_live_status(&records).is_none());

        let mut records = records;
        records.push(TraceRecord::Error {
            sequence: 3,
            reason_code: "fidelity_lost".to_owned(),
            message: "new verifier result".to_owned(),
        });
        assert_eq!(
            explicit_live_status(&records).and_then(|status| status.message),
            Some("new verifier result".to_owned())
        );
    }
}
