//! Trace JSONL formats for verification corpora.

use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sts_core::content::encounters::BossUnlockState;

/// One line from a CommunicationMod-style trace file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceLine {
    Metadata(TraceMetadata),
    State(TraceState),
    Action(TraceAction),
    Error(TraceError),
    CommandAccept(TraceCommandAccept),
    Response(TraceResponse),
    SlayTheData(TraceSlayTheData),
    Automation(TraceAutomation),
    CommandObservedTimeout(TraceCommandObservedTimeout),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceCommandAccept {
    pub step: u32,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceResponse {
    pub sequence: u64,
    pub response: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceSlayTheData {
    pub sequence: u64,
    pub event: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceAutomation {
    pub sequence: u64,
    pub event: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceCommandObservedTimeout {
    pub step: u32,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceError {
    pub step: u32,
    pub message: Value,
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
    /// Profile state supplied as a pre-run input. Boss selection is not a pure
    /// function of the seed while a profile still has unseen bosses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss_unlocks: Option<BossUnlockState>,
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

/// Parse every nonblank JSONL record into one known typed trace line.
pub fn parse_trace_jsonl(content: &str) -> Result<Vec<TraceLine>, serde_json::Error> {
    let mut lines = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;
        let parsed = match value.get("type").and_then(Value::as_str) {
            Some("metadata") => TraceLine::Metadata(serde_json::from_value(value)?),
            Some("state") => TraceLine::State(parse_state_line(value)?),
            Some("action") => TraceLine::Action(parse_action_line(value)?),
            Some("error") => TraceLine::Error(serde_json::from_value(value)?),
            Some("command_accept") => TraceLine::CommandAccept(parse_command_accept_line(value)?),
            Some("response") => TraceLine::Response(parse_response_line(value)?),
            Some("slay_the_data") => TraceLine::SlayTheData(parse_slay_the_data_line(value)?),
            Some("automation") => TraceLine::Automation(parse_automation_line(value)?),
            Some("command_observed_timeout") => {
                TraceLine::CommandObservedTimeout(parse_command_observed_timeout_line(value)?)
            }
            _ => serde_json::from_value::<TraceLine>(value)?,
        };
        lines.push(parsed);
    }
    Ok(lines)
}

fn parse_command_accept_line(value: Value) -> Result<TraceCommandAccept, serde_json::Error> {
    let accepted: TraceCommandAccept = serde_json::from_value(value)?;
    if accepted.command.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "trace command acceptance must name a command",
        ));
    }
    Ok(accepted)
}

fn parse_response_line(value: Value) -> Result<TraceResponse, serde_json::Error> {
    let response: TraceResponse = serde_json::from_value(value)?;
    if !response.response.is_object() {
        return Err(serde_json::Error::custom(
            "trace response payload must be a JSON object",
        ));
    }
    Ok(response)
}

fn parse_slay_the_data_line(value: Value) -> Result<TraceSlayTheData, serde_json::Error> {
    let guidance: TraceSlayTheData = serde_json::from_value(value)?;
    if guidance.event.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "SlayTheData trace guidance must name an event",
        ));
    }
    if !guidance.details.is_object() {
        return Err(serde_json::Error::custom(
            "SlayTheData trace guidance details must be a JSON object",
        ));
    }
    Ok(guidance)
}

fn parse_automation_line(value: Value) -> Result<TraceAutomation, serde_json::Error> {
    let automation: TraceAutomation = serde_json::from_value(value)?;
    if automation.event.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "automation trace telemetry must name an event",
        ));
    }
    if !automation.details.is_object() {
        return Err(serde_json::Error::custom(
            "automation trace telemetry details must be a JSON object",
        ));
    }
    Ok(automation)
}

fn parse_command_observed_timeout_line(
    value: Value,
) -> Result<TraceCommandObservedTimeout, serde_json::Error> {
    let timeout: TraceCommandObservedTimeout = serde_json::from_value(value)?;
    if timeout.command.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "trace command observation timeout must name a command",
        ));
    }
    Ok(timeout)
}

fn parse_state_line(value: Value) -> Result<TraceState, serde_json::Error> {
    let state: TraceState = if value.get("step").is_some() && value.get("message").is_some() {
        serde_json::from_value(value)?
    } else {
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
        }))?
    };
    if !state.message.is_object() {
        return Err(serde_json::Error::custom(
            "trace state message must be a JSON object",
        ));
    }
    Ok(state)
}

