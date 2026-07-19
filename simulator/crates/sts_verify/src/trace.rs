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
    /// Explicit non-seeded run timer input used by time-gated target logic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playtime_seconds: Option<u32>,
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
            "state" => lines.push(TraceLine::State(parse_state_line(value)?)),
            "action" => lines.push(TraceLine::Action(parse_action_line(value)?)),
            _ => {}
        }
    }
    Ok(lines)
}

fn parse_state_line(value: Value) -> Result<TraceState, serde_json::Error> {
    if value.get("step").is_some() && value.get("message").is_some() {
        return serde_json::from_value(value);
    }

    let step = value
        .pointer("/state/raw/current_state/step")
        .or_else(|| value.pointer("/state/raw/status/step"))
        .or_else(|| value.get("sequence"))
        .cloned()
        .unwrap_or(Value::Null);
    let message = value
        .pointer("/state/raw/current_state/message")
        .cloned()
        .unwrap_or(Value::Null);
    let received_at = value
        .pointer("/state/raw/current_state/received_at")
        .cloned()
        .unwrap_or(Value::Null);

    serde_json::from_value(serde_json::json!({
        "step": step,
        "received_at": received_at,
        "message": message,
    }))
}

fn parse_action_line(value: Value) -> Result<TraceAction, serde_json::Error> {
    if value.get("step").is_some() && value.get("command").is_some() {
        return serde_json::from_value(value);
    }

    let step = value
        .pointer("/action/command/source_state_seq")
        .or_else(|| value.get("sequence"))
        .cloned()
        .unwrap_or(Value::Null);
    let command = value
        .pointer("/action/command/command")
        .cloned()
        .unwrap_or(Value::Null);
    let playtime_seconds = value
        .pointer("/action/playtime_seconds")
        .or_else(|| value.pointer("/action/command/playtime_seconds"))
        .cloned()
        .unwrap_or(Value::Null);

    serde_json::from_value(serde_json::json!({
        "step": step,
        "command": command,
        "playtime_seconds": playtime_seconds,
    }))
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

    #[test]
    fn parse_trace_accepts_live_trace_session_records() {
        let content = r#"{"type":"state","sequence":7,"state":{"raw":{"current_state":{"step":6,"received_at":"now","message":{"game_state":{"floor":0}}}}}}
{"type":"action","sequence":7,"action":{"command":{"command":"CHOOSE 0","source_state_seq":6},"playtime_seconds":812}}"#;

        let lines = parse_trace_jsonl(content).expect("parses");
        assert_eq!(lines.len(), 2);
        assert!(matches!(
            &lines[0],
            TraceLine::State(state)
                if state.step == 6
                    && state.received_at.as_deref() == Some("now")
                    && state.message["game_state"]["floor"] == 0
        ));
        assert!(matches!(
            &lines[1],
            TraceLine::Action(action)
                if action.step == 6
                    && action.command == "CHOOSE 0"
                    && action.playtime_seconds == Some(812)
        ));
    }
}
