//! Trace JSONL formats for verification corpora.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One line from a CommunicationMod-style trace file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceLine {
    Metadata(TraceMetadata),
    State(TraceState),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceMetadata {
    pub schema: u32,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceState {
    pub step: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received_at: Option<String>,
    pub message: Value,
}

/// Hand-authored manual corpus fixture (one JSON object per file or line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualFixture {
    pub name: String,
    pub fixture: String,
    pub actions: Vec<Value>,
    pub rng_draws: u32,
}

/// Parse JSONL trace content into typed lines. Non-trace manual fixtures are skipped.
pub fn parse_trace_jsonl(content: &str) -> Result<Vec<TraceLine>, serde_json::Error> {
    let mut lines = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        lines.push(serde_json::from_str(line)?);
    }
    Ok(lines)
}