fn parse_action_line(value: Value) -> Result<TraceAction, serde_json::Error> {
    let action: TraceAction = if value.get("step").is_some() && value.get("command").is_some() {
        serde_json::from_value(value)?
    } else {
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
        }))?
    };
    if action.command.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "trace action command must not be empty",
        ));
    }
    Ok(action)
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
    fn parse_trace_rejects_unknown_line_types() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"state","step":0,"message":{}}
{"type":"action","step":1,"command":"PLAY 1 0"}
{"type":"exit","ended_at":"now"}"#;

        let error = parse_trace_jsonl(content).expect_err("unknown record type is invalid");
        assert!(error.to_string().contains("unknown variant `exit`"));
    }

    #[test]
    fn parse_trace_preserves_known_auxiliary_records() {
        let content = r#"{"type":"command_accept","step":1,"command":"CHOOSE 0"}
{"type":"response","sequence":2,"response":{"kind":"bridge_command_result"}}
{"type":"slay_the_data","sequence":3,"event":"send_action","details":{"step_index":1}}
{"type":"automation","sequence":4,"event":"plan_ready","details":{"state":"ready_to_send"}}
{"type":"command_observed_timeout","step":5,"command":"END"}"#;

        let lines = parse_trace_jsonl(content).expect("known auxiliary records parse");
        assert!(matches!(
            &lines[0],
            TraceLine::CommandAccept(accepted)
                if accepted.step == 1 && accepted.command == "CHOOSE 0"
        ));
        assert!(matches!(
            &lines[1],
            TraceLine::Response(response)
                if response.sequence == 2
                    && response.response["kind"] == "bridge_command_result"
        ));
        assert!(matches!(
            &lines[2],
            TraceLine::SlayTheData(guidance)
                if guidance.sequence == 3
                    && guidance.event == "send_action"
                    && guidance.details["step_index"] == 1
        ));
        assert!(matches!(
            &lines[3],
            TraceLine::Automation(automation)
                if automation.sequence == 4
                    && automation.event == "plan_ready"
                    && automation.details["state"] == "ready_to_send"
        ));
        assert!(matches!(
            &lines[4],
            TraceLine::CommandObservedTimeout(timeout)
                if timeout.step == 5 && timeout.command == "END"
        ));
    }

    #[test]
    fn parse_trace_rejects_missing_line_type() {
        let error = parse_trace_jsonl(r#"{"step":1,"command":"PLAY 1 0"}"#)
            .expect_err("missing record type is invalid");

        assert!(error.to_string().contains("missing field `type`"));
    }

    #[test]
    fn parse_trace_rejects_null_state_message() {
        let error = parse_trace_jsonl(r#"{"type":"state","step":1,"message":null}"#)
            .expect_err("null state message is invalid");

        assert!(error
            .to_string()
            .contains("trace state message must be a JSON object"));
    }

    #[test]
    fn parse_trace_rejects_empty_action_command() {
        let error = parse_trace_jsonl(r#"{"type":"action","step":1,"command":"  "}"#)
            .expect_err("empty action command is invalid");

        assert!(error
            .to_string()
            .contains("trace action command must not be empty"));
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

    #[test]
    fn parse_trace_preserves_target_command_errors() {
        let content = r#"{"type":"action","step":7,"command":"POTION USE 1"}
{"type":"error","step":7,"message":{"error":"Potion cannot be used"}}"#;

        let lines = parse_trace_jsonl(content).expect("parses");
        assert!(matches!(
            &lines[1],
            TraceLine::Error(error)
                if error.step == 7 && error.message["error"] == "Potion cannot be used"
        ));
    }

    #[test]
    fn parse_trace_preserves_explicit_boss_unlock_inputs() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod","boss_unlocks":{"guardian_seen":false,"hexaghost_seen":true,"slime_boss_seen":true,"champ_seen":true,"automaton_seen":true,"collector_seen":true,"awakened_one_seen":true,"donu_deca_seen":true,"time_eater_seen":true}}"#;

        let trace = import_communication_mod_trace(content).expect("parses");
        let unlocks = trace
            .metadata
            .and_then(|metadata| metadata.boss_unlocks)
            .expect("boss unlock inputs");
        assert!(!unlocks.guardian_seen);
        assert!(unlocks.hexaghost_seen);
    }
}
