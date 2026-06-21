//! Build prefix traces that reproduce the first seed-start or observed-state divergence.

use crate::{
    import_communication_mod_trace, verify_communication_mod_trace_with_mode, SimRealReport,
    TraceLine, TraceMetadata, VerificationMode,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimizeReport {
    pub mode: VerificationMode,
    pub failure_kind: MinimizeFailureKind,
    pub failure_step: u32,
    pub failure_command: String,
    pub failure_label: String,
    pub failure_diffs: Vec<String>,
    pub boundary_category: Option<String>,
    pub boundary_reason: Option<String>,
    pub original_action_count: usize,
    pub minimized_action_count: usize,
    pub minimized_trace: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinimizeFailureKind {
    UnexpectedDiff,
    UnsupportedBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinimizeError {
    NoFailure,
    Parse(String),
}

impl std::fmt::Display for MinimizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFailure => write!(
                f,
                "trace has no unexpected diff or expected-failure boundary to minimize"
            ),
            Self::Parse(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for MinimizeError {}

/// Run parity on `content` and return a JSONL prefix through the first failing action.
pub fn minimize_communication_mod_trace(
    content: &str,
    mode: VerificationMode,
) -> Result<MinimizeReport, MinimizeError> {
    let report = verify_communication_mod_trace_with_mode(content, mode)
        .map_err(|err| MinimizeError::Parse(err.to_string()))?;
    let trace = import_communication_mod_trace(content)
        .map_err(|err| MinimizeError::Parse(err.to_string()))?;
    let failure = locate_first_failure(&report).ok_or(MinimizeError::NoFailure)?;
    let minimized_lines = filter_trace_lines(&trace.lines, failure.step);
    let minimized_action_count = minimized_lines
        .iter()
        .filter(|line| matches!(line, TraceLine::Action(_)))
        .count();
    let metadata = minimized_metadata(trace.metadata.as_ref(), failure.step, mode);
    let minimized_trace = serialize_communication_mod_trace(&metadata, &minimized_lines);

    Ok(MinimizeReport {
        mode,
        failure_kind: failure.kind,
        failure_step: failure.step,
        failure_command: failure.command,
        failure_label: failure.label,
        failure_diffs: failure.diffs,
        boundary_category: failure.boundary_category,
        boundary_reason: failure.boundary_reason,
        original_action_count: report.total_actions,
        minimized_action_count,
        minimized_trace,
    })
}

struct LocatedFailure {
    step: u32,
    kind: MinimizeFailureKind,
    command: String,
    label: String,
    diffs: Vec<String>,
    boundary_category: Option<String>,
    boundary_reason: Option<String>,
}

fn locate_first_failure(report: &SimRealReport) -> Option<LocatedFailure> {
    if let Some(diff) = report.unexpected_diffs.first() {
        return Some(LocatedFailure {
            step: diff.action_step,
            kind: MinimizeFailureKind::UnexpectedDiff,
            command: diff.command.clone(),
            label: diff.label.clone(),
            diffs: diff.diffs.clone(),
            boundary_category: None,
            boundary_reason: None,
        });
    }

    let seed_start = report.seed_start.as_ref()?;
    if !seed_start.expected_failure {
        return None;
    }

    let unsupported = report
        .unsupported
        .iter()
        .find(|entry| entry.action_step == step_from_boundary_path(&seed_start.first_boundary.path))
        .or_else(|| report.unsupported.last())?;

    Some(LocatedFailure {
        step: unsupported.action_step,
        kind: MinimizeFailureKind::UnsupportedBoundary,
        command: unsupported.command.clone(),
        label: seed_start.first_boundary.category.clone(),
        diffs: Vec::new(),
        boundary_category: Some(seed_start.first_boundary.category.clone()),
        boundary_reason: Some(seed_start.first_boundary.reason.clone()),
    })
}

fn step_from_boundary_path(path: &str) -> u32 {
    path.strip_prefix("$.actions[step=")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|step| step.parse().ok())
        .unwrap_or(0)
}

/// Keep metadata plus every state/action line with `step <= max_step`.
pub fn filter_trace_lines(lines: &[TraceLine], max_step: u32) -> Vec<TraceLine> {
    lines
        .iter()
        .filter(|line| match line {
            TraceLine::Metadata(_) => false,
            TraceLine::State(state) => state.step <= max_step,
            TraceLine::Action(action) => action.step <= max_step,
        })
        .cloned()
        .collect()
}

fn minimized_metadata(
    original: Option<&TraceMetadata>,
    failure_step: u32,
    mode: VerificationMode,
) -> TraceMetadata {
    let mut metadata = original.cloned().unwrap_or(TraceMetadata {
        schema: 1,
        source: "communication_mod".to_owned(),
        client: None,
        mode: None,
        started_at: None,
        ended_at: None,
        event: None,
    });
    metadata.event = Some(format!("minimized_to_step={failure_step}; mode={mode:?}"));
    metadata
}

pub fn serialize_communication_mod_trace(metadata: &TraceMetadata, lines: &[TraceLine]) -> String {
    let mut out = String::new();
    out.push_str(
        &serde_json::to_string(&TraceLine::Metadata(metadata.clone()))
            .expect("metadata serializes"),
    );
    out.push('\n');
    for line in lines {
        out.push_str(&serde_json::to_string(line).expect("trace line serializes"));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{load_corpus_file, TraceAction, TraceState};

    #[test]
    fn filter_trace_lines_keeps_prefix_through_failure_step() {
        let lines = vec![
            TraceLine::State(TraceState {
                step: 0,
                received_at: None,
                message: serde_json::json!({}),
            }),
            TraceLine::Action(TraceAction {
                step: 1,
                command: "START".to_owned(),
                sent_at: None,
            }),
            TraceLine::State(TraceState {
                step: 1,
                received_at: None,
                message: serde_json::json!({}),
            }),
            TraceLine::Action(TraceAction {
                step: 2,
                command: "CHOOSE 0".to_owned(),
                sent_at: None,
            }),
            TraceLine::State(TraceState {
                step: 2,
                received_at: None,
                message: serde_json::json!({}),
            }),
        ];
        let filtered = filter_trace_lines(&lines, 1);
        assert_eq!(filtered.len(), 3);
        assert!(matches!(filtered.last(), Some(TraceLine::State(state)) if state.step == 1));
    }

    #[test]
    fn passing_seed_start_trace_has_no_minimize_target() {
        let Some(content) =
            load_corpus_file("communication_mod/trace-2026-06-18T06-04-49-264Z.jsonl")
        else {
            return;
        };
        let err = minimize_communication_mod_trace(&content, VerificationMode::SeedStart)
            .expect_err("passing trace");
        assert_eq!(err, MinimizeError::NoFailure);
    }

    #[test]
    fn minimize_seed_start_trace_produces_prefix_jsonl() {
        let Some(content) =
            load_corpus_file("communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl")
        else {
            return;
        };
        let full_report =
            verify_communication_mod_trace_with_mode(&content, VerificationMode::SeedStart)
                .expect("report");
        if full_report.unexpected_diffs.is_empty()
            && !full_report
                .seed_start
                .as_ref()
                .is_some_and(|s| s.expected_failure)
        {
            return;
        }
        let report = minimize_communication_mod_trace(&content, VerificationMode::SeedStart)
            .expect("minimize failing prefix");
        assert!(report.minimized_action_count <= report.original_action_count);
        assert!(report.minimized_trace.contains("\"type\":\"metadata\""));
        let reparsed = import_communication_mod_trace(&report.minimized_trace).expect("reparses");
        assert!(reparsed.lines.iter().all(|line| match line {
            TraceLine::State(state) => state.step <= report.failure_step,
            TraceLine::Action(action) => action.step <= report.failure_step,
            TraceLine::Metadata(_) => true,
        }));
    }
}
