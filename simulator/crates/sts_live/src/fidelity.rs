use crate::{
    fidelity_status::{unexpected_diff_status, verify_seed_start_trace},
    model::{FidelityKind, FidelityStatus, LiveResult, TraceRecord},
};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};
use sts_verify::{verify_communication_mod_trace_with_mode, VerificationMode};

pub trait FidelityChecker {
    fn check_trace(&self, path: &Path) -> LiveResult<FidelityStatus>;
}

enum TraceMode {
    ObservedState,
    SeedStart,
}

#[derive(Debug, Default, Clone)]
pub struct TraceFidelityChecker;

impl FidelityChecker for TraceFidelityChecker {
    fn check_trace(&self, path: &Path) -> LiveResult<FidelityStatus> {
        if !path.exists() {
            return Ok(FidelityStatus::unknown());
        }

        let records = read_live_trace(path)?;
        if let Some(status) = explicit_live_status(&records) {
            return Ok(status);
        }

        let seed_start_trace = communication_mod_trace(&records, TraceMode::SeedStart)?;
        if has_run_config(&records)
            && seed_start_trace.has_start
            && seed_start_trace.transitions > 0
        {
            let seed_start_status = verify_seed_start_trace(&seed_start_trace.jsonl)?;
            if !is_seed_start_waiting_for_boundary(&seed_start_status)
                && !is_seed_start_non_blocking_coverage_gap(&seed_start_status)
            {
                return Ok(seed_start_status);
            }
        }

        observed_state_status(&records)
    }
}

fn observed_state_status(records: &[TraceRecord]) -> LiveResult<FidelityStatus> {
    let communication_trace = communication_mod_trace(records, TraceMode::ObservedState)?;
    if communication_trace.states == 0 {
        return Ok(FidelityStatus::unknown());
    }
    if communication_trace.actions == 0 {
        return Ok(FidelityStatus {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: Some(
                "waiting for a recorded live action before replay fidelity is meaningful"
                    .to_owned(),
            ),
        });
    }
    if communication_trace.transitions == 0 {
        return Ok(FidelityStatus {
                kind: FidelityKind::Unknown,
                first_divergent_step: None,
                compact_diff: Vec::new(),
                message: Some(
                    "waiting for a recorded state-action-state transition before replay fidelity is meaningful"
                        .to_owned(),
                ),
            });
    }

    match verify_communication_mod_trace_with_mode(
        &communication_trace.jsonl,
        VerificationMode::ObservedState,
    ) {
        Ok(report) if !report.unexpected_diffs.is_empty() => Ok(unexpected_diff_status(&report)),
        Ok(report) if report.unsupported.iter().any(is_reportable_unsupported) => {
            let unsupported = report
                .unsupported
                .iter()
                .find(|transition| is_reportable_unsupported(transition))
                .expect("guarded by any unsupported gameplay transition");
            Ok(FidelityStatus {
                kind: FidelityKind::Unknown,
                first_divergent_step: Some(unsupported.action_step as u64),
                compact_diff: vec![unsupported.reason.clone()],
                message: Some(format!(
                    "simulator replay unsupported for {} at step {}",
                    unsupported.command, unsupported.action_step
                )),
            })
        }
        Ok(report) if report.unsupported.iter().any(is_non_reportable_unsupported) => {
            Ok(FidelityStatus {
                kind: FidelityKind::Ok,
                first_divergent_step: None,
                compact_diff: Vec::new(),
                message: Some(
                    "observed-state replay matched supported transitions; Neow, map, reward, and shop UI transitions are covered by seed-start/reward replay"
                        .to_owned(),
                ),
            })
        }
        Ok(_) => Ok(FidelityStatus {
            kind: FidelityKind::Ok,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: Some("observed-state replay matched supported transitions".to_owned()),
        }),
        Err(err) => Ok(FidelityStatus {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: vec![err.to_string()],
            message: Some("observed-state replay could not be completed".to_owned()),
        }),
    }
}

fn is_state_poll(command: &str) -> bool {
    command.eq_ignore_ascii_case("state")
}

fn is_reportable_unsupported(transition: &sts_verify::UnsupportedTransition) -> bool {
    !is_state_poll(&transition.command) && !is_non_reportable_unsupported(transition)
}

