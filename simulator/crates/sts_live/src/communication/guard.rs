use crate::model::{LiveError, LiveResult};
use serde_json::Value;
use std::time::Duration;

use super::{actions::available_commands, files::BridgeFiles};

pub(crate) fn validate_ready_for_command(
    files: &BridgeFiles,
    command: &str,
    source_state_id: Option<&str>,
    stale_after: Duration,
) -> LiveResult<()> {
    let verb = command
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if verb == "state" {
        return Ok(());
    }
    reject_stale_action(files, source_state_id)?;
    reject_unready_bridge(files, stale_after)?;
    if !available_commands(&files.summary).contains(verb.as_str()) {
        return Err(LiveError::Bridge(format!(
            "command {verb:?} is not available"
        )));
    }
    Ok(())
}

pub(crate) fn validate_ready_for_operator_control(
    files: &BridgeFiles,
    stale_after: Duration,
) -> LiveResult<()> {
    reject_unready_bridge(files, stale_after)
}

fn reject_stale_action(files: &BridgeFiles, source_state_id: Option<&str>) -> LiveResult<()> {
    if let Some(expected) = source_state_id {
        let actual = files.summary.get("state_id").and_then(Value::as_str);
        if actual != Some(expected) {
            return Err(LiveError::Bridge("stale bridge action rejected".to_owned()));
        }
    }
    Ok(())
}

fn reject_unready_bridge(files: &BridgeFiles, stale_after: Duration) -> LiveResult<()> {
    if files.summary_age.is_none_or(|age| age > stale_after) {
        return Err(LiveError::Bridge("bridge state is stale".to_owned()));
    }
    if files.status.get("pending_command").and_then(Value::as_bool) == Some(true) {
        return Err(LiveError::Bridge(
            "bridge command already pending".to_owned(),
        ));
    }
    if files
        .summary
        .get("ready_for_command")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(LiveError::Bridge(
            "bridge is not ready for a command".to_owned(),
        ));
    }
    Ok(())
}
