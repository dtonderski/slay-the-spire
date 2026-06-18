//! Trace JSONL formats for verification corpora.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One line from a CommunicationMod-style trace file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceLine {
    Metadata(TraceMetadata),
    State(TraceState),
    Action(TraceAction),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceMetadata {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceState {
    pub step: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received_at: Option<String>,
    pub message: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceAction {
    pub step: u32,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<String>,
}

/// Hand-authored manual corpus fixture (one JSON object per file or line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualFixture {
    pub name: String,
    pub fixture: String,
    pub actions: Vec<Value>,
    pub rng_draws: u32,
}

/// Parsed CommunicationMod trace with metadata, states, and actions in order.
#[derive(Debug, Clone, PartialEq)]
pub struct CommunicationModTrace {
    pub metadata: Option<TraceMetadata>,
    pub lines: Vec<TraceLine>,
}

/// Parse JSONL trace content into typed lines. Unknown `type` values are skipped.
pub fn parse_trace_jsonl(content: &str) -> Result<Vec<TraceLine>, serde_json::Error> {
    let mut lines = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;
        let Some(type_name) = value.get("type").and_then(Value::as_str) else {
            continue;
        };
        match type_name {
            "metadata" => lines.push(TraceLine::Metadata(serde_json::from_value(value)?)),
            "state" => lines.push(TraceLine::State(serde_json::from_value(value)?)),
            "action" => lines.push(TraceLine::Action(serde_json::from_value(value)?)),
            _ => {}
        }
    }
    Ok(lines)
}

/// Import a CommunicationMod trace, collecting metadata and ordered lines.
pub fn import_communication_mod_trace(
    content: &str,
) -> Result<CommunicationModTrace, serde_json::Error> {
    let lines = parse_trace_jsonl(content)?;
    let metadata = lines.iter().find_map(|line| {
        if let TraceLine::Metadata(metadata) = line {
            Some(metadata.clone())
        } else {
            None
        }
    });
    Ok(CommunicationModTrace { metadata, lines })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trace_skips_unknown_line_types() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"state","step":0,"message":{}}
{"type":"action","step":1,"command":"PLAY 1 0"}
{"type":"exit","ended_at":"now"}"#;

        let lines = parse_trace_jsonl(content).expect("parses");
        assert_eq!(lines.len(), 3);
        assert!(matches!(lines[2], TraceLine::Action(_)));
    }
}