fn is_non_reportable_unsupported(transition: &sts_verify::UnsupportedTransition) -> bool {
    transition
        .reason
        .contains("Neow/event choice side effects are unsupported")
        || transition
            .reason
            .contains("reward card-screen opening is a UI transition")
        || transition
            .reason
            .contains("reward-to-map UI transition is out-of-scope")
        || transition
            .reason
            .contains("map node selection is unsupported until exact seed-to-map parity")
        || transition
            .reason
            .contains("reward choices/gold amount are restored from observed reward state")
        || transition
            .reason
            .contains("shop UI choices are covered by seed-start shop replay")
}

fn is_seed_start_waiting_for_boundary(status: &FidelityStatus) -> bool {
    status.kind == FidelityKind::Unknown
        && status.compact_diff.is_empty()
        && status
            .message
            .as_deref()
            .is_some_and(|message| message.contains("next seed-start verifier boundary"))
}

fn is_seed_start_non_blocking_coverage_gap(status: &FidelityStatus) -> bool {
    status.kind == FidelityKind::Unknown
        && status
            .message
            .as_deref()
            .is_some_and(|message| message.contains("unexpected_seed_start_command"))
        && status
            .compact_diff
            .iter()
            .any(|diff| diff.contains("seed-start bootstrap harness did not expect command"))
}

struct CommunicationTrace {
    jsonl: String,
    states: usize,
    actions: usize,
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
    let mut actions = 0;
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
                    if matches!(mode, TraceMode::ObservedState) && !saw_state {
                        continue;
                    }
                    jsonl.push_str(&serde_json::to_string(&json!({
                        "type": "action",
                        "step": sequence,
                        "command": command,
                    }))?);
                    jsonl.push('\n');
                    actions += 1;
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
        actions,
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

#[cfg(test)]
mod tests {
    use super::{
        is_non_reportable_unsupported, is_reportable_unsupported,
        is_seed_start_non_blocking_coverage_gap,
    };
    use crate::model::{FidelityKind, FidelityStatus};
    use sts_verify::UnsupportedTransition;

    fn unsupported(reason: &str) -> UnsupportedTransition {
        UnsupportedTransition {
            action_step: 28,
            command: "CHOOSE 0".to_owned(),
            reason: reason.to_owned(),
        }
    }

    #[test]
    fn reward_screen_opening_caveat_is_not_reportable_as_live_fidelity_failure() {
        let transition = unsupported(
            "reward card-screen opening is a UI transition; card pickup is verified from CARD_REWARD",
        );

        assert!(is_non_reportable_unsupported(&transition));
        assert!(!is_reportable_unsupported(&transition));
    }

    #[test]
    fn neow_event_side_effect_caveat_is_not_reportable_as_live_fidelity_failure() {
        let transition =
            unsupported("Neow/event choice side effects are unsupported in sim-to-real replay");

        assert!(is_non_reportable_unsupported(&transition));
        assert!(!is_reportable_unsupported(&transition));
    }

    #[test]
    fn map_and_reward_wrapper_caveats_are_not_reportable_as_live_fidelity_failures() {
        for reason in [
            "reward-to-map UI transition is out-of-scope for simulator state parity",
            "map node selection is unsupported until exact seed-to-map parity is implemented",
            "reward choices/gold amount are restored from observed reward state until exact reward RNG parity is implemented",
        ] {
            let transition = unsupported(reason);

            assert!(
                is_non_reportable_unsupported(&transition),
                "expected non-reportable caveat: {reason}"
            );
            assert!(
                !is_reportable_unsupported(&transition),
                "expected caveat not to be rendered as red fidelity failure: {reason}"
            );
        }
    }

    #[test]
    fn unexpected_seed_start_command_is_a_non_blocking_live_coverage_gap() {
        let status = FidelityStatus {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: vec![
                "seed-start bootstrap harness did not expect command 'PLAY 1' in phase Event"
                    .to_owned(),
            ],
            message: Some(
                "seed-start replay reached boundary unexpected_seed_start_command: seed-start bootstrap harness did not expect command 'PLAY 1' in phase Event"
                    .to_owned(),
            ),
        };

        assert!(is_seed_start_non_blocking_coverage_gap(&status));
    }
}
