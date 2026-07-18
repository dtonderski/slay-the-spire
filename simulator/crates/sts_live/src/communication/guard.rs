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
    let commands = available_commands(&files.summary);
    if verb == "start" && menu_start_is_available(files, &commands, stale_after)? {
        return Ok(());
    }
    reject_unready_bridge(files, stale_after)?;
    if !commands.contains(verb.as_str()) {
        return Err(LiveError::Bridge(format!(
            "command {verb:?} is not available"
        )));
    }
    Ok(())
}

fn menu_start_is_available(
    files: &BridgeFiles,
    commands: &std::collections::HashSet<String>,
    stale_after: Duration,
) -> LiveResult<bool> {
    if files.summary_age.is_none_or(|age| age > stale_after) {
        return Err(LiveError::Bridge("bridge state is stale".to_owned()));
    }
    if files.status.get("pending_command").and_then(Value::as_bool) == Some(true) {
        return Err(LiveError::Bridge(
            "bridge command already pending".to_owned(),
        ));
    }
    Ok(
        files.summary.get("in_game").and_then(Value::as_bool) == Some(false)
            && commands.contains("start"),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn files(summary: Value) -> BridgeFiles {
        BridgeFiles {
            status: json!({}),
            summary,
            current_state: json!({}),
            status_age: Some(Duration::ZERO),
            summary_age: Some(Duration::ZERO),
        }
    }

    #[test]
    fn allows_explicitly_advertised_start_from_unready_menu_state() {
        let files = files(json!({
            "in_game": false,
            "ready_for_command": false,
            "available_commands": ["start", "state"]
        }));

        validate_ready_for_command(
            &files,
            "START IRONCLAD 0 SEED",
            None,
            Duration::from_secs(1),
        )
        .unwrap();
    }

    #[test]
    fn does_not_bypass_readiness_for_non_start_commands() {
        let files = files(json!({
            "in_game": false,
            "ready_for_command": false,
            "available_commands": ["choose", "state"]
        }));

        let error = validate_ready_for_command(&files, "CHOOSE 0", None, Duration::from_secs(1))
            .unwrap_err();
        assert!(error.to_string().contains("bridge is not ready"));
    }

    #[test]
    fn does_not_start_when_menu_does_not_advertise_start() {
        let files = files(json!({
            "in_game": false,
            "ready_for_command": false,
            "available_commands": ["state"]
        }));

        let error = validate_ready_for_command(
            &files,
            "START IRONCLAD 0 SEED",
            None,
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error.to_string().contains("bridge is not ready"));
    }
}
